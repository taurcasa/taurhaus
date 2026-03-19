use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

use super::cache::apply_hysteresis;
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
            let idle_result = match proc.cli_tool {
                CliTool::Codex => detect_runtime_idle_for_process_with_pane(
                    &proc,
                    tmux_pane.map(|pane| pane.pane_id.as_str()),
                ),
                _ => idle_detector(&proc),
            };
            idle_ms += idle_started.elapsed();

            let file_active = idle_result.state == SessionState::Active;
            let sessions_for_tool_in_project = sessions_per_project_tool
                .get(&(proc.project_path.clone(), proc.cli_tool))
                .copied()
                .unwrap_or(1);

            let process_signal_started = Instant::now();
            let (process_active, recent_io) = match proc.cli_tool {
                CliTool::Claude => {
                    let recent_io = proc_io::is_process_active_hysteresis(proc.pid);
                    (recent_io, recent_io)
                }
                CliTool::Gemini => (proc_io::has_api_connections(proc.pid), false),
                CliTool::Codex => {
                    let recent_io = proc_io::is_process_active_hysteresis(proc.pid);
                    (recent_io, recent_io)
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

            let state = apply_hysteresis(proc.pid, decision.raw_state);
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

pub(crate) fn deduplicate_runtime_sessions(sessions: &mut Vec<RuntimeSession>) {
    sessions.sort_by(|left, right| right.pid.cmp(&left.pid));
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
