use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use super::cache::{
    finalize_display_scan, publish_compaction_runtime_sessions, scan_inputs_with_cache,
    ScanCompletionMetrics, ScanInputs,
};
use super::classification::{
    classify_display_runtime_sessions_with, deduplicate_runtime_sessions,
    detect_runtime_idle_for_process, detect_runtime_idle_for_process_with_pane,
};
use super::daemon;
use super::{
    idle, process, tmux, ActivityAttribution, ActivityConfidence, CliTool, DisplaySession,
    RuntimeSession, SessionGroupKind,
};

#[cfg(test)]
use super::SessionState;

/// Last good display and runtime views, returned verbatim on degraded scans
/// so consumers keep data without re-running classification.
///
/// One coherent snapshot for both entry points: an authoritative scan refreshes
/// both views, a runtime scan only the runtime view, so the first degraded call
/// on either path during an outage finds whatever the other path last saw.
struct LastGoodSnapshot {
    display: Vec<DisplaySession>,
    runtime: Vec<RuntimeSession>,
}

static LAST_GOOD_SNAPSHOT: Mutex<LastGoodSnapshot> = Mutex::new(LastGoodSnapshot {
    display: Vec::new(),
    runtime: Vec::new(),
});

/// Scan for all running Claude Code sessions.
///
/// Orchestrates process scanning, tmux mapping, idle detection, and
/// proc IO activity tracking.
///
/// **Raw state** — Active if ANY of these is true (OR):
/// - **JSONL mtime**: main transcript modified < 5s ago (tool use, streaming)
/// - **Subagent mtime**: subagent file modified < 5s ago (compaction)
/// - **Proc IO** (Claude): rchar delta > 500 bytes for 2+ consecutive polls
/// - **Codex (single session/project)**: session file mtime OR proc IO hysteresis
/// - **Codex (multi session/project)**: proc IO hysteresis only (to avoid
///   broadcasting one session's file activity to all Codex sessions)
///
/// **Reported state** — applies bidirectional hysteresis on top: a state
/// change only takes effect after 2 consecutive polls agree on the new state.
///
/// The flag is `true` when the process inventory could not be read: the
/// sessions are then the last fully classified snapshot, not an observation.
pub fn scan_sessions_for_display() -> (Vec<DisplaySession>, bool) {
    let (display_sessions, _, degraded) = scan_sessions_for_authoritative_snapshot();
    (display_sessions, degraded)
}

/// Scan once and return the UI-safe display view, the full runtime view, and
/// whether the scan was degraded.
///
/// This is the authoritative combined scan used by daemon-side snapshot
/// production so consumers do not repeat process/tmux classification work.
///
/// A degraded scan (`true`) means the process inventory could not be read.
/// It is inert: no Codex binding reconciliation, idle detection, process-I/O
/// sampling, hysteresis or transition events run, and the sessions are the
/// last fully classified snapshot, which must not drive pruning, versioning,
/// or exports.
pub fn scan_sessions_for_authoritative_snapshot() -> (Vec<DisplaySession>, Vec<RuntimeSession>, bool)
{
    // Windows app: the scan is the WSL daemon hub's snapshot.
    if let Some(snapshot) = daemon::runtime_session_snapshot_via_daemon() {
        return authoritative_scan_from_daemon_snapshot(snapshot);
    }

    let scan_started = Instant::now();
    let ScanInputs {
        processes,
        pane_map,
        process_cache_hit,
        tmux_cache_hit,
        process_scan_ms,
        tmux_ms,
        degraded,
    } = scan_inputs_with_cache(
        scan_started,
        &process::scan_process_ids_cached,
        &process::scan_processes,
        &tmux::list_panes,
    );

    if degraded {
        let (display_sessions, runtime_sessions) = {
            let last_good = LAST_GOOD_SNAPSHOT
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            (last_good.display.clone(), last_good.runtime.clone())
        };
        let display_sessions = finalize_display_scan(
            display_sessions,
            None,
            ScanCompletionMetrics {
                process_scan_ms,
                tmux_ms,
                process_cache_hit,
                tmux_cache_hit,
                total_ms: scan_started.elapsed().as_millis() as u64,
                degraded: true,
                ..ScanCompletionMetrics::default()
            },
        );
        return (display_sessions, runtime_sessions, true);
    }

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
        degraded,
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
            degraded: false,
        },
    );
    remember_authoritative_snapshot(&display_sessions, &runtime_sessions);

    (display_sessions, runtime_sessions, false)
}

/// Fold the daemon hub's snapshot into the authoritative scan result.
///
/// A degraded snapshot is the hub's last good view kept for continuity, not
/// an observation: it comes back flagged, publishes nothing, prunes nothing
/// and is not remembered as last good.
fn authoritative_scan_from_daemon_snapshot(
    snapshot: crate::daemon::protocol::RuntimeSessionSnapshotResult,
) -> (Vec<DisplaySession>, Vec<RuntimeSession>, bool) {
    let degraded = snapshot.degraded;
    let runtime_sessions = snapshot.runtime_sessions;
    let display_sessions = finalize_display_scan(
        snapshot.display_sessions,
        (!degraded).then_some(runtime_sessions.as_slice()),
        ScanCompletionMetrics {
            degraded,
            ..ScanCompletionMetrics::default()
        },
    );
    if !degraded {
        remember_authoritative_snapshot(&display_sessions, &runtime_sessions);
    }
    (display_sessions, runtime_sessions, degraded)
}

fn remember_authoritative_snapshot(
    display_sessions: &[DisplaySession],
    runtime_sessions: &[RuntimeSession],
) {
    let mut last_good = LAST_GOOD_SNAPSHOT
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    last_good.display = display_sessions.to_vec();
    last_good.runtime = runtime_sessions.to_vec();
}

/// Scan for runtime reconciliation/session-id detection without hiding session metadata.
///
/// Coordination uses this path when it needs exact `(pane, tool) -> session_id`
/// correlation. Unlike the UI-facing `scan_sessions_for_display()`, this keeps session ids
/// even when activity attribution is ambiguous in multi-session projects.
///
/// The flag is `true` when the process inventory could not be read: the
/// sessions are then the last good runtime snapshot, not an observation.
pub fn scan_sessions_for_runtime() -> (Vec<RuntimeSession>, bool) {
    // Windows app: the scan is the WSL daemon hub's snapshot.
    if let Some(snapshot) = daemon::runtime_session_snapshot_via_daemon() {
        return runtime_scan_from_daemon_snapshot(snapshot);
    }

    let scan_started = Instant::now();
    let ScanInputs {
        processes,
        pane_map,
        degraded,
        ..
    } = scan_inputs_with_cache(
        scan_started,
        &process::scan_process_ids_cached,
        &process::scan_processes,
        &tmux::list_panes,
    );

    if degraded {
        // Inert: no binding reconciliation, no transcript lookups.
        let sessions = LAST_GOOD_SNAPSHOT
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .runtime
            .clone();
        return (sessions, true);
    }

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
    remember_runtime_snapshot(&sessions);
    (sessions, false)
}

/// Fold the daemon hub's snapshot into the runtime scan result.
///
/// A degraded snapshot is the hub's last good view kept for continuity, not
/// an observation: it comes back flagged (so identity detection keeps
/// polling instead of binding a cached pane->transcript mapping) and is not
/// remembered as last good.
fn runtime_scan_from_daemon_snapshot(
    snapshot: crate::daemon::protocol::RuntimeSessionSnapshotResult,
) -> (Vec<RuntimeSession>, bool) {
    let sessions = snapshot.runtime_sessions;
    if !snapshot.degraded {
        remember_runtime_snapshot(&sessions);
    }
    (sessions, snapshot.degraded)
}

fn remember_runtime_snapshot(sessions: &[RuntimeSession]) {
    LAST_GOOD_SNAPSHOT
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .runtime = sessions.to_vec();
}

#[cfg(test)]
pub(crate) fn clear_last_good_snapshot() {
    let mut last_good = LAST_GOOD_SNAPSHOT
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    last_good.display.clear();
    last_good.runtime.clear();
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
    use crate::session_scanner::idle::{
        set_binding_store_path_for_test, CODEX_RECONCILE_CALLS, CODEX_TEST_LOCK,
    };
    use crate::session_scanner::{
        clear_scan_cache, set_display_scan_compaction_hook, state_tracker_snapshot,
        StateChangeCapture, SCANNER_TEST_LOCK,
    };
    use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
    use std::sync::MutexGuard;
    use tempfile::TempDir;

    #[test]
    fn scan_sessions_combines_all_sources() {
        // `scan_sessions_with` reconciles the process-global Codex binding
        // store, which the Codex tests assert on.
        let _codex = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
                    authoritative: false,
                }
            } else {
                idle::IdleResult {
                    state: SessionState::Idle,
                    session_id: None,
                    jsonl_path: None,
                    last_output_age_secs: None,
                    authoritative: false,
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
        // `scan_sessions_with` reconciles the process-global Codex binding
        // store, which the Codex tests assert on.
        let _codex = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let sessions = scan_sessions_with(&|| vec![], &|| HashMap::new(), &|_| idle::IdleResult {
            state: SessionState::Active,
            session_id: None,
            jsonl_path: None,
            last_output_age_secs: None,
            authoritative: false,
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
                    authoritative: false,
                }
            } else {
                idle::IdleResult {
                    state: SessionState::Idle,
                    session_id: Some("rollout-456".to_string()),
                    jsonl_path: Some("/tmp/rollout-456.jsonl".to_string()),
                    last_output_age_secs: Some(41),
                    authoritative: false,
                }
            }
        }

        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // `scan_sessions_for_runtime_with` reconciles the process-global Codex
        // binding store, which the Codex tests assert on. Taken after the
        // scanner lock: `E2eScanner` acquires them in that order.
        let _codex = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
        // `scan_sessions_with` reconciles the process-global Codex binding
        // store, which the Codex tests assert on.
        let _codex = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
            authoritative: false,
        };

        let sessions = scan_sessions_with(&mock_processes, &mock_tmux, &mock_idle);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].cli_tool, CliTool::Codex);
        assert_eq!(sessions[0].tmux_pane.as_deref(), Some("%5"));
        assert_eq!(sessions[0].pid, 501);
    }

    #[test]
    fn scan_sessions_keeps_different_tools_on_same_tty() {
        // `scan_sessions_with` reconciles the process-global Codex binding
        // store, which the Codex tests assert on.
        let _codex = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
            authoritative: false,
        };

        let sessions = scan_sessions_with(&mock_processes, &mock_tmux, &mock_idle);
        assert_eq!(sessions.len(), 2);
    }

    // -----------------------------------------------------------------------
    // End-to-end fail-soft wiring: real scan entry points, injected inventory
    // -----------------------------------------------------------------------

    const INVENTORY_HEALTHY: u8 = 0;
    const INVENTORY_FAILS: u8 = 1;
    const INVENTORY_EMPTY: u8 = 2;
    /// The inventory the interactive filter produces from `live_inventory`.
    const INVENTORY_FILTERED: u8 = 3;
    static E2E_INVENTORY_MODE: AtomicU8 = AtomicU8::new(INVENTORY_HEALTHY);
    static E2E_IDLE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static E2E_COMPACTION_PUBLISHES: AtomicUsize = AtomicUsize::new(0);
    const E2E_CLAUDE_PID: u32 = 910_001;
    const E2E_CODEX_PID: u32 = 910_002;
    const E2E_PROJECT: &str = "/home/user/e2e-project";

    fn e2e_inventory() -> Option<Vec<process::ProcessInfo>> {
        match E2E_INVENTORY_MODE.load(Ordering::SeqCst) {
            INVENTORY_FAILS => None,
            INVENTORY_EMPTY => Some(Vec::new()),
            INVENTORY_FILTERED => Some(filtered_live_inventory()),
            _ => Some(vec![
                process::ProcessInfo {
                    pid: E2E_CLAUDE_PID,
                    project_path: E2E_PROJECT.to_string(),
                    tty: "/dev/pts/9101".to_string(),
                    args: "claude --continue".to_string(),
                    cli_tool: CliTool::Claude,
                },
                process::ProcessInfo {
                    pid: E2E_CODEX_PID,
                    project_path: E2E_PROJECT.to_string(),
                    tty: "/dev/pts/9102".to_string(),
                    args: "codex --yolo".to_string(),
                    cli_tool: CliTool::Codex,
                },
            ]),
        }
    }

    /// Counts calls; reports Active while healthy and Idle once the inventory
    /// fails, so any classification during a degraded scan would move the
    /// hysteresis trackers and be visible.
    fn e2e_idle(proc: &process::ProcessInfo) -> idle::IdleResult {
        E2E_IDLE_CALLS.fetch_add(1, Ordering::SeqCst);
        let state = if E2E_INVENTORY_MODE.load(Ordering::SeqCst) == INVENTORY_FAILS {
            SessionState::Idle
        } else {
            SessionState::Active
        };
        idle::IdleResult {
            state,
            session_id: Some(format!("sess-{}", proc.pid)),
            jsonl_path: Some(format!("/tmp/sess-{}.jsonl", proc.pid)),
            last_output_age_secs: Some(1),
            authoritative: false,
        }
    }

    fn e2e_compaction_publish(_sessions: &[RuntimeSession]) {
        E2E_COMPACTION_PUBLISHES.fetch_add(1, Ordering::SeqCst);
    }

    /// Holds the scanner/Codex locks, redirects the Codex binding store to a
    /// temp dir and installs the inventory/idle/compaction seams; restores
    /// everything on drop (also on panic).
    struct E2eScanner {
        _scanner: MutexGuard<'static, ()>,
        _codex: MutexGuard<'static, ()>,
        _tmp: TempDir,
    }

    impl E2eScanner {
        fn install() -> Self {
            let scanner = SCANNER_TEST_LOCK
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let codex = CODEX_TEST_LOCK
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let tmp = TempDir::new().expect("tempdir");
            set_binding_store_path_for_test(Some(tmp.path().join("codex-bindings.json")));
            clear_scan_cache();
            clear_last_good_snapshot();
            E2E_INVENTORY_MODE.store(INVENTORY_HEALTHY, Ordering::SeqCst);
            E2E_IDLE_CALLS.store(0, Ordering::SeqCst);
            E2E_COMPACTION_PUBLISHES.store(0, Ordering::SeqCst);
            process::set_inventory_provider_override(Some(e2e_inventory));
            set_runtime_idle_detector_override(Some(e2e_idle));
            set_display_scan_compaction_hook(Some(e2e_compaction_publish));
            Self {
                _scanner: scanner,
                _codex: codex,
                _tmp: tmp,
            }
        }
    }

    impl Drop for E2eScanner {
        fn drop(&mut self) {
            // A healthy empty scan prunes the trackers this test created.
            E2E_INVENTORY_MODE.store(INVENTORY_EMPTY, Ordering::SeqCst);
            let _ = scan_sessions_for_authoritative_snapshot();
            process::set_inventory_provider_override(None);
            set_runtime_idle_detector_override(None);
            set_display_scan_compaction_hook(None);
            set_binding_store_path_for_test(None);
            clear_scan_cache();
            clear_last_good_snapshot();
            E2E_INVENTORY_MODE.store(INVENTORY_HEALTHY, Ordering::SeqCst);
        }
    }

    /// `(reported, prev_raw)` hysteresis state of one PID, if tracked.
    type TrackerState = Option<(SessionState, SessionState)>;

    fn e2e_trackers() -> (TrackerState, TrackerState) {
        (
            state_tracker_snapshot(E2E_CLAUDE_PID),
            state_tracker_snapshot(E2E_CODEX_PID),
        )
    }

    // Regression: since 06b432d the authoritative scan captured `degraded`
    // but still ran `reconcile_codex_bindings` and classification (idle
    // detection, process-I/O sampling, hysteresis, activity.state.changed) on
    // the previous inventory before `finalize_display_scan` looked at the
    // flag, so repeated degraded polls advanced hysteresis and could drop
    // Codex bindings behind the hub's back. A degraded scan must return the
    // last fully classified snapshot and touch none of that state.
    #[test]
    fn degraded_authoritative_scan_is_inert_end_to_end() {
        let _harness = E2eScanner::install();

        let (display, runtime, degraded) = scan_sessions_for_authoritative_snapshot();
        assert!(!degraded);
        let mut pids: Vec<u32> = display.iter().map(|session| session.pid).collect();
        pids.sort_unstable();
        assert_eq!(pids, [E2E_CLAUDE_PID, E2E_CODEX_PID]);
        assert!(display.iter().all(|s| s.state == SessionState::Active));
        let idle_calls = E2E_IDLE_CALLS.load(Ordering::SeqCst);
        assert_eq!(idle_calls, 2);
        let reconcile_calls = CODEX_RECONCILE_CALLS.load(Ordering::SeqCst);
        assert!(reconcile_calls >= 1);
        assert_eq!(E2E_COMPACTION_PUBLISHES.load(Ordering::SeqCst), 1);
        let trackers = e2e_trackers();
        assert_eq!(
            trackers,
            (
                Some((SessionState::Active, SessionState::Active)),
                Some((SessionState::Active, SessionState::Active)),
            )
        );

        // The inventory source fails: the last classified snapshot comes back
        // flagged and nothing downstream runs.
        E2E_INVENTORY_MODE.store(INVENTORY_FAILS, Ordering::SeqCst);
        let (display_degraded, runtime_degraded, degraded) =
            scan_sessions_for_authoritative_snapshot();
        assert!(
            degraded,
            "failed inventory read must flag the scan degraded"
        );
        assert_eq!(display_degraded, display);
        assert_eq!(runtime_degraded, runtime);
        assert_eq!(
            E2E_IDLE_CALLS.load(Ordering::SeqCst),
            idle_calls,
            "degraded scan must not classify (idle detection, process-I/O, hysteresis)"
        );
        assert_eq!(
            CODEX_RECONCILE_CALLS.load(Ordering::SeqCst),
            reconcile_calls,
            "degraded scan must not reconcile Codex bindings"
        );
        assert_eq!(
            E2E_COMPACTION_PUBLISHES.load(Ordering::SeqCst),
            1,
            "degraded scan must not publish compaction sessions"
        );
        assert_eq!(
            e2e_trackers(),
            trackers,
            "degraded scan must not move hysteresis trackers"
        );

        // Recovery: classification resumes on the fresh inventory.
        E2E_INVENTORY_MODE.store(INVENTORY_HEALTHY, Ordering::SeqCst);
        let (recovered, _, degraded) = scan_sessions_for_authoritative_snapshot();
        assert!(!degraded);
        assert_eq!(recovered, display);
        assert_eq!(E2E_IDLE_CALLS.load(Ordering::SeqCst), idle_calls + 2);
        assert_eq!(E2E_COMPACTION_PUBLISHES.load(Ordering::SeqCst), 2);
    }

    // Regression: same gap on the runtime path — `scan_sessions_for_runtime`
    // reconciled Codex bindings and ran idle detection before checking the
    // flag and discarded the flag. It must return the last good runtime
    // snapshot, flagged, without touching bindings or transcripts.
    #[test]
    fn degraded_runtime_scan_is_inert_end_to_end() {
        let _harness = E2eScanner::install();

        let (sessions, degraded) = scan_sessions_for_runtime();
        assert!(!degraded);
        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().all(|session| session.session_id.is_some()));
        let idle_calls = E2E_IDLE_CALLS.load(Ordering::SeqCst);
        assert_eq!(idle_calls, 2);
        let reconcile_calls = CODEX_RECONCILE_CALLS.load(Ordering::SeqCst);

        E2E_INVENTORY_MODE.store(INVENTORY_FAILS, Ordering::SeqCst);
        let (sessions_degraded, degraded) = scan_sessions_for_runtime();
        assert!(degraded);
        assert_eq!(sessions_degraded, sessions);
        assert_eq!(E2E_IDLE_CALLS.load(Ordering::SeqCst), idle_calls);
        assert_eq!(
            CODEX_RECONCILE_CALLS.load(Ordering::SeqCst),
            reconcile_calls
        );

        E2E_INVENTORY_MODE.store(INVENTORY_HEALTHY, Ordering::SeqCst);
        let (recovered, degraded) = scan_sessions_for_runtime();
        assert!(!degraded);
        assert_eq!(recovered, sessions);
        assert_eq!(E2E_IDLE_CALLS.load(Ordering::SeqCst), idle_calls + 2);
    }

    // Regression: the authoritative and runtime last-good snapshots were
    // independent statics, so a healthy authoritative scan (the hub's usual
    // path) left the runtime fallback empty and the first degraded
    // `scan_sessions_for_runtime` call of an outage returned no sessions even
    // though a fully classified snapshot existed. There is one coherent
    // last-good snapshot: an authoritative scan seeds the runtime view too.
    #[test]
    fn authoritative_scan_seeds_runtime_fallback_for_degraded_runtime_scan() {
        let _harness = E2eScanner::install();

        let (_, runtime, degraded) = scan_sessions_for_authoritative_snapshot();
        assert!(!degraded);
        assert_eq!(runtime.len(), 2);
        assert!(runtime.iter().all(|session| session.session_id.is_some()));

        E2E_INVENTORY_MODE.store(INVENTORY_FAILS, Ordering::SeqCst);
        let (sessions, degraded) = scan_sessions_for_runtime();
        assert!(degraded);
        assert_eq!(
            sessions, runtime,
            "degraded runtime scan must return the last good runtime view from the authoritative scan"
        );
    }

    // --- non-interactive processes are not sessions ---

    /// A detached `codex exec` one-shot: no controlling terminal (`ps` TTY `?`).
    const DETACHED_PID: u32 = 920_001;
    /// A real Claude session in a tmux pane.
    const PANE_PID: u32 = 920_002;
    const PANE_PROJECT: &str = "/home/user/pane-project";
    const PANE_TTY: &str = "/dev/pts/9202";

    /// What the live 0.6.6 host's `/proc` inventory held: an automation-launched
    /// `codex exec` with no controlling terminal next to a real pane session.
    fn live_inventory() -> Vec<process::InventoryEntry> {
        vec![
            process::InventoryEntry::new(
                DETACHED_PID,
                "codex exec --json review",
                CliTool::Codex,
                false,
            ),
            process::InventoryEntry::new(PANE_PID, "claude --continue", CliTool::Claude, true),
        ]
    }

    /// Run the production interactive filter over `live_inventory`, then enrich
    /// the survivors the way `scan_processes` does (cwd + stdin tty).
    fn filtered_live_inventory() -> Vec<process::ProcessInfo> {
        let mut entries = live_inventory();
        process::retain_interactive_processes(&mut entries);
        entries
            .into_iter()
            .map(|entry| process::ProcessInfo {
                pid: entry.pid,
                project_path: PANE_PROJECT.to_string(),
                tty: PANE_TTY.to_string(),
                args: entry.args,
                cli_tool: entry.cli_tool,
            })
            .collect()
    }

    // Regression: the live 0.6.6 host ran `codex exec` one-shots launched
    // detached with stdin on /dev/null (`ps` TTY `?`). They entered the
    // inventory as tool processes, so the sidebar grew phantom session rows for
    // the project, every PID got a hysteresis tracker, and their bursty I/O
    // flipped idle<->active for ~64 `activity.state.changed` records a minute.
    // The first-sight-idle guard (PR #20, on top of 06b432d) silenced only the
    // first-sight half of that flapping. A process with no controlling terminal
    // leaves the inventory, so nothing downstream can see it.
    #[test]
    fn a_process_without_a_controlling_terminal_never_becomes_a_session() {
        let capture = StateChangeCapture::install();
        let _harness = E2eScanner::install();
        E2E_INVENTORY_MODE.store(INVENTORY_FILTERED, Ordering::SeqCst);

        let (display, runtime, degraded) = scan_sessions_for_authoritative_snapshot();

        assert!(!degraded);
        assert_eq!(
            display
                .iter()
                .map(|session| session.pid)
                .collect::<Vec<_>>(),
            vec![PANE_PID],
            "the detached one-shot must not produce a display session"
        );
        assert_eq!(
            runtime
                .iter()
                .map(|session| session.pid)
                .collect::<Vec<_>>(),
            vec![PANE_PID],
            "the detached one-shot must not produce a runtime session"
        );
        assert_eq!(
            state_tracker_snapshot(DETACHED_PID),
            None,
            "the detached one-shot must not get a hysteresis tracker"
        );
        assert!(
            capture.transitions_for(DETACHED_PID).is_empty(),
            "the detached one-shot must not emit activity.state.changed"
        );
    }

    // Regression: the guard is terminal-based, so the sessions people actually
    // run — a CLI on a pts, which is what a tmux pane and a plain terminal both
    // give it — must be completely unaffected: still classified, still tracked,
    // still reported.
    #[test]
    fn a_pts_backed_process_still_becomes_a_session() {
        let capture = StateChangeCapture::install();
        let _harness = E2eScanner::install();
        E2E_INVENTORY_MODE.store(INVENTORY_FILTERED, Ordering::SeqCst);

        let (display, _, degraded) = scan_sessions_for_authoritative_snapshot();

        assert!(!degraded);
        let session = display
            .iter()
            .find(|session| session.pid == PANE_PID)
            .expect("the pts-backed process must still be a session");
        assert_eq!(session.project_path, PANE_PROJECT);
        assert_eq!(session.cli_tool, CliTool::Claude);
        assert_eq!(session.state, SessionState::Active);
        assert_eq!(
            state_tracker_snapshot(PANE_PID),
            Some((SessionState::Active, SessionState::Active)),
            "the pts-backed process must still be tracked"
        );
        assert_eq!(
            capture.transitions_for(PANE_PID),
            vec![(None, SessionState::Active)],
            "the pts-backed process must still report its arrival"
        );
    }

    // --- Windows app path: the scan is the WSL daemon hub's snapshot ---

    const DAEMON_HEALTHY: u8 = 0;
    const DAEMON_DEGRADED: u8 = 1;
    static DAEMON_SNAPSHOT_MODE: AtomicU8 = AtomicU8::new(DAEMON_HEALTHY);
    const DAEMON_PID: u32 = 930_001;
    const DAEMON_PANE: &str = "%93";
    /// What the hub keeps across degraded cycles: the pane still mapped to the
    /// previous CLI's transcript.
    const STALE_SESSION: &str = "stale-session";
    /// What a healthy cycle observes in the same pane after the CLI restart.
    const FRESH_SESSION: &str = "fresh-session";

    fn daemon_session(session_id: &str) -> RuntimeSession {
        build_runtime_session_with_idle(
            process::ProcessInfo {
                pid: DAEMON_PID,
                project_path: "/home/user/daemon-project".to_string(),
                tty: "/dev/pts/9301".to_string(),
                args: "codex --yolo".to_string(),
                cli_tool: CliTool::Codex,
            },
            Some(&tmux::TmuxPane {
                pane_id: DAEMON_PANE.to_string(),
                tty: "/dev/pts/9301".to_string(),
                window_index: "3".to_string(),
                window_name: "daemon".to_string(),
                session_name: "taurhaus".to_string(),
            }),
            idle::IdleResult {
                state: SessionState::Active,
                session_id: Some(session_id.to_string()),
                jsonl_path: Some(format!("/home/user/.codex/sessions/{session_id}.jsonl")),
                last_output_age_secs: None,
                authoritative: false,
            },
            false,
        )
    }

    /// What the WSL daemon answers `get_runtime_session_snapshot` with: while
    /// its scanner is degraded, the hub's preserved (stale) sessions flagged
    /// degraded; once healthy, the fresh observation.
    fn scripted_daemon_snapshot() -> Option<crate::daemon::protocol::RuntimeSessionSnapshotResult> {
        let degraded = DAEMON_SNAPSHOT_MODE.load(Ordering::SeqCst) == DAEMON_DEGRADED;
        let session = daemon_session(if degraded {
            STALE_SESSION
        } else {
            FRESH_SESSION
        });
        Some(crate::daemon::protocol::RuntimeSessionSnapshotResult {
            version: 5,
            display_sessions: vec![DisplaySession::from(session.clone())],
            runtime_sessions: vec![session],
            account_observations: Vec::new(),
            focus: None,
            foreground_project_path: None,
            degraded,
            degraded_revision: 0,
        })
    }

    /// Installs the scripted daemon snapshot as the scan source and clears the
    /// app-local last-good snapshot so remembering can be observed.
    struct DaemonSnapshotHarness {
        _scanner: MutexGuard<'static, ()>,
    }

    impl DaemonSnapshotHarness {
        fn install(mode: u8) -> Self {
            let scanner = SCANNER_TEST_LOCK
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            clear_last_good_snapshot();
            DAEMON_SNAPSHOT_MODE.store(mode, Ordering::SeqCst);
            daemon::set_daemon_snapshot_override(Some(scripted_daemon_snapshot));
            Self { _scanner: scanner }
        }
    }

    impl Drop for DaemonSnapshotHarness {
        fn drop(&mut self) {
            daemon::set_daemon_snapshot_override(None);
            clear_last_good_snapshot();
            DAEMON_SNAPSHOT_MODE.store(DAEMON_HEALTHY, Ordering::SeqCst);
        }
    }

    fn last_good_runtime() -> Vec<RuntimeSession> {
        LAST_GOOD_SNAPSHOT
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .runtime
            .clone()
    }

    fn last_good_display() -> Vec<DisplaySession> {
        LAST_GOOD_SNAPSHOT
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .display
            .clone()
    }

    // Regression: on Windows (app + WSL daemon) the daemon hub kept its last
    // good runtime sessions across degraded scanner cycles, the protocol
    // carried no degradation status, and the Windows branch of
    // `scan_sessions_for_runtime` returned them as a healthy scan, so
    // `detect_runtime_session` bound the cached pane->transcript mapping of a
    // restarted CLI as a fresh observation. A degraded daemon snapshot must
    // come back flagged: continuity data, never an observation, and it must
    // not overwrite the app-local last-good snapshot.
    #[test]
    fn degraded_daemon_snapshot_flags_runtime_scan_degraded() {
        let _harness = DaemonSnapshotHarness::install(DAEMON_DEGRADED);

        let (sessions, degraded) = scan_sessions_for_runtime();
        assert!(
            degraded,
            "a degraded daemon snapshot must flag the runtime scan degraded"
        );
        assert_eq!(
            sessions,
            vec![daemon_session(STALE_SESSION)],
            "continuity: the daemon's cached sessions are still returned"
        );
        assert!(
            last_good_runtime().is_empty(),
            "a degraded daemon snapshot must not be remembered as last good"
        );

        // Recovery: the next healthy daemon snapshot is an observation again.
        DAEMON_SNAPSHOT_MODE.store(DAEMON_HEALTHY, Ordering::SeqCst);
        let (sessions, degraded) = scan_sessions_for_runtime();
        assert!(!degraded);
        assert_eq!(sessions, vec![daemon_session(FRESH_SESSION)]);
        assert_eq!(last_good_runtime(), sessions);
    }

    // Regression companion: the authoritative (display + runtime) Windows
    // branch had the same shape and discarded the daemon's status as well.
    #[test]
    fn degraded_daemon_snapshot_flags_authoritative_scan_degraded() {
        let _harness = DaemonSnapshotHarness::install(DAEMON_DEGRADED);

        let (display, runtime, degraded) = scan_sessions_for_authoritative_snapshot();
        assert!(
            degraded,
            "a degraded daemon snapshot must flag the authoritative scan degraded"
        );
        assert_eq!(runtime, vec![daemon_session(STALE_SESSION)]);
        assert_eq!(
            display,
            vec![DisplaySession::from(daemon_session(STALE_SESSION))]
        );
        assert!(last_good_display().is_empty());
        assert!(last_good_runtime().is_empty());

        DAEMON_SNAPSHOT_MODE.store(DAEMON_HEALTHY, Ordering::SeqCst);
        let (display, runtime, degraded) = scan_sessions_for_authoritative_snapshot();
        assert!(!degraded);
        assert_eq!(last_good_display(), display);
        assert_eq!(last_good_runtime(), runtime);
    }

    // Regression (end to end, any host): the Windows production path is app +
    // WSL daemon, and member identity detection runs on the app. With the
    // daemon's status dropped at the boundary, an inventory outage in the
    // daemon after a CLI restart in an existing pane bound the new member to
    // the previous transcript. Through the real `scan_sessions_for_runtime`
    // fed by a degraded daemon snapshot whose stale session matches the pane,
    // detection binds nothing and keeps asking until the window closes.
    #[test]
    fn detect_runtime_session_ignores_degraded_daemon_snapshot_with_stale_matching_pane() {
        use crate::coordination::runtime::{
            CoordinationRuntime, DetectedRuntimeSession, RealRuntimeScan, SystemCoordinationRuntime,
        };

        let _harness = DaemonSnapshotHarness::install(DAEMON_DEGRADED);
        let _real_scan = RealRuntimeScan::install();

        let detected = SystemCoordinationRuntime
            .detect_runtime_session(DAEMON_PANE, CliTool::Codex)
            .expect("detection succeeds");

        assert_eq!(
            detected,
            DetectedRuntimeSession::default(),
            "a degraded daemon snapshot must never bind the cached identity"
        );
    }

    // Regression companion: a healthy daemon snapshot is an observation and
    // binds on the first attempt.
    #[test]
    fn detect_runtime_session_binds_healthy_daemon_snapshot() {
        use crate::coordination::runtime::{
            CoordinationRuntime, DetectedRuntimeSession, RealRuntimeScan, SystemCoordinationRuntime,
        };

        let _harness = DaemonSnapshotHarness::install(DAEMON_HEALTHY);
        let _real_scan = RealRuntimeScan::install();

        let detected = SystemCoordinationRuntime
            .detect_runtime_session(DAEMON_PANE, CliTool::Codex)
            .expect("detection succeeds");

        assert_eq!(
            detected,
            DetectedRuntimeSession {
                session_id: Some(FRESH_SESSION.to_string()),
                jsonl_path: Some(std::path::PathBuf::from(
                    "/home/user/.codex/sessions/fresh-session.jsonl"
                )),
            }
        );
    }
}
