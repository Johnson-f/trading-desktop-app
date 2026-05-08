#!/usr/bin/env bash
# Terminal 1 — auto-rsync on local file save.
#
# Watches the project for source changes and, on each batch of edits,
# fires `.script/deploy.sh` to push the diff to the VPS. Pair with
# `.script/server-watch.sh` running in another terminal (which has
# `cargo watch` listening on the synced files and rebuilds the
# gateway whenever the rsync lands).
#
# Usage:
#   script/dev-watch.sh
#
# Tweak the watched extensions or debounce by editing the watchexec
# flags below.

set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v watchexec >/dev/null 2>&1; then
  echo "watchexec not found. Install with: brew install watchexec" >&2
  exit 1
fi

echo "==> watching for changes (Ctrl-C to stop)"

# --exts:    file types that should trigger a sync
# --debounce: coalesce bursts (e.g. cargo touching many files at once)
# --no-vcs-ignore: VCS isn't authoritative for what we sync; rsync's
#                  own --exclude rules are.
exec watchexec \
  --exts rs,toml,graphql \
  --debounce 500ms \
  --no-vcs-ignore \
  -- ./.script/deploy.sh
