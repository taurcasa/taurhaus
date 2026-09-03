use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::commands::coordination_types::{
    AgentRole, FastAgentSnapshot, FastTeamSnapshot, LiveAgentStatus, LiveRuntimeSnapshotFreshness,
    LiveTeamStatus, ProjectMeshSnapshotResponse, SessionStatus, TeamRuntimeState,
};
use crate::commands::runtime_snapshot::{
    daemon_runtime_session_snapshot, RuntimeSnapshotFreshness,
};
use crate::coordination::backend::bridged::{
    availability_check, AvailabilityReport as BackendAvailabilityReport,
};
#[cfg(test)]
use crate::coordination::backend::bridged::{availability_check_with_lookup, BinaryLookup};
use crate::coordination::domain::{HealthState, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::roster::{
    get_team_roster_with_attachments, get_team_roster_with_runtime_sessions, TeamMemberView,
};
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::{
    ActiveProjectTeamStore, OperationalContextSnapshotStore, TeamConfig, TeamConfigStore,
};
#[cfg(not(test))]
use crate::ProviderState;
#[cfg(test)]
use taurhaus_lib::ProviderState;

pub(super) fn coordination_get_live_team_status_impl(
    state: &CoordinationState,
    provider: Option<&ProviderState>,
    team_name: String,
) -> Result<LiveTeamStatus, String> {
    if let Some(provider) = provider {
        let snapshot_outcome = daemon_runtime_session_snapshot(provider)?;
        if let Some(snapshot) = snapshot_outcome.snapshot {
            let reconciled_offline_members = reconcile_live_presence_through_daemon(
                state,
                provider,
                &team_name,
                snapshot.runtime_sessions.clone(),
            );
            let roster = get_team_roster_with_runtime_sessions(
                state.teams_dir(),
                &team_name,
                &snapshot.runtime_sessions,
            )
            .map_err(super::map_coordination_error)?;
            let roster = apply_reconciled_offline_members(
                state.teams_dir(),
                &team_name,
                roster,
                &reconciled_offline_members,
            )
            .map_err(super::map_coordination_error)?;
            let lead_name = roster_lead_name(&roster);
            let lead_project_path = roster_lead_project_path(&roster);
            let members = roster
                .into_iter()
                .map(|member| {
                    live_agent_status_from_roster(
                        member,
                        lead_project_path.as_deref(),
                        state.teams_dir(),
                        &team_name,
                    )
                })
                .collect();

            return Ok(LiveTeamStatus {
                team_name,
                lead_name,
                runtime_snapshot_freshness: match snapshot_outcome.freshness {
                    RuntimeSnapshotFreshness::Fresh => LiveRuntimeSnapshotFreshness::Fresh,
                    RuntimeSnapshotFreshness::Cached => LiveRuntimeSnapshotFreshness::Cached,
                    RuntimeSnapshotFreshness::Unavailable => {
                        LiveRuntimeSnapshotFreshness::AttachmentsOnly
                    }
                },
                members,
            });
        }
    }

    if let Some(provider) = provider {
        let _ = reconcile_live_presence_through_daemon(state, provider, &team_name, Vec::new());
    }
    let roster = get_team_roster_with_attachments(state.teams_dir(), &team_name)
        .map_err(super::map_coordination_error)?;
    let lead_name = roster_lead_name(&roster);
    let lead_project_path = roster_lead_project_path(&roster);
    let members = roster
        .into_iter()
        .map(|member| {
            live_agent_status_from_roster(
                member,
                lead_project_path.as_deref(),
                state.teams_dir(),
                &team_name,
            )
        })
        .collect();

    Ok(LiveTeamStatus {
        team_name,
        lead_name,
        runtime_snapshot_freshness: LiveRuntimeSnapshotFreshness::AttachmentsOnly,
        members,
    })
}

#[cfg(test)]
pub(super) fn coordination_get_live_team_status_for_tests(
    state: &CoordinationState,
    team_name: String,
) -> Result<LiveTeamStatus, String> {
    crate::daemon::state_writes::reconcile_live_presence(
        state,
        crate::daemon::protocol::CoordinationReconcileLivePresenceParams {
            team_name: team_name.clone(),
            runtime_sessions: Vec::new(),
        },
    )
    .map_err(super::map_coordination_error)?;
    coordination_get_live_team_status_impl(state, None, team_name)
}

pub(super) fn coordination_get_project_mesh_snapshot_impl(
    state: &CoordinationState,
    provider: Option<&ProviderState>,
    project_path: String,
) -> Result<ProjectMeshSnapshotResponse, String> {
    let availability = availability_check();
    coordination_get_project_mesh_snapshot_with_availability(
        state,
        provider,
        project_path,
        availability,
    )
}

#[cfg(test)]
pub(super) fn coordination_get_project_mesh_snapshot_with_lookup<L: BinaryLookup + ?Sized>(
    state: &CoordinationState,
    project_path: String,
    lookup: &L,
) -> Result<ProjectMeshSnapshotResponse, String> {
    let availability = availability_check_with_lookup(lookup);
    coordination_get_project_mesh_snapshot_with_availability(
        state,
        None,
        project_path,
        availability,
    )
}

pub(super) fn derive_cross_project_status(
    lead_project_path: &Path,
    member_project_path: &Path,
) -> CrossProjectStatus {
    let lead_project_path = canonical_project_identity(&lead_project_path.display().to_string());
    let member_project_path =
        canonical_project_identity(&member_project_path.display().to_string());
    let is_cross_project = lead_project_path != member_project_path;
    let project_label = if is_cross_project {
        project_label_from_path(&member_project_path)
    } else {
        String::new()
    };

    CrossProjectStatus {
        is_cross_project,
        project_label,
    }
}

fn apply_reconciled_offline_members(
    teams_dir: &Path,
    team_name: &str,
    roster: Vec<TeamMemberView>,
    reconciled_offline_members: &std::collections::HashSet<String>,
) -> Result<Vec<TeamMemberView>, CoordinationError> {
    if reconciled_offline_members.is_empty() {
        return Ok(roster);
    }

    let attachment_by_member = get_team_roster_with_attachments(teams_dir, team_name)?
        .into_iter()
        .map(|member| (member.member_name.clone(), member))
        .collect::<HashMap<_, _>>();

    Ok(roster
        .into_iter()
        .map(|member| {
            attachment_by_member
                .get(&member.member_name)
                .cloned()
                .filter(|_| reconciled_offline_members.contains(&member.member_name))
                .unwrap_or(member)
        })
        .collect())
}

fn coordination_get_project_mesh_snapshot_with_availability(
    state: &CoordinationState,
    provider: Option<&ProviderState>,
    project_path: String,
    availability: BackendAvailabilityReport,
) -> Result<ProjectMeshSnapshotResponse, String> {
    super::validate_non_empty("project_path", &project_path)?;
    let project_path = crate::provider::path::normalize_project_path(project_path.trim());
    let mut discovery = discover_team_for_project_path(state.teams_dir(), &project_path)
        .map_err(super::map_coordination_error)?;
    if let Some(team_name) = discovery.mapping_update.take() {
        if let Err(error) =
            set_active_project_team_through_daemon(provider, state, &project_path, team_name)
        {
            discovery.warnings.push(format!(
                "failed to persist active team mapping for project '{project_path}': {error}"
            ));
            discovery.warnings.sort();
        }
    }

    let team_status = if let Some(team_name) = discovery.team_name.as_deref() {
        Some(
            get_team_roster_with_attachments(state.teams_dir(), team_name)
                .map(|roster| map_fast_team_snapshot(roster, state.teams_dir(), team_name))
                .map_err(super::map_coordination_error)?,
        )
    } else {
        None
    };
    let team_runtime_state = classify_team_runtime_state(team_status.as_ref());

    Ok(ProjectMeshSnapshotResponse {
        mesh_available: availability.mesh_available,
        tmux_available: availability.tmux_available,
        team_runtime_state,
        team_name: discovery.team_name,
        team_status,
        warnings: discovery.warnings,
    })
}

fn session_status_from_health(health: HealthState) -> SessionStatus {
    match health {
        HealthState::Healthy => SessionStatus::Active,
        HealthState::AwaitingRead
        | HealthState::SuspectedStuck
        | HealthState::Rebriefed
        | HealthState::Suppressed => SessionStatus::Idle,
        HealthState::SessionDead => SessionStatus::Offline,
    }
}

fn classify_team_runtime_state(team_status: Option<&FastTeamSnapshot>) -> TeamRuntimeState {
    let Some(team_status) = team_status else {
        return TeamRuntimeState::None;
    };

    let live_members = team_status
        .members
        .iter()
        .filter(|member| member.session_status != SessionStatus::Offline)
        .count();
    let total_members = team_status.members.len();

    if live_members == 0 {
        TeamRuntimeState::ColdResume
    } else if live_members == total_members {
        TeamRuntimeState::Active
    } else {
        TeamRuntimeState::Degraded
    }
}

#[derive(Debug, Default)]
struct ProjectPathDiscovery {
    team_name: Option<String>,
    warnings: Vec<String>,
    mapping_update: Option<Option<String>>,
}

#[derive(Debug)]
struct ProjectDiscoveryCandidate {
    team_name: String,
    runtime_state: TeamRuntimeState,
    has_team_daemon_pid: bool,
    latest_activity_ms: i64,
    runtime_record_count: usize,
    created_at_ms: i64,
}

fn discover_team_for_project_path(
    teams_dir: &Path,
    project_path: &str,
) -> Result<ProjectPathDiscovery, CoordinationError> {
    if !teams_dir.exists() {
        return Ok(ProjectPathDiscovery::default());
    }

    let active_team_name = ActiveProjectTeamStore::load_active_team(teams_dir, project_path)?;
    let had_active_mapping = active_team_name.is_some();
    if let Some(active_team_name) = active_team_name {
        match TeamConfigStore::load(teams_dir, &active_team_name) {
            Ok(config) if config_references_project(&config, project_path) => {
                return Ok(ProjectPathDiscovery {
                    team_name: Some(config.name),
                    warnings: Vec::new(),
                    mapping_update: None,
                });
            }
            Ok(_) | Err(_) => {}
        }
    }

    let mut candidates = Vec::new();
    let mut warnings = Vec::new();
    for listed_team in TeamConfigStore::list(teams_dir)? {
        match TeamConfigStore::load(teams_dir, &listed_team) {
            Ok(config) => {
                if !config_references_project(&config, project_path) {
                    continue;
                }

                match build_project_discovery_candidate(teams_dir, &config) {
                    Ok(candidate) => candidates.push(candidate),
                    Err(err) => warnings.push(format!(
                        "skipped team folder '{listed_team}' due to candidate discovery error: {err}"
                    )),
                }
            }
            Err(CoordinationError::NotFound(_)) => {}
            Err(CoordinationError::StoreError(_)) => {
                warnings.push(format!(
                    "skipped team folder '{listed_team}' because config is missing or invalid"
                ));
            }
            Err(CoordinationError::Io(err)) => {
                warnings.push(format!(
                    "skipped team folder '{listed_team}' due to IO error: {err}"
                ));
            }
            Err(other) => {
                warnings.push(format!(
                    "skipped team folder '{listed_team}' due to discovery error: {other}"
                ));
            }
        }
    }

    let team_name = candidates
        .into_iter()
        .max_by(compare_project_discovery_candidates)
        .map(|candidate| candidate.team_name);
    let mapping_update = (had_active_mapping || team_name.is_some()).then(|| team_name.clone());
    warnings.sort();

    Ok(ProjectPathDiscovery {
        team_name,
        warnings,
        mapping_update,
    })
}

fn call_state_write<T: serde::de::DeserializeOwned, P: serde::Serialize>(
    provider: &ProviderState,
    method: &str,
    params: P,
) -> Result<T, String> {
    let daemon = provider
        .daemon
        .as_ref()
        .ok_or_else(|| "daemon is unavailable".to_string())?;
    if !daemon.is_connected() {
        if !daemon.try_reconnect() {
            return Err("daemon is not connected".to_string());
        }
        #[cfg(feature = "mesh-bridged-backend")]
        if let Err(error) =
            crate::commands::settings::repush_cached_launch_settings_to_daemon(daemon)
        {
            tracing::warn!(error = %error, "Failed to repush launch settings after live state-write reconnect");
        }
    }
    let request = crate::daemon::protocol::DaemonRequest::new(
        format!("live-state-{}", uuid::Uuid::new_v4().simple()),
        method,
        params,
    );
    let response = daemon
        .send_status_request_within(&request, super::COORDINATION_DAEMON_REQUEST_TIMEOUT)
        .map_err(|error| error.to_string())?;
    if let Some(error) = response.error {
        return Err(error.message);
    }
    response
        .result
        .ok_or_else(|| format!("daemon method '{method}' returned no result"))
        .and_then(|result| serde_json::from_value(result).map_err(|error| error.to_string()))
}

fn reconcile_live_presence_through_daemon(
    state: &CoordinationState,
    provider: &ProviderState,
    team_name: &str,
    runtime_sessions: Vec<crate::session_scanner::RuntimeSession>,
) -> std::collections::HashSet<String> {
    let result =
        call_state_write::<crate::daemon::protocol::CoordinationReconcileLivePresenceResult, _>(
            provider,
            crate::daemon::protocol::method::COORDINATION_RECONCILE_LIVE_PRESENCE,
            crate::daemon::protocol::CoordinationReconcileLivePresenceParams {
                team_name: team_name.to_string(),
                runtime_sessions,
            },
        );
    match result {
        Ok(result) => {
            state.mark_live_presence_recovered(team_name);
            match result.outcome {
                crate::daemon::protocol::CoordinationReconcileLivePresenceOutcome::Reconciled => {
                    result.reconciled_offline_members.into_iter().collect()
                }
                crate::daemon::protocol::CoordinationReconcileLivePresenceOutcome::Skipped => {
                    tracing::debug!(
                        team = team_name,
                        "live presence reconciliation skipped because the daemon orchestrator is busy"
                    );
                    std::collections::HashSet::new()
                }
            }
        }
        Err(error) => {
            if state.mark_live_presence_degraded(team_name) {
                tracing::warn!(team = team_name, error = %error, "live presence reconciliation skipped because the daemon is unavailable");
            }
            std::collections::HashSet::new()
        }
    }
}

fn set_active_project_team_through_daemon(
    provider: Option<&ProviderState>,
    _state: &CoordinationState,
    project_path: &str,
    team_name: Option<String>,
) -> Result<(), String> {
    #[cfg(test)]
    if provider.is_none() {
        return crate::daemon::state_writes::set_active_project_team(
            _state.teams_dir(),
            crate::daemon::protocol::CoordinationSetActiveProjectTeamParams {
                project_path: project_path.to_string(),
                team_name,
            },
        )
        .map(|_| ())
        .map_err(|error| error.to_string());
    }

    let provider = provider.ok_or_else(|| "daemon is unavailable".to_string())?;
    call_state_write::<crate::daemon::protocol::CoordinationSetActiveProjectTeamResult, _>(
        provider,
        crate::daemon::protocol::method::COORDINATION_SET_ACTIVE_PROJECT_TEAM,
        crate::daemon::protocol::CoordinationSetActiveProjectTeamParams {
            project_path: project_path.to_string(),
            team_name,
        },
    )
    .map(|_| ())
}

fn build_project_discovery_candidate(
    teams_dir: &Path,
    config: &TeamConfig,
) -> Result<ProjectDiscoveryCandidate, CoordinationError> {
    let roster = get_team_roster_with_attachments(teams_dir, &config.name)?;
    let latest_activity_ms = roster
        .iter()
        .filter_map(TeamMemberView::latest_runtime_activity)
        .map(|timestamp| timestamp.timestamp_millis())
        .max()
        .unwrap_or(i64::MIN);
    let runtime_record_count = roster
        .iter()
        .filter(|member| member.has_runtime_record)
        .count();
    let fast_snapshot = map_fast_team_snapshot(roster, teams_dir, &config.name);

    Ok(ProjectDiscoveryCandidate {
        team_name: config.name.clone(),
        runtime_state: classify_team_runtime_state(Some(&fast_snapshot)),
        has_team_daemon_pid: teams_dir
            .join(&config.name)
            .join("daemons")
            .join("team.pid")
            .is_file(),
        latest_activity_ms,
        runtime_record_count,
        created_at_ms: config.created_at.timestamp_millis(),
    })
}

fn compare_project_discovery_candidates(
    left: &ProjectDiscoveryCandidate,
    right: &ProjectDiscoveryCandidate,
) -> Ordering {
    (
        project_runtime_state_rank(left.runtime_state),
        left.has_team_daemon_pid,
        left.latest_activity_ms,
        left.runtime_record_count,
        left.created_at_ms,
        &left.team_name,
    )
        .cmp(&(
            project_runtime_state_rank(right.runtime_state),
            right.has_team_daemon_pid,
            right.latest_activity_ms,
            right.runtime_record_count,
            right.created_at_ms,
            &right.team_name,
        ))
}

fn project_runtime_state_rank(state: TeamRuntimeState) -> u8 {
    match state {
        TeamRuntimeState::Active => 3,
        TeamRuntimeState::Degraded => 2,
        TeamRuntimeState::ColdResume => 1,
        TeamRuntimeState::None => 0,
    }
}

fn config_references_project(
    config: &crate::coordination::stores::TeamConfig,
    project_path: &str,
) -> bool {
    config.members.iter().any(|member| {
        crate::provider::path::normalize_project_path(&member.project_path.display().to_string())
            == project_path
    })
}

fn map_fast_team_snapshot(
    roster: Vec<TeamMemberView>,
    teams_dir: &Path,
    team_name: &str,
) -> FastTeamSnapshot {
    let lead_name = roster_lead_name(&roster);
    let lead_project_path = roster_lead_project_path(&roster);
    let members = roster
        .into_iter()
        .map(|member| {
            fast_agent_snapshot_from_roster(
                member,
                lead_project_path.as_deref(),
                teams_dir,
                team_name,
            )
        })
        .collect();

    FastTeamSnapshot { lead_name, members }
}

fn roster_lead_name(roster: &[TeamMemberView]) -> String {
    roster
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .or_else(|| roster.first())
        .map(|member| member.member_name.clone())
        .unwrap_or_default()
}

fn roster_lead_project_path(roster: &[TeamMemberView]) -> Option<PathBuf> {
    roster
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .or_else(|| roster.first())
        .map(|member| member.configured_project_path.clone())
}

/// The workflow hint for one member.
///
/// A member running a workflow is a headless parent: the harness reports it
/// idle for the whole run, so the roster's own health says nothing about it.
/// This is the same hint, bounded by the same window, as the one the session
/// listing carries — a member with no attachment, or a harness with no workflow
/// runs, simply gets `None`.
///
/// The daemon computes it on the host that owns the transcript and ships it on
/// the runtime session, so a value that arrived that way is the answer. Reading
/// the transcript here is only the fallback for a roster joined from
/// attachments alone, and only for a path this process can actually open: on
/// Windows the daemon runs in WSL and the path it reports is a WSL path the
/// desktop cannot read.
pub(super) fn member_workflow_activity(
    member: &TeamMemberView,
) -> Option<crate::workflow_runs::WorkflowActivity> {
    if let Some(activity) = member.workflow_activity.clone() {
        return Some(activity);
    }

    let transcript = member.jsonl_path.as_deref()?;
    if !transcript.exists() {
        return None;
    }

    crate::workflow_runs::activity_for_transcript(
        member
            .attached_cli_tool
            .unwrap_or(member.configured_cli_tool),
        transcript.to_str(),
        SystemTime::now(),
    )
}

/// The effort the lead attached to this member's current assignment.
///
/// mesh writes it onto the assignment; the operational snapshot is where
/// taurhaus reads it back. Best-effort: a member with no snapshot yet, or one
/// whose assignment carried no effort, reports `None` and the node falls back
/// to showing only the launch effort.
fn member_task_effort(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
) -> (Option<String>, Option<String>) {
    let footer = match OperationalContextSnapshotStore::load(teams_dir, team_name, member_name) {
        Ok(Some(snapshot)) => snapshot.assignment_footer,
        Ok(None) => return (None, None),
        Err(err) => {
            tracing::warn!(
                team = %team_name,
                member = %member_name,
                error = %err,
                "failed to read the operational snapshot for a member's task effort"
            );
            return (None, None);
        }
    };
    (
        non_empty(&footer.task_effort),
        non_empty(&footer.task_effort_why),
    )
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn live_agent_status_from_roster(
    member: TeamMemberView,
    lead_project_path: Option<&Path>,
    teams_dir: &Path,
    team_name: &str,
) -> LiveAgentStatus {
    let cross_project =
        member_cross_project_status(lead_project_path, member.configured_project_path.as_path());
    let workflow_activity = member_workflow_activity(&member);
    let launch_account = member.launch_account.clone();
    let (task_effort, task_effort_why) =
        member_task_effort(teams_dir, team_name, &member.member_name);
    LiveAgentStatus {
        name: member.member_name,
        role: match member.role {
            MemberRole::Lead => AgentRole::Lead,
            MemberRole::Agent => AgentRole::Member,
        },
        cli_tool: member.configured_cli_tool.to_string(),
        model: member.model.unwrap_or_default(),
        reasoning_effort: member.reasoning_effort,
        role_id: member.role_id,
        role_name: member.role_name,
        focus_area: member.focus_area,
        context_summary: member.context_summary,
        behavior_summary: member.behavior_summary,
        project_id: member.configured_project_path.display().to_string(),
        is_cross_project: cross_project.is_cross_project,
        project_label: cross_project.project_label,
        description: member.instructions,
        session_status: member
            .attached_health
            .map(session_status_from_health)
            .unwrap_or(SessionStatus::Offline),
        pane_id: member.pane_id,
        session_id: member.session_id,
        workflow_activity,
        task_effort,
        task_effort_why,
        account_applied: launch_account.account_applied,
        account_note: launch_account.account_note,
        account_note_detail: launch_account.account_note_detail,
    }
}

fn fast_agent_snapshot_from_roster(
    member: TeamMemberView,
    lead_project_path: Option<&Path>,
    teams_dir: &Path,
    team_name: &str,
) -> FastAgentSnapshot {
    let cross_project =
        member_cross_project_status(lead_project_path, member.configured_project_path.as_path());
    let workflow_activity = member_workflow_activity(&member);
    let launch_account = member.launch_account.clone();
    let (task_effort, task_effort_why) =
        member_task_effort(teams_dir, team_name, &member.member_name);
    FastAgentSnapshot {
        name: member.member_name,
        role: match member.role {
            MemberRole::Lead => AgentRole::Lead,
            MemberRole::Agent => AgentRole::Member,
        },
        cli_tool: member.configured_cli_tool.to_string(),
        model: member.model,
        reasoning_effort: member.reasoning_effort,
        role_id: member.role_id,
        role_name: member.role_name,
        focus_area: member.focus_area,
        context_summary: member.context_summary,
        behavior_summary: member.behavior_summary,
        project_id: member.configured_project_path.display().to_string(),
        is_cross_project: cross_project.is_cross_project,
        project_label: cross_project.project_label,
        description: member.instructions,
        session_status: member
            .attached_health
            .map(session_status_from_health)
            .unwrap_or(SessionStatus::Offline),
        pane_id: member.pane_id,
        session_id: member.session_id,
        workflow_activity,
        task_effort,
        task_effort_why,
        account_applied: launch_account.account_applied,
        account_note: launch_account.account_note,
        account_note_detail: launch_account.account_note_detail,
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct CrossProjectStatus {
    pub(super) is_cross_project: bool,
    pub(super) project_label: String,
}

fn member_cross_project_status(
    lead_project_path: Option<&Path>,
    member_project_path: &Path,
) -> CrossProjectStatus {
    lead_project_path
        .map(|lead_project_path| {
            derive_cross_project_status(lead_project_path, member_project_path)
        })
        .unwrap_or_default()
}

fn canonical_project_identity(project_path: &str) -> String {
    let normalized = crate::provider::path::normalize_project_path(project_path);
    if is_windows_mount_path(&normalized) {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}

fn is_windows_mount_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 7
        && path.starts_with("/mnt/")
        && bytes[5].is_ascii_alphabetic()
        && (bytes[6] == b'/' || bytes[6] == b'\\')
}

fn project_label_from_path(project_path: &str) -> String {
    Path::new(project_path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            project_path
                .rsplit('/')
                .find(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_default()
}
