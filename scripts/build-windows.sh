#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="${1:?usage: build-windows.sh <project-root> <windows-build-dir>}"
WINDOWS_BUILD_DIR="${2:?usage: build-windows.sh <project-root> <windows-build-dir>}"
USE_SCCACHE="${TAURHAUS_WINDOWS_USE_SCCACHE:-0}"

cd "$PROJECT_ROOT"

run_step() {
    local name="$1"
    shift

    local start end elapsed
    start=$(date +%s)
    echo "▸ [$name] starting..."
    "$@"
    end=$(date +%s)
    elapsed=$((end - start))
    printf '✓ [%s] %ss\n' "$name" "$elapsed"
    STEP_NAMES+=("$name")
    STEP_SECONDS+=("$elapsed")
}

STEP_NAMES=()
STEP_SECONDS=()

run_step "check_windows_prereqs" just check-windows-build-prereqs
run_step "build_daemon" just build-daemon
# The daemon is NOT installed here: the app and daemon must match PROTOCOL_VERSION
# exactly, and installing the new daemon before the new app is installed leaves
# the running app rejecting it on every reconnect. `just install-windows` installs
# both together.
run_step "bundle_daemon" just _bundle-daemon-from-build
run_step "bundle_mesh" just bundle-mesh
run_step "sync_windows" just sync-windows

echo "Note: cmd.exe may print 'UNC paths are not supported'. This is harmless."
PS_SCRIPT="$(wslpath -w "$PROJECT_ROOT/scripts/build-windows.ps1")"
WIN_PROJECT_DIR="$(wslpath -w "$WINDOWS_BUILD_DIR")"

if [ "$USE_SCCACHE" = "1" ]; then
    run_step "windows_build" sh -c 'exec powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$1" -ProjectDir "$2" -EnableSccache < /dev/null' sh "$PS_SCRIPT" "$WIN_PROJECT_DIR"
else
    run_step "windows_build" sh -c 'exec powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$1" -ProjectDir "$2" < /dev/null' sh "$PS_SCRIPT" "$WIN_PROJECT_DIR"
fi

echo
echo "WSL step summary:"
for i in "${!STEP_NAMES[@]}"; do
    printf '  %-18s %ss\n' "${STEP_NAMES[$i]}" "${STEP_SECONDS[$i]}"
done

echo
echo "✓ Windows build complete:"
ls -lh "$WINDOWS_BUILD_DIR"/src-tauri/target/release/bundle/nsis/*.exe 2>/dev/null || echo "  (no installer found)"
