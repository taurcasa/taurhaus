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
/// For Claude/Agy we keep the existing behavior: project-level file signal
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

            let tool_spec = crate::session_scanner::cli_tool::spec(proc.cli_tool);
            if let Some(mut session) = tool_spec
                .session_source()
                .process_session(&proc, tmux_pane.map(|p| p.pane_id.as_str()))
            {
                session.tmux_session = tmux_pane.map(|p| p.session_name.clone());
                session.tmux_window = tmux_pane.map(|p| p.window_index.clone());
                session.tmux_window_name = tmux_pane.map(|p| p.window_name.clone());
                return session;
            }
            let idle_started = Instant::now();
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
            let seat_observation = tool_spec
                .activity_source()
                .observation(
                    proc.pid,
                    &proc.project_path,
                    tmux_pane.map(|pane| pane.pane_id.as_str()),
                )
                .filter(|observed| matches!(observed.source, "launch_ready" | "notify"));
            let authoritative_state = tool_spec
                .activity_source()
                .authoritative_state(&proc.project_path, proc.pid, &idle_result)
                .or_else(|| {
                    seat_observation
                        .as_ref()
                        .map(|observed| idle::AuthoritativeState {
                            state: observed.state,
                            source: observed.source,
                        })
                });
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
                let recent_io = proc_io::is_process_active_hysteresis(proc.pid);
                (recent_io, recent_io)
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
                    ),
                );
            }
            let (activity_confidence, activity_attribution, project_unattributed_active) =
                if let Some(observed) = &seat_observation {
                    (
                        if observed.source == "launch_ready" {
                            ActivityConfidence::Medium
                        } else {
                            ActivityConfidence::High
                        },
                        ActivityAttribution::Attributed,
                        false,
                    )
                } else if state == SessionState::Active {
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

            let workflow_activity = crate::workflow_runs::activity_for_transcript(
                proc.cli_tool,
                idle_result.jsonl_path.as_deref(),
                std::time::SystemTime::now(),
            );
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
                source: None,
                project_unattributed_active,
                group_kind: SessionGroupKind::Standalone,
                group_id: None,
                group_label: None,
                member_name: None,
                workflow_activity,
            }
        })
        .collect();

    deduplicate_runtime_sessions(&mut sessions);
    (sessions, idle_ms, process_signal_ms, ownership_ms)
}

/// Name the evidence behind a raw activity decision, for `activity.state.changed`.
///
/// `authoritative` means the tool reported the state itself — each harness's
/// declared `ActivitySource` names its own evidence. Everything else falls back
/// to the process-IO floor and then to the transcript mtime.
fn activity_source(
    authoritative_source: Option<&'static str>,
    process_active: bool,
    file_active: bool,
) -> &'static str {
    if let Some(source) = authoritative_source {
        return source;
    }
    if process_active {
        "process_io"
    } else if file_active {
        "transcript"
    } else {
        "none"
    }
}

fn emit_activity_state_changed<T: serde::Serialize + std::fmt::Debug>(
    pid: u32,
    cli_tool: CliTool,
    from: Option<T>,
    to: T,
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

/// The host uses the same classification emitter; its semantic levels include waiting/unavailable.
pub(crate) fn classify_host_activity(session: &mut RuntimeSession, status: &serde_json::Value) {
    use super::HostActivity;
    let previous = HostActivity::from_session(session);
    let kind = status["type"].as_str();
    let available = matches!(kind, Some("active" | "idle"));
    let working = kind == Some("active")
        && status["activeFlags"]
            .as_array()
            .is_none_or(|flags| flags.is_empty());
    session.state = if kind == Some("active") {
        SessionState::Active
    } else {
        SessionState::Idle
    };
    session.activity_confidence = if available {
        ActivityConfidence::High
    } else {
        ActivityConfidence::Low
    };
    session.activity_attribution = if working || kind == Some("idle") {
        ActivityAttribution::Attributed
    } else {
        ActivityAttribution::None
    };
    session.recent_io = working;
    let source = if available {
        "host"
    } else {
        "host_unavailable"
    };
    session.source = Some(source.into());
    let next = HostActivity::from_session(session).unwrap();
    if previous.as_ref() != Some(&next) {
        emit_activity_state_changed(
            session.pid,
            session.cli_tool,
            previous.map(|s| s.state),
            next.state,
            source,
        );
    }
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
            source: None,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
            workflow_activity: None,
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

    // Regression: b9e4a855 promoted the readiness slice's raw process_io
    // observation to High-confidence working, bypassing proc_io hysteresis.
    #[test]
    fn codex_round3_process_io_observation_is_not_authoritative() {
        let _lock = SCANNER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let pid = 941_033;
        let project = "/scratch/round3";
        idle::codex_readiness::seed_observation_for_test(
            pid,
            project,
            "%round3",
            "process_io",
            SessionState::Active,
            chrono::Utc::now(),
        );
        let proc = process::ProcessInfo {
            pid,
            project_path: project.into(),
            tty: "round3-tty".into(),
            args: "codex".into(),
            cli_tool: CliTool::Codex,
        };
        let panes = HashMap::from([(
            "round3-tty".into(),
            tmux::TmuxPane {
                pane_id: "%round3".into(),
                tty: "round3-tty".into(),
                window_index: "0".into(),
                window_name: "test".into(),
                session_name: "test".into(),
            },
        )]);
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                set_runtime_idle_detector_override(None);
            }
        }
        let _reset = Reset;
        set_runtime_idle_detector_override(Some(|_| idle_result(SessionState::Idle, false)));
        let (sessions, _, _, _) =
            classify_display_runtime_sessions_with(vec![proc], panes, &HashMap::new(), &|_| {
                idle_result(SessionState::Idle, false)
            });
        assert_eq!(sessions[0].state, SessionState::Idle);
        assert!(!sessions[0].recent_io);
        assert_eq!(sessions[0].activity_confidence, ActivityConfidence::Low);
        cache::remove_state_tracker(pid);
    }

    // Regression: b9e4a855 bypassed the activity registry for launch readiness.
    #[test]
    fn codex_review_readiness_uses_activity_slice_and_classification() {
        let _lock = SCANNER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                set_runtime_idle_detector_override(None);
            }
        }
        let _reset = Reset;
        set_runtime_idle_detector_override(Some(|_| idle_result(SessionState::Idle, false)));
        let pid = 941_030;
        let tool = CliTool::Codex;
        let project = "/scratch/review";
        let pane = "%review";
        let activity = crate::session_scanner::cli_tool::spec(tool).activity_source();
        for (source, state, confidence) in [
            (
                "launch_ready",
                SessionState::Idle,
                ActivityConfidence::Medium,
            ),
            ("notify", SessionState::Idle, ActivityConfidence::High),
            ("notify", SessionState::Active, ActivityConfidence::High),
        ] {
            idle::codex_readiness::seed_observation_for_test(
                pid,
                project,
                pane,
                source,
                state,
                chrono::Utc::now(),
            );
            let observation = activity
                .observation(pid, project, Some(pane))
                .expect("registry must own readiness");
            assert_eq!(observation.source, source);
            let proc = process::ProcessInfo {
                pid,
                project_path: project.into(),
                tty: "test-tty".into(),
                args: "codex".into(),
                cli_tool: tool,
            };
            let panes = HashMap::from([(
                "test-tty".into(),
                tmux::TmuxPane {
                    pane_id: pane.into(),
                    tty: "test-tty".into(),
                    window_index: "0".into(),
                    window_name: "test".into(),
                    session_name: "test".into(),
                },
            )]);
            let (sessions, _, _, _) =
                classify_display_runtime_sessions_with(vec![proc], panes, &HashMap::new(), &|_| {
                    idle_result(SessionState::Idle, false)
                });
            let session = &sessions[0];
            assert_eq!(session.state, state);
            assert_eq!(session.activity_confidence, confidence);
            assert_eq!(
                session.activity_attribution,
                ActivityAttribution::Attributed
            );
            assert!(!session.project_unattributed_active);
            use crate::coordination::activity_export::{
                build_member_activity_snapshot, PaneActivityProbe,
            };
            use crate::coordination::activity_schema::SnapshotActivityConfidence;
            let display = crate::session_scanner::DisplaySession::from(session.clone());
            let probe = PaneActivityProbe {
                pane_alive: true,
                active_non_shell_process: true,
                ..Default::default()
            };
            let expected = if state == SessionState::Idle {
                SnapshotActivityConfidence::Idle
            } else {
                SnapshotActivityConfidence::Active
            };
            let fresh = build_member_activity_snapshot(Some(&display), &probe, chrono::Utc::now());
            assert_eq!(fresh.activity_confidence, expected);
            assert_eq!(
                fresh.evidence.get("source"),
                Some(&serde_json::json!(source))
            );
            assert_eq!(
                fresh.evidence.get("confidence"),
                Some(&serde_json::json!(confidence))
            );
            // Expiry and concurrent invalidation must preserve the classified verdict.
            idle::codex_readiness::seed_observation_for_test(
                pid,
                project,
                pane,
                source,
                state,
                chrono::Utc::now() - chrono::Duration::seconds(3),
            );
            let stale = build_member_activity_snapshot(Some(&display), &probe, chrono::Utc::now());
            assert_eq!(stale.activity_confidence, expected);
            assert!(stale.evidence.is_empty());
        }
        idle::codex_readiness::seed_observation_for_test(
            pid,
            project,
            pane,
            "launch_ready",
            SessionState::Idle,
            chrono::Utc::now() - chrono::Duration::seconds(3),
        );
        assert!(activity.observation(pid, project, Some(pane)).is_none());
        cache::remove_state_tracker(pid);
    }

    #[test]
    fn hosted_remote_inventory_never_uses_notify_or_io() {
        // Regression: 6f61f611 classified the private attached TUI as an ordinary Codex CLI.
        let _lock = SCANNER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let proc = process::ProcessInfo {
            pid: 941_090,
            project_path: tmp.path().to_string_lossy().into_owned(),
            tty: "/dev/pts/fixture".into(),
            args: format!(
                "codex --remote unix://{}/rpc.sock resume exact-thread --no-alt-screen",
                tmp.path().display()
            ),
            cli_tool: CliTool::Codex,
        };
        // Prevent the pre-fix path from discovering any real account files.
        set_runtime_idle_detector_override(Some(|_| idle_result(SessionState::Active, true)));
        let (sessions, _, _, _) = classify_display_runtime_sessions_with(
            vec![proc],
            HashMap::new(),
            &HashMap::new(),
            &|_| unreachable!(),
        );
        set_runtime_idle_detector_override(None);
        assert_eq!(sessions[0].session_id.as_deref(), Some("exact-thread"));
        assert!(!sessions[0].recent_io);
        assert_eq!(
            serde_json::to_value(&sessions[0]).unwrap()["source"],
            "host_unavailable"
        );
        assert!(cache::state_tracker_snapshot(941_090).is_none());
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
        assert_eq!(activity_source(Some("registry"), false, false,), "registry");
        assert_eq!(activity_source(Some("registry"), true, false,), "registry");
        assert_eq!(activity_source(Some("notify"), false, false,), "notify");
    }

    #[test]
    fn activity_source_names_the_driving_signal() {
        assert_eq!(activity_source(None, true, true,), "process_io");
        assert_eq!(activity_source(None, true, false,), "process_io");
        assert_eq!(activity_source(None, false, true,), "transcript");
        assert_eq!(activity_source(None, false, false,), "none");
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
