use std::cmp::Ordering;
use std::path::{Path, PathBuf};

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
use crate::coordination::stores::{ActiveProjectTeamStore, TeamConfig, TeamConfigStore};
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
            let roster = get_team_roster_with_runtime_sessions(
                state.teams_dir(),
                &team_name,
                &snapshot.runtime_sessions,
            )
            .map_err(super::map_coordination_error)?;
            let lead_name = roster_lead_name(&roster);
            let lead_project_path = roster_lead_project_path(&roster);
            let members = roster
                .into_iter()
                .map(|member| live_agent_status_from_roster(member, lead_project_path.as_deref()))
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

    state
        .with_orchestrator(|orchestrator| {
            orchestrator.reconcile_team_presence_for_live_status(&team_name)
        })
        .map_err(super::map_coordination_error)?;
    let roster = get_team_roster_with_attachments(state.teams_dir(), &team_name)
        .map_err(super::map_coordination_error)?;
    let lead_name = roster_lead_name(&roster);
    let lead_project_path = roster_lead_project_path(&roster);
    let members = roster
        .into_iter()
        .map(|member| live_agent_status_from_roster(member, lead_project_path.as_deref()))
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
    coordination_get_live_team_status_impl(state, None, team_name)
}

pub(super) fn coordination_get_project_mesh_snapshot_impl(
    state: &CoordinationState,
    project_path: String,
) -> Result<ProjectMeshSnapshotResponse, String> {
    let availability = availability_check();
    coordination_get_project_mesh_snapshot_with_availability(state, project_path, availability)
}

#[cfg(test)]
pub(super) fn coordination_get_project_mesh_snapshot_with_lookup<L: BinaryLookup + ?Sized>(
    state: &CoordinationState,
    project_path: String,
    lookup: &L,
) -> Result<ProjectMeshSnapshotResponse, String> {
    let availability = availability_check_with_lookup(lookup);
    coordination_get_project_mesh_snapshot_with_availability(state, project_path, availability)
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

fn coordination_get_project_mesh_snapshot_with_availability(
    state: &CoordinationState,
    project_path: String,
    availability: BackendAvailabilityReport,
) -> Result<ProjectMeshSnapshotResponse, String> {
    super::validate_non_empty("project_path", &project_path)?;
    let project_path = crate::provider::path::normalize_project_path(project_path.trim());
    let discovery = discover_team_for_project_path(state.teams_dir(), &project_path)
        .map_err(super::map_coordination_error)?;

    let team_status = if let Some(team_name) = discovery.team_name.as_deref() {
        Some(
            get_team_roster_with_attachments(state.teams_dir(), team_name)
                .map(map_fast_team_snapshot)
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

    if let Some(active_team_name) =
        ActiveProjectTeamStore::load_active_team(teams_dir, project_path)?
    {
        match TeamConfigStore::load(teams_dir, &active_team_name) {
            Ok(config) if config_references_project(&config, project_path) => {
                return Ok(ProjectPathDiscovery {
                    team_name: Some(config.name),
                    warnings: Vec::new(),
                });
            }
            Ok(_) | Err(_) => {
                let _ = ActiveProjectTeamStore::clear_project(teams_dir, project_path);
            }
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
    if let Some(selected_team_name) = team_name.as_deref() {
        if let Err(err) =
            ActiveProjectTeamStore::set_active_team(teams_dir, project_path, selected_team_name)
        {
            warnings.push(format!(
                "failed to persist active team mapping for project '{project_path}': {err}"
            ));
        }
    }
    warnings.sort();

    Ok(ProjectPathDiscovery {
        team_name,
        warnings,
    })
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
    let fast_snapshot = map_fast_team_snapshot(roster);

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

fn map_fast_team_snapshot(roster: Vec<TeamMemberView>) -> FastTeamSnapshot {
    let lead_name = roster_lead_name(&roster);
    let lead_project_path = roster_lead_project_path(&roster);
    let members = roster
        .into_iter()
        .map(|member| fast_agent_snapshot_from_roster(member, lead_project_path.as_deref()))
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

fn live_agent_status_from_roster(
    member: TeamMemberView,
    lead_project_path: Option<&Path>,
) -> LiveAgentStatus {
    let cross_project =
        member_cross_project_status(lead_project_path, member.configured_project_path.as_path());
    LiveAgentStatus {
        name: member.member_name,
        role: match member.role {
            MemberRole::Lead => AgentRole::Lead,
            MemberRole::Agent => AgentRole::Member,
        },
        cli_tool: member.configured_cli_tool.to_string(),
        model: String::new(),
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
    }
}

fn fast_agent_snapshot_from_roster(
    member: TeamMemberView,
    lead_project_path: Option<&Path>,
) -> FastAgentSnapshot {
    let cross_project =
        member_cross_project_status(lead_project_path, member.configured_project_path.as_path());
    FastAgentSnapshot {
        name: member.member_name,
        role: match member.role {
            MemberRole::Lead => AgentRole::Lead,
            MemberRole::Agent => AgentRole::Member,
        },
        cli_tool: member.configured_cli_tool.to_string(),
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
