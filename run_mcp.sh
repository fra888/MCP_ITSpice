#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"

if [[ ! -x "$DIR/target/debug/MCP_ITSpice" ]]; then
    (cd "$DIR" && cargo build --quiet >&2)
fi

exec "$DIR/target/debug/MCP_ITSpice" --mcp "$@"
