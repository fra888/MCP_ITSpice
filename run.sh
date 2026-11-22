#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"

cd "$DIR"
cargo build --quiet
exec "$DIR/target/debug/MCP_ITSpice" "$@"
