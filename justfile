# taurhaus development recipes
# Single file by design — split into `import`s when this exceeds ~1200 lines.

# Project paths
project   := justfile_directory()
win_dir   := "/mnt/d/taurhaus_build"
win_drive := "D:\\taurhaus_build"

# macOS remote build host (Scaleway Mac mini)
mac_host  := "m1@62.210.195.235"
mac_dir   := "~/projects/taurhaus"

# Run frontend dev server only
dev-frontend:
    bun run dev

# Run full Tauri dev (frontend + backend)
dev: ensure-tauri-resources
    bun run dev:tauri

# Ensure required Tauri resource files exist for local compile/test lanes.
ensure-tauri-resources:
    @mkdir -p src-tauri/resources
    @if [ ! -e src-tauri/resources/taurhaus-daemon ]; then touch src-tauri/resources/taurhaus-daemon; fi
    @if [ ! -e src-tauri/resources/mesh ]; then touch src-tauri/resources/mesh; fi
    @if [ ! -s src-tauri/resources/mesh.version ]; then echo "0.0.0-dev" > src-tauri/resources/mesh.version; fi

# Full quality gate (pre-commit): formatting + lint + typecheck + all non-E2E tests.
# Use this when you need the definitive "is this ready?" signal.
check:
    #!/usr/bin/env bash
    set -euo pipefail
    log_dir=".check-logs"
    mkdir -p "$log_dir"
    log_path="$log_dir/check-$(date +%F-%H%M%S).log"
    : > "$log_path"
    exec > >(tee -a "$log_path") 2>&1
    trap 'status=$?; if [ "$status" -ne 0 ]; then echo "just check failed with exit code $status"; fi' EXIT
    echo "Logging full check output to $log_path"
    ls -1dt "$log_dir"/check-*.log 2>/dev/null | tail -n +6 | xargs -r rm -f
    just fmt
    just lint
    just typecheck
    just test
    echo "Full quality gate passed."

# Backward-compatible alias for the full quality gate.
# Prefer: `just check`.
check-full: check

# Fast formatting + compilation + type/test checks for iteration.
# Use `just check` for the full quality gate.
check-quick: ensure-tauri-resources
    cd src-tauri && cargo fmt
    cd src-tauri && cargo check --tests
    bun run typecheck
    bun run test
    @echo "Quick check passed."

# Enforce Rust formatting.
fmt:
    cd src-tauri && cargo fmt --check

# Lint everything
lint: ensure-tauri-resources
    cd src-tauri && cargo clippy --all-targets -- -D warnings
    bun run lint

# Typecheck frontend code
typecheck:
    bun run typecheck

# Generate a comprehensive quality KPI report (tests, coverage, build health, code size, E2E inventory).
# Use this at milestones/pre-release checkpoints for a single health snapshot.
metrics:
    ./scripts/metrics.sh

# Run the unified resource monitor (live table by default).
# Example: just monitor --samples 1 --interval 1
monitor *ARGS:
    python3 scripts/resource-monitor.py {{ARGS}}

# Run all non-E2E tests (Rust unit + Rust integration/system + frontend unit).
# This is the primary "does everything work?" test command.
test: test-rust test-frontend

# Fast feedback lane for local iteration.
# Runs Rust compile-check only (no Rust test execution) + frontend unit tests.
test-fast: test-rust-fast test-frontend

# Backward-compatible alias for full non-E2E tests.
# Prefer: `just test`.
test-full: test

# Run all Rust test lanes (compile-check + unit execution + integration/system execution).
test-rust: test-rust-fast test-rust-unit test-rust-integration

# Rust fast lane: compile all Rust tests without executing them.
# Use for quick compile feedback.
test-rust-fast: ensure-tauri-resources
    cd src-tauri && cargo check --tests

# Rust unit-test execution lane (excludes heavy daemon/network/watcher suites).
test-rust-unit: ensure-tauri-resources
    cd src-tauri && cargo test --lib --bins -- --test-threads=1 --skip daemon::server::tests:: --skip daemon::event_listener::tests:: --skip provider::daemon_client::tests:: --skip daemon::launcher::tests:: --skip fs::watcher::tests::watcher_starts_and_stops --skip fs::watcher::tests::unwatch_all_clears_everything

# Rust integration/system lane (serialized, includes heavy suites).
test-rust-integration: ensure-tauri-resources
    cd src-tauri && cargo test --tests -- --test-threads=1
    cd src-tauri && cargo test --lib daemon::server::tests:: -- --test-threads=1
    cd src-tauri && cargo test --lib daemon::event_listener::tests:: -- --test-threads=1
    cd src-tauri && cargo test --lib provider::daemon_client::tests:: -- --test-threads=1
    cd src-tauri && cargo test --lib daemon::launcher::tests:: -- --test-threads=1
    cd src-tauri && cargo test --lib fs::watcher::tests::watcher_starts_and_stops -- --test-threads=1
    cd src-tauri && cargo test --lib fs::watcher::tests::unwatch_all_clears_everything -- --test-threads=1

# Bisect default Rust unit-test lane by module groups with checkpoints
test-rust-bisect-unit:
    ./scripts/rust-test-bisect.sh unit

# Bisect heavy daemon/network/watcher suites with checkpoints
test-rust-bisect-heavy:
    ./scripts/rust-test-bisect.sh heavy

# Bisect the commands module into sub-groups
test-rust-bisect-commands:
    ./scripts/rust-test-bisect.sh commands

# Bisect the coordination module into sub-groups
test-rust-bisect-coordination:
    ./scripts/rust-test-bisect.sh coordination

# Bisect coordination orchestrator tests one-by-one
test-rust-bisect-orchestrator:
    ./scripts/rust-test-bisect.sh orchestrator

# Run frontend tests
test-frontend:
    bun run test

# Run browser-mode visual screenshot tests only.
test-visual:
    bunx vitest run --config vitest.visual.config.js

# Run frontend tests in watch mode
test-watch:
    bun run test:watch

# Build the Tauri debug binary for E2E tests.
# IMPORTANT: Always use this (not raw `cargo build`) — Tauri needs --debug --no-bundle
# for embedded asset serving. A plain `cargo build` produces a binary that tries to
# connect to a dev server, resulting in a blank page.
build-e2e:
    bunx tauri build --debug --no-bundle

# Run E2E tests — Tier 1 only.
# By default this does NOT run install-daemon (to avoid killing/restarting
# a live daemon during local E2E). Opt in with E2E_INSTALL_DAEMON=1.
# Builds the app automatically unless E2E_SKIP_BUILD=1 is set.
test-e2e: e2e-prepare-daemon
    bunx wdio run e2e/wdio.conf.js --exclude 'e2e/specs/daemon-integration.js'

# Run E2E tests — Tier 1 + Tier 2 (daemon must be running)
# By default this does NOT run install-daemon (to avoid killing/restarting
# a live daemon during local E2E). Opt in with E2E_INSTALL_DAEMON=1.
test-e2e-full: e2e-prepare-daemon
    bunx wdio run e2e/wdio.conf.js

# Run a single E2E spec file.
# By default this does NOT run install-daemon (to avoid killing/restarting
# a live daemon during local E2E). Opt in with E2E_INSTALL_DAEMON=1.
# Builds by default (safe). Set E2E_SKIP_BUILD=1 explicitly if you already built.
test-e2e-spec SPEC: e2e-prepare-daemon
    bunx wdio run e2e/wdio.conf.js --spec e2e/specs/{{SPEC}}.js

# Optional daemon prep for E2E runs.
# Default is safe/no-op. Set E2E_INSTALL_DAEMON=1 to rebuild/reinstall daemon.
e2e-prepare-daemon:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "${E2E_INSTALL_DAEMON:-0}" = "1" ]; then
        echo "▸ E2E_INSTALL_DAEMON=1 -> running install-daemon"
        just install-daemon
    else
        echo "▸ Skipping install-daemon for E2E (set E2E_INSTALL_DAEMON=1 to enable)"
    fi

# Reset database (delete SQLite file)
db-reset:
    @echo "DB reset not yet configured — SQLite module pending"

# Run database migrations
db-migrate:
    @echo "DB migrations not yet configured — SQLite module pending"

# Build for Linux
build-linux: bundle-daemon bundle-mesh
    bun run tauri build

# Build the WSL daemon binary (Linux target)
build-daemon:
    @mkdir -p src-tauri/resources && touch src-tauri/resources/taurhaus-daemon
    cd src-tauri && cargo build --release --bin taurhaus-daemon

# Build the mesh CLI binary (Linux target, local workspace).
build-mesh:
    #!/usr/bin/env bash
    set -euo pipefail
    MESH_PROJECT="${MESH_PROJECT:-/home/mstie/projects/mesh}"
    if [ ! -d "$MESH_PROJECT" ]; then
        echo "✗ Mesh project not found at $MESH_PROJECT"
        exit 1
    fi
    echo "▸ Building mesh from $MESH_PROJECT…"
    cd "$MESH_PROJECT" && cargo build --release --bin mesh

# Verify built mesh binary matches pinned lock manifest (Phase 0: --version string only).
mesh-verify-lock: build-mesh
    #!/usr/bin/env bash
    set -euo pipefail
    LOCK_FILE="src-tauri/resources/mesh.lock.json"
    MESH_PROJECT="${MESH_PROJECT:-/home/mstie/projects/mesh}"
    MESH_BIN="$MESH_PROJECT/target/release/mesh"
    if [ ! -f "$LOCK_FILE" ]; then
        echo "✗ Lock manifest not found at $LOCK_FILE"
        exit 1
    fi
    if [ ! -x "$MESH_BIN" ]; then
        echo "✗ Built mesh binary not found at $MESH_BIN"
        exit 1
    fi
    LOCK_VERSION=$(grep '"version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"version":[[:space:]]*"([^"]+)".*/\1/')
    if [ -z "$LOCK_VERSION" ]; then
        echo "✗ Could not parse locked version from $LOCK_FILE"
        exit 1
    fi
    BUILT_VERSION="$("$MESH_BIN" --version | awk '{print $2}')"
    if [ -z "$BUILT_VERSION" ]; then
        echo "✗ Could not determine mesh version from $MESH_BIN --version"
        exit 1
    fi
    if [ "$BUILT_VERSION" != "$LOCK_VERSION" ]; then
        echo "✗ Mesh version mismatch: lock=$LOCK_VERSION built=$BUILT_VERSION"
        exit 1
    fi
    echo "✓ Mesh lock verification passed (version $LOCK_VERSION)"

# Intentional entry point for bumping mesh lock manifest.
update-mesh-lock version protocol_version="1" schema_version="1" git_commit="":
    #!/usr/bin/env bash
    set -euo pipefail
    LOCK_FILE="src-tauri/resources/mesh.lock.json"
    mkdir -p src-tauri/resources
    GIT_COMMIT="{{git_commit}}"
    if [ -z "$GIT_COMMIT" ]; then
        GIT_COMMIT_JSON=null
    else
        GIT_COMMIT_JSON="\"$GIT_COMMIT\""
    fi
    cat > "$LOCK_FILE" <<JSON
    {
      "version": "{{version}}",
      "protocol_version": {{protocol_version}},
      "schema_version": {{schema_version}},
      "git_commit": $GIT_COMMIT_JSON
    }
    JSON
    echo "✓ Updated mesh lock manifest at $LOCK_FILE"

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

    # Ensure resource placeholder exists (Tauri build script validates it)
    mkdir -p src-tauri/resources && touch src-tauri/resources/taurhaus-daemon

    # Build
    echo "▸ Building daemon…"
    cd src-tauri && cargo build --release --bin "$DAEMON_BIN"
    cd ..

    # Install (atomic swap avoids "Text file busy" when replacing a running binary)
    mkdir -p "$INSTALL_DIR"
    TMP_BIN="$INSTALL_DIR/.${DAEMON_BIN}.new"
    install -m 755 "src-tauri/target/release/$DAEMON_BIN" "$TMP_BIN"
    mv -f "$TMP_BIN" "$INSTALL_DIR/$DAEMON_BIN"
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

# Install mesh CLI to ~/.local/bin/ (WSL)
# Delegates build to build-mesh, then installs alongside the daemon.
install-mesh: build-mesh
    #!/usr/bin/env bash
    set -euo pipefail

    MESH_PROJECT="${MESH_PROJECT:-/home/mstie/projects/mesh}"
    INSTALL_DIR="/home/mstie/.local/bin"
    MESH_BIN="mesh"
    MESH_PATH="$MESH_PROJECT/target/release/$MESH_BIN"
    if [ ! -x "$MESH_PATH" ]; then
        echo "✗ Built mesh binary not found at $MESH_PATH"
        exit 1
    fi

    mkdir -p "$INSTALL_DIR"
    TMP_BIN="$INSTALL_DIR/.${MESH_BIN}.new"
    install -m 755 "$MESH_PATH" "$TMP_BIN"
    mv -f "$TMP_BIN" "$INSTALL_DIR/$MESH_BIN"
    echo "✓ Installed $MESH_BIN to $INSTALL_DIR/"
    "$INSTALL_DIR/$MESH_BIN" --version 2>/dev/null || "$INSTALL_DIR/$MESH_BIN" --help 2>&1 | head -1

# Copy mesh binary to Tauri resources for bundling, plus lock-derived metadata.
bundle-mesh: mesh-verify-lock
    #!/usr/bin/env bash
    set -euo pipefail
    MESH_PROJECT="${MESH_PROJECT:-/home/mstie/projects/mesh}"
    LOCK_FILE="src-tauri/resources/mesh.lock.json"
    MESH_BIN="$MESH_PROJECT/target/release/mesh"
    if [ ! -x "$MESH_BIN" ]; then
        echo "✗ Built mesh binary not found at $MESH_BIN"
        exit 1
    fi
    if [ ! -f "$LOCK_FILE" ]; then
        echo "✗ Lock manifest not found at $LOCK_FILE"
        exit 1
    fi
    LOCK_VERSION=$(grep '"version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"version":[[:space:]]*"([^"]+)".*/\1/')
    LOCK_PROTOCOL=$(grep '"protocol_version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"protocol_version":[[:space:]]*([0-9]+).*/\1/')
    LOCK_SCHEMA=$(grep '"schema_version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"schema_version":[[:space:]]*([0-9]+).*/\1/')
    LOCK_GIT_COMMIT_RAW=$(grep '"git_commit"' "$LOCK_FILE" | head -1 | sed -E 's/.*"git_commit":[[:space:]]*(.*)/\1/' | sed -E 's/[[:space:]]*$//' | sed -E 's/,$//')
    if [ -z "$LOCK_VERSION" ] || [ -z "$LOCK_PROTOCOL" ] || [ -z "$LOCK_SCHEMA" ]; then
        echo "✗ Could not parse required fields from $LOCK_FILE"
        exit 1
    fi
    MESH_VERSION="$("$MESH_BIN" --version | awk '{print $2}')"
    if [ -z "$MESH_VERSION" ]; then
        echo "✗ Could not determine mesh version from $MESH_BIN --version"
        exit 1
    fi
    if [ "$MESH_VERSION" != "$LOCK_VERSION" ]; then
        echo "✗ Mesh version mismatch during bundle: lock=$LOCK_VERSION built=$MESH_VERSION"
        exit 1
    fi
    if [ -z "$LOCK_GIT_COMMIT_RAW" ]; then
        LOCK_GIT_COMMIT_RAW=null
    fi
    echo "▸ Bundling mesh binary into src-tauri/resources/…"
    mkdir -p src-tauri/resources
    cp "$MESH_BIN" src-tauri/resources/mesh
    echo "$MESH_VERSION" > src-tauri/resources/mesh.version
    cat > src-tauri/resources/mesh.manifest.json <<JSON
    {
      "version": "$LOCK_VERSION",
      "protocol_version": $LOCK_PROTOCOL,
      "schema_version": $LOCK_SCHEMA,
      "git_commit": $LOCK_GIT_COMMIT_RAW,
      "bundled_at_utc": "$(date -u -Iseconds | sed 's/+00:00/Z/')"
    }
    JSON
    echo "✓ Mesh binary bundled (version $MESH_VERSION)"

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
build-windows: install-daemon bundle-daemon mesh-verify-lock bundle-mesh sync-windows
    @echo "Note: cmd.exe may print 'UNC paths are not supported'. This is harmless."
    @echo "▸ Installing frontend dependencies on Windows…"
    cmd.exe /c "cd /d {{win_drive}} && (bun --version >NUL 2>&1 && bun install --frozen-lockfile || %USERPROFILE%\\.bun\\bin\\bun.exe install --frozen-lockfile)"
    @echo ""
    @echo "▸ Building Windows NSIS installer (cargo tauri)…"
    cmd.exe /c "cd /d {{win_drive}} && set PATH=%USERPROFILE%\\.bun\\bin;%PATH% && cargo tauri build --bundles nsis"
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
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}}/src-tauri && cargo test 2>&1'"
    echo "✓ macOS tests passed"

# Build macOS app bundle (arm64) on remote Mac
# Uses zsh -ilc for full login shell (fnm, cargo, homebrew, API keys, NODE_EXTRA_CA_CERTS).
build-macos: sync-macos
    #!/usr/bin/env bash
    set -euo pipefail
    MESH_PROJECT="${MESH_PROJECT:-/home/mstie/projects/mesh}"
    LOCK_FILE="{{project}}/src-tauri/resources/mesh.lock.json"
    if [ ! -d "$MESH_PROJECT" ]; then
        echo "✗ Mesh project not found at $MESH_PROJECT"
        exit 1
    fi
    if [ ! -f "$LOCK_FILE" ]; then
        echo "✗ Lock manifest not found at $LOCK_FILE"
        exit 1
    fi
    LOCK_VERSION=$(grep '"version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"version":[[:space:]]*"([^"]+)".*/\1/')
    LOCK_PROTOCOL=$(grep '"protocol_version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"protocol_version":[[:space:]]*([0-9]+).*/\1/')
    LOCK_SCHEMA=$(grep '"schema_version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"schema_version":[[:space:]]*([0-9]+).*/\1/')
    LOCK_GIT_COMMIT_RAW=$(grep '"git_commit"' "$LOCK_FILE" | head -1 | sed -E 's/.*"git_commit":[[:space:]]*(.*)/\1/' | sed -E 's/[[:space:]]*$//' | sed -E 's/,$//')
    if [ -z "$LOCK_VERSION" ] || [ -z "$LOCK_PROTOCOL" ] || [ -z "$LOCK_SCHEMA" ]; then
        echo "✗ Could not parse required fields from $LOCK_FILE"
        exit 1
    fi
    if [ -z "$LOCK_GIT_COMMIT_RAW" ]; then
        LOCK_GIT_COMMIT_RAW=null
    fi
    echo "▸ Syncing mesh source to macOS…"
    rsync -az --delete --exclude='target' --exclude='.git' "$MESH_PROJECT"/ {{mac_host}}:~/projects/mesh/
    echo "✓ Mesh sync complete"
    echo ""
    echo "▸ Installing frontend dependencies on macOS…"
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}} && bun install --frozen-lockfile'"
    echo ""
    echo "▸ Creating resource placeholders…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p {{mac_dir}}/src-tauri/resources && touch {{mac_dir}}/src-tauri/resources/taurhaus-daemon {{mac_dir}}/src-tauri/resources/mesh && echo 0.0.0-dev > {{mac_dir}}/src-tauri/resources/mesh.version'"
    echo ""
    echo "▸ Building daemon on macOS…"
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}}/src-tauri && cargo build --release --bin taurhaus-daemon'"
    echo ""
    echo "▸ Building mesh on macOS…"
    ssh {{mac_host}} "zsh -ilc 'cd ~/projects/mesh && cargo build --release --bin mesh'"
    REMOTE_MESH_VERSION=$(ssh {{mac_host}} "zsh -ilc '~/projects/mesh/target/release/mesh --version | cut -d \" \" -f2'")
    if [ "$REMOTE_MESH_VERSION" != "$LOCK_VERSION" ]; then
        echo "✗ Remote mesh version mismatch: lock=$LOCK_VERSION remote=$REMOTE_MESH_VERSION"
        exit 1
    fi
    echo "✓ Remote mesh version matches lock ($LOCK_VERSION)"
    echo ""
    echo "▸ Installing daemon to ~/.local/bin/ on macOS…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p ~/.local/bin && cp {{mac_dir}}/src-tauri/target/release/taurhaus-daemon ~/.local/bin/ && codesign --force --sign - ~/.local/bin/taurhaus-daemon'"
    echo ""
    echo "▸ Installing mesh to ~/.local/bin/ on macOS…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p ~/.local/bin && cp ~/projects/mesh/target/release/mesh ~/.local/bin/ && chmod 755 ~/.local/bin/mesh && codesign --force --sign - ~/.local/bin/mesh'"
    echo ""
    echo "▸ Bundling daemon + mesh into resources…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p {{mac_dir}}/src-tauri/resources && cp {{mac_dir}}/src-tauri/target/release/taurhaus-daemon {{mac_dir}}/src-tauri/resources/ && codesign --force --sign - {{mac_dir}}/src-tauri/resources/taurhaus-daemon'"
    ssh {{mac_host}} "zsh -ilc 'cp ~/projects/mesh/target/release/mesh {{mac_dir}}/src-tauri/resources/mesh && chmod 755 {{mac_dir}}/src-tauri/resources/mesh && codesign --force --sign - {{mac_dir}}/src-tauri/resources/mesh && ~/projects/mesh/target/release/mesh --version | cut -d \" \" -f2 > {{mac_dir}}/src-tauri/resources/mesh.version'"
    TMP_MANIFEST=$(mktemp)
    cat > "$TMP_MANIFEST" <<JSON
    {
      "version": "$LOCK_VERSION",
      "protocol_version": $LOCK_PROTOCOL,
      "schema_version": $LOCK_SCHEMA,
      "git_commit": $LOCK_GIT_COMMIT_RAW,
      "bundled_at_utc": "$(date -u -Iseconds | sed 's/+00:00/Z/')"
    }
    JSON
    scp "$TMP_MANIFEST" {{mac_host}}:{{mac_dir}}/src-tauri/resources/mesh.manifest.json >/dev/null
    rm -f "$TMP_MANIFEST"
    echo ""
    echo "▸ Building macOS app (cargo tauri build)…"
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}} && cargo tauri build 2>&1'"
    echo ""
    echo "▸ Copying build artifacts locally…"
    mkdir -p {{project}}/builds/macos-aarch64
    scp {{mac_host}}:{{mac_dir}}/src-tauri/target/release/bundle/dmg/*.dmg {{project}}/builds/macos-aarch64/
    scp {{mac_host}}:{{mac_dir}}/src-tauri/target/release/taurhaus-daemon {{project}}/builds/macos-aarch64/taurhaus-daemon-aarch64
    echo ""
    echo "✓ macOS build complete — artifacts in builds/macos-aarch64/"

# Run macOS E2E test suite on remote Mac
test-macos-e2e: sync-macos
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▸ Running E2E tests on macOS…"
    ssh {{mac_host}} "zsh -ilc 'bash {{mac_dir}}/scripts/macos-e2e-test.sh'" 2>&1
    echo ""
    echo "✓ macOS E2E tests complete"

# Build Intel (x86_64) macOS DMG on remote Mac
build-macos-intel: sync-macos
    #!/usr/bin/env bash
    set -euo pipefail
    MESH_PROJECT="${MESH_PROJECT:-/home/mstie/projects/mesh}"
    LOCK_FILE="{{project}}/src-tauri/resources/mesh.lock.json"
    if [ ! -d "$MESH_PROJECT" ]; then
        echo "✗ Mesh project not found at $MESH_PROJECT"
        exit 1
    fi
    if [ ! -f "$LOCK_FILE" ]; then
        echo "✗ Lock manifest not found at $LOCK_FILE"
        exit 1
    fi
    LOCK_VERSION=$(grep '"version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"version":[[:space:]]*"([^"]+)".*/\1/')
    LOCK_PROTOCOL=$(grep '"protocol_version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"protocol_version":[[:space:]]*([0-9]+).*/\1/')
    LOCK_SCHEMA=$(grep '"schema_version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"schema_version":[[:space:]]*([0-9]+).*/\1/')
    LOCK_GIT_COMMIT_RAW=$(grep '"git_commit"' "$LOCK_FILE" | head -1 | sed -E 's/.*"git_commit":[[:space:]]*(.*)/\1/' | sed -E 's/[[:space:]]*$//' | sed -E 's/,$//')
    if [ -z "$LOCK_VERSION" ] || [ -z "$LOCK_PROTOCOL" ] || [ -z "$LOCK_SCHEMA" ]; then
        echo "✗ Could not parse required fields from $LOCK_FILE"
        exit 1
    fi
    if [ -z "$LOCK_GIT_COMMIT_RAW" ]; then
        LOCK_GIT_COMMIT_RAW=null
    fi
    echo "▸ Syncing mesh source to macOS…"
    rsync -az --delete --exclude='target' --exclude='.git' "$MESH_PROJECT"/ {{mac_host}}:~/projects/mesh/
    echo "✓ Mesh sync complete"
    echo ""
    echo "▸ Installing frontend dependencies on macOS…"
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}} && bun install --frozen-lockfile'"
    echo ""
    echo "▸ Creating resource placeholders…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p {{mac_dir}}/src-tauri/resources && touch {{mac_dir}}/src-tauri/resources/taurhaus-daemon {{mac_dir}}/src-tauri/resources/mesh && echo 0.0.0-dev > {{mac_dir}}/src-tauri/resources/mesh.version'"
    echo ""
    echo "▸ Building daemon on macOS…"
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}}/src-tauri && cargo build --release --bin taurhaus-daemon'"
    echo ""
    echo "▸ Building mesh for x86_64 on macOS…"
    ssh {{mac_host}} "zsh -ilc 'cd ~/projects/mesh && cargo build --release --bin mesh --target x86_64-apple-darwin'"
    REMOTE_MESH_VERSION=$(ssh {{mac_host}} "zsh -ilc '~/projects/mesh/target/x86_64-apple-darwin/release/mesh --version | cut -d \" \" -f2'")
    if [ "$REMOTE_MESH_VERSION" != "$LOCK_VERSION" ]; then
        echo "✗ Remote mesh version mismatch: lock=$LOCK_VERSION remote=$REMOTE_MESH_VERSION"
        exit 1
    fi
    echo "✓ Remote mesh version matches lock ($LOCK_VERSION)"
    echo ""
    echo "▸ Bundling daemon + mesh into resources…"
    ssh {{mac_host}} "zsh -ilc 'cp {{mac_dir}}/src-tauri/target/release/taurhaus-daemon {{mac_dir}}/src-tauri/resources/ && codesign --force --sign - {{mac_dir}}/src-tauri/resources/taurhaus-daemon'"
    ssh {{mac_host}} "zsh -ilc 'cp ~/projects/mesh/target/x86_64-apple-darwin/release/mesh {{mac_dir}}/src-tauri/resources/mesh && chmod 755 {{mac_dir}}/src-tauri/resources/mesh && codesign --force --sign - {{mac_dir}}/src-tauri/resources/mesh && ~/projects/mesh/target/x86_64-apple-darwin/release/mesh --version | cut -d \" \" -f2 > {{mac_dir}}/src-tauri/resources/mesh.version'"
    TMP_MANIFEST=$(mktemp)
    cat > "$TMP_MANIFEST" <<JSON
    {
      "version": "$LOCK_VERSION",
      "protocol_version": $LOCK_PROTOCOL,
      "schema_version": $LOCK_SCHEMA,
      "git_commit": $LOCK_GIT_COMMIT_RAW,
      "bundled_at_utc": "$(date -u -Iseconds | sed 's/+00:00/Z/')"
    }
    JSON
    scp "$TMP_MANIFEST" {{mac_host}}:{{mac_dir}}/src-tauri/resources/mesh.manifest.json >/dev/null
    rm -f "$TMP_MANIFEST"
    echo ""
    echo "▸ Building Intel (x86_64) macOS app…"
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}} && bun run build && cargo tauri build --target x86_64-apple-darwin 2>&1'"
    echo ""
    echo "▸ Copying build artifacts locally…"
    mkdir -p {{project}}/builds/macos-x86_64
    scp {{mac_host}}:{{mac_dir}}/src-tauri/target/x86_64-apple-darwin/release/bundle/dmg/*.dmg {{project}}/builds/macos-x86_64/
    scp {{mac_host}}:{{mac_dir}}/src-tauri/target/x86_64-apple-darwin/release/taurhaus-daemon {{project}}/builds/macos-x86_64/taurhaus-daemon-x86_64
    echo ""
    echo "✓ Intel macOS build complete — artifacts in builds/macos-x86_64/"

# Build universal macOS binary (arm64 + x86_64) on remote Mac
# The daemon is a [[bin]] target — Tauri's universal bundler expects it at
# target/universal-apple-darwin/release/taurhaus-daemon, so we build both
# architectures and lipo them together before running cargo tauri build.
build-macos-universal: sync-macos
    #!/usr/bin/env bash
    set -euo pipefail
    MESH_PROJECT="${MESH_PROJECT:-/home/mstie/projects/mesh}"
    LOCK_FILE="{{project}}/src-tauri/resources/mesh.lock.json"
    if [ ! -d "$MESH_PROJECT" ]; then
        echo "✗ Mesh project not found at $MESH_PROJECT"
        exit 1
    fi
    if [ ! -f "$LOCK_FILE" ]; then
        echo "✗ Lock manifest not found at $LOCK_FILE"
        exit 1
    fi
    LOCK_VERSION=$(grep '"version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"version":[[:space:]]*"([^"]+)".*/\1/')
    LOCK_PROTOCOL=$(grep '"protocol_version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"protocol_version":[[:space:]]*([0-9]+).*/\1/')
    LOCK_SCHEMA=$(grep '"schema_version"' "$LOCK_FILE" | head -1 | sed -E 's/.*"schema_version":[[:space:]]*([0-9]+).*/\1/')
    LOCK_GIT_COMMIT_RAW=$(grep '"git_commit"' "$LOCK_FILE" | head -1 | sed -E 's/.*"git_commit":[[:space:]]*(.*)/\1/' | sed -E 's/[[:space:]]*$//' | sed -E 's/,$//')
    if [ -z "$LOCK_VERSION" ] || [ -z "$LOCK_PROTOCOL" ] || [ -z "$LOCK_SCHEMA" ]; then
        echo "✗ Could not parse required fields from $LOCK_FILE"
        exit 1
    fi
    if [ -z "$LOCK_GIT_COMMIT_RAW" ]; then
        LOCK_GIT_COMMIT_RAW=null
    fi
    echo "▸ Syncing mesh source to macOS…"
    rsync -az --delete --exclude='target' --exclude='.git' "$MESH_PROJECT"/ {{mac_host}}:~/projects/mesh/
    echo "✓ Mesh sync complete"
    echo ""
    echo "▸ Installing frontend dependencies on macOS…"
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}} && bun install --frozen-lockfile'"
    echo ""
    echo "▸ Creating resource placeholders…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p {{mac_dir}}/src-tauri/resources && touch {{mac_dir}}/src-tauri/resources/taurhaus-daemon {{mac_dir}}/src-tauri/resources/mesh && echo 0.0.0-dev > {{mac_dir}}/src-tauri/resources/mesh.version'"
    echo ""
    echo "▸ Building daemon for arm64…"
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}}/src-tauri && cargo build --release --bin taurhaus-daemon --target aarch64-apple-darwin'"
    echo ""
    echo "▸ Building daemon for x86_64…"
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}}/src-tauri && cargo build --release --bin taurhaus-daemon --target x86_64-apple-darwin'"
    echo ""
    echo "▸ Building mesh for arm64…"
    ssh {{mac_host}} "zsh -ilc 'cd ~/projects/mesh && cargo build --release --bin mesh --target aarch64-apple-darwin'"
    echo ""
    echo "▸ Building mesh for x86_64…"
    ssh {{mac_host}} "zsh -ilc 'cd ~/projects/mesh && cargo build --release --bin mesh --target x86_64-apple-darwin'"
    echo ""
    echo "▸ Creating universal daemon binary with lipo…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p {{mac_dir}}/src-tauri/target/universal-apple-darwin/release && lipo -create {{mac_dir}}/src-tauri/target/aarch64-apple-darwin/release/taurhaus-daemon {{mac_dir}}/src-tauri/target/x86_64-apple-darwin/release/taurhaus-daemon -output {{mac_dir}}/src-tauri/target/universal-apple-darwin/release/taurhaus-daemon && codesign --force --sign - {{mac_dir}}/src-tauri/target/universal-apple-darwin/release/taurhaus-daemon'"
    echo ""
    echo "▸ Creating universal mesh binary with lipo…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p ~/projects/mesh/target/universal-apple-darwin/release && lipo -create ~/projects/mesh/target/aarch64-apple-darwin/release/mesh ~/projects/mesh/target/x86_64-apple-darwin/release/mesh -output ~/projects/mesh/target/universal-apple-darwin/release/mesh && codesign --force --sign - ~/projects/mesh/target/universal-apple-darwin/release/mesh'"
    REMOTE_MESH_VERSION=$(ssh {{mac_host}} "zsh -ilc '~/projects/mesh/target/universal-apple-darwin/release/mesh --version | cut -d \" \" -f2'")
    if [ "$REMOTE_MESH_VERSION" != "$LOCK_VERSION" ]; then
        echo "✗ Remote mesh version mismatch: lock=$LOCK_VERSION remote=$REMOTE_MESH_VERSION"
        exit 1
    fi
    echo "✓ Remote mesh version matches lock ($LOCK_VERSION)"
    echo ""
    echo "▸ Bundling daemon + mesh into resources…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p {{mac_dir}}/src-tauri/resources && cp {{mac_dir}}/src-tauri/target/universal-apple-darwin/release/taurhaus-daemon {{mac_dir}}/src-tauri/resources/ && codesign --force --sign - {{mac_dir}}/src-tauri/resources/taurhaus-daemon'"
    ssh {{mac_host}} "zsh -ilc 'cp ~/projects/mesh/target/universal-apple-darwin/release/mesh {{mac_dir}}/src-tauri/resources/mesh && chmod 755 {{mac_dir}}/src-tauri/resources/mesh && codesign --force --sign - {{mac_dir}}/src-tauri/resources/mesh && ~/projects/mesh/target/universal-apple-darwin/release/mesh --version | cut -d \" \" -f2 > {{mac_dir}}/src-tauri/resources/mesh.version'"
    TMP_MANIFEST=$(mktemp)
    cat > "$TMP_MANIFEST" <<JSON
    {
      "version": "$LOCK_VERSION",
      "protocol_version": $LOCK_PROTOCOL,
      "schema_version": $LOCK_SCHEMA,
      "git_commit": $LOCK_GIT_COMMIT_RAW,
      "bundled_at_utc": "$(date -u -Iseconds | sed 's/+00:00/Z/')"
    }
    JSON
    scp "$TMP_MANIFEST" {{mac_host}}:{{mac_dir}}/src-tauri/resources/mesh.manifest.json >/dev/null
    rm -f "$TMP_MANIFEST"
    echo ""
    echo "▸ Building universal macOS app (arm64 + x86_64)…"
    ssh {{mac_host}} "zsh -ilc 'cd {{mac_dir}} && cargo tauri build --target universal-apple-darwin 2>&1'"
    echo ""
    echo "▸ Copying build artifacts locally…"
    mkdir -p {{project}}/builds/macos-universal
    scp {{mac_host}}:{{mac_dir}}/src-tauri/target/universal-apple-darwin/release/bundle/dmg/*.dmg {{project}}/builds/macos-universal/
    echo ""
    echo "✓ Universal macOS build complete — artifacts in builds/macos-universal/"

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

# ── Release ──────────────────────────────────────────────────────────────────
# Creates a GitHub Release with build artifacts from builds/.
#
# Workflow:
#   1. Update version:  just bump 0.4.0
#   2. Build:           just build-windows && just build-macos-universal
#   3. Release:         just release
#
# The version is read from tauri.conf.json. Artifacts are matched by glob
# from builds/ — if a platform dir is empty or missing, it's skipped.

# Bump version in all version-bearing files
bump version:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "▸ Bumping version to {{version}}…"

    # tauri.conf.json
    sed -i 's/"version": "[^"]*"/"version": "{{version}}"/' src-tauri/tauri.conf.json
    echo "  ✓ src-tauri/tauri.conf.json"

    # Cargo.toml (first version = line in [package])
    sed -i '0,/^version = "[^"]*"/s//version = "{{version}}"/' src-tauri/Cargo.toml
    echo "  ✓ src-tauri/Cargo.toml"

    # package.json
    sed -i 's/"version": "[^"]*"/"version": "{{version}}"/' package.json
    echo "  ✓ package.json"

    # Cargo.lock (regenerate to pick up version change)
    (cd src-tauri && cargo check --quiet 2>/dev/null)
    echo "  ✓ Cargo.lock"

    # CHANGELOG.md — add new section under [Unreleased] if not already present
    if ! grep -q "## \[{{version}}\]" CHANGELOG.md; then
        DATE=$(date +%Y-%m-%d)
        sed -i "/^## \[Unreleased\]/a\\\\n## [{{version}}] - $DATE" CHANGELOG.md
        echo "  ✓ CHANGELOG.md (added [{{version}}] section — fill in changes before releasing)"
    else
        echo "  · CHANGELOG.md already has [{{version}}]"
    fi

    echo ""
    echo "Next: edit CHANGELOG.md, commit, build, then run: just release"

# Create GitHub Release from current version and upload artifacts
release:
    #!/usr/bin/env bash
    set -euo pipefail

    # Read version from tauri.conf.json
    VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed 's/.*"\([0-9][^"]*\)".*/\1/')
    TAG="v$VERSION"

    echo "▸ Creating release $TAG…"
    echo ""

    # Verify we're on main and clean
    BRANCH=$(git branch --show-current)
    if [ "$BRANCH" != "main" ]; then
        echo "✗ Must be on main branch (currently on $BRANCH)"
        exit 1
    fi
    if [ -n "$(git status --porcelain)" ]; then
        echo "✗ Working tree is dirty — commit or stash changes first"
        exit 1
    fi

    # Check tag doesn't already exist
    if git tag -l "$TAG" | grep -q "$TAG"; then
        echo "✗ Tag $TAG already exists"
        exit 1
    fi

    # Push to remote before creating release
    echo "▸ Pushing to origin…"
    git push origin main
    echo ""

    # Collect artifacts (only matching current version)
    ARTIFACTS=()
    for f in builds/macos-universal/taurhaus_${VERSION}_*.dmg; do
        [ -f "$f" ] && ARTIFACTS+=("$f")
    done
    for f in builds/macos-aarch64/taurhaus_${VERSION}_*.dmg; do
        [ -f "$f" ] && ARTIFACTS+=("$f")
    done
    for f in builds/macos-x86_64/taurhaus_${VERSION}_*.dmg; do
        [ -f "$f" ] && ARTIFACTS+=("$f")
    done
    WIN_NSIS="{{win_dir}}/src-tauri/target/release/bundle/nsis"
    for f in "$WIN_NSIS"/taurhaus_${VERSION}_*.exe; do
        [ -f "$f" ] && ARTIFACTS+=("$f")
    done

    if [ ${#ARTIFACTS[@]} -eq 0 ]; then
        echo "✗ No build artifacts found in builds/ or Windows NSIS output"
        echo "  Run build recipes first: just build-windows && just build-macos-universal"
        exit 1
    fi

    echo "  Artifacts to upload:"
    for f in "${ARTIFACTS[@]}"; do
        SIZE=$(du -h "$f" | cut -f1)
        echo "    $f ($SIZE)"
    done
    echo ""

    # Extract changelog section for this version
    NOTES=$(awk "/^## \[$VERSION\]/{found=1; next} /^## \[/{if(found) exit} found{print}" CHANGELOG.md)
    if [ -z "$NOTES" ]; then
        NOTES="Release $TAG"
    fi

    # Create release with artifacts
    gh release create "$TAG" \
        --title "$TAG" \
        --notes "$NOTES" \
        "${ARTIFACTS[@]}"

    echo ""
    echo "✓ Release $TAG published: https://github.com/taurcasa/taurhaus/releases/tag/$TAG"

# Take screenshot of current app state
screenshot:
    @echo "Screenshot recipe not yet configured"

# Agent pre-completion quality gate.
# Codex/Gemini agents must run this before reporting a task as done.
# Referenced in AGENTS.md — extend here as new friction points surface.
agent-quality: check-quick
    @echo "Agent quality gate passed."

# Bootstrap infographic manifest from taursult MCP generation DB.
# Pulls prompts, settings, and sha256 for all tagged generations.
bootstrap-infographic-manifest:
    ./scripts/bootstrap-infographic-manifest.sh

# Security audit (integration tasks + phase boundaries)
security-audit:
    cd src-tauri && cargo audit 2>/dev/null || echo "cargo-audit not installed — run: cargo install cargo-audit"
