#!/bin/bash
# Bake the gateway URL into the desktop binary at compile time.
# Read by `option_env!("ZANED_SERVER_URL")` in apps/desktop/src/api_client.rs;
# unset → falls back to http://localhost:8765.
ZANED_SERVER_URL="${ZANED_SERVER_URL:-http://vmi3280791.contaboserver.net:8765}" \
RUST_BACKTRACE=full \
cargo run -p Zaned
