use crate::commands::projects::DbState;
use crate::coordination::errors::CoordinationError;
use crate::coordination::operational_context::sync_member_snapshot;
use crate::coordination::state::CoordinationState;

pub(super) fn sync_member_snapshot_after_change(
    state: &CoordinationState,
    db: &DbState,
    team_name: &str,
    member_name: &str,
) -> Result<(), CoordinationError> {
    let conn =
        db.0.lock()
            .map_err(|_| CoordinationError::StoreError("db mutex poisoned".to_string()))?;
    let teams_dir = state.team_teams_dir(team_name)?;
    sync_member_snapshot(&teams_dir, &conn, team_name, member_name)
}

pub(super) fn sync_active_team_projects_after_change(
    state: &CoordinationState,
    team_name: &str,
) -> Result<(), CoordinationError> {
    let teams_dir = state.team_teams_dir(team_name)?;
    crate::coordination::stores::active_project::sync_team_from_config(&teams_dir, team_name)
}
