//! Daemon-owned host for the final small team-state write intents.

use std::collections::HashMap;
use std::path::Path;

use crate::coordination::errors::CoordinationError;
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::{ActiveProjectTeamStore, TeamConfig, TeamConfigStore};
use crate::daemon::protocol::{
    CoordinationPublishOperationalSnapshotsParams, CoordinationPublishOperationalSnapshotsResult,
    CoordinationReconcileLivePresenceOutcome, CoordinationReconcileLivePresenceParams,
    CoordinationReconcileLivePresenceResult, CoordinationSetActiveProjectTeamParams,
    CoordinationSetActiveProjectTeamResult,
};

pub(crate) fn publish_operational_snapshots(
    teams_dir: &Path,
    params: CoordinationPublishOperationalSnapshotsParams,
) -> Result<CoordinationPublishOperationalSnapshotsResult, CoordinationError> {
    let mut configs = HashMap::<String, TeamConfig>::new();
    let mut published = 0;
    let mut skipped = 0;

    for publication in params.publications {
        let team_name = &publication.snapshot.team_name;
        let config = match configs.entry(team_name.clone()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                match TeamConfigStore::load(teams_dir, team_name) {
                    Ok(config) => entry.insert(config),
                    Err(error) => {
                        tracing::warn!(
                            team = %team_name,
                            error = %error,
                            "skipping operational snapshot for unreadable team config"
                        );
                        skipped += 1;
                        continue;
                    }
                }
            }
        };
        if !config
            .members
            .iter()
            .any(|member| member.name == publication.snapshot.member_name)
        {
            tracing::warn!(
                team = %team_name,
                member = %publication.snapshot.member_name,
                "skipping an operational snapshot that does not belong to the team"
            );
            skipped += 1;
            continue;
        }

        let wrote = crate::coordination::operational_context::publish_member_operation_snapshot(
            teams_dir,
            &publication.snapshot,
            publication.task_state_changed_at,
        )?;
        if wrote {
            published += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(CoordinationPublishOperationalSnapshotsResult { published, skipped })
}

pub(crate) fn reconcile_live_presence(
    state: &CoordinationState,
    params: CoordinationReconcileLivePresenceParams,
) -> Result<CoordinationReconcileLivePresenceResult, CoordinationError> {
    let Some(mut reconciled_offline_members) = state.try_with_orchestrator(|orchestrator| {
        orchestrator.reconcile_team_presence_for_live_status_with_runtime_sessions(
            &params.team_name,
            &params.runtime_sessions,
        )
    })?
    else {
        return Ok(CoordinationReconcileLivePresenceResult {
            outcome: CoordinationReconcileLivePresenceOutcome::Skipped,
            reconciled_offline_members: Vec::new(),
        });
    };
    let mut reconciled_offline_members = reconciled_offline_members.drain().collect::<Vec<_>>();
    reconciled_offline_members.sort();
    Ok(CoordinationReconcileLivePresenceResult {
        outcome: CoordinationReconcileLivePresenceOutcome::Reconciled,
        reconciled_offline_members,
    })
}

pub(crate) fn set_active_project_team(
    teams_dir: &Path,
    params: CoordinationSetActiveProjectTeamParams,
) -> Result<CoordinationSetActiveProjectTeamResult, CoordinationError> {
    match params.team_name {
        Some(team_name) => {
            ActiveProjectTeamStore::set_active_team(teams_dir, &params.project_path, &team_name)?
        }
        None => ActiveProjectTeamStore::clear_project(teams_dir, &params.project_path)?,
    }
    Ok(CoordinationSetActiveProjectTeamResult {})
}

#[cfg(test)]
mod tests {
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    use chrono::{DateTime, Utc};
    use tempfile::TempDir;

    use super::*;
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::domain::{Member, MemberRole};
    use crate::coordination::stores::{
        OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
        OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot,
    };
    use crate::daemon::protocol::CoordinationOperationalSnapshotPublication;
    use crate::session_scanner::cli_tool::CliTool;

    fn save_team(teams_dir: &Path) {
        TeamConfigStore::save(
            teams_dir,
            "architecture-final",
            &TeamConfig {
                schema_version: 1,
                name: "architecture-final".to_string(),
                description: None,
                created_at: Utc::now(),
                members: vec![Member {
                    name: "builder".to_string(),
                    role: MemberRole::Agent,
                    role_id: None,
                    role_name: None,
                    focus_area: None,
                    context_summary: None,
                    behavior_summary: None,
                    communication_style: None,
                    runtime_compact_summary: None,
                    instructions: None,
                    behavioral_contract: None,
                    quality_gates: None,
                    handoff_expectations: None,
                    definition_of_done: None,
                    phase_scope: None,
                    mode: None,
                    inherits_from: None,
                    required_artifacts: None,
                    capabilities: None,
                    model: None,
                    reasoning_effort: None,
                    project_path: "/work/taurhaus".into(),
                    cli_tool: CliTool::Codex,
                    extra: Default::default(),
                }],
                extra: Default::default(),
            },
        )
        .expect("save team");
    }

    // Regression: d593f81b counted a snapshot dropped by the newer-wins guard
    // as published, so the protocol result reported attempts instead of writes.
    #[test]
    fn snapshot_intent_reuses_the_newer_wins_publication_guard() {
        let teams = TempDir::new().expect("teams");
        save_team(teams.path());
        let newer_at = DateTime::parse_from_rfc3339("2026-09-02T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let older_at = DateTime::parse_from_rfc3339("2026-09-02T11:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let newer = OperationalContextSnapshot {
            version: 1,
            team_name: "architecture-final".to_string(),
            member_name: "builder".to_string(),
            updated_at: newer_at,
            task: OperationalTaskSnapshot {
                id: "newer".to_string(),
                ..Default::default()
            },
            assignment_footer: OperationalAssignmentFooterSnapshot::default(),
            ownership: OperationalOwnershipSnapshot::default(),
            working_set: OperationalWorkingSetSnapshot {
                project_path: "/work/taurhaus".to_string(),
                focal_files: Vec::new(),
            },
        };
        OperationalContextSnapshotStore::save(teams.path(), &newer).expect("seed newer");
        let mut older = newer.clone();
        older.updated_at = older_at;
        older.task.id = "older".to_string();

        let result = publish_operational_snapshots(
            teams.path(),
            CoordinationPublishOperationalSnapshotsParams {
                publications: vec![CoordinationOperationalSnapshotPublication {
                    snapshot: older,
                    task_state_changed_at: None,
                }],
            },
        )
        .expect("publish intent");

        assert_eq!(result.published, 0);
        assert_eq!(result.skipped, 1);
        let stored =
            OperationalContextSnapshotStore::load(teams.path(), "architecture-final", "builder")
                .expect("load")
                .expect("snapshot");
        assert_eq!(stored.updated_at, newer_at);
        assert_eq!(stored.task.id, "newer");
    }

    // Regression: d593f81b made one unreadable team config abort the entire
    // protocol-22 publication batch instead of preserving the tolerant scan path.
    #[test]
    fn snapshot_intent_skips_unreadable_team_config_and_continues_batch() {
        let teams = TempDir::new().expect("teams");
        save_team(teams.path());
        let snapshot = OperationalContextSnapshot {
            version: 1,
            team_name: "architecture-final".to_string(),
            member_name: "builder".to_string(),
            updated_at: Utc::now(),
            task: OperationalTaskSnapshot {
                id: "publish-me".to_string(),
                ..Default::default()
            },
            assignment_footer: OperationalAssignmentFooterSnapshot::default(),
            ownership: OperationalOwnershipSnapshot::default(),
            working_set: OperationalWorkingSetSnapshot {
                project_path: "/work/taurhaus".to_string(),
                focal_files: Vec::new(),
            },
        };
        let mut missing_team_snapshot = snapshot.clone();
        missing_team_snapshot.team_name = "missing-team".to_string();

        let result = publish_operational_snapshots(
            teams.path(),
            CoordinationPublishOperationalSnapshotsParams {
                publications: vec![
                    CoordinationOperationalSnapshotPublication {
                        snapshot: missing_team_snapshot,
                        task_state_changed_at: None,
                    },
                    CoordinationOperationalSnapshotPublication {
                        snapshot,
                        task_state_changed_at: None,
                    },
                ],
            },
        )
        .expect("publish valid entries despite unreadable team config");

        assert_eq!(result.published, 1);
        assert_eq!(result.skipped, 1);
        assert!(OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "builder"
        )
        .expect("load published snapshot")
        .is_some());
    }

    #[test]
    fn active_project_intent_sets_and_clears_the_mapping() {
        let teams = TempDir::new().expect("teams");
        set_active_project_team(
            teams.path(),
            CoordinationSetActiveProjectTeamParams {
                project_path: "/work/taurhaus".to_string(),
                team_name: Some("architecture-final".to_string()),
            },
        )
        .expect("set mapping");
        assert_eq!(
            ActiveProjectTeamStore::load_active_team(teams.path(), "/work/taurhaus")
                .expect("load mapping")
                .as_deref(),
            Some("architecture-final")
        );

        set_active_project_team(
            teams.path(),
            CoordinationSetActiveProjectTeamParams {
                project_path: "/work/taurhaus".to_string(),
                team_name: None,
            },
        )
        .expect("clear mapping");
        assert_eq!(
            ActiveProjectTeamStore::load_active_team(teams.path(), "/work/taurhaus")
                .expect("load cleared mapping"),
            None
        );
    }

    // Regression: d593f81b routed the two-second live-status poll through the
    // process-wide orchestrator mutex with a blocking lock, so a long team
    // mutation made the reconcile RPC time out and disconnect the daemon pool.
    #[test]
    fn live_presence_reconcile_skips_without_waiting_for_a_busy_orchestrator() {
        let teams = TempDir::new().expect("teams");
        let state = Arc::new(CoordinationState::with_components(
            teams.path().to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
        ));
        let (locked_tx, locked_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let holder_state = state.clone();
        let holder = std::thread::spawn(move || {
            holder_state.with_orchestrator(|_| {
                locked_tx.send(()).expect("announce held lock");
                release_rx.recv().expect("release held lock");
                Ok(())
            })
        });
        locked_rx.recv().expect("orchestrator lock held");

        let (result_tx, result_rx) = mpsc::channel();
        let reconcile_state = state.clone();
        let reconcile = std::thread::spawn(move || {
            let result = reconcile_live_presence(
                &reconcile_state,
                CoordinationReconcileLivePresenceParams {
                    team_name: "architecture-final".to_string(),
                    runtime_sessions: Vec::new(),
                },
            );
            result_tx.send(result).expect("send reconcile result");
        });

        let prompt_result = result_rx.recv_timeout(Duration::from_millis(100));
        release_tx.send(()).expect("release orchestrator lock");
        holder
            .join()
            .expect("lock holder thread")
            .expect("lock holder operation");
        reconcile.join().expect("reconcile thread");

        let result = prompt_result
            .expect("busy live-presence reconciliation must return without waiting")
            .expect("busy reconciliation is a successful skip");
        assert_eq!(
            result.outcome,
            CoordinationReconcileLivePresenceOutcome::Skipped
        );
        assert!(result.reconciled_offline_members.is_empty());
    }
}
