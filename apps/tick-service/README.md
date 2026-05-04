# tick-service

Long-lived service that streams real-time price updates from Yahoo Finance,
aggregates them into 1-minute OHLCV candles in Redis, and broadcasts the
live state to desktop clients over WebSocket.

- Source: Yahoo Finance protobuf WebSocket via `markets::streaming::PriceStream`
- State: Redis (in-progress hash + delta stream + today's bars sorted set)
- Server: `axum` WebSocket at `/ws`
- Storage: **Redis only** — finalized bars get a 36-hour TTL and are
  superseded by historical-service's overnight FMP backfill into ClickHouse.
  This service never touches ClickHouse.

## Environment

| Var | Required | Default | Notes |
|---|---|---|---|
| `REDIS_URL` | yes | — | e.g. `redis://:pw@host:6379` |
| `MAX_WARM_SYMBOLS` | no | `50` | Soft cap on how many symbols are kept subscribed. Oldest zero-ref symbol is evicted when cap is hit; active subscriptions are never dropped. |
| `TICK_SERVICE_WS_BIND` | no | `0.0.0.0:8765` | host:port for the WS server |
| `TODAY_BARS_TTL_SECS` | no | `129600` (36h) | TTL on `tick:bars:{date}:{symbol}` |
| `UPDATES_STREAM_MAXLEN` | no | `200` | bounded length for `tick:updates:{symbol}` stream |
| `COLD_SUBSCRIPTION_TTL_SECS` | no | `300` | reserved for future use |
| `RUST_LOG` | no | `tick_service=info,markets=warn` | tracing filter |

## Redis schema

```
tick:active                              (set)    persisted list of subscribed symbols across restarts
tick:current:{symbol}                    (hash)   in-progress 1-min candle
tick:updates:{symbol}                    (stream MAXLEN ~ 200)
tick:bars:{YYYY-MM-DD}:{symbol}          (sorted set, TTL 36h, score = bucket_start unix sec)
```

## How it works

1. **Boot:** load env, connect Redis. If `tick:active` has members, re-prime the `PriceStream` from that set (rehydrate) and restore subscription state. Otherwise start cold with no subscriptions.
2. **Subscribe:** when a desktop client sends `{"type":"subscribe","symbols":["AAPL"]}`, the coordinator admits the symbol — adding it to Yahoo's WebSocket and persisting it to `tick:active`. Seeding today's already-elapsed bars runs in a background task (non-blocking for the WS response). Subsequent subscribers for the same symbol increment its ref count.
3. **Coordinator:** owns the `PriceStream`. Each tick:
   - Filter: only `Equity`/`ETF` in `RegularMarket` hours.
   - Convert price to cents, compute minute bucket.
   - Run the `apply_tick` Lua script: atomic update of the in-progress candle hash + append to delta stream. Bucket transitions are detected and handled inside the script.
4. **Boundary sweeper:** once a minute, find any `tick:current:{symbol}` whose `bucket_start` is for an already-completed minute (low-liquidity symbol, end-of-session, etc.) and finalize it — write to today's sorted set, emit a `finalize` stream event. Iterates the live subscription snapshot (not a static list).
5. **Eviction (soft cap):** when `MAX_WARM_SYMBOLS` is reached and a new symbol is admitted, the oldest zero-ref symbol is evicted — removed from Yahoo's WebSocket and from `tick:active`. Symbols with active clients are never evicted (the cap goes over if needed).
6. **WebSocket server:** clients connect to `/ws`. On `subscribe`, the server sends a `seed` (current candle), a `today` (all finalized bars for today), then tails the symbol's stream and forwards `delta`/`finalize` events. On disconnect all symbols are released (ref counts decremented).

## WebSocket protocol

All frames are JSON.

**Client → server:**

```json
{"type":"subscribe","symbols":["AAPL","MSFT"]}
{"type":"unsubscribe","symbols":["MSFT"]}
{"type":"ping"}
```

**Server → client:**

```json
{"type":"seed","symbol":"AAPL","candle":{"ts":1714571520,"o":18422,"h":18450,"l":18415,"c":18438,"v":14523}}
{"type":"today","symbol":"AAPL","bars":[{"ts":1714571520,"o":18422,"h":18450,"l":18415,"c":18438,"v":14523}]}
{"type":"delta","symbol":"AAPL","ts":1714571547231,"c":18438,"h":null,"l":null,"v":14523}
{"type":"finalize","symbol":"AAPL","candle":{"ts":1714571520,"o":18422,"h":18450,"l":18415,"c":18438,"v":14523}}
{"type":"pong"}
{"type":"error","code":"redis_unavailable","message":"..."}
```

`o`/`h`/`l`/`c` are integer cents (divide by 100 to render in dollars).
`v` is the bucket's accumulated volume (i64).

## Running locally

```bash
cp apps/tick-service/.env.example apps/tick-service/.env
$EDITOR apps/tick-service/.env
cargo run -p tick-service
```

The binary auto-loads `apps/tick-service/.env`. WS server listens on `TICK_SERVICE_WS_BIND` (`0.0.0.0:8765` by default).

## Smoke testing

```bash
# 1. Run the service
cargo run -p tick-service

# 2. In another terminal, connect a WebSocket client (e.g. websocat)
websocat ws://localhost:8765/ws
> {"type":"subscribe","symbols":["AAPL"]}
# Expect: {"type":"seed",...} {"type":"today",...}
# Today may be empty initially — seeding runs in the background.
# {"type":"delta",...} events arrive as Yahoo ticks land.

# 3. Check tick:active to see which symbols are subscribed
redis-cli SMEMBERS tick:active

# 4. Check Redis state directly
redis-cli HGETALL tick:current:AAPL
redis-cli ZRANGE tick:bars:2026-05-04:AAPL 0 -1
redis-cli XRANGE tick:updates:AAPL - +
```

## Deploying on the VPS

```bash
# Build a release binary on the VPS
cd /opt/zaned-historical    # same checkout as historical-service
cargo build -p tick-service --release
install -m 755 target/release/tick-service /usr/local/bin/tick-service

# Env file
cat > /etc/tick-service.env << 'EOF'
REDIS_URL=redis://:johnson@localhost:6379
MAX_WARM_SYMBOLS=50
TICK_SERVICE_WS_BIND=0.0.0.0:8765
RUST_LOG=tick_service=info
EOF
chmod 600 /etc/tick-service.env

# systemd unit
install -m 644 apps/tick-service/systemd/tick-service.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now tick-service
journalctl -u tick-service -f
```

## Inspecting state

```bash
# Live tail
ssh myserver 'journalctl -u tick-service -f'

# Active symbol set
ssh myserver 'docker exec redis redis-cli SMEMBERS tick:active'

# Current candle for AAPL
ssh myserver 'docker exec redis redis-cli HGETALL tick:current:AAPL'

# Today's finalized bars for AAPL
ssh myserver 'docker exec redis redis-cli ZRANGE tick:bars:'"$(date -u +%F)"':AAPL 0 -1 WITHSCORES'

# Latest 10 stream entries
ssh myserver 'docker exec redis redis-cli XREVRANGE tick:updates:AAPL + - COUNT 10'
```
