#!/usr/bin/env bash
# Terminal 2 — auto-rebuild + restart the gateway on the VPS.
#
# Opens an SSH session to `myserver` and starts `cargo watch` against
# the rsync'd source tree at ~/Zaned. Whenever `dev-watch.sh` lands a
# new sync, cargo-watch notices the file changes, recompiles, and
# restarts `zaned-server`. Output (build errors, gateway logs)
# streams back here.
#
# Usage:
#   script/server-watch.sh
#
# Stop with Ctrl-C — the SSH session and the remote `cargo watch`
# both exit cleanly.

set -euo pipefail

REMOTE_HOST="${ZANED_REMOTE_HOST:-myserver}"
REMOTE_PATH="${ZANED_REMOTE_PATH:-~/Zaned}"

echo "==> ssh $REMOTE_HOST -> cargo watch (Ctrl-C to stop)"

# bash -lc loads the login profile so cargo / cargo-watch are on PATH.
# `-t` forces TTY allocation so cargo-watch's terminal-clearing
# behaviour and coloured output work.
exec ssh -t "$REMOTE_HOST" \
  "bash -lc 'cd $REMOTE_PATH && RUST_BACKTRACE=full cargo watch -x \"run -p zaned-server\"'"
