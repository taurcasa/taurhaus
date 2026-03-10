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

run_step "build_daemon" just build-daemon
run_step "install_daemon" just _install-daemon-from-build
run_step "bundle_daemon" just _bundle-daemon-from-build
run_step "bundle_mesh" just bundle-mesh
run_step "sync_windows" just sync-windows

echo "Note: cmd.exe may print 'UNC paths are not supported'. This is harmless."
PS_SCRIPT="$(wslpath -w "$PROJECT_ROOT/scripts/build-windows.ps1")"
WIN_PROJECT_DIR="$(wslpath -w "$WINDOWS_BUILD_DIR")"

if [ "$USE_SCCACHE" = "1" ]; then
    run_step "windows_build" powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$PS_SCRIPT" -ProjectDir "$WIN_PROJECT_DIR" -EnableSccache
else
    run_step "windows_build" powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$PS_SCRIPT" -ProjectDir "$WIN_PROJECT_DIR"
fi

echo
echo "WSL step summary:"
for i in "${!STEP_NAMES[@]}"; do
    printf '  %-18s %ss\n' "${STEP_NAMES[$i]}" "${STEP_SECONDS[$i]}"
done

echo
echo "✓ Windows build complete:"
ls -lh "$WINDOWS_BUILD_DIR"/src-tauri/target/release/bundle/nsis/*.exe 2>/dev/null || echo "  (no installer found)"
