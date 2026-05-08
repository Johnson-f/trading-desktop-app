#!/usr/bin/env bash
# Push the local Zaned source tree to the VPS over rsync.
#
# Excluded:
#   target/        — build output, machine-specific, gigabytes
#   node_modules/  — JS deps, regenerable
#   .DS_Store      — macOS junk
#   *.swp          — vim/editor swap files
#   .env           — VPS keeps its own (with localhost service URLs)
#
# Used by `watchexec` for the auto-sync loop:
#   watchexec --exts rs,toml,graphql --debounce 500ms -- ./sync.sh
set -euo pipefail

rsync -az --delete \
  --exclude target/ \
  --exclude node_modules/ \
  --exclude .DS_Store \
  --exclude '*.swp' \
  ~/Zaned/ myserver:~/Zaned/

echo "synced"
