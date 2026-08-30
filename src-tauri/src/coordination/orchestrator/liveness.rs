use std::collections::{HashMap, HashSet};

use chrono::Utc;

use crate::coordination::domain::HealthState;
use crate::coordination::errors::CoordinationError;
use crate::coordination::runtime::{
    pane_belongs_to_member, quarantine_foreign_member, LivePane, PaneOwnership,
};
use crate::coordination::stores::lock::acquire_team_lock;
use crate::coordination::stores::{
    MemberRuntimeSnapshot, MemberRuntimeStore, RuntimeCommitOutcome, TeamConfigStore,
};
use crate::coordination::validation::validate_team_name;
use crate::session_scanner::cli_tool::spec;

use super::helpers::{team_is_self_heal_candidate, team_should_ensure_daemon};
use super::{CoordinationOrchestrator, TeamSelfHealResult};

impl CoordinationOrchestrator {
    /// Reconcile only pane-backed presence drift for live-status reads.
    ///
    /// This keeps UI polling on a cheap path: it updates members that have
    /// clearly gone offline (missing/dead/shell pane) without performing the
    /// heavier daemon discovery/restart work from full liveness reconciliation.
    pub fn reconcile_team_presence_for_live_status(
        &mut self,
        team_name: &str,
    ) -> Result<(), CoordinationError> {
        self.reconcile_team_presence_for_live_status_with_runtime_sessions(team_name, &[])?;
        Ok(())
    }

    /// Reconcile pane-backed presence drift for live-status reads, including
    /// daemon runtime snapshots that may still advertise stale members.
    pub fn reconcile_team_presence_for_live_status_with_runtime_sessions(
        &mut self,
        team_name: &str,
        runtime_sessions: &[crate::session_scanner::RuntimeSession],
    ) -> Result<HashSet<String>, CoordinationError> {
        validate_team_name(team_name)?;
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let members_by_name = config
            .members
            .into_iter()
            .map(|member| (member.name.clone(), member))
            .collect::<HashMap<_, _>>();
        let runtime_records = MemberRuntimeStore::load_all(&self.teams_dir, team_name)?;
        let runtime_sessions_by_member = runtime_sessions
            .iter()
            .filter(|session| session.group_id.as_deref() == Some(team_name))
            .filter_map(|session| {
                session
                    .member_name
                    .as_ref()
                    .map(|member_name| (member_name.clone(), session))
            })
            .collect::<HashMap<_, _>>();
        let mut reconciled_members = HashSet::new();

        for (member_name, mut runtime) in runtime_records {
            let expected = MemberRuntimeSnapshot::capture(&runtime);
            let Some(member) = members_by_name.get(&member_name) else {
                continue;
            };

            let snapshot_pane_id = runtime_sessions_by_member
                .get(&member_name)
                .and_then(|session| session.tmux_pane.clone());
            let pane_id = runtime.pane_id.clone().or(snapshot_pane_id.clone());

            let mut foreign_reason = None;
            let mut foreign_live_pane: Option<LivePane> = None;
            let offline_detected = match pane_id.as_deref() {
                None => true,
                Some(pane_id) => match self.runtime.live_pane(pane_id)? {
                    None => true,
                    Some(live_pane) if live_pane.is_dead || live_pane.is_shell() => true,
                    Some(live_pane) => {
                        let mut ownership_record = runtime.clone();
                        ownership_record.cli_tool.get_or_insert(member.cli_tool);
                        ownership_record
                            .project_path
                            .get_or_insert_with(|| member.project_path.clone());
                        if ownership_record.pane_id.is_none() {
                            ownership_record.pane_id = Some(pane_id.to_string());
                        }
                        match pane_belongs_to_member(&ownership_record, &live_pane) {
                            PaneOwnership::Owned => false,
                            PaneOwnership::Foreign { reason } => {
                                foreign_reason = Some(reason);
                                foreign_live_pane = Some(live_pane);
                                true
                            }
                        }
                    }
                },
            };

            if let (Some(reason), Some(live_pane)) =
                (foreign_reason.as_deref(), foreign_live_pane.as_ref())
            {
                if quarantine_foreign_member(
                    &self.teams_dir,
                    self.runtime.as_ref(),
                    team_name,
                    &member_name,
                    &runtime,
                    live_pane,
                    reason,
                )? {
                    reconciled_members.insert(member_name);
                }
                continue;
            }

            if !offline_detected || runtime.health == HealthState::SessionDead {
                continue;
            }

            runtime.health = HealthState::SessionDead;
            runtime.session_id = None;
            runtime.jsonl_path = None;
            if runtime.pane_id.is_none() {
                runtime.pane_id = snapshot_pane_id;
            }

            let mut daemon_pid_to_terminate = None;
            if !spec(member.cli_tool).capabilities.native_inbox_poller {
                if let Some(pid) = runtime.daemon_pid {
                    match self.runtime.is_process_running_by_pid(pid) {
                        Ok(true) => daemon_pid_to_terminate = Some(pid),
                        Ok(false) => {}
                        Err(err) => {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pid = pid,
                                error = %err,
                                "failed to check daemon pid during live-status presence reconciliation"
                            );
                        }
                    }
                }
                runtime.daemon_pid = None;
            }
            let guard = acquire_team_lock(&self.teams_dir, team_name)?;
            let outcome = MemberRuntimeStore::commit_if_unchanged(
                &guard,
                &self.teams_dir,
                team_name,
                &member_name,
                &expected,
                |current| {
                    current.pane_id = runtime.pane_id.clone();
                    current.session_id = runtime.session_id.clone();
                    current.jsonl_path = runtime.jsonl_path.clone();
                    current.daemon_pid = runtime.daemon_pid;
                    current.health = runtime.health;
                },
            )?;
            drop(guard);
            if outcome == RuntimeCommitOutcome::Committed {
                if let Some(pid) = daemon_pid_to_terminate {
                    if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                        tracing::warn!(
                            team = %team_name,
                            member = %member_name,
                            pid = pid,
                            error = %err,
                            "failed to terminate stale daemon during live-status presence reconciliation"
                        );
                    }
                }
                reconciled_members.insert(member_name);
            }
        }

        Ok(reconciled_members)
    }

    /// Reconcile member liveness for a team using pane + daemon state.
    ///
    /// This is a write-on-drift repair pass for explicit recovery and background
    /// self-heal flows. It is intentionally not used on UI-critical snapshot paths.
    pub fn reconcile_team_liveness(&mut self, team_name: &str) -> Result<(), CoordinationError> {
        validate_team_name(team_name)?;
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let members_by_name = config
            .members
            .into_iter()
            .map(|member| (member.name.clone(), member))
            .collect::<HashMap<_, _>>();
        let runtime_records = MemberRuntimeStore::load_all(&self.teams_dir, team_name)?;

        for (member_name, mut runtime) in runtime_records {
            let expected = MemberRuntimeSnapshot::capture(&runtime);
            let Some(member) = members_by_name.get(&member_name) else {
                continue;
            };
            let mut metadata_backfilled = false;
            if runtime.cli_tool.is_none() {
                runtime.cli_tool = Some(member.cli_tool);
                metadata_backfilled = true;
            }
            if runtime.project_path.is_none() {
                runtime.project_path = Some(member.project_path.clone());
                metadata_backfilled = true;
            }

            let mut foreign_reason = None;
            let mut foreign_live_pane: Option<LivePane> = None;
            let (offline_detected, reason) = match runtime.pane_id.as_deref() {
                None => (true, "missing_pane_id"),
                Some(pane_id) => match self.runtime.live_pane(pane_id)? {
                    None => (true, "pane_missing"),
                    Some(live_pane) if live_pane.is_dead => (true, "pane_dead"),
                    Some(live_pane) if live_pane.is_shell() => (true, "pane_shell"),
                    Some(live_pane) => match pane_belongs_to_member(&runtime, &live_pane) {
                        PaneOwnership::Owned => (false, "pane_active"),
                        PaneOwnership::Foreign { reason } => {
                            foreign_reason = Some(reason);
                            foreign_live_pane = Some(live_pane);
                            (true, "pane_foreign")
                        }
                    },
                },
            };

            if offline_detected {
                if let (Some(foreign_reason), Some(live_pane)) =
                    (foreign_reason.as_deref(), foreign_live_pane.as_ref())
                {
                    quarantine_foreign_member(
                        &self.teams_dir,
                        self.runtime.as_ref(),
                        team_name,
                        &member_name,
                        &runtime,
                        live_pane,
                        foreign_reason,
                    )?;
                    tracing::info!(
                        team = %team_name,
                        member = %member_name,
                        reason,
                        "reconciled foreign member pane to offline"
                    );
                    continue;
                }
                if runtime.health == HealthState::SessionDead && !metadata_backfilled {
                    continue;
                }

                runtime.health = HealthState::SessionDead;
                runtime.session_id = None;
                runtime.jsonl_path = None;

                let mut daemon_pid_to_terminate = None;
                if !spec(member.cli_tool).capabilities.native_inbox_poller {
                    if let Some(pid) = runtime.daemon_pid {
                        match self.runtime.is_process_running_by_pid(pid) {
                            Ok(true) => daemon_pid_to_terminate = Some(pid),
                            Ok(false) => {}
                            Err(err) => {
                                tracing::warn!(
                                    team = %team_name,
                                    member = %member_name,
                                    pid = pid,
                                    error = %err,
                                    "failed to check daemon pid during liveness reconciliation"
                                );
                            }
                        }
                        runtime.daemon_pid = None;
                    }
                }
                let guard = acquire_team_lock(&self.teams_dir, team_name)?;
                let outcome = MemberRuntimeStore::commit_if_unchanged(
                    &guard,
                    &self.teams_dir,
                    team_name,
                    &member_name,
                    &expected,
                    |current| {
                        if current.cli_tool.is_none() {
                            current.cli_tool = runtime.cli_tool;
                        }
                        if current.project_path.is_none() {
                            current.project_path = runtime.project_path.clone();
                        }
                        current.session_id = runtime.session_id.clone();
                        current.jsonl_path = runtime.jsonl_path.clone();
                        current.daemon_pid = runtime.daemon_pid;
                        current.health = runtime.health;
                    },
                )?;
                drop(guard);
                if outcome == RuntimeCommitOutcome::Committed {
                    if let Some(pid) = daemon_pid_to_terminate {
                        if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pid = pid,
                                error = %err,
                                "failed to terminate stale daemon during liveness reconciliation"
                            );
                        }
                    }
                    tracing::info!(
                        team = %team_name,
                        member = %member_name,
                        reason,
                        "reconciled member liveness drift to offline"
                    );
                }
                continue;
            }

            let mut runtime_changed = metadata_backfilled;
            let mut spawned_daemon_pid = None;
            if !spec(member.cli_tool).capabilities.native_inbox_poller {
                let pane_id = runtime.pane_id.as_deref();
                let discovered_daemon_pids = if let Some(pane_id) = pane_id {
                    match self.runtime.find_existing_mesh_daemon_pids(
                        pane_id,
                        team_name,
                        &member_name,
                    ) {
                        Ok(pids) => pids,
                        Err(err) => {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pane_id = %pane_id,
                                error = %err,
                                "failed to discover existing mesh daemons during liveness reconciliation"
                            );
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                };

                let mut retained_daemon_pid = None;
                let daemon_needs_restart = match runtime.daemon_pid {
                    Some(pid) => match self.runtime.is_process_running_by_pid(pid) {
                        Ok(true) => match self.runtime.mesh_daemon_uses_current_binary(pid) {
                            Ok(true) => {
                                retained_daemon_pid = Some(pid);
                                false
                            }
                            Ok(false) => {
                                if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                                    tracing::warn!(
                                        team = %team_name,
                                        member = %member_name,
                                        pid = pid,
                                        error = %err,
                                        "failed to terminate binary-drifted mesh daemon during liveness reconciliation"
                                    );
                                }
                                runtime.daemon_pid = None;
                                runtime_changed = true;
                                tracing::info!(
                                    team = %team_name,
                                    member = %member_name,
                                    pid = pid,
                                    "detected running mesh daemon binary drift during liveness reconciliation"
                                );
                                true
                            }
                            Err(err) => {
                                tracing::warn!(
                                    team = %team_name,
                                    member = %member_name,
                                    pid = pid,
                                    error = %err,
                                    "failed to verify mesh daemon binary identity during liveness reconciliation"
                                );
                                false
                            }
                        },
                        Ok(false) => {
                            runtime.daemon_pid = None;
                            runtime_changed = true;
                            false
                        }
                        Err(err) => {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pid = pid,
                                error = %err,
                                "failed to verify daemon pid during liveness reconciliation"
                            );
                            false
                        }
                    },
                    None => false,
                };

                if retained_daemon_pid.is_none() && !discovered_daemon_pids.is_empty() {
                    retained_daemon_pid = discovered_daemon_pids.first().copied();
                    runtime.daemon_pid = retained_daemon_pid;
                    runtime_changed = true;
                    if let Some(pid) = retained_daemon_pid {
                        tracing::info!(
                            team = %team_name,
                            member = %member_name,
                            pane_id = %runtime.pane_id.as_deref().unwrap_or_default(),
                            pid = pid,
                            "adopted existing mesh daemon during liveness reconciliation"
                        );
                    }
                }

                if let Some(retained_pid) = retained_daemon_pid {
                    for duplicate_pid in discovered_daemon_pids
                        .into_iter()
                        .filter(|pid| *pid != retained_pid)
                    {
                        if let Err(err) = self.runtime.terminate_process_by_pid(duplicate_pid) {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pid = duplicate_pid,
                                retained_pid = retained_pid,
                                error = %err,
                                "failed to terminate duplicate mesh daemon during liveness reconciliation"
                            );
                        } else {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pid = duplicate_pid,
                                retained_pid = retained_pid,
                                "terminated duplicate mesh daemon during liveness reconciliation"
                            );
                        }
                    }
                } else if daemon_needs_restart || runtime.daemon_pid.is_none() {
                    if let Some(pane_id) = pane_id {
                        match self
                            .runtime
                            .spawn_mesh_daemon(pane_id, team_name, &member_name)
                        {
                            Ok(pid) => {
                                runtime.daemon_pid = Some(pid);
                                runtime_changed = true;
                                spawned_daemon_pid = Some(pid);
                                tracing::info!(
                                    team = %team_name,
                                    member = %member_name,
                                    pane_id = %pane_id,
                                    pid = pid,
                                    "restarted mesh daemon during liveness reconciliation"
                                );
                            }
                            Err(err) => {
                                tracing::warn!(
                                    team = %team_name,
                                    member = %member_name,
                                    pane_id = %pane_id,
                                    error = %err,
                                    "failed to restart mesh daemon during liveness reconciliation"
                                );
                            }
                        }
                    }
                }
            }

            if spec(member.cli_tool).capabilities.runtime_session_capture {
                if let Some(pane_id) = runtime.pane_id.as_deref() {
                    match self
                        .runtime
                        .detect_runtime_session(pane_id, member.cli_tool)
                    {
                        Ok(detected) => {
                            let next_session_id =
                                if detected.session_id.is_some() || runtime.session_id.is_none() {
                                    detected.session_id
                                } else {
                                    runtime.session_id.clone()
                                };
                            let session_id_changed = next_session_id != runtime.session_id;
                            let next_jsonl_path = if session_id_changed
                                || detected.jsonl_path.is_some()
                                || runtime.jsonl_path.is_none()
                            {
                                detected.jsonl_path
                            } else {
                                runtime.jsonl_path.clone()
                            };
                            if session_id_changed || next_jsonl_path != runtime.jsonl_path {
                                runtime.session_id = next_session_id;
                                runtime.jsonl_path = next_jsonl_path;
                                runtime_changed = true;
                            }
                        }
                        Err(err) => {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pane_id = %pane_id,
                                error = %err,
                                "failed to refresh runtime session metadata during liveness reconciliation"
                            );
                        }
                    }
                }
            }

            if runtime.health != HealthState::SessionDead && !runtime_changed {
                continue;
            }

            runtime.health = HealthState::Healthy;
            runtime.last_seen_at = Some(Utc::now());
            let guard = acquire_team_lock(&self.teams_dir, team_name)?;
            let outcome = MemberRuntimeStore::commit_if_unchanged(
                &guard,
                &self.teams_dir,
                team_name,
                &member_name,
                &expected,
                |current| {
                    if current.cli_tool.is_none() {
                        current.cli_tool = runtime.cli_tool;
                    }
                    if current.project_path.is_none() {
                        current.project_path = runtime.project_path.clone();
                    }
                    current.session_id = runtime.session_id.clone();
                    current.jsonl_path = runtime.jsonl_path.clone();
                    current.daemon_pid = runtime.daemon_pid;
                    current.health = runtime.health;
                    current.last_seen_at = runtime.last_seen_at;
                },
            )?;
            drop(guard);
            if outcome == RuntimeCommitOutcome::Committed {
                tracing::info!(
                    team = %team_name,
                    member = %member_name,
                    reason,
                    "reconciled member liveness drift to healthy"
                );
            } else if let Some(pid) = spawned_daemon_pid {
                // The record moved under this pass: the daemon it just spawned
                // was never committed, so terminate it rather than leave an
                // unrecorded duplicate for the next pass to find.
                match self.runtime.terminate_process_by_pid(pid) {
                    Ok(()) => tracing::info!(
                        team = %team_name,
                        member = %member_name,
                        pid = pid,
                        "rolled back a mesh daemon spawned under a skipped commit"
                    ),
                    Err(err) => tracing::warn!(
                        team = %team_name,
                        member = %member_name,
                        pid = pid,
                        error = %err,
                        "failed to roll back a mesh daemon spawned under a skipped commit"
                    ),
                }
            }
        }

        Ok(())
    }

    /// Run a bounded self-heal pass for a single team.
    ///
    /// This repairs per-member liveness drift and ensures the team daemon is
    /// running, but only when the persisted runtime indicates there is active or
    /// recoverable team state worth healing.
    pub fn trigger_team_self_heal(
        &mut self,
        team_name: &str,
    ) -> Result<TeamSelfHealResult, CoordinationError> {
        validate_team_name(team_name)?;

        let initial_status = self.get_team_status_fast(team_name)?;
        let team_daemon_binary_drifted =
            !self.runtime.team_daemon_uses_current_binary(team_name)?;
        let runtime_candidate_found = team_is_self_heal_candidate(&initial_status.members_runtime)
            || team_daemon_binary_drifted;
        if !runtime_candidate_found {
            return Ok(TeamSelfHealResult {
                team_name: team_name.to_string(),
                runtime_candidate_found: false,
                member_liveness_reconciled: false,
                team_daemon_ensured: false,
            });
        }

        self.reconcile_team_liveness(team_name)?;

        if team_daemon_binary_drifted {
            tracing::info!(
                team = %team_name,
                "detected running team daemon binary drift during self-heal"
            );
            self.stop_team_daemon_best_effort(team_name);
        }

        let refreshed_status = self.get_team_status_fast(team_name)?;
        let should_ensure_team_daemon = team_daemon_binary_drifted
            || team_should_ensure_daemon(&refreshed_status.members_runtime);
        let team_daemon_ensured =
            should_ensure_team_daemon && self.ensure_team_daemon_running_best_effort(team_name);

        Ok(TeamSelfHealResult {
            team_name: team_name.to_string(),
            runtime_candidate_found,
            member_liveness_reconciled: true,
            team_daemon_ensured,
        })
    }

    pub(super) fn reconcile_team_runtime_state(
        &self,
        team_name: &str,
    ) -> Result<(), CoordinationError> {
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let member_names = config
            .members
            .iter()
            .map(|member| member.name.clone())
            .collect::<HashSet<_>>();
        let runtime_records = MemberRuntimeStore::load_all(&self.teams_dir, team_name)?;

        for (member_name, mut runtime) in runtime_records {
            let expected = MemberRuntimeSnapshot::capture(&runtime);
            if !member_names.contains(&member_name) {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    "orphan runtime record found during startup reconciliation"
                );
                self.teardown_member_resources_best_effort(
                    team_name,
                    &member_name,
                    None,
                    Some(&runtime),
                );
                if let Err(err) =
                    MemberRuntimeStore::delete(&self.teams_dir, team_name, &member_name)
                {
                    tracing::warn!(
                        team = %team_name,
                        member = %member_name,
                        error = %err,
                        "failed to delete orphan runtime record during startup reconciliation"
                    );
                }
                continue;
            }

            let Some(pid) = runtime.daemon_pid else {
                continue;
            };

            match self.runtime.is_process_running_by_pid(pid) {
                Ok(true) => {}
                Ok(false) => {
                    runtime.daemon_pid = None;
                    runtime.health = HealthState::SessionDead;
                    let guard = acquire_team_lock(&self.teams_dir, team_name)?;
                    let outcome = MemberRuntimeStore::commit_if_unchanged(
                        &guard,
                        &self.teams_dir,
                        team_name,
                        &member_name,
                        &expected,
                        |current| {
                            current.daemon_pid = None;
                            current.health = HealthState::SessionDead;
                        },
                    )?;
                    drop(guard);
                    if outcome == RuntimeCommitOutcome::Committed {
                        tracing::info!(
                            team = %team_name,
                            member = %member_name,
                            pid = pid,
                            "cleared stale daemon pid during startup reconciliation"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        team = %team_name,
                        member = %member_name,
                        pid = pid,
                        error = %err,
                        "failed to verify daemon pid during startup reconciliation"
                    );
                }
            }
        }

        Ok(())
    }
}
