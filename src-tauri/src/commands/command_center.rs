use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::logging::LogFileState;
use crate::commands::projects::DbState;
use crate::commands::terminal_settings::load_terminal_settings;
use crate::daemon::protocol::{self, LaunchMode};
use crate::errors::SanitizeErr;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::control::{resolve_configured_tool_command, TMUX_SESSION_NAME};
use crate::session_scanner::{ClaudeSession, SessionState};
use crate::ProviderState;
use serde_json::{Map, Value};

static SESSION_ACTIVITY_RECONCILE_QUEUED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn list_cli_sessions(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
) -> Result<Vec<ClaudeSession>, String> {
    let span = IpcCommandSpan::start("list_cli_sessions");
    let result = {
        if let Some(ref daemon) = provider.daemon {
            if daemon.is_connected() {
                let id = "list-sessions";
                let request = protocol::DaemonRequest::new(
                    id,
                    protocol::method::LIST_CLAUDE_SESSIONS,
                    serde_json::Value::Null,
                );
                match daemon.send_status_request(&request) {
                    Ok(response) if response.is_ok() => {
                        let mut sessions = decode_daemon_session_list(response.result)?;

                        if !crate::daemon::launcher::is_native_daemon() {
                            if let Some(ref distro) = provider.wsl_distro {
                                for session in &mut sessions {
                                    if session.project_path.starts_with('/') {
                                        session.project_path = crate::provider::path::to_windows(
                                            &session.project_path,
                                            distro,
                                        );
                                    }
                                }
                            }
                        }

                        promote_activity_from_sessions(&app, db.inner(), &sessions);
                        return Ok(sessions);
                    }
                    Ok(response) => {
                        tracing::warn!(error = ?response.error, "Daemon returned error for session listing");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to reach daemon for session listing");
                    }
                }
            }
        }

        let fallback = crate::session_scanner::scan_sessions();
        tracing::debug!(count = fallback.len(), "list_cli_sessions: fallback scan");
        promote_activity_from_sessions(&app, db.inner(), &fallback);
        Ok(fallback)
    };
    span.finish_result(&result);
    result
}

fn normalize_project_path_key(path: &str) -> String {
    let normalized = path
        .trim_end_matches('/')
        .trim_end_matches('\\')
        .to_string();
    if cfg!(windows) {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}

fn resolve_project_path(db: &DbState, project_id: &str) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = crate::db::queries::get_project(&conn, project_id)
        .sanitize_err()?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;
    Ok(project.path)
}

fn enqueue_activity_watch_reconcile(app: tauri::AppHandle, reason: &'static str) {
    if SESSION_ACTIVITY_RECONCILE_QUEUED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    #[cfg(test)]
    {
        crate::startup::watchers::reconcile_activity_watches(&app, reason);
        SESSION_ACTIVITY_RECONCILE_QUEUED.store(false, Ordering::Release);
    }

    #[cfg(not(test))]
    {
        std::thread::spawn(move || {
            struct ResetQueuedFlag;
            impl Drop for ResetQueuedFlag {
                fn drop(&mut self) {
                    SESSION_ACTIVITY_RECONCILE_QUEUED.store(false, Ordering::Release);
                }
            }

            let _reset_queued_flag = ResetQueuedFlag;
            crate::startup::watchers::reconcile_activity_watches(&app, reason);
        });
    }
}

fn promote_activity_from_sessions(
    app: &tauri::AppHandle,
    db: &DbState,
    sessions: &[ClaudeSession],
) {
    match promote_activity_from_sessions_impl(db, sessions) {
        Ok(promoted) if promoted > 0 => {
            enqueue_activity_watch_reconcile(app.clone(), "session_activity_detected");
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Failed to promote project activity from session scan"
            );
        }
    }
}

fn promote_activity_from_sessions_impl(
    db: &DbState,
    sessions: &[ClaudeSession],
) -> Result<usize, String> {
    let mut active_paths = HashSet::new();
    for session in sessions {
        if session.state != SessionState::Active {
            continue;
        }
        active_paths.insert(normalize_project_path_key(&session.project_path));
    }
    if active_paths.is_empty() {
        return Ok(0);
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let settings = crate::db::settings_queries::get_all_settings(&conn).sanitize_err()?;
    let projects =
        crate::services::project::list_projects(&conn, &settings.thresholds).sanitize_err()?;

    let mut by_path = HashMap::new();
    for project in projects {
        by_path.insert(
            normalize_project_path_key(&project.path),
            (project.id, project.activity_state),
        );
    }

    let mut promoted = 0usize;
    for path in active_paths {
        let Some((project_id, state)) = by_path.get(&path) else {
            continue;
        };
        if *state == crate::models::ActivityState::Active {
            continue;
        }
        crate::services::project::touch_activity(&conn, project_id).sanitize_err()?;
        promoted += 1;
    }

    Ok(promoted)
}

#[tauri::command]
pub fn launch_cli_session(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    log_file: State<'_, LogFileState>,
    project_id: String,
    mode: LaunchMode,
    cli_tool: Option<CliTool>,
) -> Result<protocol::LaunchSessionResult, String> {
    let span = IpcCommandSpan::start("launch_cli_session");
    let result = launch_cli_session_impl(
        db.inner(),
        provider.inner(),
        log_file.inner(),
        project_id,
        mode,
        cli_tool,
    );
    span.finish_result(&result);
    result
}

fn launch_cli_session_impl(
    db: &DbState,
    provider: &ProviderState,
    log_file: &LogFileState,
    project_id: String,
    mode: LaunchMode,
    cli_tool: Option<CliTool>,
) -> Result<protocol::LaunchSessionResult, String> {
    let tool = cli_tool.unwrap_or(CliTool::Claude);

    let mut launch_fields = Map::new();
    launch_fields.insert("project_id".to_string(), Value::String(project_id.clone()));
    launch_fields.insert("mode".to_string(), Value::String(format!("{mode:?}")));
    launch_fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
    log_file.emit(
        "info",
        "command_center",
        "command_center.launch.start",
        Some("Launching CLI session".to_string()),
        launch_fields,
    );

    let project_path = resolve_project_path(db, &project_id)?;

    let linux_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());

    let mut path_fields = Map::new();
    path_fields.insert("db_path".to_string(), Value::String(project_path.clone()));
    path_fields.insert("linux_path".to_string(), Value::String(linux_path.clone()));
    log_file.emit(
        "debug",
        "command_center",
        "command_center.launch.path_resolved",
        Some("Resolved project path for launch".to_string()),
        path_fields,
    );

    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "launch-session";
            let ts = load_terminal_settings(db);
            let tool_cmd = resolve_configured_tool_command(&ts.cli_commands, tool, mode);
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::LAUNCH_SESSION,
                protocol::LaunchSessionParams {
                    project_path: linux_path.clone(),
                    mode,
                    cli_tool: tool,
                    tmux_layout: ts.tmux_layout.clone(),
                    command_override: Some(tool_cmd),
                },
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    let result = decode_daemon_launch_result(response.result)?;

                    let mut success_fields = Map::new();
                    success_fields.insert(
                        "tmux_window".to_string(),
                        Value::String(result.tmux_window.clone()),
                    );
                    success_fields.insert(
                        "tmux_pane".to_string(),
                        Value::String(result.tmux_pane.clone()),
                    );
                    log_file.emit(
                        "info",
                        "command_center",
                        "command_center.launch.daemon_success",
                        Some("Launch succeeded via daemon".to_string()),
                        success_fields,
                    );

                    let tmux_session = result.tmux_session.as_deref().unwrap_or(TMUX_SESSION_NAME);
                    let ts = load_terminal_settings(db);
                    let _ = crate::terminal::handle_terminal(
                        crate::terminal::TerminalIntent::EnsureOpen {
                            distro: provider.wsl_distro.clone(),
                            tmux_session: tmux_session.to_string(),
                            emulator: ts.emulator,
                            custom_command: ts.custom_command,
                        },
                    );
                    return Ok(result);
                }
                Ok(response) => {
                    let msg = response
                        .error
                        .map(|e| e.message)
                        .unwrap_or_else(|| "Unknown error".to_string());
                    let mut fail_fields = Map::new();
                    fail_fields.insert("error".to_string(), Value::String(msg.clone()));
                    log_file.emit(
                        "warn",
                        "command_center",
                        "command_center.launch.daemon_failed",
                        Some("Launch failed via daemon".to_string()),
                        fail_fields,
                    );
                    return Err(format!("Failed to launch session: {msg}"));
                }
                Err(e) => {
                    let mut unreachable_fields = Map::new();
                    unreachable_fields.insert("error".to_string(), Value::String(e.to_string()));
                    log_file.emit(
                        "warn",
                        "command_center",
                        "command_center.launch.daemon_unreachable",
                        Some("Daemon unreachable during launch".to_string()),
                        unreachable_fields,
                    );
                    tracing::warn!(error = %e, "Daemon unreachable for launch");
                }
            }
        }
    }

    log_file.emit(
        "info",
        "command_center",
        "command_center.launch.local_fallback",
        Some("Falling back to local tmux launch".to_string()),
        Map::new(),
    );
    let ts = load_terminal_settings(db);
    let tool_cmd = resolve_configured_tool_command(&ts.cli_commands, tool, mode);
    let (session, window, pane) = crate::session_scanner::control::launch_in_tmux_with_layout(
        &linux_path,
        mode,
        tool,
        &ts.tmux_layout,
        Some(&tool_cmd),
    )
    .map_err(|e| format!("Failed to launch session: {e}"))?;

    #[cfg(target_os = "macos")]
    {
        let _ = crate::terminal::handle_terminal(crate::terminal::TerminalIntent::EnsureOpen {
            distro: None,
            tmux_session: session.clone(),
            emulator: ts.emulator,
            custom_command: ts.custom_command,
        });
    }

    Ok(protocol::LaunchSessionResult {
        tmux_session: Some(session),
        tmux_window: window,
        tmux_pane: pane,
    })
}

#[tauri::command]
pub fn stop_cli_session(
    provider: State<'_, ProviderState>,
    tmux_pane: String,
    cli_tool: Option<CliTool>,
) -> Result<(), String> {
    let span = IpcCommandSpan::start("stop_cli_session");
    let result = stop_cli_session_impl(provider.inner(), tmux_pane, cli_tool);
    span.finish_result(&result);
    result
}

fn stop_cli_session_impl(
    provider: &ProviderState,
    tmux_pane: String,
    cli_tool: Option<CliTool>,
) -> Result<(), String> {
    let tool = cli_tool.unwrap_or(CliTool::Claude);

    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "stop-session";
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::STOP_SESSION,
                protocol::StopSessionParams {
                    tmux_pane: tmux_pane.clone(),
                    cli_tool: tool,
                },
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => return Ok(()),
                Ok(response) => {
                    let msg = response
                        .error
                        .map(|e| e.message)
                        .unwrap_or_else(|| "Unknown error".to_string());
                    return Err(format!("Failed to stop session: {msg}"));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Daemon unreachable for stop");
                }
            }
        }
    }

    crate::session_scanner::control::stop_session(&tmux_pane, tool)
        .map_err(|e| format!("Failed to stop session: {e}"))
}

#[tauri::command]
pub fn navigate_to_session(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    log_file: State<'_, LogFileState>,
    tmux_session: String,
    tmux_window: String,
    tmux_pane: String,
    open_terminal: Option<bool>,
) -> Result<(), String> {
    let span = IpcCommandSpan::start("navigate_to_session");
    let result = {
        let should_open = open_terminal.unwrap_or(false);

        let mut navigation_fields = Map::new();
        navigation_fields.insert(
            "tmux_session".to_string(),
            Value::String(tmux_session.clone()),
        );
        navigation_fields.insert(
            "tmux_window".to_string(),
            Value::String(tmux_window.clone()),
        );
        navigation_fields.insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
        navigation_fields.insert("open_terminal".to_string(), Value::Bool(should_open));
        log_file.emit(
            "info",
            "command_center",
            "command_center.navigate",
            Some("Navigate to tmux session".to_string()),
            navigation_fields,
        );
        if let Some(ref daemon) = provider.daemon {
            if daemon.is_connected() {
                let id = "navigate-session";
                let request = protocol::DaemonRequest::new(
                    id,
                    protocol::method::NAVIGATE_TO_SESSION,
                    protocol::NavigateToSessionParams {
                        tmux_session: tmux_session.clone(),
                        tmux_window: tmux_window.clone(),
                        tmux_pane: tmux_pane.clone(),
                    },
                );
                match daemon.send_status_request(&request) {
                    Ok(response) if response.is_ok() => {
                        let ts = load_terminal_settings(&db);
                        let intent = if should_open || cfg!(target_os = "macos") {
                            crate::terminal::TerminalIntent::EnsureOpen {
                                distro: provider.wsl_distro.clone(),
                                tmux_session: tmux_session.clone(),
                                emulator: ts.emulator,
                                custom_command: ts.custom_command,
                            }
                        } else {
                            crate::terminal::TerminalIntent::FocusOnly {
                                emulator: ts.emulator,
                            }
                        };
                        let _ = crate::terminal::handle_terminal(intent);
                        return Ok(());
                    }
                    Ok(response) => {
                        let msg = response
                            .error
                            .map(|e| e.message)
                            .unwrap_or_else(|| "Unknown error".to_string());
                        return Err(format!("Failed to navigate: {msg}"));
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Daemon unreachable for navigate");
                    }
                }
            }
        }

        crate::session_scanner::control::navigate_to_pane(&tmux_session, &tmux_window, &tmux_pane)
            .map_err(|e| format!("Failed to navigate: {e}"))
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn record_session_activity(
    db: State<'_, DbState>,
    project_id: String,
    cli_tool: CliTool,
    started_at: String,
    ended_at: String,
    active_duration_ms: i64,
    total_duration_ms: i64,
) -> Result<(), String> {
    let span = IpcCommandSpan::start("record_session_activity");
    let result = record_session_activity_impl(
        db.inner(),
        project_id,
        cli_tool,
        started_at,
        ended_at,
        active_duration_ms,
        total_duration_ms,
    );
    span.finish_result(&result);
    result
}

fn record_session_activity_impl(
    db: &DbState,
    project_id: String,
    cli_tool: CliTool,
    started_at: String,
    ended_at: String,
    active_duration_ms: i64,
    total_duration_ms: i64,
) -> Result<(), String> {
    let project_path = resolve_project_path(db, &project_id)?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let cli_tool = cli_tool.to_string();
    crate::db::activity_queries::insert_session_activity(
        &conn,
        &project_path,
        &cli_tool,
        &started_at,
        &ended_at,
        active_duration_ms,
        total_duration_ms,
    )
    .sanitize_err()
}

#[tauri::command]
pub fn get_project_activity(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<crate::db::activity_queries::ProjectActivityStats, String> {
    let span = IpcCommandSpan::start("get_project_activity");
    let result = {
        let project_path = resolve_project_path(db.inner(), &project_id)?;
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::db::activity_queries::get_project_activity(&conn, &project_path).sanitize_err()
    };
    span.finish_result(&result);
    result
}

fn decode_daemon_session_list(
    payload: Option<serde_json::Value>,
) -> Result<Vec<ClaudeSession>, String> {
    match payload {
        Some(value) => serde_json::from_value(value).map_err(|e| {
            tracing::warn!(error = %e, "Failed to deserialize session list from daemon");
            format!("Session list decode error: {e}")
        }),
        None => Ok(Vec::new()),
    }
}

fn decode_daemon_launch_result(
    payload: Option<serde_json::Value>,
) -> Result<protocol::LaunchSessionResult, String> {
    let value = payload.ok_or_else(|| "Invalid launch result from daemon".to_string())?;
    serde_json::from_value(value).map_err(|e| {
        tracing::warn!(error = %e, "Failed to deserialize launch result from daemon");
        format!("Invalid launch result from daemon: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::thread;
    use tempfile::NamedTempFile;

    struct StubDaemon {
        addr: String,
        last_request: std::sync::Arc<Mutex<Option<protocol::DaemonRequest>>>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for StubDaemon {
        fn drop(&mut self) {
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn setup_db_with_project(project_id: &str, project_path: &str) -> (DbState, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("temp db");
        let conn = crate::db::init_db(tmp.path()).expect("init db");

        let now = chrono::Utc::now().to_rfc3339();
        crate::db::queries::insert_project(
            &conn,
            &crate::models::Project {
                id: project_id.to_string(),
                name: "test-project".to_string(),
                path: project_path.to_string(),
                description: None,
                last_activity_at: None,
                hero_preference: None,
                created_at: now.clone(),
                updated_at: now,
                cached_branch: None,
                cached_is_dirty: None,
            },
        )
        .expect("insert project");

        (DbState(Mutex::new(conn)), tmp)
    }

    fn setup_log_file() -> (LogFileState, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("temp log");
        let state = LogFileState::new(tmp.path().to_path_buf()).expect("create log sink");
        (state, tmp)
    }

    fn start_stub_daemon(response: serde_json::Value) -> StubDaemon {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub daemon");
        let addr = listener.local_addr().expect("stub daemon addr");
        let addr_string = format!("127.0.0.1:{}", addr.port());
        let request_slot = std::sync::Arc::new(Mutex::new(None));
        let request_slot_clone = request_slot.clone();

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept daemon client");
            let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
            let mut line = String::new();
            reader.read_line(&mut line).expect("read request");

            let request: protocol::DaemonRequest =
                serde_json::from_str(&line).expect("parse daemon request");
            if let Ok(mut slot) = request_slot_clone.lock() {
                *slot = Some(request.clone());
            }

            let mut resp = response;
            if let Some(map) = resp.as_object_mut() {
                map.insert("id".to_string(), serde_json::Value::String(request.id));
            }

            let mut writer = stream;
            let payload = serde_json::to_string(&resp).expect("serialize daemon response");
            writer
                .write_all(payload.as_bytes())
                .expect("write daemon response");
            writer.write_all(b"\n").expect("write newline");
            writer.flush().expect("flush daemon response");
        });

        StubDaemon {
            addr: addr_string,
            last_request: request_slot,
            handle: Some(handle),
        }
    }

    fn start_unreachable_stub_daemon() -> StubDaemon {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind unreachable daemon");
        let addr = listener.local_addr().expect("unreachable daemon addr");
        let addr_string = format!("127.0.0.1:{}", addr.port());
        let request_slot = std::sync::Arc::new(Mutex::new(None));

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept daemon client");
            drop(stream);
        });

        StubDaemon {
            addr: addr_string,
            last_request: request_slot,
            handle: Some(handle),
        }
    }

    #[test]
    fn launch_mode_deserializes_valid_values_and_rejects_invalid() {
        for (raw, expected) in [
            ("\"continue\"", LaunchMode::Continue),
            ("\"fresh\"", LaunchMode::Fresh),
            ("\"resume\"", LaunchMode::Resume),
        ] {
            let mode: LaunchMode = serde_json::from_str(raw).unwrap();
            assert_eq!(mode, expected);
        }
        assert!(serde_json::from_str::<LaunchMode>("\"invalid\"").is_err());
    }

    fn active_session_for(path: &str) -> ClaudeSession {
        ClaudeSession {
            pid: 1234,
            project_path: path.to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "codex --yolo".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some("%1".to_string()),
            tmux_window_name: Some("work".to_string()),
            state: SessionState::Active,
            session_id: Some("sid".to_string()),
            jsonl_path: Some("/tmp/sid.jsonl".to_string()),
            activity_confidence: crate::session_scanner::ActivityConfidence::High,
            activity_attribution: crate::session_scanner::ActivityAttribution::Attributed,
            project_unattributed_active: false,
        }
    }

    #[test]
    fn promote_activity_from_sessions_touches_dormant_project_once() {
        let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
        let sessions = vec![active_session_for("/tmp/project")];

        let promoted =
            promote_activity_from_sessions_impl(&db, &sessions).expect("promote activity");
        assert_eq!(promoted, 1);

        let promoted_again =
            promote_activity_from_sessions_impl(&db, &sessions).expect("promote activity again");
        assert_eq!(promoted_again, 0);
    }

    #[test]
    fn daemon_session_decode_handles_missing_invalid_and_valid_payloads() {
        assert!(decode_daemon_session_list(None).unwrap().is_empty());
        assert!(
            decode_daemon_session_list(Some(serde_json::json!({"not": "a session list"}))).is_err()
        );
        assert!(decode_daemon_session_list(Some(serde_json::json!([])))
            .unwrap()
            .is_empty());

        let payload = Some(serde_json::json!([
            {"pid": 1234, "project_path": "/tmp/project-a", "tty": "/dev/pts/1", "args": "claude --continue", "cli_tool": "claude", "tmux_session": "taurhaus", "tmux_window": "1", "tmux_pane": "%1", "tmux_window_name": "a", "state": "active", "session_id": "sess-a", "jsonl_path": "/tmp/a.jsonl"},
            {"pid": 5678, "project_path": "/tmp/project-b", "tty": "/dev/pts/2", "args": "codex --yolo", "cli_tool": "codex", "tmux_session": null, "tmux_window": null, "tmux_pane": null, "tmux_window_name": null, "state": "idle", "session_id": null, "jsonl_path": null}
        ]));
        let sessions = decode_daemon_session_list(payload).expect("valid session payload");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].project_path, "/tmp/project-a");
        assert_eq!(sessions[1].project_path, "/tmp/project-b");
    }

    #[test]
    fn daemon_launch_decode_handles_missing_invalid_and_valid_payloads() {
        let payload = Some(
            serde_json::json!({"tmux_session": "taurhaus", "tmux_window": "1", "tmux_pane": "%2"}),
        );
        let result = decode_daemon_launch_result(payload).expect("valid launch payload");
        assert_eq!(result.tmux_session.as_deref(), Some("taurhaus"));
        assert_eq!(result.tmux_window, "1");
        assert_eq!(result.tmux_pane, "%2");

        let err = decode_daemon_launch_result(Some(serde_json::json!({"unexpected": "shape"})))
            .expect_err("invalid payload should error");
        assert!(err.contains("Invalid launch result from daemon"));
        assert_eq!(
            decode_daemon_launch_result(None).expect_err("missing payload should error"),
            "Invalid launch result from daemon"
        );
    }

    #[test]
    fn resolve_tool_command_defaults_are_non_empty_and_match_expected_values() {
        let cmds = crate::models::CliCommandSettings::default();
        for tool in [CliTool::Claude, CliTool::Codex, CliTool::Gemini] {
            for mode in [LaunchMode::Continue, LaunchMode::Fresh, LaunchMode::Resume] {
                let command = resolve_configured_tool_command(&cmds, tool, mode);
                assert!(
                    !command.trim().is_empty(),
                    "command must be non-empty for {tool:?}/{mode:?}"
                );
            }
        }
        for (tool, mode, expected) in [
            (
                CliTool::Claude,
                LaunchMode::Continue,
                "claude --dangerously-skip-permissions --continue",
            ),
            (
                CliTool::Claude,
                LaunchMode::Fresh,
                "claude --dangerously-skip-permissions",
            ),
            (
                CliTool::Claude,
                LaunchMode::Resume,
                "claude --dangerously-skip-permissions --resume",
            ),
            (CliTool::Codex, LaunchMode::Continue, "codex --yolo"),
            (CliTool::Codex, LaunchMode::Fresh, "codex --yolo"),
            (
                CliTool::Codex,
                LaunchMode::Resume,
                "codex resume --last --yolo",
            ),
            (
                CliTool::Gemini,
                LaunchMode::Continue,
                "gemini --yolo --resume",
            ),
            (CliTool::Gemini, LaunchMode::Fresh, "gemini --yolo"),
            (
                CliTool::Gemini,
                LaunchMode::Resume,
                "gemini --yolo --resume",
            ),
        ] {
            assert_eq!(resolve_configured_tool_command(&cmds, tool, mode), expected);
        }
    }

    #[test]
    fn load_terminal_settings_returns_default_on_query_and_lock_errors() {
        let db = DbState(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
        assert_eq!(
            load_terminal_settings(&db),
            crate::models::TerminalSettings::default()
        );

        let poisoned = DbState(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = poisoned.0.lock().unwrap();
            panic!("intentional poison");
        }));
        assert_eq!(
            load_terminal_settings(&poisoned),
            crate::models::TerminalSettings::default()
        );
    }

    #[test]
    fn launch_cli_session_uses_daemon_success_response() {
        let daemon = start_stub_daemon(serde_json::json!({
            "result": {
                "tmux_session": "taurhaus",
                "tmux_window": "2",
                "tmux_pane": "%7"
            },
            "error": null
        }));
        let provider = ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: Some(
                crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                    .expect("connect daemon provider"),
            ),
            wsl_distro: None,
        };
        let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
        let (log_file, _log_file) = setup_log_file();

        let result = launch_cli_session_impl(
            &db,
            &provider,
            &log_file,
            "p1".to_string(),
            LaunchMode::Fresh,
            Some(CliTool::Claude),
        )
        .expect("daemon launch should succeed");

        assert_eq!(result.tmux_session.as_deref(), Some("taurhaus"));
        assert_eq!(result.tmux_window, "2");
        assert_eq!(result.tmux_pane, "%7");

        let request = daemon
            .last_request
            .lock()
            .expect("request slot")
            .clone()
            .expect("captured request");
        assert_eq!(request.method, protocol::method::LAUNCH_SESSION);
    }

    #[test]
    fn launch_cli_session_surfaces_daemon_error_message() {
        let daemon = start_stub_daemon(serde_json::json!({
            "result": null,
            "error": {
                "code": "LAUNCH_ERROR",
                "message": "simulated launch failure"
            }
        }));
        let provider = ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: Some(
                crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                    .expect("connect daemon provider"),
            ),
            wsl_distro: None,
        };
        let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
        let (log_file, _log_file) = setup_log_file();

        let err = launch_cli_session_impl(
            &db,
            &provider,
            &log_file,
            "p1".to_string(),
            LaunchMode::Fresh,
            Some(CliTool::Claude),
        )
        .expect_err("daemon launch should return error");

        assert!(err.contains("simulated launch failure"));
    }

    #[test]
    fn stop_cli_session_surfaces_daemon_error_message() {
        let daemon = start_stub_daemon(serde_json::json!({
            "result": null,
            "error": {
                "code": "STOP_ERROR",
                "message": "cannot stop session"
            }
        }));
        let provider = ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: Some(
                crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                    .expect("connect daemon provider"),
            ),
            wsl_distro: None,
        };

        let err = stop_cli_session_impl(&provider, "%10".to_string(), Some(CliTool::Codex))
            .expect_err("daemon stop should return error");
        assert!(err.contains("cannot stop session"));
    }

    #[test]
    fn record_session_activity_persists_lowercase_cli_tool_from_enum() {
        let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
        record_session_activity_impl(
            &db,
            "p1".to_string(),
            CliTool::Gemini,
            "2026-03-04T10:00:00Z".to_string(),
            "2026-03-04T11:00:00Z".to_string(),
            1_000,
            2_000,
        )
        .expect("record activity");

        let conn = db.0.lock().expect("db lock");
        let stored_tool: String = conn
            .query_row("SELECT cli_tool FROM session_activity LIMIT 1", [], |row| {
                row.get(0)
            })
            .expect("query cli_tool");
        assert_eq!(stored_tool, "gemini");
    }

    #[test]
    fn launch_codex_resume_returns_project_not_found_for_invalid_project_id() {
        let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
        let provider = ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: None,
            wsl_distro: None,
        };
        let (log_file, _log_file) = setup_log_file();

        let err = launch_cli_session_impl(
            &db,
            &provider,
            &log_file,
            "missing-project".to_string(),
            LaunchMode::Resume,
            Some(CliTool::Codex),
        )
        .expect_err("missing project should fail");

        assert_eq!(err, "Project not found: missing-project");
    }

    #[test]
    fn launch_codex_resume_surfaces_fallback_error_when_daemon_is_unreachable() {
        let daemon = start_unreachable_stub_daemon();
        let provider = ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: Some(
                crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                    .expect("connect daemon provider"),
            ),
            wsl_distro: None,
        };
        let (db, _db_file) = setup_db_with_project("p1", "/path/that/does/not/exist");
        let (log_file, _log_file) = setup_log_file();

        let err = launch_cli_session_impl(
            &db,
            &provider,
            &log_file,
            "p1".to_string(),
            LaunchMode::Resume,
            Some(CliTool::Codex),
        )
        .expect_err("daemon-unreachable fallback should still fail with useful error");

        assert!(
            err.contains("Failed to launch session: Project path does not exist"),
            "unexpected error: {err}"
        );
    }
}
