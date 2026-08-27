use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

use super::cache::{apply_hysteresis, record_authoritative_state};
use super::{
    idle, proc_io, process, tmux, ActivityAttribution, ActivityConfidence, CliTool, RuntimeSession,
    SessionGroupKind, SessionState,
};

#[allow(clippy::type_complexity)]
#[cfg(test)]
static RUNTIME_IDLE_DETECTOR_OVERRIDE: OnceLock<
    Mutex<Option<fn(&process::ProcessInfo) -> idle::IdleResult>>,
> = OnceLock::new();

pub(crate) fn detect_runtime_idle_for_process(proc: &process::ProcessInfo) -> idle::IdleResult {
    detect_runtime_idle_for_process_with_pane(proc, None)
}

pub(crate) fn detect_runtime_idle_for_process_with_pane(
    proc: &process::ProcessInfo,
    pane_id: Option<&str>,
) -> idle::IdleResult {
    #[cfg(test)]
    if let Some(detector) = RUNTIME_IDLE_DETECTOR_OVERRIDE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .copied()
    {
        return detector(proc);
    }

    idle::detect_runtime_idle(&proc.project_path, proc.pid, pane_id, proc.cli_tool)
}

/// Compute a process's raw state from tool-specific signals.
///
/// For Claude/Gemini we keep the existing behavior: project-level file signal
/// OR process-level signal marks the process active.
///
/// Codex needs special handling for multi-session projects: the file signal is
/// project-scoped (shared transcript activity), so using it directly marks all
/// Codex sessions active when only one is working. When multiple Codex
/// sessions are present for a project, we ignore the shared file signal and
/// rely on per-PID IO activity to disambiguate each session.
pub(crate) struct ActivityDecision {
    pub raw_state: SessionState,
    pub confidence: ActivityConfidence,
    pub attribution: ActivityAttribution,
    pub project_unattributed_active: bool,
    #[cfg(test)]
    pub keep_session_metadata: bool,
}

pub(crate) fn compute_activity_decision(
    file_active: bool,
    process_active: bool,
    sessions_for_tool_in_project: usize,
    deterministic_file_owner: bool,
) -> ActivityDecision {
    if process_active {
        return ActivityDecision {
            raw_state: SessionState::Active,
            confidence: ActivityConfidence::High,
            attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            #[cfg(test)]
            keep_session_metadata: true,
        };
    }

    if file_active {
        if sessions_for_tool_in_project <= 1 {
            return ActivityDecision {
                raw_state: SessionState::Active,
                confidence: ActivityConfidence::Medium,
                attribution: ActivityAttribution::Attributed,
                project_unattributed_active: false,
                #[cfg(test)]
                keep_session_metadata: true,
            };
        }

        if deterministic_file_owner {
            return ActivityDecision {
                raw_state: SessionState::Active,
                confidence: ActivityConfidence::High,
                attribution: ActivityAttribution::Attributed,
                project_unattributed_active: false,
                #[cfg(test)]
                keep_session_metadata: true,
            };
        }

        return ActivityDecision {
            raw_state: SessionState::Idle,
            confidence: ActivityConfidence::Low,
            attribution: ActivityAttribution::Unattributed,
            project_unattributed_active: true,
            #[cfg(test)]
            keep_session_metadata: false,
        };
    }

    ActivityDecision {
        raw_state: SessionState::Idle,
        confidence: ActivityConfidence::Low,
        attribution: ActivityAttribution::None,
        project_unattributed_active: false,
        #[cfg(test)]
        keep_session_metadata: sessions_for_tool_in_project <= 1,
    }
}

pub(crate) fn classify_display_runtime_sessions_with<H>(
    processes: Vec<process::ProcessInfo>,
    pane_map: HashMap<String, tmux::TmuxPane>,
    sessions_per_project_tool: &HashMap<(String, CliTool), usize>,
    idle_detector: &H,
) -> (Vec<RuntimeSession>, Duration, Duration, Duration)
where
    H: Fn(&process::ProcessInfo) -> idle::IdleResult,
{
    let mut idle_ms = Duration::default();
    let mut process_signal_ms = Duration::default();
    let mut ownership_ms = Duration::default();

    let mut sessions: Vec<RuntimeSession> = processes
        .into_iter()
        .map(|proc| {
            let tmux_pane = pane_map.get(&proc.tty);

            let idle_started = Instant::now();
            let tool_spec = crate::session_scanner::cli_tool::spec(proc.cli_tool);
            let idle_result = if tool_spec.pane_binding {
                detect_runtime_idle_for_process_with_pane(
                    &proc,
                    tmux_pane.map(|pane| pane.pane_id.as_str()),
                )
            } else {
                idle_detector(&proc)
            };
            idle_ms += idle_started.elapsed();

            // The tool reported this state itself (Claude sessions registry or
            // Codex notify): it replaces the file signal rather than
            // supplementing it.
            let authoritative_state = tool_spec.activity_source().authoritative_state(
                &proc.project_path,
                proc.pid,
                &idle_result,
            );
            let authoritative = authoritative_state.is_some();
            let observed_state = authoritative_state
                .map(|reported| reported.state)
                .unwrap_or(idle_result.state);
            let authoritative_active = authoritative && observed_state == SessionState::Active;
            let file_active = !authoritative && idle_result.state == SessionState::Active;
            let sessions_for_tool_in_project = sessions_per_project_tool
                .get(&(proc.project_path.clone(), proc.cli_tool))
                .copied()
                .unwrap_or(1);

            let process_signal_started = Instant::now();
            // `recent_io` carries "confirmed working now" downstream
            // (`coordination::activity_export`); an authoritative status is at
            // least as strong as an rchar burst, and the rchar poll is skipped.
            let (process_active, recent_io) = if authoritative {
                (authoritative_active, authoritative_active)
            } else {
                match tool_spec.process_activity_signal {
                    crate::session_scanner::cli_tool::ProcessActivitySignal::ReadChars => {
                        let recent_io = proc_io::is_process_active_hysteresis(proc.pid);
                        (recent_io, recent_io)
                    }
                    crate::session_scanner::cli_tool::ProcessActivitySignal::Tcp => {
                        (proc_io::has_api_connections(proc.pid), false)
                    }
                }
            };
            process_signal_ms += process_signal_started.elapsed();

            let ownership_started = Instant::now();
            let deterministic_file_owner = file_active
                && !process_active
                && sessions_for_tool_in_project > 1
                && idle_result
                    .jsonl_path
                    .as_deref()
                    .is_some_and(|path| crate::platform::process_has_open_path(proc.pid, path));
            ownership_ms += ownership_started.elapsed();

            let decision = compute_activity_decision(
                file_active,
                process_active,
                sessions_for_tool_in_project,
                deterministic_file_owner,
            );

            // Hysteresis smooths a noisy heuristic; an authoritative status has
            // no noise to smooth, so it lands on the poll that observed it.
            let (state, previous_state) = if authoritative {
                (
                    observed_state,
                    record_authoritative_state(proc.pid, observed_state),
                )
            } else {
                apply_hysteresis(proc.pid, decision.raw_state)
            };
            // First sight of a PID is not a transition. The display scan
            // prunes the tracker of every PID it does not return
            // (`cache::retain_state_trackers`), so unbound `codex exec`
            // processes re-enter classification with `previous_state == None`
            // on every cycle; emitting on those turned the sink into a
            // heartbeat. First sight earns an event only when the process
            // arrives active.
            let is_transition = match previous_state {
                Some(previous) => previous != state,
                None => state != SessionState::Idle,
            };
            if is_transition {
                emit_activity_state_changed(
                    proc.pid,
                    proc.cli_tool,
                    previous_state,
                    state,
                    activity_source(
                        authoritative_state.map(|reported| reported.source),
                        process_active,
                        file_active,
                        tool_spec.process_activity_signal,
                    ),
                );
            }
            let (activity_confidence, activity_attribution, project_unattributed_active) =
                if state == SessionState::Active {
                    (decision.confidence, decision.attribution, false)
                } else if decision.project_unattributed_active {
                    (
                        ActivityConfidence::Low,
                        ActivityAttribution::Unattributed,
                        true,
                    )
                } else {
                    (ActivityConfidence::Low, ActivityAttribution::None, false)
                };

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
                state,
                session_id: idle_result.session_id,
                jsonl_path: idle_result.jsonl_path,
                recent_io,
                last_output_age_secs: idle_result.last_output_age_secs,
                activity_confidence,
                activity_attribution,
                project_unattributed_active,
                group_kind: SessionGroupKind::Standalone,
                group_id: None,
                group_label: None,
                member_name: None,
            }
        })
        .collect();

    deduplicate_runtime_sessions(&mut sessions);
    (sessions, idle_ms, process_signal_ms, ownership_ms)
}

/// Name the evidence behind a raw activity decision, for `activity.state.changed`.
///
/// `authoritative` means the tool reported the state itself. Today the only
/// such source is the Claude sessions registry; PR 13 adds Codex `-c notify`.
fn activity_source(
    authoritative_source: Option<&'static str>,
    process_active: bool,
    file_active: bool,
    process_signal: crate::session_scanner::cli_tool::ProcessActivitySignal,
) -> &'static str {
    if let Some(source) = authoritative_source {
        return source;
    }
    if process_active {
        match process_signal {
            crate::session_scanner::cli_tool::ProcessActivitySignal::ReadChars => "process_io",
            crate::session_scanner::cli_tool::ProcessActivitySignal::Tcp => "tcp",
        }
    } else if file_active {
        "transcript"
    } else {
        "none"
    }
}

fn emit_activity_state_changed(
    pid: u32,
    cli_tool: CliTool,
    from: Option<SessionState>,
    to: SessionState,
    source: &'static str,
) {
    tracing::info!(pid, tool = %cli_tool, ?from, ?to, source, "session activity state changed");
    let mut fields = serde_json::Map::new();
    fields.insert("pid".to_string(), serde_json::Value::from(pid));
    fields.insert(
        "tool".to_string(),
        serde_json::Value::String(cli_tool.to_string()),
    );
    fields.insert(
        "from".to_string(),
        serde_json::to_value(from).unwrap_or(serde_json::Value::Null),
    );
    fields.insert(
        "to".to_string(),
        serde_json::to_value(to).unwrap_or(serde_json::Value::Null),
    );
    fields.insert(
        "source".to_string(),
        serde_json::Value::String(source.to_string()),
    );
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "activity.state.changed",
        Some("Session activity state changed".to_string()),
        fields,
    );
}

pub(crate) fn deduplicate_runtime_sessions(sessions: &mut Vec<RuntimeSession>) {
    sessions.sort_by_key(|session| std::cmp::Reverse(session.pid));
    let mut seen = HashSet::<(String, CliTool)>::new();
    sessions.retain(|session| seen.insert((session.tty.clone(), session.cli_tool)));
}

#[cfg(test)]
pub(crate) fn set_runtime_idle_detector_override(
    detector: Option<fn(&process::ProcessInfo) -> idle::IdleResult>,
) {
    *RUNTIME_IDLE_DETECTOR_OVERRIDE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = detector;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::{cache, StateChangeCapture, SCANNER_TEST_LOCK};

    fn runtime_session(pid: u32, tty: &str, cli_tool: CliTool) -> RuntimeSession {
        RuntimeSession {
            pid,
            project_path: "/home/user/proj-a".to_string(),
            tty: tty.to_string(),
            args: "tool".to_string(),
            cli_tool,
            tmux_session: None,
            tmux_window: None,
            tmux_pane: None,
            tmux_window_name: None,
            state: SessionState::Idle,
            session_id: None,
            jsonl_path: None,
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: ActivityConfidence::Low,
            activity_attribution: ActivityAttribution::None,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    fn claude_process(pid: u32) -> process::ProcessInfo {
        process::ProcessInfo {
            pid,
            project_path: "/home/user/proj-a".to_string(),
            tty: "/dev/pts/9".to_string(),
            args: "claude".to_string(),
            cli_tool: CliTool::Claude,
        }
    }

    fn idle_result(state: SessionState, authoritative: bool) -> idle::IdleResult {
        idle::IdleResult {
            state,
            session_id: Some("session-1".to_string()),
            jsonl_path: None,
            last_output_age_secs: None,
            authoritative,
        }
    }

    /// One classification poll against the process-global hysteresis trackers.
    ///
    /// The caller holds `SCANNER_TEST_LOCK` for the whole sequence: these polls
    /// depend on tracker continuity, and other scanner tests prune the tracker
    /// map wholesale (`retain_state_trackers(&[])`, `E2eScanner::drop`).
    fn classify_once(pid: u32, result: idle::IdleResult) -> RuntimeSession {
        let sessions_per_project_tool =
            HashMap::from([(("/home/user/proj-a".to_string(), CliTool::Claude), 1)]);
        let (sessions, _, _, _) = classify_display_runtime_sessions_with(
            vec![claude_process(pid)],
            HashMap::new(),
            &sessions_per_project_tool,
            &move |_: &process::ProcessInfo| result.clone(),
        );
        sessions.into_iter().next().expect("one session")
    }

    // Regression: PR 2 commit 06b432d added `activity.state.changed` and gated
    // it on `previous_state != Some(state)`, which is also true the first time
    // a PID is seen. On the live 0.6.6 host the display scan prunes the tracker
    // of every PID it does not return (`cache::retain_state_trackers`), so
    // unbound `codex exec` PIDs were re-classified from an empty tracker on
    // every cycle and re-emitted `None -> idle` forever: ~232 events/minute of
    // pure noise in the JSONL sink. First sight of an idle process is not a
    // transition.
    #[test]
    fn first_sight_of_an_idle_process_emits_no_state_change() {
        let capture = StateChangeCapture::install();
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 941_010;
        cache::remove_state_tracker(pid);

        assert_eq!(
            classify_once(pid, idle_result(SessionState::Idle, false)).state,
            SessionState::Idle
        );

        assert!(
            capture.transitions_for(pid).is_empty(),
            "first sight of an idle PID must not emit activity.state.changed"
        );
        cache::remove_state_tracker(pid);
    }

    // Regression: same commit 06b432d — the fix must not silence a process that
    // arrives already working. First sight of an *active* PID is real news.
    #[test]
    fn first_sight_of_an_active_process_emits_the_arrival() {
        let capture = StateChangeCapture::install();
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 941_011;
        cache::remove_state_tracker(pid);

        assert_eq!(
            classify_once(pid, idle_result(SessionState::Active, false)).state,
            SessionState::Active
        );

        assert_eq!(
            capture.transitions_for(pid),
            vec![(None, SessionState::Active)]
        );
        cache::remove_state_tracker(pid);
    }

    // Regression: same commit 06b432d — suppressing first-sight idle must leave
    // genuine transitions on a tracked PID untouched, in both directions.
    #[test]
    fn a_tracked_process_still_emits_both_transition_directions() {
        let capture = StateChangeCapture::install();
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 941_012;
        cache::remove_state_tracker(pid);

        // Poll 1 establishes the tracker at idle; hysteresis then needs a
        // second consistent raw reading before each reported flip.
        classify_once(pid, idle_result(SessionState::Idle, false));
        classify_once(pid, idle_result(SessionState::Active, false));
        assert_eq!(
            classify_once(pid, idle_result(SessionState::Active, false)).state,
            SessionState::Active
        );
        classify_once(pid, idle_result(SessionState::Idle, false));
        assert_eq!(
            classify_once(pid, idle_result(SessionState::Idle, false)).state,
            SessionState::Idle
        );

        assert_eq!(
            capture.transitions_for(pid),
            vec![
                (Some(SessionState::Idle), SessionState::Active),
                (Some(SessionState::Active), SessionState::Idle),
            ]
        );
        cache::remove_state_tracker(pid);
    }

    // Regression: 9a66d1c classified every Claude session from transcript mtime
    // plus rchar hysteresis, so a state the session itself reported was still
    // delayed a poll and diluted to Low confidence. An authoritative result
    // (the sessions registry) is the state — nothing to smooth.
    #[test]
    fn authoritative_result_skips_hysteresis_and_lands_on_the_first_poll() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 941_001;
        assert_eq!(
            classify_once(pid, idle_result(SessionState::Idle, true)).state,
            SessionState::Idle
        );

        let flipped = classify_once(pid, idle_result(SessionState::Active, true));
        assert_eq!(flipped.state, SessionState::Active);
        assert_eq!(flipped.activity_confidence, ActivityConfidence::High);
        assert_eq!(
            flipped.activity_attribution,
            ActivityAttribution::Attributed
        );
        cache::remove_state_tracker(pid);
    }

    #[test]
    fn heuristic_result_still_needs_two_polls_to_flip() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 941_002;
        assert_eq!(
            classify_once(pid, idle_result(SessionState::Idle, false)).state,
            SessionState::Idle
        );
        // Hysteresis holds the reported state for one cycle on a raw flip.
        assert_eq!(
            classify_once(pid, idle_result(SessionState::Active, false)).state,
            SessionState::Idle
        );
        assert_eq!(
            classify_once(pid, idle_result(SessionState::Active, false)).state,
            SessionState::Active
        );
        cache::remove_state_tracker(pid);
    }

    #[test]
    fn authoritative_result_replaces_the_rchar_signal() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // The PID does not exist, so `/proc/<pid>/io` yields nothing: an active
        // `recent_io` can only have come from the authoritative source.
        let session = classify_once(941_003, idle_result(SessionState::Active, true));
        assert!(session.recent_io);

        let idle = classify_once(941_004, idle_result(SessionState::Idle, true));
        assert!(!idle.recent_io);

        cache::remove_state_tracker(941_003);
        cache::remove_state_tracker(941_004);
    }

    #[test]
    fn activity_source_names_the_native_source_when_authoritative() {
        assert_eq!(
            activity_source(
                Some("registry"),
                false,
                false,
                crate::session_scanner::cli_tool::ProcessActivitySignal::ReadChars,
            ),
            "registry"
        );
        assert_eq!(
            activity_source(
                Some("registry"),
                true,
                false,
                crate::session_scanner::cli_tool::ProcessActivitySignal::ReadChars,
            ),
            "registry"
        );
        assert_eq!(
            activity_source(
                Some("notify"),
                false,
                false,
                crate::session_scanner::cli_tool::ProcessActivitySignal::ReadChars,
            ),
            "notify"
        );
    }

    #[test]
    fn activity_source_names_the_driving_signal() {
        assert_eq!(
            activity_source(
                None,
                true,
                true,
                crate::session_scanner::cli_tool::ProcessActivitySignal::ReadChars,
            ),
            "process_io"
        );
        assert_eq!(
            activity_source(
                None,
                true,
                false,
                crate::session_scanner::cli_tool::ProcessActivitySignal::ReadChars,
            ),
            "process_io"
        );
        assert_eq!(
            activity_source(
                None,
                true,
                false,
                crate::session_scanner::cli_tool::ProcessActivitySignal::Tcp,
            ),
            "tcp"
        );
        assert_eq!(
            activity_source(
                None,
                false,
                true,
                crate::session_scanner::cli_tool::ProcessActivitySignal::ReadChars,
            ),
            "transcript"
        );
        assert_eq!(
            activity_source(
                None,
                false,
                false,
                crate::session_scanner::cli_tool::ProcessActivitySignal::Tcp,
            ),
            "none"
        );
    }

    #[test]
    fn multi_session_file_signal_becomes_unattributed_without_owner() {
        let decision = compute_activity_decision(true, false, 3, false);
        assert_eq!(decision.raw_state, SessionState::Idle);
        assert_eq!(decision.attribution, ActivityAttribution::Unattributed);
        assert!(decision.project_unattributed_active);
        assert!(!decision.keep_session_metadata);
    }

    #[test]
    fn single_session_file_signal_is_attributed_medium_confidence() {
        let decision = compute_activity_decision(true, false, 1, false);
        assert_eq!(decision.raw_state, SessionState::Active);
        assert_eq!(decision.confidence, ActivityConfidence::Medium);
        assert_eq!(decision.attribution, ActivityAttribution::Attributed);
        assert!(decision.keep_session_metadata);
    }

    #[test]
    fn process_signal_is_high_confidence_attributed() {
        let decision = compute_activity_decision(false, true, 3, false);
        assert_eq!(decision.raw_state, SessionState::Active);
        assert_eq!(decision.confidence, ActivityConfidence::High);
        assert_eq!(decision.attribution, ActivityAttribution::Attributed);
    }

    #[test]
    fn deterministic_owner_resolves_multi_session_file_signal() {
        let decision = compute_activity_decision(true, false, 3, true);
        assert_eq!(decision.raw_state, SessionState::Active);
        assert_eq!(decision.confidence, ActivityConfidence::High);
        assert_eq!(decision.attribution, ActivityAttribution::Attributed);
        assert!(!decision.project_unattributed_active);
        assert!(decision.keep_session_metadata);
    }

    #[test]
    fn deduplicate_runtime_sessions_keeps_highest_pid_per_tty_and_tool() {
        let mut sessions = vec![
            runtime_session(500, "/dev/pts/3", CliTool::Codex),
            runtime_session(501, "/dev/pts/3", CliTool::Codex),
            runtime_session(700, "/dev/pts/4", CliTool::Claude),
        ];

        deduplicate_runtime_sessions(&mut sessions);

        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().any(|session| session.pid == 501));
        assert!(sessions.iter().any(|session| session.pid == 700));
    }
}
