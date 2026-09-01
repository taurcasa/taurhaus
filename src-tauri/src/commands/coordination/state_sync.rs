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
    sync_member_snapshot(state.teams_dir(), &conn, team_name, member_name)
}

pub(super) fn sync_active_team_projects_after_change(
    state: &CoordinationState,
    team_name: &str,
) -> Result<(), CoordinationError> {
    crate::coordination::stores::active_project::sync_team_from_config(state.teams_dir(), team_name)
}
