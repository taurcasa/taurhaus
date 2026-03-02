#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-unit}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_LOG_DIR="$ROOT_DIR/target"
mkdir -p "$DEFAULT_LOG_DIR"
LOG_FILE="${TAURHAUS_BISECT_LOG:-$DEFAULT_LOG_DIR/rust-test-bisect.log}"

timestamp() {
  date -Is
}

log() {
  echo "[$(timestamp)] $*" | tee -a "$LOG_FILE"
}

run_group() {
  local label="$1"
  shift

  log "START $label"
  (
    cd "$ROOT_DIR/src-tauri"
    "$@"
  )
  log "PASS  $label"
}

run_unit_mode() {
  log "MODE unit (default fast-lane bisect)"

  run_group "claude_code" cargo test --lib claude_code:: -- --test-threads=1
  run_group "commands" cargo test --lib commands:: -- --test-threads=1
  run_group "coordination" cargo test --lib coordination:: -- --test-threads=1
  run_group "daemon-auth" cargo test --lib daemon::auth:: -- --test-threads=1
  run_group "daemon-protocol" cargo test --lib daemon::protocol:: -- --test-threads=1
  run_group "db" cargo test --lib db:: -- --test-threads=1
  run_group "fs-watcher" cargo test --lib fs::watcher::tests:: -- --test-threads=1 \
    --skip fs::watcher::tests::watcher_starts_and_stops \
    --skip fs::watcher::tests::unwatch_all_clears_everything
  run_group "git" cargo test --lib git:: -- --test-threads=1
  run_group "models" cargo test --lib models:: -- --test-threads=1
  run_group "platform" cargo test --lib platform:: -- --test-threads=1
  run_group "provider-local" cargo test --lib provider::local:: -- --test-threads=1
  run_group "provider-path" cargo test --lib provider::path:: -- --test-threads=1
  run_group "provider-routing" cargo test --lib provider::tests:: -- --test-threads=1
  run_group "search" cargo test --lib search:: -- --test-threads=1
  run_group "services" cargo test --lib services:: -- --test-threads=1
  run_group "session" cargo test --lib session:: -- --test-threads=1
  run_group "session-scanner" cargo test --lib session_scanner:: -- --test-threads=1
  run_group "task-scanner" cargo test --lib task_scanner:: -- --test-threads=1
  run_group "terminal" cargo test --lib terminal:: -- --test-threads=1
}

run_heavy_mode() {
  log "MODE heavy (known daemon/network/watcher suites)"

  run_group "daemon-server" cargo test --lib daemon::server::tests:: -- --test-threads=1
  run_group "daemon-event-listener" cargo test --lib daemon::event_listener::tests:: -- --test-threads=1
  run_group "provider-daemon-client" cargo test --lib provider::daemon_client::tests:: -- --test-threads=1
  run_group "daemon-launcher" cargo test --lib daemon::launcher::tests:: -- --test-threads=1
  run_group "fs-watcher-start-stop" cargo test --lib fs::watcher::tests::watcher_starts_and_stops -- --test-threads=1
  run_group "fs-watcher-unwatch-all" cargo test --lib fs::watcher::tests::unwatch_all_clears_everything -- --test-threads=1
}

run_commands_mode() {
  log "MODE commands (sub-bisect for commands module)"

  run_group "commands-command-center" cargo test --lib commands::command_center:: -- --test-threads=1
  run_group "commands-coordination" cargo test --lib commands::coordination:: -- --test-threads=1
  run_group "commands-daemon" cargo test --lib commands::daemon:: -- --test-threads=1
  run_group "commands-projects" cargo test --lib commands::projects:: -- --test-threads=1
}

run_coordination_mode() {
  log "MODE coordination (sub-bisect for coordination module)"

  run_group "coordination-audit" cargo test --lib coordination::audit:: -- --test-threads=1
  run_group "coordination-backend" cargo test --lib coordination::backend:: -- --test-threads=1
  run_group "coordination-consumer" cargo test --lib coordination::consumer:: -- --test-threads=1
  run_group "coordination-delivery" cargo test --lib coordination::delivery:: -- --test-threads=1
  run_group "coordination-domain" cargo test --lib coordination::domain:: -- --test-threads=1
  run_group "coordination-errors" cargo test --lib coordination::errors:: -- --test-threads=1
  run_group "coordination-events" cargo test --lib coordination::events:: -- --test-threads=1
  run_group "coordination-orchestrator" cargo test --lib coordination::orchestrator:: -- --test-threads=1
  run_group "coordination-reconcile" cargo test --lib coordination::reconcile:: -- --test-threads=1
  run_group "coordination-requests" cargo test --lib coordination::requests:: -- --test-threads=1
  run_group "coordination-state" cargo test --lib coordination::state:: -- --test-threads=1
  run_group "coordination-stores-config" cargo test --lib coordination::stores::config:: -- --test-threads=1
  run_group "coordination-stores-lock" cargo test --lib coordination::stores::lock:: -- --test-threads=1
  run_group "coordination-stores-runtime" cargo test --lib coordination::stores::runtime:: -- --test-threads=1
}

run_orchestrator_mode() {
  log "MODE orchestrator (per-test bisect for coordination::orchestrator::tests)"

  local tests_file="$ROOT_DIR/src-tauri/src/coordination/orchestrator/tests.rs"
  mapfile -t test_names < <(
    awk '
      /^\s*#\[test\]/{want=1; next}
      want && /^\s*fn[[:space:]]+[A-Za-z0-9_]+/{
        name=$0
        sub(/^[[:space:]]*fn[[:space:]]+/, "", name)
        sub(/\(.*/, "", name)
        print name
        want=0
      }
    ' "$tests_file"
  )

  for test_name in "${test_names[@]}"; do
    run_group "coordination-orchestrator::$test_name" \
      cargo test --lib "coordination::orchestrator::tests::$test_name" -- --exact --test-threads=1
  done
}

log "BEGIN rust-test-bisect mode=$MODE"
if BOOT_TIME="$(uptime -s 2>/dev/null)"; then
  log "BOOT  $BOOT_TIME"
fi

case "$MODE" in
  unit)
    run_unit_mode
    ;;
  heavy)
    run_heavy_mode
    ;;
  commands)
    run_commands_mode
    ;;
  coordination)
    run_coordination_mode
    ;;
  orchestrator)
    run_orchestrator_mode
    ;;
  *)
    echo "Usage: $0 [unit|heavy|commands|coordination|orchestrator]" >&2
    exit 2
    ;;
esac

log "DONE  rust-test-bisect mode=$MODE"
