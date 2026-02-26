# taurhaus development recipes

# Project paths
project   := justfile_directory()
win_dir   := "/mnt/d/taurhaus_build"
win_drive := "D:\\taurhaus_build"

# macOS remote build host (Scaleway Mac mini)
mac_host  := "m1@62.210.195.235"
mac_dir   := "~/projects/taurhaus"

# Run frontend dev server only
dev-frontend:
    npm run dev

# Run full Tauri dev (frontend + backend)
# Creates placeholder daemon resource if missing (Tauri validates at compile time)
dev:
    @mkdir -p src-tauri/resources && touch src-tauri/resources/taurhaus-daemon
    npm run dev:tauri

# Run all checks (quality gate)
check: lint test
    @echo "All checks passed."

# Lint everything
lint:
    cd src-tauri && cargo clippy --all-targets -- -D warnings
    npm run check

# Run all tests
test: test-rust test-frontend

# Run Rust tests
test-rust:
    cd src-tauri && cargo test

# Run frontend tests
test-frontend:
    npm run test

# Run frontend tests in watch mode
test-watch:
    npm run test:watch

# Run E2E tests (builds Tauri debug binary with embedded frontend, then tests via tauri-driver)
# Set E2E_SKIP_BUILD=1 to skip the build step if you already have a fresh binary.
test-e2e:
    npx wdio run e2e/wdio.conf.js

# Reset database (delete SQLite file)
db-reset:
    @echo "DB reset not yet configured — SQLite module pending"

# Run database migrations
db-migrate:
    @echo "DB migrations not yet configured — SQLite module pending"

# Build for Linux
build-linux:
    npm run tauri build

# Build the WSL daemon binary (Linux target)
build-daemon:
    cd src-tauri && cargo build --release --bin taurhaus-daemon

# Install daemon to ~/.local/bin/ (WSL)
# Automatically stops a running daemon before install and restarts it after.
install-daemon:
    #!/usr/bin/env bash
    set -euo pipefail

    DAEMON_BIN="taurhaus-daemon"
    INSTALL_DIR="$HOME/.local/bin"
    WAS_RUNNING=false

    # Check if daemon is currently running
    if pgrep -x "$DAEMON_BIN" >/dev/null 2>&1; then
        echo "▸ Stopping running $DAEMON_BIN…"
        pkill -x "$DAEMON_BIN" || true
        # Wait for it to actually exit (up to 5s)
        for i in $(seq 1 10); do
            if ! pgrep -x "$DAEMON_BIN" >/dev/null 2>&1; then break; fi
            sleep 0.5
        done
        if pgrep -x "$DAEMON_BIN" >/dev/null 2>&1; then
            echo "✗ Could not stop $DAEMON_BIN — force killing"
            pkill -9 -x "$DAEMON_BIN" || true
            sleep 0.5
        fi
        echo "✓ Daemon stopped"
        WAS_RUNNING=true
    fi

    # Build
    echo "▸ Building daemon…"
    cd src-tauri && cargo build --release --bin "$DAEMON_BIN"
    cd ..

    # Install
    mkdir -p "$INSTALL_DIR"
    cp "src-tauri/target/release/$DAEMON_BIN" "$INSTALL_DIR/"
    echo "✓ Installed $DAEMON_BIN to $INSTALL_DIR/"

    # Restart if it was running before
    if [ "$WAS_RUNNING" = true ]; then
        echo "▸ Restarting daemon…"
        nohup "$INSTALL_DIR/$DAEMON_BIN" >/dev/null 2>&1 &
        sleep 0.5
        if pgrep -x "$DAEMON_BIN" >/dev/null 2>&1; then
            echo "✓ Daemon restarted (PID $(pgrep -x $DAEMON_BIN))"
        else
            echo "⚠ Daemon did not restart — start it manually"
        fi
    fi

# Run the daemon in foreground (for development)
run-daemon:
    cd src-tauri && cargo run --bin taurhaus-daemon -- --verbose

# ── Windows Build (via WSL2 interop) ─────────────────────────────────────────

# Sync source to Windows build directory
sync-windows:
    @echo "▸ Syncing source to {{win_dir}}…"
    rsync -a --delete \
        --exclude='node_modules' \
        --exclude='target' \
        --exclude='dist' \
        --exclude='.git' \
        {{project}}/ {{win_dir}}/
    @echo "✓ Sync complete"

# Copy daemon binary to Tauri resources for bundling
bundle-daemon: build-daemon
    @echo "▸ Bundling daemon binary into src-tauri/resources/…"
    mkdir -p src-tauri/resources
    cp src-tauri/target/release/taurhaus-daemon src-tauri/resources/taurhaus-daemon
    @echo "✓ Daemon binary bundled"

# Build Windows NSIS installer (syncs first, builds natively on Windows)
# Also rebuilds the WSL daemon to keep them in sync.
build-windows: install-daemon bundle-daemon sync-windows
    @echo "Note: cmd.exe may print 'UNC paths are not supported'. This is harmless."
    @echo "▸ Installing frontend dependencies on Windows…"
    cmd.exe /c "cd /d {{win_drive}} && npm install"
    @echo ""
    @echo "▸ Building Windows NSIS installer (cargo tauri)…"
    cmd.exe /c "cd /d {{win_drive}} && cargo tauri build --bundles nsis"
    @echo ""
    @echo "✓ Windows build complete:"
    @ls -lh {{win_dir}}/src-tauri/target/release/bundle/nsis/*.exe 2>/dev/null || echo "  (no installer found)"

# ── macOS Build (via SSH to remote Mac mini) ─────────────────────────────────

# Sync source to remote Mac
sync-macos:
    @echo "▸ Syncing source to {{mac_host}}:{{mac_dir}}…"
    rsync -az --delete \
        --exclude='node_modules' \
        --exclude='target' \
        --exclude='dist' \
        --exclude='.git' \
        {{project}}/ {{mac_host}}:{{mac_dir}}/
    @echo "✓ Sync complete"

# Run tests on remote Mac
test-macos: sync-macos
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▸ Running tests on macOS…"
    ssh {{mac_host}} 'cd {{mac_dir}} && export PATH="$HOME/.homebrew/bin:$HOME/.cargo/bin:$PATH" && cd src-tauri && cargo test 2>&1'
    echo "✓ macOS tests passed"

# Build macOS app bundle (arm64) on remote Mac
build-macos: sync-macos
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▸ Installing frontend dependencies on macOS…"
    ssh {{mac_host}} 'cd {{mac_dir}} && export PATH="$HOME/.homebrew/bin:$HOME/.cargo/bin:$PATH" && npm install'
    echo ""
    echo "▸ Building daemon on macOS…"
    ssh {{mac_host}} 'cd {{mac_dir}} && export PATH="$HOME/.homebrew/bin:$HOME/.cargo/bin:$PATH" && cd src-tauri && cargo build --release --bin taurhaus-daemon'
    echo ""
    echo "▸ Installing daemon to ~/.local/bin/ on macOS…"
    ssh {{mac_host}} 'mkdir -p ~/.local/bin && cp {{mac_dir}}/src-tauri/target/release/taurhaus-daemon ~/.local/bin/ && codesign --force --sign - ~/.local/bin/taurhaus-daemon'
    echo ""
    echo "▸ Bundling daemon into resources…"
    ssh {{mac_host}} 'mkdir -p {{mac_dir}}/src-tauri/resources && cp {{mac_dir}}/src-tauri/target/release/taurhaus-daemon {{mac_dir}}/src-tauri/resources/ && codesign --force --sign - {{mac_dir}}/src-tauri/resources/taurhaus-daemon'
    echo ""
    echo "▸ Building macOS app (cargo tauri build)…"
    ssh {{mac_host}} 'cd {{mac_dir}} && export PATH="$HOME/.homebrew/bin:$HOME/.cargo/bin:$PATH" && cargo tauri build 2>&1'
    echo ""
    echo "✓ macOS build complete"

# Build universal macOS binary (arm64 + x86_64) on remote Mac
build-macos-universal: sync-macos
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▸ Installing frontend dependencies on macOS…"
    ssh {{mac_host}} 'cd {{mac_dir}} && export PATH="$HOME/.homebrew/bin:$HOME/.cargo/bin:$PATH" && npm install'
    echo ""
    echo "▸ Building universal macOS binary (arm64 + x86_64)…"
    ssh {{mac_host}} 'cd {{mac_dir}} && export PATH="$HOME/.homebrew/bin:$HOME/.cargo/bin:$PATH" && cargo tauri build --target universal-apple-darwin 2>&1'
    echo ""
    echo "✓ Universal macOS build complete"

# ── Daemon Connectivity Tests ────────────────────────────────────────────────
# Run these in order to verify the daemon chain step by step.

# Step 1: Linux-to-Linux TCP (daemon + client both in WSL)
# Proves: daemon starts, listens, responds to NDJSON protocol
test-daemon-local:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "── Step 1: Linux → Linux daemon connectivity ──"
    echo ""
    # Build daemon
    cd src-tauri && cargo build --bin taurhaus-daemon 2>&1 | tail -1
    echo "✓ Daemon built"
    # Start daemon on a random-ish port to avoid conflicts
    PORT=17299
    ./target/debug/taurhaus-daemon --port $PORT &
    DAEMON_PID=$!
    sleep 0.5
    # Ping it
    RESP=$(echo "{\"id\":\"t1\",\"method\":\"ping\",\"params\":null}" | nc -w 2 localhost $PORT 2>/dev/null || true)
    kill $DAEMON_PID 2>/dev/null || true
    wait $DAEMON_PID 2>/dev/null || true
    if echo "$RESP" | grep -q '"version"'; then
        echo "✓ Ping response: $RESP"
        echo ""
        echo "PASS: Linux → Linux TCP works"
    else
        echo "✗ No valid response. Got: $RESP"
        echo ""
        echo "FAIL: Daemon didn't respond"
        exit 1
    fi

# Step 2: Windows-to-Linux TCP (PowerShell client → WSL daemon)
# Proves: Windows processes can reach the daemon over localhost
test-daemon-windows:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "── Step 2: Windows → Linux daemon connectivity ──"
    echo ""
    # Build daemon
    cd src-tauri && cargo build --bin taurhaus-daemon 2>&1 | tail -1
    echo "✓ Daemon built"
    PORT=17299
    ./target/debug/taurhaus-daemon --port $PORT &
    DAEMON_PID=$!
    sleep 0.5
    echo "✓ Daemon running on :$PORT (PID $DAEMON_PID)"
    echo ""
    # Test TCP reachability from Windows via PowerShell
    echo "Testing TCP port from Windows PowerShell…"
    REACH=$(powershell.exe -NoProfile -Command "(Test-NetConnection -ComputerName localhost -Port $PORT -WarningAction SilentlyContinue).TcpTestSucceeded" 2>/dev/null | tr -d '\r\n')
    if [ "$REACH" = "True" ]; then
        echo "✓ Windows can reach localhost:$PORT"
    else
        echo "✗ Windows cannot reach localhost:$PORT (got: '$REACH')"
        echo ""
        echo "  This usually means WSL2 is in NAT mode (not mirrored)."
        echo "  Fix: add to %USERPROFILE%\\.wslconfig:"
        echo "    [wsl2]"
        echo "    networkingMode=mirrored"
        echo "  Then: wsl --shutdown && restart WSL"
        echo ""
        kill $DAEMON_PID 2>/dev/null || true
        echo "FAIL: TCP port not reachable from Windows"
        exit 1
    fi
    echo ""
    # Send actual NDJSON ping from Windows PowerShell
    echo "Sending NDJSON ping from PowerShell…"
    # Write a temp PS1 script with port baked in (avoids env-var passing issues)
    PSSCRIPT=$(mktemp /tmp/daemon-test-XXXX.ps1)
    printf '%s\n' \
        "\$c = New-Object System.Net.Sockets.TcpClient('localhost', $PORT)" \
        '$s = $c.GetStream()' \
        '$w = New-Object System.IO.StreamWriter($s)' \
        '$r = New-Object System.IO.StreamReader($s)' \
        '$w.WriteLine('"'"'{"id":"t1","method":"ping","params":null}'"'"')' \
        '$w.Flush()' \
        '$line = $r.ReadLine()' \
        'Write-Output $line' \
        '$c.Close()' > "$PSSCRIPT"
    # Convert WSL path to Windows path for PowerShell
    WIN_SCRIPT=$(wslpath -w "$PSSCRIPT")
    RESP=$(powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$WIN_SCRIPT" 2>/dev/null | tr -d '\r')
    rm -f "$PSSCRIPT"
    kill $DAEMON_PID 2>/dev/null || true
    wait $DAEMON_PID 2>/dev/null || true
    if echo "$RESP" | grep -q '"version"'; then
        echo "✓ Ping response: $RESP"
        echo ""
        echo "PASS: Windows → Linux NDJSON works"
    else
        echo "✗ No valid response. Got: $RESP"
        echo ""
        echo "FAIL: NDJSON protocol failed from Windows"
        exit 1
    fi

# Step 3: Auto-start test (Windows launches daemon via wsl.exe)
# Proves: the app's auto-start mechanism works
test-daemon-autostart:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "── Step 3: Auto-start via wsl.exe ──"
    echo ""
    # Ensure daemon is installed
    if [ ! -f ~/.local/bin/taurhaus-daemon ]; then
        echo "Daemon not installed. Run: just install-daemon"
        exit 1
    fi
    echo "✓ Daemon binary found at ~/.local/bin/taurhaus-daemon"
    PORT=17299
    # Kill any existing daemon on this port
    pkill -f "taurhaus-daemon.*--port $PORT" 2>/dev/null || true
    sleep 0.3
    # Launch from Windows side (same as the app would)
    DISTRO=$(wsl.exe -l -q 2>/dev/null | head -1 | tr -d '\r\0' || echo "")
    if [ -z "$DISTRO" ]; then
        echo "✗ Could not detect WSL distro"
        exit 1
    fi
    echo "Using distro: $DISTRO"
    echo "Launching daemon via: wsl.exe -d $DISTRO …"
    wsl.exe -d "$DISTRO" -- ~/.local/bin/taurhaus-daemon --port $PORT --idle-timeout 30 &
    WSL_PID=$!
    sleep 2
    # Check if it's reachable
    RESP=$(echo "{\"id\":\"t1\",\"method\":\"ping\",\"params\":null}" | nc -w 2 localhost $PORT 2>/dev/null || true)
    # Clean up
    echo "{\"id\":\"s1\",\"method\":\"shutdown\",\"params\":null}" | nc -w 2 localhost $PORT 2>/dev/null || true
    sleep 0.5
    kill $WSL_PID 2>/dev/null || true
    if echo "$RESP" | grep -q '"version"'; then
        echo "✓ Ping response: $RESP"
        echo ""
        echo "PASS: Auto-start via wsl.exe works"
    else
        echo "✗ No valid response. Got: $RESP"
        echo ""
        echo "FAIL: Daemon didn't start via wsl.exe"
        exit 1
    fi

# Run all daemon connectivity tests in order
test-daemon-connectivity: test-daemon-local test-daemon-windows test-daemon-autostart
    @echo ""
    @echo "═══════════════════════════════════════"
    @echo "  All daemon connectivity tests passed"
    @echo "═══════════════════════════════════════"

# Take screenshot of current app state
screenshot:
    @echo "Screenshot recipe not yet configured"

# Security audit (integration tasks + phase boundaries)
security-audit:
    cd src-tauri && cargo audit 2>/dev/null || echo "cargo-audit not installed — run: cargo install cargo-audit"
