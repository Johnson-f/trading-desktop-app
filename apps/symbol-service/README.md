# symbol-service

Long-lived service that keeps a Typesense `tickers` collection in sync with the
universe of active US stocks pulled daily from Yahoo Finance.

## Environment

| Var | Required | Default | Notes |
|---|---|---|---|
| `TYPESENSE_URL` | yes | — | e.g. `http://vmi3204337.contaboserver.net:8108` |
| `TYPESENSE_API_KEY` | yes | — | master key for Typesense |
| `REDIS_URL` | yes | — | e.g. `redis://:pw@host:6379` |
| `SYMBOL_SERVICE_SCHEDULE_HOUR_UTC` | no | `2` | 0–23, hour-of-day UTC for daily run |
| `SYMBOL_SERVICE_COLLECTION` | no | `tickers` | Typesense collection name |
| `RUST_LOG` | no | `symbol_service=info,markets=warn` | tracing filter |

## How it works

On boot:
1. Ensure the Typesense collection exists (idempotent).
2. Check Redis for `symbol_service:current_run` — if found, resume that run immediately.
3. Sleep until the next scheduled hour, then run a sync pass; repeat.

Each sync pass:
1. Acquire `symbol_service:run_lock` (Redis `SET NX`, 6h TTL). Skip if already held.
2. For each US exchange (`NMS`, `NYQ`, `ASE`):
   - Page Yahoo's screener (size 250) with filters: region=us, exchange, avg_daily_vol_3m > 50k, market_cap > $1M.
   - Filter out non-equity quote types and junk symbols (warrants, units, rights, preferreds).
   - Batch-upsert to Typesense (`action=upsert`).
   - Persist `(exchange, offset, total_upserted)` to Redis after each successful batch.
3. Compare `seen` set with Typesense's full id list and delete any docs Yahoo no longer reports (skipped on resumed runs to avoid wiping unprocessed exchanges).
4. `RENAME current_run last_completed_run` and release the lock.

## Crash recovery

Redis is the only state. On every page boundary, the `current_run` cursor is updated
*after* the Typesense write succeeds. Any failure just leaves the cursor pointing at
the next page to fetch — replaying it is a no-op because Typesense imports are
upserts. A new process boot picks up exactly where the previous one left off.

## Running locally

Copy the example env file and fill in real values:

```bash
cp apps/symbol-service/.env.example apps/symbol-service/.env
$EDITOR apps/symbol-service/.env
cargo run -p symbol-service
```

`apps/symbol-service/.env` is gitignored. The binary auto-loads it via `dotenvy`
when started outside systemd; in production the systemd unit's `EnvironmentFile=`
populates the env first and `dotenvy` is a no-op.

## Deploying on the VPS

1. Build a release binary and ship it to `/usr/local/bin/symbol-service`.
2. Populate `/etc/symbol-service.env` (same key=value format as `.env.example`,
   no `export`, no quotes) and lock it down: `chmod 600 /etc/symbol-service.env`.
3. Drop `systemd/symbol-service.service` into `/etc/systemd/system/`.

```bash
systemctl daemon-reload
systemctl enable --now symbol-service
journalctl -u symbol-service -f
```
