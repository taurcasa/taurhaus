#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT
cd "$repo_root"

# Regression: bcf1f3924f7186b704b5bc5542c937e75c3bac7f captured $? after
# `! wait -n`, so a failed lane returned success (found by the Opus
# tests/procedures review, 2026-08-30).
failure_output="$tmp_dir/failure.log"
failure_status=0
TAURHAUS_CHECK_SEED_FAILURE=rust just check >"$failure_output" 2>&1 || failure_status=$?
if [ "$failure_status" -ne 3 ]; then
    echo "seeded just check failure exited $failure_status, expected 3" >&2
    sed -n '1,120p' "$failure_output" >&2
    exit 1
fi
if rg -Fq "Full quality gate passed." "$failure_output"; then
    echo "seeded just check failure printed the success line" >&2
    exit 1
fi

green_output="$tmp_dir/green.log"
green_status=0
TAURHAUS_CHECK_SEED_FAILURE=green just check >"$green_output" 2>&1 || green_status=$?
if [ "$green_status" -ne 0 ]; then
    echo "seeded green just check exited $green_status, expected 0" >&2
    sed -n '1,120p' "$green_output" >&2
    exit 1
fi
if ! rg -Fq "Full quality gate passed." "$green_output"; then
    echo "seeded green just check omitted the success line" >&2
    exit 1
fi

echo "just check gate guard passed."
