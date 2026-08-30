#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tmp_dir=$(mktemp -d)
work_dir="$tmp_dir/work"
mkdir -p "$work_dir"
cp "$repo_root/package.json" "$work_dir/package.json"

rust_peer_pid_file="$tmp_dir/rust-peer.pid"
frontend_peer_pid_file="$tmp_dir/frontend-peer.pid"

cleanup() {
    for pid_file in "$rust_peer_pid_file" "$frontend_peer_pid_file"; do
        if [ ! -s "$pid_file" ]; then
            continue
        fi
        pid=$(sed -n '1p' "$pid_file")
        if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
    done
    rm -rf "$tmp_dir"
}
trap cleanup EXIT

# Regression: bcf1f3924f7186b704b5bc5542c937e75c3bac7f captured $? after
# `! wait -n`, so a failed lane returned success (found by the Opus
# tests/procedures review, 2026-08-30). Seeded runs must also remain visibly
# test-only, isolate their logs, and stop the peer lane before returning.
# Use grep, not ripgrep: rg is absent from the target's non-interactive PATH.
assert_seed_warning() {
    local seed="$1"
    local output="$2"
    if ! grep -Fq "WARNING: TAURHAUS_CHECK_SEED_FAILURE=$seed" "$output" ||
        ! grep -Fq "NOT a real gate" "$output"; then
        echo "seeded $seed just check omitted the test-only warning" >&2
        sed -n '1,120p' "$output" >&2
        exit 1
    fi
}

assert_isolated_log() {
    local log_dir="$1"
    local output="$2"
    if ! grep -Fq "Logging full check output to $log_dir/" "$output"; then
        echo "seeded just check ignored its isolated log directory" >&2
        sed -n '1,120p' "$output" >&2
        exit 1
    fi
    if ! find "$log_dir" -maxdepth 1 -type f -name 'check-*.log' -print -quit | grep -q .; then
        echo "seeded just check did not create a log in $log_dir" >&2
        exit 1
    fi
}

assert_peer_stopped() {
    local pid_file="$1"
    local seed="$2"
    if [ ! -s "$pid_file" ]; then
        echo "seeded $seed just check did not identify its peer lane" >&2
        exit 1
    fi
    local pid
    pid=$(sed -n '1p' "$pid_file")
    if [[ ! "$pid" =~ ^[0-9]+$ ]]; then
        echo "seeded $seed just check wrote an invalid peer pid: $pid" >&2
        exit 1
    fi
    if kill -0 "$pid" 2>/dev/null; then
        echo "seeded $seed just check left peer lane $pid running" >&2
        exit 1
    fi
}

failure_output="$tmp_dir/failure.log"
failure_log_dir="$tmp_dir/failure-check-logs"
failure_status=0
TAURHAUS_CHECK_LOG_DIR="$failure_log_dir" \
    TAURHAUS_CHECK_SEED_FAILURE=rust \
    TAURHAUS_CHECK_SEED_PEER_PID_FILE="$rust_peer_pid_file" \
    just --justfile "$repo_root/justfile" --working-directory "$work_dir" check \
    >"$failure_output" 2>&1 || failure_status=$?
if [ "$failure_status" -ne 3 ]; then
    echo "seeded just check failure exited $failure_status, expected 3" >&2
    sed -n '1,120p' "$failure_output" >&2
    exit 1
fi
if grep -Fq "Full quality gate passed." "$failure_output"; then
    echo "seeded just check failure printed the success line" >&2
    exit 1
fi
assert_isolated_log "$failure_log_dir" "$failure_output"
assert_seed_warning rust "$failure_output"
assert_peer_stopped "$rust_peer_pid_file" rust

frontend_output="$tmp_dir/frontend-failure.log"
frontend_log_dir="$tmp_dir/frontend-check-logs"
frontend_status=0
TAURHAUS_CHECK_LOG_DIR="$frontend_log_dir" \
    TAURHAUS_CHECK_SEED_FAILURE=frontend \
    TAURHAUS_CHECK_SEED_PEER_PID_FILE="$frontend_peer_pid_file" \
    just --justfile "$repo_root/justfile" --working-directory "$work_dir" check \
    >"$frontend_output" 2>&1 || frontend_status=$?
if [ "$frontend_status" -ne 3 ]; then
    echo "seeded frontend just check failure exited $frontend_status, expected 3" >&2
    sed -n '1,120p' "$frontend_output" >&2
    exit 1
fi
if grep -Fq "Full quality gate passed." "$frontend_output"; then
    echo "seeded frontend just check failure printed the success line" >&2
    exit 1
fi
assert_isolated_log "$frontend_log_dir" "$frontend_output"
assert_seed_warning frontend "$frontend_output"
assert_peer_stopped "$frontend_peer_pid_file" frontend

green_output="$tmp_dir/green.log"
green_log_dir="$tmp_dir/green-check-logs"
green_status=0
TAURHAUS_CHECK_LOG_DIR="$green_log_dir" \
    TAURHAUS_CHECK_SEED_FAILURE=green \
    just --justfile "$repo_root/justfile" --working-directory "$work_dir" check \
    >"$green_output" 2>&1 || green_status=$?
if [ "$green_status" -ne 0 ]; then
    echo "seeded green just check exited $green_status, expected 0" >&2
    sed -n '1,120p' "$green_output" >&2
    exit 1
fi
if ! grep -Fq "Full quality gate passed." "$green_output"; then
    echo "seeded green just check omitted the success line" >&2
    exit 1
fi
assert_isolated_log "$green_log_dir" "$green_output"
assert_seed_warning green "$green_output"

echo "just check gate guard passed."
