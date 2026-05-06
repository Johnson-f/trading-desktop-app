#!/usr/bin/env bash
# Run the full test suite across every workspace member.
#
# Usage:
#   script/testing.sh                 # run all tests
#   script/testing.sh -p chart-core   # forward args to cargo test
#
# Exits with cargo's exit code (non-zero on first failure unless
# --no-fail-fast is passed).

set -euo pipefail

# Run from repo root so relative paths resolve regardless of cwd.
cd "$(dirname "$0")/.."

echo "==> cargo test --workspace $*"
cargo test --workspace --no-fail-fast "$@"
