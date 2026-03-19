use std::collections::HashMap;
use std::time::Instant;

use super::cache::{
    finalize_display_scan, publish_compaction_runtime_sessions, scan_inputs_with_cache,
    ScanCompletionMetrics,
};
use super::classification::{
    classify_display_runtime_sessions_with, deduplicate_runtime_sessions,
    detect_runtime_idle_for_process, detect_runtime_idle_for_process_with_pane,
};
#[cfg(target_os = "windows")]
use super::daemon;
use super::{
    idle, process, tmux, ActivityAttribution, ActivityConfidence, CliTool, DisplaySession,
    RuntimeSession, SessionGroupKind,
};

#[cfg(test)]
use super::SessionState;

/// Scan for all running Claude Code sessions.
///
/// Orchestrates process scanning, tmux mapping, idle detection, and
/// proc IO activity tracking.
///
/// **Raw state** — Active if ANY of these is true (OR):
/// - **JSONL mtime**: main transcript modified < 5s ago (tool use, streaming)
/// - **Subagent mtime**: subagent file modified < 5s ago (compaction)
/// - **Proc IO** (Claude): rchar delta > 500 bytes for 2+ consecutive polls
/// - **TCP sockets** (Gemini only): ESTABLISHED connection to remote port 443
/// - **Codex (single session/project)**: session file mtime OR proc IO hysteresis
/// - **Codex (multi session/project)**: proc IO hysteresis only (to avoid
///   broadcasting one session's file activity to all Codex sessions)
///
/// **Reported state** — applies bidirectional hysteresis on top: a state
/// change only takes effect after 2 consecutive polls agree on the new state.
pub fn scan_sessions_for_display() -> Vec<DisplaySession> {
    scan_sessions_for_authoritative_snapshot().0
}

/// Scan once and return both the UI-safe display view and the full runtime view.
///
/// This is the authoritative combined scan used by daemon-side snapshot
/// production so consumers do not repeat process/tmux classification work.
pub fn scan_sessions_for_authoritative_snapshot() -> (Vec<DisplaySession>, Vec<RuntimeSession>) {
    #[cfg(target_os = "windows")]
    if let Some(display_sessions) = daemon::scan_display_sessions_via_daemon() {
        let runtime_sessions = daemon::scan_runtime_sessions_via_daemon().unwrap_or_default();
        let display_sessions = finalize_display_scan(
            display_sessions,
            Some(&runtime_sessions),
            ScanCompletionMetrics::default(),
        );
        return (display_sessions, runtime_sessions);
    }

    let scan_started = Instant::now();
    let (processes, pane_map, process_cache_hit, tmux_cache_hit, process_scan_ms, tmux_ms) =
        scan_inputs_with_cache(
            scan_started,
            &process::scan_process_ids_cached,
            &process::scan_processes,
            &tmux::list_panes,
        );

    let mut sessions_per_project_tool: HashMap<(String, CliTool), usize> = HashMap::new();
    for proc in &processes {
        *sessions_per_project_tool
            .entry((proc.project_path.clone(), proc.cli_tool))
            .or_default() += 1;
    }
    idle::reconcile_codex_bindings(&processes, &pane_map);

    let classify_started = Instant::now();
    let (runtime_sessions, idle_ms, process_signal_ms, ownership_ms) =
        classify_display_runtime_sessions_with(
            processes,
            pane_map,
            &sessions_per_project_tool,
            &detect_runtime_idle_for_process,
        );

    let classify_ms = classify_started.elapsed().as_millis() as u64;
    let total_ms = scan_started.elapsed().as_millis() as u64;
    tracing::debug!(
        process_scan_ms,
        tmux_ms,
        process_cache_hit,
        tmux_cache_hit,
        classify_ms,
        idle_ms = idle_ms.as_millis() as u64,
        process_signal_ms = process_signal_ms.as_millis() as u64,
        ownership_ms = ownership_ms.as_millis() as u64,
        total_ms,
        sessions = runtime_sessions.len(),
        "session_scanner metrics"
    );

    let display_sessions: Vec<DisplaySession> = runtime_sessions
        .iter()
        .cloned()
        .map(DisplaySession::from)
        .collect();
    let display_sessions = finalize_display_scan(
        display_sessions,
        Some(&runtime_sessions),
        ScanCompletionMetrics {
            process_scan_ms,
            tmux_ms,
            process_cache_hit,
            tmux_cache_hit,
            classify_ms,
            idle_ms: idle_ms.as_millis() as u64,
            process_signal_ms: process_signal_ms.as_millis() as u64,
            ownership_ms: ownership_ms.as_millis() as u64,
            total_ms,
        },
    );

    (display_sessions, runtime_sessions)
}

/// Scan for runtime reconciliation/session-id detection without hiding session metadata.
///
/// Coordination uses this path when it needs exact `(pane, tool) -> session_id`
/// correlation. Unlike the UI-facing `scan_sessions_for_display()`, this keeps session ids
/// even when activity attribution is ambiguous in multi-session projects.
pub fn scan_sessions_for_runtime() -> Vec<RuntimeSession> {
    #[cfg(target_os = "windows")]
    if let Some(sessions) = daemon::scan_runtime_sessions_via_daemon() {
        return sessions;
    }

    let scan_started = Instant::now();
    let (processes, pane_map, ..) = scan_inputs_with_cache(
        scan_started,
        &process::scan_process_ids_cached,
        &process::scan_processes,
        &tmux::list_panes,
    );
    idle::reconcile_codex_bindings(&processes, &pane_map);

    let mut sessions: Vec<RuntimeSession> = processes
        .into_iter()
        .map(|proc| {
            let tmux_pane = pane_map.get(&proc.tty);
            build_runtime_session(proc, tmux_pane, false)
        })
        .collect();

    deduplicate_runtime_sessions(&mut sessions);
    publish_compaction_runtime_sessions(&sessions);
    sessions
}

/// Testable version of scan_sessions that accepts injectable functions.
pub fn scan_sessions_with<F, G, H>(
    process_scanner: &F,
    tmux_lister: &G,
    idle_detector: &H,
) -> Vec<DisplaySession>
where
    F: Fn() -> Vec<process::ProcessInfo>,
    G: Fn() -> HashMap<String, tmux::TmuxPane>,
    H: Fn(&str) -> idle::IdleResult,
{
    let processes = process_scanner();
    let pane_map = tmux_lister();
    idle::reconcile_codex_bindings(&processes, &pane_map);

    let mut sessions: Vec<RuntimeSession> = processes
        .into_iter()
        .map(|proc| {
            let tmux_pane = pane_map.get(&proc.tty);
            let idle_result = idle_detector(&proc.project_path);
            build_runtime_session_with_idle(proc, tmux_pane, idle_result, false)
        })
        .collect();

    deduplicate_runtime_sessions(&mut sessions);
    sessions.into_iter().map(DisplaySession::from).collect()
}

#[cfg(test)]
pub(crate) fn scan_sessions_for_runtime_with<F, G>(
    process_scanner: &F,
    tmux_lister: &G,
) -> Vec<RuntimeSession>
where
    F: Fn() -> Vec<process::ProcessInfo>,
    G: Fn() -> HashMap<String, tmux::TmuxPane>,
{
    let processes = process_scanner();
    let pane_map = tmux_lister();
    idle::reconcile_codex_bindings(&processes, &pane_map);

    let mut sessions: Vec<RuntimeSession> = processes
        .into_iter()
        .map(|proc| {
            let tmux_pane = pane_map.get(&proc.tty);
            build_runtime_session(proc, tmux_pane, false)
        })
        .collect();

    deduplicate_runtime_sessions(&mut sessions);
    sessions
}

fn build_runtime_session(
    proc: process::ProcessInfo,
    tmux_pane: Option<&tmux::TmuxPane>,
    recent_io: bool,
) -> RuntimeSession {
    let idle_result = detect_runtime_idle_for_process_with_pane(
        &proc,
        tmux_pane.map(|pane| pane.pane_id.as_str()),
    );
    build_runtime_session_with_idle(proc, tmux_pane, idle_result, recent_io)
}

fn build_runtime_session_with_idle(
    proc: process::ProcessInfo,
    tmux_pane: Option<&tmux::TmuxPane>,
    idle_result: idle::IdleResult,
    recent_io: bool,
) -> RuntimeSession {
    RuntimeSession {
        pid: proc.pid,
        project_path: proc.project_path,
        tty: proc.tty,
        args: proc.args,
        cli_tool: proc.cli_tool,
        tmux_session: tmux_pane.map(|pane| pane.session_name.clone()),
        tmux_window: tmux_pane.map(|pane| pane.window_index.clone()),
        tmux_pane: tmux_pane.map(|pane| pane.pane_id.clone()),
        tmux_window_name: tmux_pane.map(|pane| pane.window_name.clone()),
        state: idle_result.state,
        session_id: idle_result.session_id,
        jsonl_path: idle_result.jsonl_path,
        recent_io,
        last_output_age_secs: idle_result.last_output_age_secs,
        activity_confidence: ActivityConfidence::Low,
        activity_attribution: ActivityAttribution::None,
        project_unattributed_active: false,
        group_kind: SessionGroupKind::Standalone,
        group_id: None,
        group_label: None,
        member_name: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::classification::set_runtime_idle_detector_override;

    #[test]
    fn scan_sessions_combines_all_sources() {
        let mock_processes = || {
            vec![
                process::ProcessInfo {
                    pid: 100,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/1".to_string(),
                    args: "claude --continue".to_string(),
                    cli_tool: CliTool::Claude,
                },
                process::ProcessInfo {
                    pid: 200,
                    project_path: "/home/user/proj-b".to_string(),
                    tty: "/dev/pts/5".to_string(),
                    args: "claude".to_string(),
                    cli_tool: CliTool::Claude,
                },
            ]
        };

        let mock_tmux = || {
            HashMap::from([(
                "/dev/pts/1".to_string(),
                tmux::TmuxPane {
                    pane_id: "%0".to_string(),
                    tty: "/dev/pts/1".to_string(),
                    window_index: "0".to_string(),
                    window_name: "proj-a".to_string(),
                    session_name: "main".to_string(),
                },
            )])
        };

        let mock_idle = |path: &str| -> idle::IdleResult {
            if path.contains("proj-a") {
                idle::IdleResult {
                    state: SessionState::Active,
                    session_id: Some("sess-aaa".to_string()),
                    jsonl_path: Some(
                        "/home/user/.claude/projects/proj-a/sess-aaa.jsonl".to_string(),
                    ),
                    last_output_age_secs: None,
                }
            } else {
                idle::IdleResult {
                    state: SessionState::Idle,
                    session_id: None,
                    jsonl_path: None,
                    last_output_age_secs: None,
                }
            }
        };

        let sessions = scan_sessions_with(&mock_processes, &mock_tmux, &mock_idle);
        assert_eq!(sessions.len(), 2);

        let first = sessions.iter().find(|session| session.pid == 100).unwrap();
        assert_eq!(first.project_path, "/home/user/proj-a");
        assert_eq!(first.cli_tool, CliTool::Claude);
        assert_eq!(first.tmux_session.as_deref(), Some("main"));
        assert_eq!(first.tmux_pane.as_deref(), Some("%0"));
        assert_eq!(first.state, SessionState::Active);

        let second = sessions.iter().find(|session| session.pid == 200).unwrap();
        assert_eq!(second.project_path, "/home/user/proj-b");
        assert_eq!(second.cli_tool, CliTool::Claude);
        assert!(second.tmux_session.is_none());
        assert!(second.tmux_pane.is_none());
        assert_eq!(second.state, SessionState::Idle);

        let display_json = serde_json::to_value(first).unwrap();
        assert!(display_json.get("session_id").is_none());
        let display_json = serde_json::to_value(second).unwrap();
        assert!(display_json.get("session_id").is_none());
    }

    #[test]
    fn scan_sessions_empty_when_no_processes() {
        let sessions = scan_sessions_with(&|| vec![], &|| HashMap::new(), &|_| idle::IdleResult {
            state: SessionState::Active,
            session_id: None,
            jsonl_path: None,
            last_output_age_secs: None,
        });
        assert!(sessions.is_empty());
    }

    #[test]
    fn scan_sessions_for_runtime_uses_distinct_codex_metadata_per_pid_in_multi_session_project() {
        let mock_processes = || {
            vec![
                process::ProcessInfo {
                    pid: 100,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/1".to_string(),
                    args: "codex".to_string(),
                    cli_tool: CliTool::Codex,
                },
                process::ProcessInfo {
                    pid: 200,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/2".to_string(),
                    args: "codex resume --last".to_string(),
                    cli_tool: CliTool::Codex,
                },
            ]
        };

        let mock_tmux = || {
            HashMap::from([
                (
                    "/dev/pts/1".to_string(),
                    tmux::TmuxPane {
                        pane_id: "%1".to_string(),
                        tty: "/dev/pts/1".to_string(),
                        window_index: "1".to_string(),
                        window_name: "proj-a".to_string(),
                        session_name: "main".to_string(),
                    },
                ),
                (
                    "/dev/pts/2".to_string(),
                    tmux::TmuxPane {
                        pane_id: "%2".to_string(),
                        tty: "/dev/pts/2".to_string(),
                        window_index: "2".to_string(),
                        window_name: "proj-a".to_string(),
                        session_name: "main".to_string(),
                    },
                ),
            ])
        };

        fn runtime_idle_by_pid(proc: &process::ProcessInfo) -> idle::IdleResult {
            assert_eq!(proc.project_path, "/home/user/proj-a");
            assert_eq!(proc.cli_tool, CliTool::Codex);
            if proc.pid == 100 {
                idle::IdleResult {
                    state: SessionState::Idle,
                    session_id: Some("rollout-123".to_string()),
                    jsonl_path: Some("/tmp/rollout-123.jsonl".to_string()),
                    last_output_age_secs: Some(42),
                }
            } else {
                idle::IdleResult {
                    state: SessionState::Idle,
                    session_id: Some("rollout-456".to_string()),
                    jsonl_path: Some("/tmp/rollout-456.jsonl".to_string()),
                    last_output_age_secs: Some(41),
                }
            }
        }

        set_runtime_idle_detector_override(Some(runtime_idle_by_pid));
        let sessions = scan_sessions_for_runtime_with(&mock_processes, &mock_tmux);
        set_runtime_idle_detector_override(None);

        assert_eq!(sessions.len(), 2);
        let first = sessions.iter().find(|session| session.pid == 100).unwrap();
        let second = sessions.iter().find(|session| session.pid == 200).unwrap();
        assert_eq!(first.session_id.as_deref(), Some("rollout-123"));
        assert_eq!(second.session_id.as_deref(), Some("rollout-456"));
        assert_eq!(first.jsonl_path.as_deref(), Some("/tmp/rollout-123.jsonl"));
        assert_eq!(second.jsonl_path.as_deref(), Some("/tmp/rollout-456.jsonl"));
    }

    #[test]
    fn scan_sessions_deduplicates_same_tty_same_tool() {
        let mock_processes = || {
            vec![
                process::ProcessInfo {
                    pid: 500,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/3".to_string(),
                    args: "node /path/to/bin/codex --yolo".to_string(),
                    cli_tool: CliTool::Codex,
                },
                process::ProcessInfo {
                    pid: 501,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/3".to_string(),
                    args: "/path/to/codex/codex --yolo".to_string(),
                    cli_tool: CliTool::Codex,
                },
            ]
        };

        let mock_tmux = || {
            HashMap::from([(
                "/dev/pts/3".to_string(),
                tmux::TmuxPane {
                    pane_id: "%5".to_string(),
                    tty: "/dev/pts/3".to_string(),
                    window_index: "2".to_string(),
                    window_name: "proj-a".to_string(),
                    session_name: "0".to_string(),
                },
            )])
        };

        let mock_idle = |_: &str| idle::IdleResult {
            state: SessionState::Idle,
            session_id: None,
            jsonl_path: None,
            last_output_age_secs: None,
        };

        let sessions = scan_sessions_with(&mock_processes, &mock_tmux, &mock_idle);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].cli_tool, CliTool::Codex);
        assert_eq!(sessions[0].tmux_pane.as_deref(), Some("%5"));
        assert_eq!(sessions[0].pid, 501);
    }

    #[test]
    fn scan_sessions_keeps_different_tools_on_same_tty() {
        let mock_processes = || {
            vec![
                process::ProcessInfo {
                    pid: 600,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/4".to_string(),
                    args: "claude --continue".to_string(),
                    cli_tool: CliTool::Claude,
                },
                process::ProcessInfo {
                    pid: 700,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/5".to_string(),
                    args: "codex --yolo".to_string(),
                    cli_tool: CliTool::Codex,
                },
            ]
        };

        let mock_tmux = || HashMap::new();
        let mock_idle = |_: &str| idle::IdleResult {
            state: SessionState::Active,
            session_id: None,
            jsonl_path: None,
            last_output_age_secs: None,
        };

        let sessions = scan_sessions_with(&mock_processes, &mock_tmux, &mock_idle);
        assert_eq!(sessions.len(), 2);
    }
}
