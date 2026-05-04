# historical-service

Long-lived service that fills a ClickHouse `bars_1m` table with 1-minute
OHLCV history for the active US equity universe, and keeps it fresh daily.

- Source for deep history: **FMP** (`/stable/historical-chart/1min`)
- Source for daily incremental: **Yahoo** via the `markets` workspace crate
- Storage: **ClickHouse** (`market_data.bars_1m`, `ReplacingMergeTree(version)`)
- Coordination: **Redis** (single global run-lock; no per-symbol cursor)

The desktop app reads ClickHouse directly. This service only writes.

## Environment

| Var | Required | Default | Notes |
|---|---|---|---|
| `CLICKHOUSE_URL` | yes | — | e.g. `http://vmi3204337.contaboserver.net:8123` |
| `CLICKHOUSE_USER` | no | `default` | |
| `CLICKHOUSE_PASSWORD` | no | empty | |
| `CLICKHOUSE_DATABASE` | no | `market_data` | |
| `FMP_API_KEY` | yes | — | |
| `FMP_BASE_URL` | no | `https://financialmodelingprep.com/stable` | overrideable for tests |
| `FMP_RATE_LIMIT_RPM` | no | `300` | starting pace; halves on 429, recovers after 5 clean responses |
| `REDIS_URL` | yes | — | e.g. `redis://:pw@host:6379` |
| `HISTORICAL_SERVICE_SCHEDULE_HOUR_UTC` | no | `2` | 0–23, daily wake-up hour |
| `HISTORICAL_SERVICE_CONCURRENCY` | no | `4` | symbols processed in parallel; ceiling on in-flight FMP requests |
| `BACKFILL_EARLIEST` | no | `2005-01-01` | floor for symbols with no data yet |
| `RUST_LOG` | no | `historical_service=info,markets=warn` | tracing filter |

## How it works

On boot:
1. Ensures the `bars_1m` table exists (idempotent).
2. Connects Redis, force-releases any stale lock, acquires a fresh one.
3. Fetches the universe from Yahoo's screener (sorted by market cap desc).
4. Enters the sync loop.

Each iteration:
1. One ClickHouse query: `SELECT symbol, max(ts) FROM bars_1m WHERE symbol IN (...) GROUP BY symbol`.
2. For each symbol, decides per-call: FMP if gap > 29 days, otherwise Yahoo.
3. Fetches a chunk (yearly for FMP, single call for Yahoo).
4. Async-inserts into ClickHouse.

When the universe is fully caught up, sleeps until the next 02:00 UTC tick.

## Crash recovery

ClickHouse `MAX(ts)` is the only source of truth for "where am I." Any
failed insert just means the next loop's fresh `MAX(ts)` query covers the
same range — `ReplacingMergeTree(version)` deduplicates by sort key.
Redis holds nothing but a global lock with a 6h TTL.

## Running locally

```bash
cp apps/historical-service/.env.example apps/historical-service/.env
$EDITOR apps/historical-service/.env
cargo run -p historical-service
```

`apps/historical-service/.env` is gitignored. The binary auto-loads it via
`dotenvy` when started outside systemd; in production the systemd unit's
`EnvironmentFile=` populates the env first and `dotenvy` is a no-op.

## Deploying on the VPS

1. Build a release binary and ship it to `/usr/local/bin/historical-service`.
2. Populate `/etc/historical-service.env` (same key=value format as `.env.example`,
   no `export`, no quotes) and lock it down: `chmod 600 /etc/historical-service.env`.
3. Drop `systemd/historical-service.service` into `/etc/systemd/system/`.

```bash
systemctl daemon-reload
systemctl enable --now historical-service
journalctl -u historical-service -f
```

## Inspecting progress

```sql
-- in clickhouse-client
SELECT count(distinct symbol) AS symbols_with_any_data,
       sum(rows)              AS total_rows,
       min(ts), max(ts)
FROM market_data.bars_1m;

SELECT symbol, min(ts), max(ts), count() AS rows
FROM market_data.bars_1m
GROUP BY symbol
ORDER BY rows DESC
LIMIT 20;
```

```bash
# lock state
redis-cli -u "$REDIS_URL" GET historical_service:run_lock
```
