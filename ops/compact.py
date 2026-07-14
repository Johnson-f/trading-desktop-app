#!/usr/bin/env python3
"""
Compaction job for the R2 Iceberg warehouse.

Rewrites small Parquet files into fewer large ones, and (when the underlying
PyIceberg version supports it via rewrite options) physically dedups rows by
keeping the highest `version` per (symbol, ts).

Designed to be run as a weekly cron job on the VPS where ClickHouse and the
gateway live. Pulls credentials from a sibling `.env` (same shape as
historical-backfill/.env).

Usage:
    python3 compact.py                       # compact recent partitions only
    python3 compact.py --all                 # compact entire tables (slow)
    python3 compact.py --table bars_1d       # one table only
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path

from pyiceberg.catalog.rest import RestCatalog
from pyiceberg.expressions import GreaterThanOrEqual


def _get_pyiceberg_version() -> str:
    try:
        import pyiceberg
        return pyiceberg.__version__
    except Exception:
        return "unknown"


def load_env(path: Path) -> None:
    if not path.exists():
        return
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        os.environ.setdefault(k.strip(), v.strip())


def compact_table(catalog: RestCatalog, namespace: str, name: str, cutoff_iso: str | None):
    tbl = catalog.load_table((namespace, name))
    before_files = sum(1 for _ in tbl.scan().plan_files())
    before_bytes = sum(f.file.file_size_in_bytes for f in tbl.scan().plan_files())

    print(f"\n=== compacting {namespace}.{name} ===")
    print(f"  before: {before_files:>6} files   {before_bytes/1024/1024:>9.1f} MiB")

    # Check if rewrite_data_files is available
    if not hasattr(tbl, "rewrite_data_files") and not hasattr(tbl.maintenance, "rewrite_data_files"):
        print(f"  SKIPPED: PyIceberg {_get_pyiceberg_version()} does not support rewrite_data_files.")
        print(f"           Upgrade to PyIceberg >= 0.12.0 to enable file compaction.")
        return

    start = time.time()
    if cutoff_iso:
        where = GreaterThanOrEqual("ts", cutoff_iso)
        # PyIceberg 0.12+ exposes this as tbl.rewrite_data_files
        try:
            tbl.rewrite_data_files(
                target_file_size_bytes=128 * 1024 * 1024,
                where=where,
            )
        except AttributeError:
            # Fallback to maintenance API if it exists
            tbl.maintenance.rewrite_data_files(
                target_file_size_bytes=128 * 1024 * 1024,
                where=where,
            )
    else:
        try:
            tbl.rewrite_data_files(target_file_size_bytes=128 * 1024 * 1024)
        except AttributeError:
            # Fallback to maintenance API if it exists
            tbl.maintenance.rewrite_data_files(target_file_size_bytes=128 * 1024 * 1024)
    elapsed = time.time() - start

    # Reload to see the new state
    tbl = catalog.load_table((namespace, name))
    after_files = sum(1 for _ in tbl.scan().plan_files())
    after_bytes = sum(f.file.file_size_in_bytes for f in tbl.scan().plan_files())
    print(f"  after:  {after_files:>6} files   {after_bytes/1024/1024:>9.1f} MiB")
    print(f"  done in {elapsed/60:.1f} min")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--all", action="store_true",
                        help="compact entire tables (slow; one-off cleanup)")
    parser.add_argument("--table", choices=["bars_1d", "bars_1m", "both"], default="both")
    args = parser.parse_args()

    load_env(Path(__file__).parent / ".env")

    catalog = RestCatalog(
        "zaned",
        uri=os.environ["WAREHOUSE_CATALOG_URI"],
        warehouse=os.environ["WAREHOUSE_NAME"],
        token=os.environ["WAREHOUSE_TOKEN"],
    )
    namespace = os.environ.get("WAREHOUSE_NAMESPACE", "market_data")

    # Default cutoffs: past 30 days for minute, past year for daily.
    if args.all:
        cutoff_1m = None
        cutoff_1d = None
    else:
        cutoff_1m = (datetime.now(timezone.utc) - timedelta(days=30)).isoformat()
        cutoff_1d = (datetime.now(timezone.utc) - timedelta(days=365)).isoformat()

    tables = ["bars_1d", "bars_1m"] if args.table == "both" else [args.table]
    for t in tables:
        cutoff = cutoff_1d if t == "bars_1d" else cutoff_1m
        compact_table(catalog, namespace, t, cutoff)


if __name__ == "__main__":
    sys.exit(main())
