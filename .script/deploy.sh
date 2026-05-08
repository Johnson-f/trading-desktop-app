#!/usr/bin/env bash
# Push the local source tree to the VPS (myserver:~/Zaned/) over rsync.
#
# Usage:
#   script/deploy.sh
#
# What it syncs:
#   - All source under ~/Zaned/
#   - The local apps/server/gateway/.env (kept in lockstep with the VPS).
#
# What it never sends:
#   - target/        gigabytes of build artefacts, machine-specific
#   - node_modules/  regenerable
#   - .git/          rsync-only workflow, no need on the VPS
#   - editor / OS scratch files
#
# Returns rsync's exit code so callers (e.g. dev-watch.sh) can detect
# transient network failures.

set -euo pipefail

# Run from repo root regardless of cwd.
cd "$(dirname "$0")/.."

REMOTE="${ZANED_REMOTE:-myserver:~/Zaned/}"

rsync -az --delete \
  --exclude target/ \
  --exclude node_modules/ \
  --exclude .git/ \
  --exclude .DS_Store \
  --exclude '*.swp' \
  ./ "$REMOTE"

echo "synced -> $REMOTE"
