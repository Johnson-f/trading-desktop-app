# historical-backfill

Yahoo-Finance-backed historical OHLCV backfill into a Cloudflare R2
Data Catalog (Apache Iceberg via REST). Two passes per cycle:

- **Daily** (`bars_1d`): full daily history (`range=max, interval=1d`)
  for every symbol in the universe. Incrementally appended past each
  symbol's high-water mark in the catalog.
- **Minute** (`bars_1m`): rolling 5-day window (`range=5d, interval=1m`)
  per cycle. Stateless — readers dedupe via the `version` column
  (the ReplacingMergeTree semantic preserved in V1).

Single writer per warehouse — the gateway embeds this crate via
`tokio::spawn(historical_backfill::run(cfg))`. No Redis, no probe phase.

## Environment

| Var | Required | Default | Notes |
|---|---|---|---|
| `WAREHOUSE_CATALOG_URI` | yes | — | e.g. `https://catalog.cloudflarestorage.com/<account_hash>/<bucket>` from `wrangler r2 bucket catalog enable` |
| `WAREHOUSE_NAME` | yes | — | Warehouse identifier; Cloudflare formats as `<account_hash>_<bucket_name>` |
| `WAREHOUSE_TOKEN` | yes | — | R2 API token with Admin Read & Write permissions |
| `WAREHOUSE_NAMESPACE` | no | `market_data` | Iceberg namespace under which the tables live |
| `SCHEDULE_HOUR_UTC` | no | `2` | UTC hour of day to start each cycle |
| `YAHOO_WORKERS` | no | `8` | Bounded concurrency for Yahoo HTTP fetches |
| `RUST_LOG` | no | `historical_backfill=info,markets=warn` | Tracing filter |

## One-time R2 setup

```bash
# 1. Create the bucket (pick a name; must be unique within your account)
npx wrangler r2 bucket create zaned-warehouse

# 2. Enable the Iceberg Data Catalog on it
npx wrangler r2 bucket catalog enable zaned-warehouse
# → outputs Catalog URI and Warehouse name; copy both into your .env

# 3. Create an R2 API token with "Admin Read & Write" scoped to that bucket
#    (Cloudflare dashboard → R2 → Manage R2 API Tokens → Create API token).
#    The token is shown once; copy into WAREHOUSE_TOKEN.
```

The 10 GB free tier is sufficient for ~3,500 symbols of daily history
plus a rolling 5-day minute window indefinitely; egress is free for any
volume.

## Running locally

```bash
cp .env.example .env
$EDITOR .env   # fill in the three WAREHOUSE_* values
cargo run -p historical-backfill   # if a binary exists; otherwise via the gateway
```

The library exposes `pub async fn run(cfg: Config) -> Result<()>`. The
gateway's binary calls it inside a `tokio::spawn` at boot. Cancellation
is implicit when the runtime drops on shutdown.

## Iceberg table layout

Both tables share the same schema and partition spec:

| Field | Type | Notes |
|---|---|---|
| symbol | String | LowCardinality-ish (dictionary-encoded in Parquet) |
| ts | Timestamp(µs, UTC) | Sort key + partition source |
| open / high / low / close | Int (i32 cents) | OHLC as integer cents; divide by 100 to render |
| volume | Long (i64) | Promoted from u32 to dodge i32 overflow on heavy ETFs |
| version | Long (i64) | Unix-second write timestamp; reader-side dedupe key |

- Partition: `year(ts)` — same for daily and minute.
- Sort order: `(symbol ASC, ts ASC)` — matches `(symbol, ts)` range-query pattern.
- File format: Parquet with ZSTD-3 compression.

## Inspecting state

R2 Data Catalog speaks the standard Iceberg REST protocol. Any
Iceberg-aware engine connects directly:

```bash
# PyIceberg quickstart (replace the three <PLACEHOLDER> values)
pip install pyiceberg pyarrow
python <<'PY'
from pyiceberg.catalog.rest import RestCatalog
catalog = RestCatalog(
    name="zaned",
    uri="<WAREHOUSE_CATALOG_URI>",
    warehouse="<WAREHOUSE_NAME>",
    token="<WAREHOUSE_TOKEN>",
)
print(list(catalog.list_tables("market_data")))
t = catalog.load_table(("market_data", "bars_1d"))
print(t.scan(row_filter="symbol = 'AAPL'").to_arrow())
PY
```

DuckDB (≥ 1.3) has a native Iceberg REST extension:

```sql
INSTALL iceberg;
LOAD iceberg;
ATTACH 'r2_warehouse' AS r2 (TYPE iceberg,
  ENDPOINT '<WAREHOUSE_CATALOG_URI>',
  WAREHOUSE '<WAREHOUSE_NAME>',
  TOKEN '<WAREHOUSE_TOKEN>');
SELECT count(*) FROM r2.market_data.bars_1d;
```

## Tests

```bash
# Unit tests (no Docker required):
cargo test -p historical-backfill --lib

# Integration tests (require Docker — spin up apache/iceberg-rest-fixture).
# Must run serially: tests share /tmp/iceberg_test_wh on the host.
cargo test -p historical-backfill --test cycle           -- --test-threads=1
cargo test -p historical-backfill --test warehouse_append -- --test-threads=1
cargo test -p historical-backfill --test warehouse_tables -- --test-threads=1
```

## Deploy on the VPS

The crate is library-only. Deployment is via the gateway's binary that
embeds it. The gateway's systemd unit file needs the four required
`WAREHOUSE_*` env vars set; the rest have defaults.

## Known limitations

- **Orphan files on partial failure.** Between the writer's `close()`
  and the catalog `commit`, a transient network error leaves Parquet
  files in R2 that no snapshot references. Run periodic
  `ExpireSnapshots` + orphan-file cleanup (Iceberg standard maintenance).
- **HWM scan reads data files.** V1 walks the table via Arrow scan with
  a `(symbol, ts)` projection — fine at millions of rows, replaceable
  with manifest-stats reads at billions.
- **Single writer.** Multiple writers to the same warehouse will hit
  optimistic-concurrency conflicts; the loop has no retry-on-conflict
  logic in V1. The gateway is the only writer in production.
