#!/usr/bin/env bash
# Optimize images for documentation.
# Resizes to max 1600px wide and compresses to quality 85.
#
# Usage:
#   ./scripts/optimize-doc-image.sh <input> [output]
#   ./scripts/optimize-doc-image.sh <input> [output] [max-width] [quality]
#
# Examples:
#   ./scripts/optimize-doc-image.sh ~/taursult/output/images/gemini/img.jpg docs/my-diagram.jpg
#   ./scripts/optimize-doc-image.sh raw.jpg docs/diagram.jpg 1200 90
#
# If output is omitted, the input file is overwritten in place.
# Requires ImageMagick (convert command).

set -euo pipefail

MAX_WIDTH="${3:-1600}"
QUALITY="${4:-85}"
INPUT="$1"
OUTPUT="${2:-$1}"

if [[ -z "${INPUT:-}" ]]; then
  echo "Usage: $0 <input> [output] [max-width] [quality]"
  echo "  max-width  default: 1600"
  echo "  quality    default: 85"
  exit 1
fi

if ! command -v convert &>/dev/null; then
  echo "Error: ImageMagick 'convert' not found. Install with: sudo apt install imagemagick" >&2
  exit 1
fi

if [[ ! -f "$INPUT" ]]; then
  echo "Error: input file not found: $INPUT" >&2
  exit 1
fi

BEFORE=$(stat --printf='%s' "$INPUT" 2>/dev/null || stat -f '%z' "$INPUT")

convert "$INPUT" -resize "${MAX_WIDTH}x>" -quality "$QUALITY" "$OUTPUT"

AFTER=$(stat --printf='%s' "$OUTPUT" 2>/dev/null || stat -f '%z' "$OUTPUT")
DIMS=$(identify -format '%wx%h' "$OUTPUT" 2>/dev/null || echo "unknown")

echo "$(basename "$OUTPUT"): ${DIMS}, $(numfmt --to=iec "$AFTER" 2>/dev/null || echo "${AFTER} bytes") (was $(numfmt --to=iec "$BEFORE" 2>/dev/null || echo "${BEFORE} bytes"))"
