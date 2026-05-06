# historical-service

Yahoo-Finance-backed historical OHLCV backfill for the Zaned ClickHouse
warehouse. Two passes per cycle:

- **Daily** (`bars_1d`): full history (`range=max, interval=1d`) for every
  symbol in the universe, incrementally appended past each symbol's
  ClickHouse high water mark.
- **Minute** (`bars_1m`): rolling 5-day window (`range=5d, interval=1m`)
  refreshed each cycle. ReplacingMergeTree(version) dedupes against the
  prior cycle's bars; the legacy 2005→present 1-min history that earlier
  FMP runs populated for ~250 symbols is preserved untouched (those bars
  sit at older `ts` values outside the 5-day window).

No FMP. No Redis. No probe phase.

## Environment

| Var | Required | Default | Notes |
|---|---|---|---|
| `CLICKHOUSE_URL` | yes | — | e.g. `http://localhost:8123` |
| `CLICKHOUSE_PASSWORD` | yes | — | |
| `CLICKHOUSE_USER` | no | `default` | |
| `CLICKHOUSE_DATABASE` | no | `market_data` | |
| `SCHEDULE_HOUR_UTC` | no | `2` | UTC hour of day to run the cycle |
| `YAHOO_WORKERS` | no | `8` | bounded concurrency for Yahoo fetches |
| `RUST_LOG` | no | `historical_service=info,markets=warn` | tracing filter |

## Running locally

```bash
cp apps/historical-service/.env.example apps/historical-service/.env
$EDITOR apps/historical-service/.env
cargo run -p historical-service
```

The binary auto-loads `apps/historical-service/.env`. Boot triggers an
immediate cycle, then schedules the next at `SCHEDULE_HOUR_UTC`.

## Schema

```sql
-- Daily: full history per symbol.
CREATE TABLE bars_1d (
    symbol  LowCardinality(String),
    ts      DateTime,
    open    Decimal32(2), high Decimal32(2), low Decimal32(2), close Decimal32(2),
    volume  UInt32,
    version UInt32
)
ENGINE = ReplacingMergeTree(version)
PARTITION BY toYear(ts)
ORDER BY (symbol, ts);

-- 1-min: rolling 5-day window + any pre-existing legacy bars.
CREATE TABLE bars_1m (...same shape, same engine...);
```

## Deploy on the VPS

```bash
cargo build -p historical-service --release
sudo install -m 755 target/release/historical-service /usr/local/bin/
sudo install -m 644 apps/historical-service/systemd/historical-service.service /etc/systemd/system/
sudo tee /etc/historical-service.env >/dev/null <<'EOF'
CLICKHOUSE_URL=http://localhost:8123
CLICKHOUSE_PASSWORD=...
EOF
sudo systemctl daemon-reload
sudo systemctl enable --now historical-service
journalctl -u historical-service -f
```

## Inspecting state

```bash
# how many symbols have at least one daily bar?
docker exec clickhouse clickhouse-client --query \
  "SELECT uniqExact(symbol) FROM market_data.bars_1d"

# 1-min coverage of the rolling window
docker exec clickhouse clickhouse-client --query \
  "SELECT uniqExact(symbol) FROM market_data.bars_1m WHERE ts > now() - INTERVAL 5 DAY"

# legacy 1-min cohort (pre-FMP-deletion deep history)
docker exec clickhouse clickhouse-client --query \
  "SELECT uniqExact(symbol) FROM market_data.bars_1m WHERE ts < toDate('2025-01-01')"
```
