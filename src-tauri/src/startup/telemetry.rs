use std::path::PathBuf;

use serde_json::{Map, Value};

use super::setup::SetupPaths;
use super::SetupContext;

pub(crate) fn emit_startup_event(
    level: &str,
    event: &str,
    message: &'static str,
    fields: Map<String, Value>,
) {
    crate::commands::logging::emit_global(
        level,
        "backend",
        event,
        Some(message.to_string()),
        fields,
    );
}

fn startup_base_fields() -> Map<String, Value> {
    Map::new()
}

fn insert_u64(fields: &mut Map<String, Value>, key: &str, value: u64) {
    fields.insert(
        key.to_string(),
        Value::Number(serde_json::Number::from(value)),
    );
}

pub(super) fn emit_startup_app_started() {
    let mut fields = startup_base_fields();
    fields.insert(
        "app_version".to_string(),
        Value::String(env!("CARGO_PKG_VERSION").to_string()),
    );
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    fields.insert("platform".to_string(), Value::String(platform.to_string()));
    fields.insert(
        "pid".to_string(),
        Value::Number(serde_json::Number::from(std::process::id())),
    );
    emit_startup_event(
        "info",
        "startup.app.started",
        "Startup bootstrap entered",
        fields,
    );
}

pub(super) fn emit_startup_paths_resolved(setup_paths: &SetupPaths) {
    let mut fields = startup_base_fields();
    fields.insert(
        "data_dir".to_string(),
        Value::String(setup_paths.data_dir.display().to_string()),
    );
    fields.insert(
        "db_path".to_string(),
        Value::String(setup_paths.db_path.display().to_string()),
    );
    fields.insert(
        "log_path".to_string(),
        Value::String(setup_paths.log_path.display().to_string()),
    );
    fields.insert(
        "used_data_dir_override".to_string(),
        Value::Bool(super::setup::data_dir_override_enabled()),
    );
    fields.insert(
        "used_claude_dir_override".to_string(),
        Value::Bool(super::setup::claude_dir_override_enabled()),
    );
    emit_startup_event(
        "info",
        "startup.paths.resolved",
        "Startup paths resolved",
        fields,
    );
}

pub(super) fn emit_startup_logging_initialized(setup_paths: &SetupPaths) {
    let mut fields = startup_base_fields();
    fields.insert(
        "log_path".to_string(),
        Value::String(setup_paths.log_path.display().to_string()),
    );
    fields.insert("format".to_string(), Value::String("jsonl".to_string()));
    fields.insert("rotation_enabled".to_string(), Value::Bool(true));
    emit_startup_event(
        "info",
        "startup.logging.initialized",
        "Startup logging sink initialized",
        fields,
    );
}

pub(super) fn emit_startup_database_started(setup_paths: &SetupPaths) {
    let mut fields = startup_base_fields();
    fields.insert(
        "db_path".to_string(),
        Value::String(setup_paths.db_path.display().to_string()),
    );
    emit_startup_event(
        "info",
        "startup.database.started",
        "Startup database initialization started",
        fields,
    );
}

pub(super) fn emit_startup_database_completed(setup_paths: &SetupPaths, duration_ms: u64) {
    let mut fields = startup_base_fields();
    fields.insert(
        "db_path".to_string(),
        Value::String(setup_paths.db_path.display().to_string()),
    );
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    fields.insert(
        "migration_count".to_string(),
        Value::Number(serde_json::Number::from(0_u64)),
    );
    emit_startup_event(
        "info",
        "startup.database.completed",
        "Startup database initialization completed",
        fields,
    );
}

pub(super) fn emit_startup_database_failed(setup_paths: &SetupPaths, error: &str) {
    let mut fields = startup_base_fields();
    fields.insert(
        "db_path".to_string(),
        Value::String(setup_paths.db_path.display().to_string()),
    );
    fields.insert(
        "error.code".to_string(),
        Value::String("STARTUP_DATABASE_INIT_FAILED".to_string()),
    );
    fields.insert(
        "error.message".to_string(),
        Value::String(error.to_string()),
    );
    emit_startup_event(
        "error",
        "startup.database.failed",
        "Startup database initialization failed",
        fields,
    );
}

pub(super) fn emit_startup_daemon_phase_started() {
    let fields = startup_base_fields();
    emit_startup_event(
        "info",
        "startup.daemon_phase.started",
        "Startup daemon phase determination started",
        fields,
    );
}

pub(super) fn emit_startup_daemon_phase_completed(
    context: &SetupContext,
    wsl_distro: Option<&str>,
    duration_ms: u64,
) {
    let mut fields = startup_base_fields();
    if let Some(wsl_distro) = wsl_distro {
        fields.insert(
            "wsl_distro".to_string(),
            Value::String(wsl_distro.to_string()),
        );
    }
    if let Some(daemon_addr) = context.daemon_addr.as_ref() {
        fields.insert(
            "daemon_addr".to_string(),
            Value::String(daemon_addr.clone()),
        );
    }
    fields.insert(
        "daemon_connected_at_startup".to_string(),
        Value::Bool(context.daemon_connected_at_startup),
    );
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "info",
        "startup.daemon_phase.completed",
        "Startup daemon phase determination completed",
        fields,
    );
}

pub(super) fn emit_startup_daemon_connect_succeeded(daemon_addr: &str, duration_ms: u64) {
    let mut fields = startup_base_fields();
    fields.insert(
        "daemon_addr".to_string(),
        Value::String(daemon_addr.to_string()),
    );
    fields.insert("mode".to_string(), Value::String("fast_path".to_string()));
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "info",
        "startup.daemon_connect.succeeded",
        "Startup daemon fast-path connect succeeded",
        fields,
    );
}

pub(super) fn emit_startup_daemon_connect_deferred(
    daemon_addr: &str,
    wsl_distro: Option<&str>,
    reason: &str,
    duration_ms: u64,
) {
    let mut fields = startup_base_fields();
    fields.insert(
        "daemon_addr".to_string(),
        Value::String(daemon_addr.to_string()),
    );
    if let Some(wsl_distro) = wsl_distro {
        fields.insert(
            "wsl_distro".to_string(),
            Value::String(wsl_distro.to_string()),
        );
    }
    fields.insert("reason".to_string(), Value::String(reason.to_string()));
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "warn",
        "startup.daemon_connect.deferred",
        "Startup daemon fast-path connect deferred",
        fields,
    );
}

pub(super) fn emit_startup_orchestration_started() {
    let mut fields = startup_base_fields();
    let steps = vec![
        Value::String("daemon".to_string()),
        Value::String("watchers".to_string()),
        Value::String("compaction".to_string()),
        Value::String("search".to_string()),
        Value::String("background_tasks".to_string()),
    ];
    fields.insert("steps".to_string(), Value::Array(steps));
    emit_startup_event(
        "info",
        "startup.orchestration.started",
        "Startup orchestration started",
        fields,
    );
}

pub(super) fn emit_startup_orchestration_completed(duration_ms: u64) {
    let mut fields = startup_base_fields();
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "info",
        "startup.orchestration.completed",
        "Startup orchestration completed",
        fields,
    );
}

pub(super) fn emit_startup_watchers_initialized(
    duration_ms: u64,
    local_watcher_enabled: bool,
    daemon_watch_bootstrap: bool,
) {
    let mut fields = startup_base_fields();
    fields.insert(
        "local_watcher_enabled".to_string(),
        Value::Bool(local_watcher_enabled),
    );
    fields.insert(
        "daemon_watch_bootstrap".to_string(),
        Value::Bool(daemon_watch_bootstrap),
    );
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "info",
        "startup.watchers.initialized",
        "Startup watchers initialized",
        fields,
    );
}

pub(super) fn emit_startup_search_initialized(
    index_path: PathBuf,
    doc_count: u64,
    duration_ms: u64,
) {
    let mut fields = startup_base_fields();
    fields.insert(
        "index_path".to_string(),
        Value::String(index_path.display().to_string()),
    );
    fields.insert(
        "doc_count".to_string(),
        Value::Number(serde_json::Number::from(doc_count)),
    );
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "info",
        "startup.search.initialized",
        "Startup search initialized",
        fields,
    );
}

pub(super) fn emit_startup_init_failed(
    event: &'static str,
    message: &'static str,
    code: &'static str,
    phase: &'static str,
    duration_ms: u64,
    error: &dyn std::error::Error,
) {
    let mut fields = startup_base_fields();
    fields.insert("error.code".to_string(), Value::String(code.to_string()));
    fields.insert(
        "error.message".to_string(),
        Value::String(error.to_string()),
    );
    fields.insert("phase".to_string(), Value::String(phase.to_string()));
    insert_u64(&mut fields, "duration_ms", duration_ms);
    fields.insert("fatal".to_string(), Value::Bool(true));
    fields.insert("degraded".to_string(), Value::Bool(false));
    emit_startup_event("error", event, message, fields);
}

pub(super) fn emit_startup_background_task_started(task_group: &str) {
    let mut fields = startup_base_fields();
    fields.insert(
        "task_group".to_string(),
        Value::String(task_group.to_string()),
    );
    emit_startup_event(
        "info",
        "startup.background_tasks.started",
        "Startup background task started",
        fields,
    );
}

pub(super) fn emit_startup_background_task_completed(task_group: &str, duration_ms: u64) {
    let mut fields = startup_base_fields();
    fields.insert(
        "task_group".to_string(),
        Value::String(task_group.to_string()),
    );
    insert_u64(&mut fields, "duration_ms", duration_ms);
    emit_startup_event(
        "info",
        "startup.background_tasks.completed",
        "Startup background task completed",
        fields,
    );
}

pub(super) fn emit_startup_self_heal_started(initial_delay_ms: u64, check_interval_ms: u64) {
    let mut fields = startup_base_fields();
    insert_u64(&mut fields, "initial_delay_ms", initial_delay_ms);
    insert_u64(&mut fields, "check_interval_ms", check_interval_ms);
    emit_startup_event(
        "info",
        "startup.self_heal.started",
        "Startup self-heal monitor started",
        fields,
    );
}

pub(super) fn emit_startup_self_heal_completed(
    duration_ms: u64,
    teams_scanned: u64,
    teams_skipped: u64,
    teams_reconciled: u64,
    team_daemons_ensured: u64,
    team_errors: u64,
) {
    let mut fields = startup_base_fields();
    insert_u64(&mut fields, "duration_ms", duration_ms);
    insert_u64(&mut fields, "teams_scanned", teams_scanned);
    insert_u64(&mut fields, "teams_skipped", teams_skipped);
    insert_u64(&mut fields, "teams_reconciled", teams_reconciled);
    insert_u64(&mut fields, "team_daemons_ensured", team_daemons_ensured);
    insert_u64(&mut fields, "team_errors", team_errors);
    emit_startup_event(
        "info",
        "startup.self_heal.completed",
        "Startup self-heal pass completed",
        fields,
    );
}

pub(super) fn emit_startup_self_heal_failed(duration_ms: u64, error: &str) {
    let mut fields = startup_base_fields();
    insert_u64(&mut fields, "duration_ms", duration_ms);
    fields.insert(
        "error.code".to_string(),
        Value::String("STARTUP_SELF_HEAL_FAILED".to_string()),
    );
    fields.insert(
        "error.message".to_string(),
        Value::String(error.to_string()),
    );
    emit_startup_event(
        "warn",
        "startup.self_heal.failed",
        "Startup self-heal pass failed",
        fields,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::logging::{install_global_sink, LogFileState};
    use std::path::Path;
    use std::time::Duration;

    fn read_lines(path: &Path) -> Vec<String> {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    fn wait_for_lines(path: &Path, expected_minimum: usize) -> Vec<String> {
        for _ in 0..100 {
            let lines = read_lines(path);
            if lines.len() >= expected_minimum {
                return lines;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        read_lines(path)
    }

    #[test]
    fn startup_emitters_use_inventory_specific_event_names() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join("startup-events.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let setup_paths = SetupPaths {
            data_dir: dir.path().join("data"),
            log_path: log_path.clone(),
            db_path: dir.path().join("taurhaus.db"),
        };
        let context = SetupContext {
            data_dir: setup_paths.data_dir.clone(),
            log_path: setup_paths.log_path.clone(),
            db_path: setup_paths.db_path.clone(),
            wsl_distro: Some("Ubuntu".to_string()),
            daemon_addr: Some("127.0.0.1:17233".to_string()),
            daemon_connected_at_startup: false,
        };

        emit_startup_app_started();
        emit_startup_paths_resolved(&setup_paths);
        emit_startup_logging_initialized(&setup_paths);
        emit_startup_database_started(&setup_paths);
        emit_startup_database_completed(&setup_paths, 1);
        emit_startup_daemon_phase_started();
        emit_startup_daemon_phase_completed(&context, Some("Ubuntu"), 2);
        emit_startup_daemon_connect_succeeded("127.0.0.1:17233", 1);
        emit_startup_daemon_connect_deferred(
            "127.0.0.1:17233",
            Some("Ubuntu"),
            "daemon_unavailable_at_startup",
            1,
        );
        emit_startup_orchestration_started();
        emit_startup_orchestration_completed(3);
        emit_startup_watchers_initialized(4, true, false);
        emit_startup_search_initialized(context.data_dir.join("search_index"), 0, 5);

        let lines = wait_for_lines(&log_path, 13);
        let events: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("valid json"))
            .collect();

        let expected = [
            "startup.app.started",
            "startup.paths.resolved",
            "startup.logging.initialized",
            "startup.database.started",
            "startup.database.completed",
            "startup.daemon_phase.started",
            "startup.daemon_phase.completed",
            "startup.daemon_connect.succeeded",
            "startup.daemon_connect.deferred",
            "startup.orchestration.started",
            "startup.orchestration.completed",
            "startup.watchers.initialized",
            "startup.search.initialized",
        ];
        for event_name in expected {
            assert!(
                events.iter().any(|value| value["event"] == event_name),
                "missing expected startup event: {event_name}"
            );
        }
        assert!(
            events
                .iter()
                .all(|value| value["event"] != "startup.phase.started"
                    && value["event"] != "startup.phase.completed"
                    && value["event"] != "startup.phase.failed"),
            "legacy generic startup phase events must not be emitted"
        );
    }
}
