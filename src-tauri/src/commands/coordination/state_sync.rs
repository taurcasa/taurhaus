use crate::commands::projects::DbState;
use crate::coordination::errors::CoordinationError;
use crate::coordination::operational_context::sync_member_snapshot;
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::{ActiveProjectTeamStore, TeamConfigStore};

pub(super) fn sync_member_snapshot_after_change(
    state: &CoordinationState,
    db: &DbState,
    team_name: &str,
    member_name: &str,
) -> Result<(), CoordinationError> {
    let conn =
        db.0.lock()
            .map_err(|_| CoordinationError::StoreError("db mutex poisoned".to_string()))?;
    sync_member_snapshot(state.teams_dir(), &conn, team_name, member_name)
}

pub(super) fn sync_active_team_projects_after_change(
    state: &CoordinationState,
    team_name: &str,
) -> Result<(), CoordinationError> {
    let config = TeamConfigStore::load(state.teams_dir(), team_name)?;
    let project_paths = config
        .members
        .iter()
        .map(|member| member.project_path.display().to_string())
        .collect::<Vec<_>>();
    ActiveProjectTeamStore::sync_team(state.teams_dir(), team_name, &project_paths)
}
