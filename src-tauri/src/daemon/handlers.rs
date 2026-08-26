use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::daemon::protocol::{self, DaemonRequest, DaemonResponse};
use crate::daemon::watch::{handle_unwatch, handle_watch, WatchRuntime};
use crate::project_provider::ProjectProvider;
use crate::task_scanner::claude_index::{
    build_claude_source_index_with_live_sessions, ClaudeSourceIndex,
};

#[derive(Debug, Clone)]
struct ProjectTaskScanCache {
    cycle_id: u64,
    sessions: Vec<crate::session_scanner::RuntimeSession>,
    claude_index: ClaudeSourceIndex,
}

#[derive(Debug, Default)]
pub(crate) struct ProjectTaskScanCacheState {
    cache: Mutex<Option<ProjectTaskScanCache>>,
}

/// Dispatch a request to the appropriate handler.
pub(crate) fn dispatch(
    request: &DaemonRequest,
    provider: &dyn ProjectProvider,
    start_time: Instant,
    writer: &Arc<Mutex<TcpStream>>,
    watch_runtime: &mut WatchRuntime,
    project_task_scan_cache: &ProjectTaskScanCacheState,
) -> DaemonResponse {
    tracing::debug!(method = %request.method, id = %request.id, "Received request");
    match request.method.as_str() {
        protocol::method::PING => handle_ping(&request.id, start_time),
        protocol::method::GIT_STATUS => handle_git_status(&request.id, &request.params, provider),
        protocol::method::GIT_LOG => handle_git_log(&request.id, &request.params, provider),
        protocol::method::GIT_LATEST_COMMIT_TIME => {
            handle_git_latest_commit_time(&request.id, &request.params, provider)
        }
        protocol::method::FILE_TREE => handle_file_tree(&request.id, &request.params, provider),
        protocol::method::READ_FILE => handle_read_file(&request.id, &request.params, provider),
        protocol::method::READ_README => handle_read_readme(&request.id, &request.params, provider),
        protocol::method::READ_ASSET => handle_read_asset(&request.id, &request.params, provider),
        protocol::method::SCAN_SESSIONS => {
            handle_scan_sessions(&request.id, &request.params, provider)
        }
        protocol::method::LIST_DISPLAY_SESSIONS => handle_list_display_sessions(&request.id),
        protocol::method::GET_RUNTIME_SESSION_SNAPSHOT => {
            handle_get_runtime_session_snapshot(&request.id)
        }
        protocol::method::LIST_RUNTIME_SESSIONS => handle_list_runtime_sessions(&request.id),
        protocol::method::WAIT_SESSION_UPDATES => {
            handle_wait_session_updates(&request.id, &request.params)
        }
        protocol::method::LAUNCH_SESSION => handle_launch_session(&request.id, &request.params),
        protocol::method::STOP_SESSION => handle_stop_session(&request.id, &request.params),
        protocol::method::NAVIGATE_TO_SESSION => {
            handle_navigate_to_session(&request.id, &request.params)
        }
        protocol::method::GET_PROJECT_TASKS => {
            handle_get_project_tasks(&request.id, &request.params, project_task_scan_cache)
        }
        protocol::method::GIT_COMMITS_IN_RANGE => {
            handle_git_commits_in_range(&request.id, &request.params, provider)
        }
        protocol::method::GIT_COMMIT_FILES => {
            handle_git_commit_files(&request.id, &request.params, provider)
        }
        protocol::method::GIT_COMMIT_DIFF => {
            handle_git_commit_diff(&request.id, &request.params, provider)
        }
        protocol::method::WATCH => {
            handle_watch(&request.id, &request.params, writer, watch_runtime)
        }
        protocol::method::UNWATCH => handle_unwatch(&request.id, &request.params, watch_runtime),
        protocol::method::SHUTDOWN => {
            DaemonResponse::ok(&request.id, serde_json::json!({"ok": true}))
        }
        protocol::method::SET_CODEX_COMPACTION_MODE => {
            handle_set_codex_compaction_mode(&request.id, &request.params)
        }
        protocol::method::LIST_CLAUDE_ACCOUNTS => handle_list_claude_accounts(&request.id),
        protocol::method::CLAUDE_PROJECT_TRANSCRIPT => {
            handle_claude_project_transcript(&request.id, &request.params)
        }
        _ => DaemonResponse::err(
            &request.id,
            "UNKNOWN_METHOD",
            format!("Unknown method: {}", request.method),
        ),
    }
}

/// Claude subscriptions on the daemon's host — the Windows app cannot read the
/// WSL home itself.
fn handle_list_claude_accounts(id: &str) -> DaemonResponse {
    DaemonResponse::ok(
        id,
        protocol::ClaudeAccountsResult {
            accounts: crate::session_scanner::claude_accounts::detect_claude_accounts_cached(),
        },
    )
}

/// The newest transcript a project has under any detected config dir — the
/// account `--resume` has to run in. The files are the daemon's to read: on
/// Windows they live in WSL, and the app never scans them.
fn handle_claude_project_transcript(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::ClaudeProjectTranscriptParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => {
                return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string());
            }
        };

    // The scan's config dirs, not its accounts: a `.claude.json` caught
    // mid-rewrite names no account, and the transcripts beside it are still
    // the only record of which subscription owns the project's history.
    let config_dirs = crate::session_scanner::claude_accounts::transcript_config_dirs();
    DaemonResponse::ok(
        id,
        protocol::ClaudeProjectTranscriptResult {
            transcript: crate::session_scanner::claude_accounts::newest_project_transcript(
                &config_dirs,
                &params.project_path,
            )
            .map(|path| path.display().to_string()),
        },
    )
}

fn handle_set_codex_compaction_mode(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::SetCodexCompactionModeParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match crate::daemon::compaction::request_mode_and_wait(params.mode) {
        Ok(()) => DaemonResponse::ok(id, serde_json::json!({"ok": true})),
        Err(error) => DaemonResponse::err(id, "COMPACTION_MODE_APPLY_FAILED", error),
    }
}

pub(crate) fn handle_ping(id: &str, start_time: Instant) -> DaemonResponse {
    let data_root = crate::daemon_api::data_identity_paths();
    DaemonResponse::ok(
        id,
        protocol::PingResult {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: protocol::PROTOCOL_VERSION,
            uptime_secs: start_time.elapsed().as_secs(),
            data_root: data_root.display().to_string(),
        },
    )
}

pub(crate) fn handle_git_status(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.git_status(&params.path) {
        Ok(status) => DaemonResponse::ok(id, status),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_git_log(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::GitLogParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.all_commits(&params.path, params.limit, params.offset) {
        Ok(commits) => DaemonResponse::ok(id, commits),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_git_latest_commit_time(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.latest_commit_time(&params.path) {
        Ok(time) => DaemonResponse::ok(
            id,
            protocol::LatestCommitTimeResult {
                timestamp: time.map(|t| t.to_rfc3339()),
            },
        ),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_git_commits_in_range(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::GitCommitsInRangeParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.commits_in_range(
        &params.path,
        &params.after,
        &params.before,
        params.commit_limit,
    ) {
        Ok(result) => DaemonResponse::ok(
            id,
            protocol::GitCommitsInRangeResult {
                commits: result.commits,
                files: result.files,
                truncated: result.truncated,
                total_count: result.total_count,
            },
        ),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_git_commit_files(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::GitCommitFilesParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.commit_files(&params.path, &params.hash) {
        Ok(files) => DaemonResponse::ok(id, protocol::GitCommitFilesResult { files }),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_git_commit_diff(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::GitCommitDiffParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.commit_diff(&params.path, &params.hash, &params.file_path) {
        Ok(hunks) => DaemonResponse::ok(id, protocol::GitCommitDiffResult { hunks }),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_file_tree(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.file_tree(&params.path) {
        Ok(tree) => DaemonResponse::ok(id, tree),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_read_file(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::ReadFileParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.read_file(&params.path, &params.relative) {
        Ok(content) => DaemonResponse::ok(id, content),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_read_readme(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.read_readme(&params.path) {
        Ok(content) => DaemonResponse::ok(id, content),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_read_asset(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::ReadFileParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.read_asset(&params.path, &params.relative) {
        Ok(bytes) => {
            let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
            DaemonResponse::ok(id, protocol::ReadAssetResult { data })
        }
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_scan_sessions(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.scan_session_files(&params.path) {
        Ok(paths) => DaemonResponse::ok(
            id,
            protocol::ScanSessionsResult {
                paths: paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
            },
        ),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_list_display_sessions(id: &str) -> DaemonResponse {
    let sessions = crate::daemon::session_activity::SessionActivityHub::global()
        .snapshot()
        .sessions;
    DaemonResponse::ok(id, sessions)
}

pub(crate) fn handle_get_runtime_session_snapshot(id: &str) -> DaemonResponse {
    let snapshot = crate::daemon::session_activity::SessionActivityHub::global().runtime_snapshot();
    DaemonResponse::ok(
        id,
        protocol::RuntimeSessionSnapshotResult {
            version: snapshot.version,
            display_sessions: snapshot.display_sessions,
            runtime_sessions: snapshot.runtime_sessions,
            focus: snapshot.focus,
            foreground_project_path: snapshot.focus_project_path,
            degraded: snapshot.degraded,
            degraded_revision: snapshot.degraded_revision,
        },
    )
}

pub(crate) fn handle_list_runtime_sessions(id: &str) -> DaemonResponse {
    let sessions = crate::daemon::session_activity::SessionActivityHub::global()
        .runtime_snapshot()
        .runtime_sessions;
    DaemonResponse::ok(id, sessions)
}

pub(crate) fn handle_wait_session_updates(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::WaitSessionUpdatesParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };

    let update = crate::daemon::session_activity::SessionActivityHub::global().wait_for_update(
        params.since_version,
        params.since_degraded_revision,
        std::time::Duration::from_millis(params.timeout_ms),
    );

    DaemonResponse::ok(
        id,
        protocol::WaitSessionUpdatesResult {
            version: update.snapshot.version,
            changed: update.changed,
            sessions: update.snapshot.sessions,
            focus: update.focus,
            focus_project_path: update.focus_project_path,
            degraded: update.degraded,
            degraded_revision: update.degraded_revision,
        },
    )
}

pub(crate) fn handle_launch_session(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::LaunchSessionParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match crate::session_scanner::control::launch_in_tmux_with_layout(
        &params.project_path,
        params.mode,
        params.cli_tool,
        &params.tmux_layout,
        params.command_override.as_deref(),
    ) {
        Ok((session, window, pane)) => DaemonResponse::ok(
            id,
            protocol::LaunchSessionResult {
                tmux_session: Some(session),
                tmux_window: window,
                tmux_pane: pane,
            },
        ),
        Err(e) => DaemonResponse::err(id, "LAUNCH_ERROR", e),
    }
}

pub(crate) fn handle_stop_session(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::StopSessionParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match crate::session_scanner::control::stop_session(&params.tmux_pane, params.cli_tool) {
        Ok(()) => DaemonResponse::ok(id, serde_json::json!({"ok": true})),
        Err(e) => DaemonResponse::err(id, "STOP_ERROR", e),
    }
}

pub(crate) fn handle_navigate_to_session(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::NavigateToSessionParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match crate::session_scanner::control::navigate_to_pane(
        &params.tmux_session,
        &params.tmux_window,
        &params.tmux_pane,
    ) {
        Ok(()) => DaemonResponse::ok(id, serde_json::json!({"ok": true})),
        Err(e) => DaemonResponse::err(id, "NAVIGATE_ERROR", e),
    }
}

pub(crate) fn handle_get_project_tasks(
    id: &str,
    params: &serde_json::Value,
    project_task_scan_cache: &ProjectTaskScanCacheState,
) -> DaemonResponse {
    let params: protocol::ProjectTasksParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(_) => match serde_json::from_value::<protocol::PathParams>(params.clone()) {
            Ok(p) => protocol::ProjectTasksParams {
                path: p.path,
                scan_cycle_id: None,
            },
            Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
        },
    };

    let (all_sessions, claude_index) =
        load_project_task_scan_inputs(params.scan_cycle_id, project_task_scan_cache);
    let project_sessions: Vec<crate::session_scanner::RuntimeSession> = all_sessions
        .into_iter()
        .filter(|s| s.project_path == params.path)
        .collect();

    let result = crate::task_scanner::get_tasks_for_project_with_index(
        &params.path,
        &project_sessions,
        Some(&claude_index),
    );
    DaemonResponse::ok(id, result)
}

fn load_project_task_scan_inputs(
    cycle_id: Option<u64>,
    project_task_scan_cache: &ProjectTaskScanCacheState,
) -> (
    Vec<crate::session_scanner::RuntimeSession>,
    ClaudeSourceIndex,
) {
    if let Some(cycle_id) = cycle_id {
        let mut guard = project_task_scan_cache
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(ref cached) = *guard {
            if cached.cycle_id == cycle_id {
                return (cached.sessions.clone(), cached.claude_index.clone());
            }
        }

        // Continuity read: task-source lookup only (see bootstrap.rs); a
        // degraded scan keeps the last good snapshot, nothing is bound to it.
        let (sessions, _degraded) = crate::session_scanner::scan_sessions_for_runtime();
        let claude_index = build_claude_source_index_with_live_sessions(&sessions);
        *guard = Some(ProjectTaskScanCache {
            cycle_id,
            sessions: sessions.clone(),
            claude_index: claude_index.clone(),
        });
        return (sessions, claude_index);
    }

    // Continuity read: same task-source lookup as above, uncached.
    let (sessions, _degraded) = crate::session_scanner::scan_sessions_for_runtime();
    let claude_index = build_claude_source_index_with_live_sessions(&sessions);
    (sessions, claude_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::claude_accounts::{install_scan_override, ClaudeScan};
    use tempfile::TempDir;

    // Regression: 760f776 answered `claude-project-transcript` from the config
    // dirs of successfully parsed accounts only. On Windows this handler is the
    // *only* thing that can see the transcripts, and a `.claude.json` caught
    // mid-rewrite names no account — so the daemon reported no history and the
    // app resumed the project in whichever subscription its own choice named.
    #[test]
    fn the_transcript_handler_reads_config_dirs_that_name_no_account() {
        let home = TempDir::new().expect("home");
        let config_dir = home.path().join(".claude-account2");
        let project_path = "/home/user/projects/daemon-side";
        let dir = config_dir
            .join("projects")
            .join(crate::session_scanner::idle::path_to_slug(project_path));
        std::fs::create_dir_all(&dir).expect("transcript dir");
        let transcript = dir.join("abc.jsonl");
        std::fs::write(&transcript, "{}\n").expect("transcript");
        let _scan = install_scan_override(ClaudeScan {
            config_dirs: vec![config_dir],
            accounts: Vec::new(),
        });

        let response = handle_claude_project_transcript(
            "req-1",
            &serde_json::json!({ "project_path": project_path }),
        );

        assert!(response.is_ok(), "{response:?}");
        let result: protocol::ClaudeProjectTranscriptResult =
            serde_json::from_value(response.result.expect("result")).expect("decode");
        assert_eq!(
            result.transcript.as_deref(),
            Some(transcript.display().to_string().as_str())
        );
    }
}
