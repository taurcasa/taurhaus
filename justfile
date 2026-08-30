# taurhaus development recipes
# Single file by design — split into `import`s when this exceeds ~1200 lines.

# Project paths
project   := justfile_directory()
win_dir   := env_var_or_default("TAURHAUS_WINDOWS_BUILD_DIR", "/mnt/c/taurhaus_build")
windows_bun_version := `node -p 'require("./package.json").packageManager.split("@").slice(1).join("@")'`
# Top-level-only by specification; Cargo `tests/<dir>/main.rs` targets require explicit future handling.
integration_test_args := `for test_file in src-tauri/tests/*.rs; do test_name="${test_file##*/}"; printf -- '--test %s ' "${test_name%.rs}"; done`
heavy_rust_test_filters := "daemon::server::tests:: daemon::event_listener::tests:: provider::daemon_client::tests:: daemon::launcher::tests:: fs::watcher::tests::watcher_starts_and_stops fs::watcher::tests::unwatch_all_clears_everything"

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
    @if [ ! -s src-tauri/resources/mesh.manifest.json ]; then printf '%s\n' '{"version":"0.0.0-dev","protocol_version":1,"schema_version":1,"git_commit":null,"bundled_at_utc":"unknown"}' > src-tauri/resources/mesh.manifest.json; fi

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
    run_rust_lane() {
        just lint-rust
        just test-rust
    }
    run_frontend_lane() {
        just lint-frontend
        just lint-workflows
        just typecheck
        just test-frontend
    }
    run_rust_lane &
    rust_pid=$!
    run_frontend_lane &
    frontend_pid=$!
    pids=("$rust_pid" "$frontend_pid")
    while [ "${#pids[@]}" -gt 0 ]; do
        if ! wait -n "${pids[@]}"; then
            status=$?
            kill "$rust_pid" "$frontend_pid" 2>/dev/null || true
            wait "$rust_pid" 2>/dev/null || true
            wait "$frontend_pid" 2>/dev/null || true
            exit "$status"
        fi
        next_pids=()
        for pid in "${pids[@]}"; do
            if kill -0 "$pid" 2>/dev/null; then
                next_pids+=("$pid")
            fi
        done
        pids=("${next_pids[@]}")
    done
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

# Lint everything and enforce reproducible frontend structure checks.
lint: lint-rust lint-frontend lint-workflows

# Lint Rust code with clippy.
lint-rust: ensure-tauri-resources
    cd src-tauri && cargo clippy --all-targets -- -D warnings

# Lint frontend code and enforce structural checks.
lint-frontend:
    bun run lint

# Syntax-check the versioned Workflow procedures in .claude/workflows (parse only, never run).
lint-workflows:
    bun scripts/check-workflow-scripts.mjs

# Typecheck frontend code
typecheck:
    bun run typecheck

# Generate a comprehensive quality KPI report (tests, coverage, build health, code size, E2E inventory).
# Use this at milestones/pre-release checkpoints for a single health snapshot.
metrics:
    ./scripts/metrics.sh

# Analyze compaction reinjection pipeline health from current + rotated JSONL logs.
# Example: just analyze-compaction --team taurhaus-team --last 24h
analyze-compaction *ARGS:
    python3 scripts/analyze-compaction.py {{ARGS}}

# Trigger a real managed Claude compaction and verify the hook + delivery path.
test-compaction-claude TEAM MEMBER *ARGS:
    python3 scripts/test-compaction-claude.py --team {{TEAM}} --member {{MEMBER}} {{ARGS}}

# Trigger a real managed Codex compaction and verify the native hook delivery path.
test-compaction-codex TEAM MEMBER *ARGS:
    python3 scripts/test-compaction-codex.py --team {{TEAM}} --member {{MEMBER}} {{ARGS}}

# Generic compaction test entry point by tool.
test-compaction TOOL TEAM MEMBER *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{TOOL}}" in
        claude)
            python3 scripts/test-compaction-claude.py --team {{TEAM}} --member {{MEMBER}} {{ARGS}}
            ;;
        codex)
            python3 scripts/test-compaction-codex.py --team {{TEAM}} --member {{MEMBER}} {{ARGS}}
            ;;
        *)
            echo "Unsupported tool '{{TOOL}}' (expected: claude or codex)" >&2
            exit 1
            ;;
    esac

# Run the unified resource monitor (live table by default).
# Example: just monitor --samples 1 --interval 1
monitor *ARGS:
    python3 scripts/resource-monitor.py {{ARGS}}

# Regenerate documentation infographics from the manifest prompts (needs .env).
# Example: just infographics --id scanner-pipeline
# Workflow: docs/operations/infographics.md
infographics *ARGS:
    python3 scripts/generate-infographics.py {{ARGS}}

# Show which infographics would be regenerated, and what the run would cost.
infographics-dry-run:
    python3 scripts/generate-infographics.py --dry-run --stale

# Write the Claude role templates into a project's .claude/agents directory,
# where Claude Code and the Workflow API resolve a subagent by name.
# Only generated files are replaced; a hand-written agent is reported as skipped.
# Example: just export-agents ~/projects/taurhaus
export-agents PROJECT: ensure-tauri-resources
    #!/usr/bin/env bash
    set -euo pipefail
    # Resolve PROJECT against the directory the command was typed in, before
    # cargo runs from src-tauri — otherwise a relative path lands in the Rust
    # subdirectory. A path that is not an existing directory fails here.
    project="$(cd -- {{quote(invocation_directory())}} && cd -- {{quote(PROJECT)}} && pwd)"
    cd src-tauri && cargo run --bin taurhaus -- --export-agent-definitions "$project"

# Run all non-E2E tests (Rust unit + Rust integration/system + frontend unit + script unit).
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
    cd src-tauri && heavy_test_filters="{{heavy_rust_test_filters}}"; skip_args=""; for test_filter in $heavy_test_filters; do skip_args="$skip_args --skip $test_filter"; done; cargo test --lib --bins -- --test-threads=1 $skip_args

# Rust integration/system lane (serialized, includes heavy suites).
test-rust-integration: ensure-tauri-resources
    cd src-tauri && cargo test {{integration_test_args}} -- --test-threads=1
    cd src-tauri && for test_filter in {{heavy_rust_test_filters}}; do echo "▸ $test_filter"; cargo test --lib "$test_filter" -- --test-threads=1 || exit; done

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

# Shoot one visual-host fixture at one viewport with Windows Edge headless.
#
# The browser-mode lane above renders components in isolation; this one renders
# a fixture at a real window size, which is the only way to see a popup that
# depends on the viewport. Starts the visual host on port 5211 if nothing is
# already listening there, and stops only a server it started itself.
visual-shot COMPONENT SCENARIO VIEWPORT="laptop" THEME="light" OUT="":
    ./scripts/visual-shot.sh "{{COMPONENT}}" "{{SCENARIO}}" "{{VIEWPORT}}" "{{THEME}}" "{{OUT}}"

# Stop the visual host started by `just visual-shot` (never another one).
visual-shot-stop:
    ./scripts/visual-shot.sh --stop

# Regenerate README screenshots from the dedicated visual shot list.
capture-readme-screenshots:
    ./scripts/export-readme-screenshots.sh

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
# compaction-codex-hooks spends real Codex and Claude subscription turns, so it
# is never part of a suite run: `e2e/specList.js` keeps every paid lane out of
# the config's spec list. Start it by name with test-e2e-spec.
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
    @echo "No standalone db-reset workflow is wired. Delete the app data SQLite file manually if needed; schema migrations run automatically on app startup."

# Run database migrations
db-migrate:
    @echo "Schema migrations are already embedded and run automatically on app startup. No separate db-migrate step is required."

# Build for Linux
build-linux: bundle-daemon bundle-mesh
    bun run tauri build

# Build the WSL daemon binary (Linux target)
build-daemon:
    #!/usr/bin/env bash
    set -euo pipefail
    missing=()
    command -v pkg-config >/dev/null 2>&1 || missing+=("pkg-config")
    pkg-config --exists glib-2.0 2>/dev/null || missing+=("libglib2.0-dev")
    pkg-config --exists gtk+-3.0 2>/dev/null || missing+=("libgtk-3-dev")
    pkg-config --exists webkit2gtk-4.1 2>/dev/null || missing+=("libwebkit2gtk-4.1-dev")
    pkg-config --exists libsoup-3.0 2>/dev/null || missing+=("libsoup-3.0-dev")
    if [ ${#missing[@]} -gt 0 ]; then
        echo "✗ Missing system packages required to build the daemon:"
        echo ""
        echo "  sudo apt install ${missing[*]}"
        echo ""
        exit 1
    fi
    mkdir -p src-tauri/resources && touch src-tauri/resources/taurhaus-daemon
    cd src-tauri && cargo build --release --bin taurhaus-daemon

# Resolve a usable mesh CLI binary, building from the local workspace when available.
build-mesh:
    #!/usr/bin/env bash
    set -euo pipefail
    MESH_BIN_PATH="$(./scripts/resolve-mesh-binary.sh)"
    echo "✓ Mesh binary ready at $MESH_BIN_PATH"

# Verify built mesh binary matches the pinned JSON compatibility contract.
mesh-verify-lock:
    #!/usr/bin/env bash
    set -euo pipefail
    LOCK_FILE="src-tauri/resources/mesh.lock.json"
    MESH_BIN="$(./scripts/resolve-mesh-binary.sh)"
    if [ ! -f "$LOCK_FILE" ]; then
        echo "✗ Lock manifest not found at $LOCK_FILE"
        exit 1
    fi
    if [ ! -x "$MESH_BIN" ]; then
        echo "✗ Built mesh binary not found at $MESH_BIN"
        exit 1
    fi
    MESH_JSON="$("$MESH_BIN" version --json)"
    LOCK_FILE="$LOCK_FILE" MESH_JSON="$MESH_JSON" python3 - <<'PY'
    import json
    import os
    import sys

    with open(os.environ["LOCK_FILE"], "r", encoding="utf-8") as handle:
        lock = json.load(handle)

    try:
        built = json.loads(os.environ["MESH_JSON"])
    except json.JSONDecodeError as exc:
        print(f"✗ Could not parse mesh version --json output: {exc}")
        sys.exit(1)

    issues = []
    for key in ("version", "protocol_version", "schema_version"):
        if built.get(key) != lock.get(key):
            issues.append(f"{key}: lock={lock.get(key)!r} built={built.get(key)!r}")

    expected_commit = lock.get("git_commit")
    if expected_commit is not None and built.get("git_commit") != expected_commit:
        issues.append(
            f"git_commit: lock={expected_commit!r} built={built.get('git_commit')!r}"
        )

    if issues:
        print("✗ Mesh compatibility mismatch:")
        for issue in issues:
            print(f"  - {issue}")
        sys.exit(1)

    print(
        "✓ Mesh lock verification passed "
        f"(version {lock['version']}, protocol {lock['protocol_version']}, schema {lock['schema_version']})"
    )
    PY

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
install-daemon: build-daemon
    just _install-daemon-from-build

_install-daemon-from-build:
    #!/usr/bin/env bash
    set -euo pipefail

    DAEMON_BIN="taurhaus-daemon"
    INSTALL_DIR="$HOME/.local/bin"
    WAS_RUNNING=false
    PRESERVED_ENV=()
    PRESERVED_ARGS=()
    RESTART_DATA_DIR="${TAURHAUS_DATA_DIR:-$HOME/.local/share/com.taurhaus.dev}"
    RESTART_PORT=17233

    # Check if daemon is currently running. Capture its TAURHAUS_*/RUST_LOG env
    # and CLI args first so the restart retains the same data/path authority.
    OLD_PID="$(pgrep -x "$DAEMON_BIN" | head -1 || true)"
    if [ -n "$OLD_PID" ]; then
        if [ -r "/proc/$OLD_PID/environ" ]; then
            while IFS= read -r -d '' kv; do
                case "$kv" in
                    TAURHAUS_DATA_DIR=*) RESTART_DATA_DIR="${kv#*=}" ;;
                    TAURHAUS_*=*|RUST_LOG=*) PRESERVED_ENV+=("$kv") ;;
                esac
            done < "/proc/$OLD_PID/environ"
        fi
        if [ -r "/proc/$OLD_PID/cmdline" ]; then
            mapfile -d '' -t CMD < "/proc/$OLD_PID/cmdline"
            PRESERVED_ARGS=("${CMD[@]:1}")
        fi

        # Normalize identity flags so repeated installs do not accumulate
        # duplicate --data-dir/--port arguments. The rebuilt daemon supports both.
        EXTRA_ARGS=()
        for ((i=0; i<${#PRESERVED_ARGS[@]}; i++)); do
            case "${PRESERVED_ARGS[$i]}" in
                --data-dir)
                    i=$((i + 1))
                    RESTART_DATA_DIR="${PRESERVED_ARGS[$i]:-$RESTART_DATA_DIR}"
                    ;;
                --port|-p)
                    i=$((i + 1))
                    RESTART_PORT="${PRESERVED_ARGS[$i]:-$RESTART_PORT}"
                    ;;
                *) EXTRA_ARGS+=("${PRESERVED_ARGS[$i]}") ;;
            esac
        done
        [ -n "$RESTART_DATA_DIR" ] || RESTART_DATA_DIR="$HOME/.local/share/com.taurhaus.dev"
        PRESERVED_ARGS=(--data-dir "$RESTART_DATA_DIR" --port "$RESTART_PORT" "${EXTRA_ARGS[@]}")
        PRESERVED_ENV+=("TAURHAUS_DATA_DIR=$RESTART_DATA_DIR")
        echo "▸ Stopping running $DAEMON_BIN (PID $OLD_PID)…"
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

    # Install (atomic swap avoids "Text file busy" when replacing a running binary)
    mkdir -p "$INSTALL_DIR"
    TMP_BIN="$INSTALL_DIR/.${DAEMON_BIN}.new"
    install -m 755 "src-tauri/target/release/$DAEMON_BIN" "$TMP_BIN"
    mv -f "$TMP_BIN" "$INSTALL_DIR/$DAEMON_BIN"
    echo "✓ Installed $DAEMON_BIN to $INSTALL_DIR/"

    # Restart if it was running before: same env + args, fully detached from this
    # shell (setsid + nohup + stdin from /dev/null), retrying while the listen
    # port is still held by lingering FIN_WAIT2 sockets of the old process
    # (common under WSL mirrored networking; clears within ~60s).
    if [ "$WAS_RUNNING" = true ]; then
        echo "▸ Restarting daemon…"
        if [ "${#PRESERVED_ENV[@]}" -gt 0 ]; then
            echo "  env: ${PRESERVED_ENV[*]}"
        else
            echo "  env: (none preserved — previous daemon had no TAURHAUS_* env)"
        fi
        [ "${#PRESERVED_ARGS[@]}" -gt 0 ] && echo "  args: ${PRESERVED_ARGS[*]}"
        STARTED=false
        for attempt in $(seq 1 15); do
            if [ "${#PRESERVED_ENV[@]}" -gt 0 ]; then
                env "${PRESERVED_ENV[@]}" setsid nohup "$INSTALL_DIR/$DAEMON_BIN" "${PRESERVED_ARGS[@]}" >/dev/null 2>&1 </dev/null &
            else
                setsid nohup "$INSTALL_DIR/$DAEMON_BIN" "${PRESERVED_ARGS[@]}" >/dev/null 2>&1 </dev/null &
            fi
            disown || true
            sleep 2
            if pgrep -x "$DAEMON_BIN" >/dev/null 2>&1; then
                STARTED=true
                break
            fi
            echo "  · attempt $attempt: not up yet (port likely still in use) — retrying"
            sleep 3
        done
        if [ "$STARTED" = true ]; then
            echo "✓ Daemon restarted (PID $(pgrep -x $DAEMON_BIN | head -1))"
        else
            echo "⚠ Daemon did not restart after 15 attempts — start it manually:"
            echo "    ${PRESERVED_ENV[*]:-} $INSTALL_DIR/$DAEMON_BIN ${PRESERVED_ARGS[*]:-}"
            exit 1
        fi
    fi

# Install mesh CLI to ~/.local/bin/ (WSL)
# Installs a lock-matching mesh binary alongside the daemon.
install-mesh: mesh-verify-lock
    #!/usr/bin/env bash
    set -euo pipefail

    INSTALL_DIR="${MESH_INSTALL_DIR:-$HOME/.local/bin}"
    MESH_BIN="mesh"
    MESH_PATH="$(./scripts/resolve-mesh-binary.sh)"
    if [ ! -x "$MESH_PATH" ]; then
        echo "✗ Built mesh binary not found at $MESH_PATH"
        exit 1
    fi

    mkdir -p "$INSTALL_DIR"
    TMP_BIN="$INSTALL_DIR/.${MESH_BIN}.new"
    install -m 755 "$MESH_PATH" "$TMP_BIN"
    mv -f "$TMP_BIN" "$INSTALL_DIR/$MESH_BIN"
    echo "✓ Installed $MESH_BIN to $INSTALL_DIR/"
    "$INSTALL_DIR/$MESH_BIN" version --json

# Copy mesh binary to Tauri resources for bundling, plus lock-derived metadata.
bundle-mesh: mesh-verify-lock
    #!/usr/bin/env bash
    set -euo pipefail
    LOCK_FILE="src-tauri/resources/mesh.lock.json"
    MESH_BIN="$(./scripts/resolve-mesh-binary.sh)"
    if [ ! -x "$MESH_BIN" ]; then
        echo "✗ Built mesh binary not found at $MESH_BIN"
        exit 1
    fi
    if [ ! -f "$LOCK_FILE" ]; then
        echo "✗ Lock manifest not found at $LOCK_FILE"
        exit 1
    fi
    mapfile -t LOCK_FIELDS < <(LOCK_FILE="$LOCK_FILE" python3 - <<'PY'
    import json
    import os

    with open(os.environ["LOCK_FILE"], "r", encoding="utf-8") as handle:
        lock = json.load(handle)

    print(lock["version"])
    print(lock["protocol_version"])
    print(lock["schema_version"])
    print("null" if lock.get("git_commit") is None else json.dumps(lock["git_commit"]))
    PY
    )
    LOCK_VERSION="${LOCK_FIELDS[0]}"
    LOCK_PROTOCOL="${LOCK_FIELDS[1]}"
    LOCK_SCHEMA="${LOCK_FIELDS[2]}"
    LOCK_GIT_COMMIT_RAW="${LOCK_FIELDS[3]}"
    echo "▸ Bundling mesh binary into src-tauri/resources/…"
    mkdir -p src-tauri/resources
    # A stray directory at resources/mesh would turn `cp` into resources/mesh/mesh
    # and ship a corrupt bundle (v0.6.4). Always bundle onto a regular file.
    if [ -d src-tauri/resources/mesh ]; then
        echo "  ! src-tauri/resources/mesh is a directory — removing it"
        rm -rf src-tauri/resources/mesh
    fi
    cp "$MESH_BIN" src-tauri/resources/mesh
    [ -f src-tauri/resources/mesh ] || { echo "✗ src-tauri/resources/mesh is not a regular file after bundling"; exit 1; }
    printf '%s\n' "$LOCK_VERSION" > src-tauri/resources/mesh.version
    cat > src-tauri/resources/mesh.manifest.json <<JSON
    {
      "version": "$LOCK_VERSION",
      "protocol_version": $LOCK_PROTOCOL,
      "schema_version": $LOCK_SCHEMA,
      "git_commit": $LOCK_GIT_COMMIT_RAW,
      "bundled_at_utc": "$(date -u -Iseconds | sed 's/+00:00/Z/')"
    }
    JSON
    echo "✓ Mesh binary bundled (version $LOCK_VERSION)"

# Run the daemon in foreground (for development)
run-daemon:
    cd src-tauri && cargo run --bin taurhaus-daemon -- --verbose

# ── Windows Build (via WSL2 interop) ─────────────────────────────────────────

# Verify native Windows build prerequisites before starting a release build.
check-windows-build-prereqs:
    #!/usr/bin/env bash
    set -euo pipefail
    PS_SCRIPT="$(wslpath -w "{{project}}/scripts/windows-build-prereqs.ps1")"
    sh -c 'exec powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$1" -CheckOnly < /dev/null' sh "$PS_SCRIPT"

# Install native Windows build prerequisites via WSL interop.
install-windows-build-prereqs:
    #!/usr/bin/env bash
    set -euo pipefail
    PS_SCRIPT="$(wslpath -w "{{project}}/scripts/windows-build-prereqs.ps1")"
    sh -c 'exec powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "$1" -Install -BunVersion "$2" < /dev/null' sh "$PS_SCRIPT" "{{windows_bun_version}}"

# Install native Windows build prerequisites and keep the elevated window open on failure.
install-windows-build-prereqs-visible:
    #!/usr/bin/env bash
    set -euo pipefail
    PS_SCRIPT="$(wslpath -w "{{project}}/scripts/windows-build-prereqs.ps1")"
    sh -c 'exec powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "$1" -Install -BunVersion "$2" -PauseOnError < /dev/null' sh "$PS_SCRIPT" "{{windows_bun_version}}"

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
    just _bundle-daemon-from-build

_bundle-daemon-from-build:
    @echo "▸ Bundling daemon binary into src-tauri/resources/…"
    mkdir -p src-tauri/resources
    cp src-tauri/target/release/taurhaus-daemon src-tauri/resources/taurhaus-daemon
    @echo "✓ Daemon binary bundled"

# Build Windows NSIS installer (syncs first, builds natively on Windows)
build-windows:
    ./scripts/build-windows.sh "{{project}}" "{{win_dir}}"

# Build Windows NSIS installer with optional sccache integration.
build-windows-sccache:
    TAURHAUS_WINDOWS_USE_SCCACHE=1 ./scripts/build-windows.sh "{{project}}" "{{win_dir}}"

# Run the latest Windows NSIS installer in silent mode and verify the installed exe hash.
install-windows:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION=$(node -p 'require("./package.json").version')
    INSTALLER="{{win_dir}}/src-tauri/target/release/bundle/nsis/taurhaus_${VERSION}_x64-setup.exe"
    BUILT_EXE="{{win_dir}}/src-tauri/target/release/taurhaus.exe"
    PS_SCRIPT="$(wslpath -w "{{project}}/scripts/install-windows-silent.ps1")"
    WIN_INSTALLER="$(wslpath -w "$INSTALLER")"
    WIN_BUILT_EXE="$(wslpath -w "$BUILT_EXE")"
    if [ ! -f "$INSTALLER" ]; then
        echo "✗ Windows installer not found at $INSTALLER"
        echo "  Run: just build-windows"
        exit 1
    fi
    if [ ! -f "$BUILT_EXE" ]; then
        echo "✗ Built Windows exe not found at $BUILT_EXE"
        echo "  Run: just build-windows"
        exit 1
    fi
    sh -c 'exec powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$1" -InstallerPath "$2" -BuiltExePath "$3" < /dev/null' sh "$PS_SCRIPT" "$WIN_INSTALLER" "$WIN_BUILT_EXE"
    # The app is installed; now the WSL daemon it bundles, restarted with the
    # captured env/args, so app and daemon match PROTOCOL_VERSION from here on.
    just _install-daemon-from-build

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
    MESH_PROJECT="${MESH_PROJECT:-$HOME/projects/mesh}"
    LOCK_FILE="{{project}}/src-tauri/resources/mesh.lock.json"
    if [ ! -d "$MESH_PROJECT" ]; then
        echo "✗ Mesh project not found at $MESH_PROJECT"
        exit 1
    fi
    if [ ! -f "$LOCK_FILE" ]; then
        echo "✗ Lock manifest not found at $LOCK_FILE"
        exit 1
    fi
    mapfile -t LOCK_FIELDS < <(LOCK_FILE="$LOCK_FILE" python3 - <<'PY'
    import json
    import os

    with open(os.environ["LOCK_FILE"], "r", encoding="utf-8") as handle:
        lock = json.load(handle)

    print(lock["version"])
    print(lock["protocol_version"])
    print(lock["schema_version"])
    print("null" if lock.get("git_commit") is None else json.dumps(lock["git_commit"]))
    PY
    )
    LOCK_VERSION="${LOCK_FIELDS[0]}"
    LOCK_PROTOCOL="${LOCK_FIELDS[1]}"
    LOCK_SCHEMA="${LOCK_FIELDS[2]}"
    LOCK_GIT_COMMIT_RAW="${LOCK_FIELDS[3]}"
    echo "▸ Syncing mesh source to macOS…"
    rsync -az --delete --exclude='target' "$MESH_PROJECT"/ {{mac_host}}:~/projects/mesh/
    echo "✓ Mesh sync complete"
    echo ""
    echo "▸ Cleaning remote mesh target to refresh build metadata…"
    ssh {{mac_host}} "zsh -ilc 'cd ~/projects/mesh && cargo clean'"
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
    REMOTE_MESH_JSON=$(ssh {{mac_host}} "zsh -ilc '~/projects/mesh/target/release/mesh version --json'")
    LOCK_FILE="$LOCK_FILE" INSTALLED_MESH_JSON="$REMOTE_MESH_JSON" python3 - <<'PY'
    import json
    import os
    import sys

    with open(os.environ["LOCK_FILE"], "r", encoding="utf-8") as handle:
        lock = json.load(handle)
    installed = json.loads(os.environ["INSTALLED_MESH_JSON"])

    issues = []
    for key in ("version", "protocol_version", "schema_version"):
        if installed.get(key) != lock.get(key):
            issues.append(f"{key}: lock={lock.get(key)!r} installed={installed.get(key)!r}")

    expected_commit = lock.get("git_commit")
    if expected_commit is not None and installed.get("git_commit") != expected_commit:
        issues.append(
            f"git_commit: lock={expected_commit!r} installed={installed.get('git_commit')!r}"
        )

    if issues:
        print("✗ Remote mesh compatibility mismatch:")
        for issue in issues:
            print(f"  - {issue}")
        sys.exit(1)
    PY
    echo "✓ Remote mesh compatibility matches lock ($LOCK_VERSION)"
    echo ""
    echo "▸ Installing daemon to ~/.local/bin/ on macOS…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p ~/.local/bin && cp {{mac_dir}}/src-tauri/target/release/taurhaus-daemon ~/.local/bin/ && codesign --force --sign - ~/.local/bin/taurhaus-daemon'"
    echo ""
    echo "▸ Installing mesh to ~/.local/bin/ on macOS…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p ~/.local/bin && cp ~/projects/mesh/target/release/mesh ~/.local/bin/ && chmod 755 ~/.local/bin/mesh && codesign --force --sign - ~/.local/bin/mesh'"
    echo ""
    echo "▸ Bundling daemon + mesh into resources…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p {{mac_dir}}/src-tauri/resources && cp {{mac_dir}}/src-tauri/target/release/taurhaus-daemon {{mac_dir}}/src-tauri/resources/ && codesign --force --sign - {{mac_dir}}/src-tauri/resources/taurhaus-daemon'"
    ssh {{mac_host}} "zsh -ilc 'cp ~/projects/mesh/target/release/mesh {{mac_dir}}/src-tauri/resources/mesh && chmod 755 {{mac_dir}}/src-tauri/resources/mesh && codesign --force --sign - {{mac_dir}}/src-tauri/resources/mesh && printf \"%s\\n\" \"$LOCK_VERSION\" > {{mac_dir}}/src-tauri/resources/mesh.version'"
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
    MESH_PROJECT="${MESH_PROJECT:-$HOME/projects/mesh}"
    LOCK_FILE="{{project}}/src-tauri/resources/mesh.lock.json"
    if [ ! -d "$MESH_PROJECT" ]; then
        echo "✗ Mesh project not found at $MESH_PROJECT"
        exit 1
    fi
    if [ ! -f "$LOCK_FILE" ]; then
        echo "✗ Lock manifest not found at $LOCK_FILE"
        exit 1
    fi
    mapfile -t LOCK_FIELDS < <(LOCK_FILE="$LOCK_FILE" python3 - <<'PY'
    import json
    import os

    with open(os.environ["LOCK_FILE"], "r", encoding="utf-8") as handle:
        lock = json.load(handle)

    print(lock["version"])
    print(lock["protocol_version"])
    print(lock["schema_version"])
    print("null" if lock.get("git_commit") is None else json.dumps(lock["git_commit"]))
    PY
    )
    LOCK_VERSION="${LOCK_FIELDS[0]}"
    LOCK_PROTOCOL="${LOCK_FIELDS[1]}"
    LOCK_SCHEMA="${LOCK_FIELDS[2]}"
    LOCK_GIT_COMMIT_RAW="${LOCK_FIELDS[3]}"
    echo "▸ Syncing mesh source to macOS…"
    rsync -az --delete --exclude='target' "$MESH_PROJECT"/ {{mac_host}}:~/projects/mesh/
    echo "✓ Mesh sync complete"
    echo ""
    echo "▸ Cleaning remote mesh target to refresh build metadata…"
    ssh {{mac_host}} "zsh -ilc 'cd ~/projects/mesh && cargo clean'"
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
    REMOTE_MESH_JSON=$(ssh {{mac_host}} "zsh -ilc '~/projects/mesh/target/x86_64-apple-darwin/release/mesh version --json'")
    LOCK_FILE="$LOCK_FILE" INSTALLED_MESH_JSON="$REMOTE_MESH_JSON" python3 - <<'PY'
    import json
    import os
    import sys

    with open(os.environ["LOCK_FILE"], "r", encoding="utf-8") as handle:
        lock = json.load(handle)
    installed = json.loads(os.environ["INSTALLED_MESH_JSON"])

    issues = []
    for key in ("version", "protocol_version", "schema_version"):
        if installed.get(key) != lock.get(key):
            issues.append(f"{key}: lock={lock.get(key)!r} installed={installed.get(key)!r}")

    expected_commit = lock.get("git_commit")
    if expected_commit is not None and installed.get("git_commit") != expected_commit:
        issues.append(
            f"git_commit: lock={expected_commit!r} installed={installed.get('git_commit')!r}"
        )

    if issues:
        print("✗ Remote mesh compatibility mismatch:")
        for issue in issues:
            print(f"  - {issue}")
        sys.exit(1)
    PY
    echo "✓ Remote mesh compatibility matches lock ($LOCK_VERSION)"
    echo ""
    echo "▸ Bundling daemon + mesh into resources…"
    ssh {{mac_host}} "zsh -ilc 'cp {{mac_dir}}/src-tauri/target/release/taurhaus-daemon {{mac_dir}}/src-tauri/resources/ && codesign --force --sign - {{mac_dir}}/src-tauri/resources/taurhaus-daemon'"
    ssh {{mac_host}} "zsh -ilc 'cp ~/projects/mesh/target/x86_64-apple-darwin/release/mesh {{mac_dir}}/src-tauri/resources/mesh && chmod 755 {{mac_dir}}/src-tauri/resources/mesh && codesign --force --sign - {{mac_dir}}/src-tauri/resources/mesh && printf \"%s\\n\" \"$LOCK_VERSION\" > {{mac_dir}}/src-tauri/resources/mesh.version'"
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
    MESH_PROJECT="${MESH_PROJECT:-$HOME/projects/mesh}"
    LOCK_FILE="{{project}}/src-tauri/resources/mesh.lock.json"
    if [ ! -d "$MESH_PROJECT" ]; then
        echo "✗ Mesh project not found at $MESH_PROJECT"
        exit 1
    fi
    if [ ! -f "$LOCK_FILE" ]; then
        echo "✗ Lock manifest not found at $LOCK_FILE"
        exit 1
    fi
    mapfile -t LOCK_FIELDS < <(LOCK_FILE="$LOCK_FILE" python3 - <<'PY'
    import json
    import os

    with open(os.environ["LOCK_FILE"], "r", encoding="utf-8") as handle:
        lock = json.load(handle)

    print(lock["version"])
    print(lock["protocol_version"])
    print(lock["schema_version"])
    print("null" if lock.get("git_commit") is None else json.dumps(lock["git_commit"]))
    PY
    )
    LOCK_VERSION="${LOCK_FIELDS[0]}"
    LOCK_PROTOCOL="${LOCK_FIELDS[1]}"
    LOCK_SCHEMA="${LOCK_FIELDS[2]}"
    LOCK_GIT_COMMIT_RAW="${LOCK_FIELDS[3]}"
    echo "▸ Syncing mesh source to macOS…"
    rsync -az --delete --exclude='target' "$MESH_PROJECT"/ {{mac_host}}:~/projects/mesh/
    echo "✓ Mesh sync complete"
    echo ""
    echo "▸ Cleaning remote mesh target to refresh build metadata…"
    ssh {{mac_host}} "zsh -ilc 'cd ~/projects/mesh && cargo clean'"
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
    REMOTE_MESH_JSON=$(ssh {{mac_host}} "zsh -ilc '~/projects/mesh/target/universal-apple-darwin/release/mesh version --json'")
    LOCK_FILE="$LOCK_FILE" INSTALLED_MESH_JSON="$REMOTE_MESH_JSON" python3 - <<'PY'
    import json
    import os
    import sys

    with open(os.environ["LOCK_FILE"], "r", encoding="utf-8") as handle:
        lock = json.load(handle)
    installed = json.loads(os.environ["INSTALLED_MESH_JSON"])

    issues = []
    for key in ("version", "protocol_version", "schema_version"):
        if installed.get(key) != lock.get(key):
            issues.append(f"{key}: lock={lock.get(key)!r} installed={installed.get(key)!r}")

    expected_commit = lock.get("git_commit")
    if expected_commit is not None and installed.get("git_commit") != expected_commit:
        issues.append(
            f"git_commit: lock={expected_commit!r} installed={installed.get('git_commit')!r}"
        )

    if issues:
        print("✗ Remote mesh compatibility mismatch:")
        for issue in issues:
            print(f"  - {issue}")
        sys.exit(1)
    PY
    echo "✓ Remote mesh compatibility matches lock ($LOCK_VERSION)"
    echo ""
    echo "▸ Bundling daemon + mesh into resources…"
    ssh {{mac_host}} "zsh -ilc 'mkdir -p {{mac_dir}}/src-tauri/resources && cp {{mac_dir}}/src-tauri/target/universal-apple-darwin/release/taurhaus-daemon {{mac_dir}}/src-tauri/resources/ && codesign --force --sign - {{mac_dir}}/src-tauri/resources/taurhaus-daemon'"
    ssh {{mac_host}} "zsh -ilc 'cp ~/projects/mesh/target/universal-apple-darwin/release/mesh {{mac_dir}}/src-tauri/resources/mesh && chmod 755 {{mac_dir}}/src-tauri/resources/mesh && codesign --force --sign - {{mac_dir}}/src-tauri/resources/mesh && printf \"%s\\n\" \"$LOCK_VERSION\" > {{mac_dir}}/src-tauri/resources/mesh.version'"
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
# Agents must run this before reporting a task as done.
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
