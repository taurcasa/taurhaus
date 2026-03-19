use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use crate::coordination::runtime::CoordinationRuntime;

use super::signal_sources;
use super::transitions::set_if_newer;
use super::types::{
    MemberKey, MemberSignalContext, MemberStallState, MeshMemberSignal, MeshMemberStatus,
    SessionScannerFn, SessionSignal, SignalSnapshot,
};

pub(super) fn collect_signals_for_members(
    member_keys: &[MemberKey],
    member_signal_contexts: &Arc<Mutex<HashMap<MemberKey, MemberSignalContext>>>,
    runtime: &dyn CoordinationRuntime,
    session_scanner: &SessionScannerFn,
    mesh_signal_reader: &super::types::MeshSignalReaderFn,
    require_medium_confidence: bool,
    now: DateTime<Utc>,
) -> Vec<SignalSnapshot> {
    if member_keys.is_empty() {
        return Vec::new();
    }

    let probe_tmux_signals = signal_sources::host_supports_tmux_signals();
    let probe_mesh_signals = signal_sources::host_supports_mesh_signals();
    let contexts = member_signal_contexts
        .lock()
        .map(|contexts| contexts.clone())
        .unwrap_or_default();
    let any_session_context = contexts
        .values()
        .any(|context| context.pane_id.is_some() || context.project_path.is_some());
    let sessions = if probe_tmux_signals && any_session_context {
        session_scanner(now)
    } else {
        Vec::new()
    };
    let sessions_by_pane: HashMap<String, SessionSignal> = sessions
        .iter()
        .filter_map(|signal| signal.pane_id.clone().map(|pane| (pane, signal.clone())))
        .collect();
    let sessions_by_project = latest_session_per_project(&sessions);

    let mut mesh_by_team: HashMap<String, HashMap<String, MeshMemberSignal>> = HashMap::new();
    let mut snapshots = Vec::with_capacity(member_keys.len());

    for key in member_keys {
        let context = contexts.get(key).cloned().unwrap_or_default();
        let matched_session =
            matched_session_signal(&context, &sessions_by_pane, &sessions_by_project);
        let mesh_signal = if probe_mesh_signals && !key.team_name.trim().is_empty() {
            let mesh_signals = mesh_by_team
                .entry(key.team_name.clone())
                .or_insert_with(|| mesh_signal_reader(&key.team_name));
            mesh_signals
                .get(&key.member_name)
                .cloned()
                .unwrap_or_default()
        } else {
            MeshMemberSignal::default()
        };

        let (pane_exists, pane_is_dead, pane_is_shell, pane_current_command) = if probe_tmux_signals
        {
            collect_pane_snapshot(runtime, context.pane_id.as_deref())
        } else {
            (None, None, None, None)
        };

        let mut snapshot = SignalSnapshot {
            team_name: key.team_name.clone(),
            member_name: key.member_name.clone(),
            observed_at: matched_session
                .as_ref()
                .map(|signal| signal.observed_at)
                .unwrap_or(now),
            session_state: matched_session.as_ref().map(|signal| signal.state),
            session_confidence: matched_session.as_ref().map(|signal| signal.confidence),
            pane_exists,
            pane_is_dead,
            pane_is_shell,
            pane_current_command,
            mesh_last_activity_at: mesh_signal.last_activity_at,
            mesh_status: mesh_signal.status,
            coordination_event_at: context.coordination_event_at,
            project_file_write_at: context.project_file_write_at,
            runtime_last_seen_at: context.last_seen_at,
            strongest_signal: None,
        };
        snapshot.strongest_signal = snapshot.classify(require_medium_confidence);
        snapshots.push(snapshot);
    }

    snapshots
}

pub(super) fn ingest_session_signal(
    member_states: &Arc<Mutex<HashMap<MemberKey, MemberStallState>>>,
    team_name: &str,
    member_name: &str,
    observed_at: DateTime<Utc>,
    is_strong: bool,
) {
    let key = MemberKey {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
    };
    if let Ok(mut states) = member_states.lock() {
        if let Some(state) = states.get_mut(&key) {
            set_if_newer(&mut state.last_any_signal_at, observed_at);
            if is_strong {
                set_if_newer(&mut state.last_strong_signal_at, observed_at);
            }
        }
    }
}

pub(super) fn ingest_pane_check(
    member_states: &Arc<Mutex<HashMap<MemberKey, MemberStallState>>>,
    team_name: &str,
    member_name: &str,
    observed_at: DateTime<Utc>,
    pane_alive: bool,
) -> bool {
    if !pane_alive {
        return false;
    }

    let key = MemberKey {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
    };
    if let Ok(mut states) = member_states.lock() {
        if let Some(state) = states.get_mut(&key) {
            set_if_newer(&mut state.last_any_signal_at, observed_at);
            return true;
        }
    }
    false
}

pub(super) fn ingest_mesh_heartbeat(
    member_states: &Arc<Mutex<HashMap<MemberKey, MemberStallState>>>,
    team_name: &str,
    member_name: &str,
    observed_at: DateTime<Utc>,
) {
    let key = MemberKey {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
    };
    if let Ok(mut states) = member_states.lock() {
        if let Some(state) = states.get_mut(&key) {
            set_if_newer(&mut state.last_any_signal_at, observed_at);
        }
    }
}

pub(super) fn ingest_mesh_status(
    member_states: &Arc<Mutex<HashMap<MemberKey, MemberStallState>>>,
    team_name: &str,
    member_name: &str,
    observed_at: DateTime<Utc>,
    status: MeshMemberStatus,
) -> bool {
    let key = MemberKey {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
    };
    if let Ok(mut states) = member_states.lock() {
        if let Some(state) = states.get_mut(&key) {
            match status {
                MeshMemberStatus::Working | MeshMemberStatus::Investigating => {
                    set_if_newer(&mut state.last_any_signal_at, observed_at);
                    if matches!(status, MeshMemberStatus::Working) {
                        set_if_newer(&mut state.last_strong_signal_at, observed_at);
                    }
                    return true;
                }
                MeshMemberStatus::Blocked | MeshMemberStatus::Idle | MeshMemberStatus::Unknown => {}
            }
        }
    }
    false
}

pub(super) fn build_signal_snapshot_index(
    snapshots: &[SignalSnapshot],
) -> HashMap<MemberKey, SignalSnapshot> {
    let mut index = HashMap::with_capacity(snapshots.len());
    for snapshot in snapshots {
        index.insert(
            MemberKey {
                team_name: snapshot.team_name.clone(),
                member_name: snapshot.member_name.clone(),
            },
            snapshot.clone(),
        );
    }
    index
}

fn collect_pane_snapshot(
    runtime: &dyn CoordinationRuntime,
    pane_id: Option<&str>,
) -> (Option<bool>, Option<bool>, Option<bool>, Option<String>) {
    if !signal_sources::host_supports_tmux_signals() {
        return (None, None, None, None);
    }

    let Some(pane_id) = pane_id else {
        return (None, None, None, None);
    };

    let pane_exists = runtime.pane_exists(pane_id).ok();
    if pane_exists != Some(true) {
        return (pane_exists, None, None, None);
    }

    let pane_is_dead = runtime.pane_is_dead(pane_id).ok();
    if pane_is_dead == Some(true) {
        return (pane_exists, pane_is_dead, None, None);
    }

    let pane_is_shell = runtime.pane_is_shell(pane_id).ok();
    let pane_current_command = runtime.pane_current_command(pane_id).ok().flatten();
    (
        pane_exists,
        pane_is_dead,
        pane_is_shell,
        pane_current_command,
    )
}

fn latest_session_per_project(sessions: &[SessionSignal]) -> HashMap<String, SessionSignal> {
    let mut by_project = HashMap::new();
    for session in sessions {
        by_project
            .entry(session.project_path.clone())
            .and_modify(|current: &mut SessionSignal| {
                if session.observed_at >= current.observed_at
                    || session.confidence_rank() > current.confidence_rank()
                {
                    *current = session.clone();
                }
            })
            .or_insert_with(|| session.clone());
    }
    by_project
}

fn matched_session_signal(
    context: &MemberSignalContext,
    sessions_by_pane: &HashMap<String, SessionSignal>,
    sessions_by_project: &HashMap<String, SessionSignal>,
) -> Option<SessionSignal> {
    if let Some(pane_id) = context.pane_id.as_deref() {
        if let Some(signal) = sessions_by_pane.get(pane_id) {
            return Some(signal.clone());
        }
    }
    context
        .project_path
        .as_ref()
        .and_then(|path| sessions_by_project.get(path))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::coordination::runtime::RecordingCoordinationRuntime;
    use crate::session_scanner::{ActivityConfidence, SessionState};

    use super::super::service::StallDetectorService;
    use super::super::types::{
        MemberSignalContext, MeshSignalReaderFn, SignalStrength, StallDetectorConfig, StallStage,
    };

    fn ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    #[test]
    fn ingest_session_signal_updates_strong_and_any_timestamps() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let at = ts("2026-03-05T12:00:00Z");
        service.ingest_session_signal("team-a", "agent-a", at, true);

        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.last_strong_signal_at, Some(at));
        assert_eq!(state.last_any_signal_at, Some(at));
    }

    #[test]
    fn ingest_pane_check_updates_any_timestamp_only() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let at = ts("2026-03-05T12:00:00Z");
        service.ingest_pane_check("team-a", "agent-a", at, true, false);

        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.last_strong_signal_at, None);
        assert_eq!(state.last_any_signal_at, Some(at));
    }

    #[test]
    fn collect_signals_classifies_active_medium_confidence_session_as_strong() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|now| {
                vec![SessionSignal {
                    pane_id: Some("%11".to_string()),
                    project_path: "/repo".to_string(),
                    observed_at: now,
                    state: SessionState::Active,
                    confidence: ActivityConfidence::Medium,
                }]
            }),
            Arc::new(|_: &str| HashMap::new()) as Arc<MeshSignalReaderFn>,
        );

        let now = ts("2026-03-05T12:00:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.upsert_member_signal_context(
            "team-a",
            "agent-a",
            MemberSignalContext {
                pane_id: Some("%11".to_string()),
                project_path: Some("/repo".to_string()),
                ..MemberSignalContext::default()
            },
        );

        let snapshots = service.collect_signals_at(now);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].strongest_signal, Some(SignalStrength::Strong));
    }

    #[test]
    fn collect_signals_classifies_non_shell_command_as_medium() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_pane_exists("%22", true);
        runtime.set_pane_dead("%22", false);
        runtime.set_pane_shell("%22", false);
        runtime.set_pane_current_command("%22", Some("cargo test"));

        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|_| Vec::new()),
            Arc::new(|_: &str| HashMap::new()) as Arc<MeshSignalReaderFn>,
        );

        let now = ts("2026-03-05T12:00:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.upsert_member_signal_context(
            "team-a",
            "agent-a",
            MemberSignalContext {
                pane_id: Some("%22".to_string()),
                ..MemberSignalContext::default()
            },
        );

        let snapshots = service.collect_signals_at(now);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].strongest_signal, Some(SignalStrength::Medium));
    }

    #[test]
    fn collect_signals_classifies_last_seen_as_weak() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|_| Vec::new()),
            Arc::new(|_: &str| HashMap::new()) as Arc<MeshSignalReaderFn>,
        );

        let now = ts("2026-03-05T12:00:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.upsert_member_signal_context(
            "team-a",
            "agent-a",
            MemberSignalContext {
                last_seen_at: Some(now),
                ..MemberSignalContext::default()
            },
        );

        let snapshots = service.collect_signals_at(now);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].strongest_signal, Some(SignalStrength::Weak));
    }

    #[test]
    fn poll_once_applies_collected_strong_signal_before_threshold_check() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|now| {
                vec![SessionSignal {
                    pane_id: Some("%33".to_string()),
                    project_path: "/repo".to_string(),
                    observed_at: now,
                    state: SessionState::Active,
                    confidence: ActivityConfidence::High,
                }]
            }),
            Arc::new(|_: &str| HashMap::new()),
        );

        let now = ts("2026-03-05T12:10:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - chrono::Duration::seconds(600),
        );
        service.upsert_member_signal_context(
            "team-a",
            "agent-a",
            MemberSignalContext {
                pane_id: Some("%33".to_string()),
                project_path: Some("/repo".to_string()),
                ..MemberSignalContext::default()
            },
        );

        let transitions = service.poll_once_at(now);
        assert!(transitions.is_empty());
        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.last_strong_signal_at, Some(now));
        assert_eq!(state.stage, StallStage::Healthy);
    }

    #[test]
    fn collect_signals_skips_session_scan_without_context() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let scanner_calls = Arc::new(AtomicUsize::new(0));
        let scanner_calls_ref = scanner_calls.clone();
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(move |_| {
                scanner_calls_ref.fetch_add(1, Ordering::Relaxed);
                Vec::new()
            }),
            Arc::new(|_: &str| HashMap::new()) as Arc<MeshSignalReaderFn>,
        );

        let now = ts("2026-03-05T13:40:00Z");
        service.upsert_member("team-a", "agent-a", now);
        let snapshots = service.collect_signals_at(now);

        assert_eq!(snapshots.len(), 1);
        assert_eq!(scanner_calls.load(Ordering::Relaxed), 0);
        assert!(snapshots[0].session_state.is_none());
    }

    #[test]
    fn blank_team_name_skips_mesh_signal_reader() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mesh_calls = Arc::new(AtomicUsize::new(0));
        let mesh_calls_ref = mesh_calls.clone();
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|_| Vec::new()),
            Arc::new(move |_| {
                mesh_calls_ref.fetch_add(1, Ordering::Relaxed);
                HashMap::new()
            }),
        );

        let now = ts("2026-03-05T13:50:00Z");
        service.upsert_member("", "agent-a", now);
        let snapshots = service.collect_signals_at(now);

        assert_eq!(snapshots.len(), 1);
        assert_eq!(mesh_calls.load(Ordering::Relaxed), 0);
        assert!(snapshots[0].mesh_status.is_none());
        assert!(snapshots[0].mesh_last_activity_at.is_none());
    }
}
