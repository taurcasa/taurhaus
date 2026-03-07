#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ARTIFACT_DIR="src/test/visual/__screenshots__/readme"
OUTPUT_DIR="docs/screenshots"
SPEC="src/test/visual/specs/readme.visual.test.js"

FILES=(
  "readme-hero-overview.png"
  "readme-sidebar-live-supervision.png"
  "readme-task-board-context.png"
  "readme-search-overlay.png"
  "readme-mesh-setup-composition.png"
  "readme-mesh-runtime-canvas.png"
  "readme-mesh-recovery-resume.png"
  "readme-git-context-inspection.png"
)

mkdir -p "$OUTPUT_DIR"

bunx vitest run --config vitest.visual.config.js "$SPEC"

for file in "${FILES[@]}"; do
  src="$ARTIFACT_DIR/$file"
  dst="$OUTPUT_DIR/$file"

  if [[ ! -f "$src" ]]; then
    echo "Missing visual artifact: $src" >&2
    exit 1
  fi

  ./scripts/optimize-doc-image.sh "$src" "$dst"
done

echo "Exported README screenshots to $OUTPUT_DIR"
