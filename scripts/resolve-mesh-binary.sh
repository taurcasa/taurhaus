#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ -n "${MESH_BIN:-}" ] && [ -x "${MESH_BIN}" ] && [ -s "${MESH_BIN}" ]; then
    printf '%s\n' "${MESH_BIN}"
    exit 0
fi

MESH_PROJECT="${MESH_PROJECT:-$HOME/projects/mesh}"
PROJECT_BINARY="$MESH_PROJECT/target/release/mesh"

if [ -d "$MESH_PROJECT" ]; then
    if [ ! -x "$PROJECT_BINARY" ] || [ ! -s "$PROJECT_BINARY" ]; then
        echo "▸ Building mesh from $MESH_PROJECT…" >&2
        (cd "$MESH_PROJECT" && cargo build --release --bin mesh) >&2
    fi
    if [ -x "$PROJECT_BINARY" ] && [ -s "$PROJECT_BINARY" ]; then
        printf '%s\n' "$PROJECT_BINARY"
        exit 0
    fi
fi

for candidate in \
    "$PROJECT_ROOT/src-tauri/resources/mesh" \
    "$HOME/.local/bin/mesh"
do
    if [ -x "$candidate" ] && [ -s "$candidate" ]; then
        printf '%s\n' "$candidate"
        exit 0
    fi
done

echo "✗ No mesh binary available." >&2
echo "  Looked for a built mesh workspace at: $PROJECT_BINARY" >&2
echo "  Also checked: $PROJECT_ROOT/src-tauri/resources/mesh and $HOME/.local/bin/mesh" >&2
echo "  Set MESH_BIN=/path/to/mesh to reuse an existing lock-matching binary," >&2
echo "  or set MESH_PROJECT=/path/to/mesh to build from source." >&2
exit 1
