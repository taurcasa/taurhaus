use std::collections::{HashMap, HashSet};

use chrono::{Duration, Utc};

use super::*;

pub(super) fn promote_activity_from_sessions(
    app: &tauri::AppHandle,
    db: &DbState,
    sessions: &[DisplaySession],
) {
    match promote_activity_from_sessions_impl(db, sessions) {
        Ok(promoted) if promoted > 0 => {
            enqueue_activity_watch_reconcile(app.clone(), "session_activity_detected");
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Failed to promote project activity from session scan"
            );
        }
    }
}

pub(super) fn promote_activity_from_sessions_impl(
    db: &DbState,
    sessions: &[DisplaySession],
) -> Result<usize, String> {
    let mut active_paths = HashSet::new();
    let mut unattributed_paths = HashSet::new();
    for session in sessions {
        let normalized_path = crate::provider::path::normalize_project_path(&session.project_path);
        if session.state == crate::session_scanner::SessionState::Active {
            active_paths.insert(normalized_path);
        } else if session.project_unattributed_active {
            unattributed_paths.insert(normalized_path);
        }
    }
    if active_paths.is_empty() && unattributed_paths.is_empty() {
        return Ok(0);
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let settings = crate::db::settings_queries::get_all_settings(&conn).sanitize_err()?;
    let projects =
        crate::services::project::list_projects(&conn, &settings.thresholds).sanitize_err()?;

    let now = Utc::now();
    let recent_floor = (now - Duration::days(settings.thresholds.active_days.max(1))).to_rfc3339();
    let mut by_path = HashMap::new();
    for project in projects {
        by_path.insert(
            crate::provider::path::normalize_project_path(&project.path),
            (project.id, project.activity_state),
        );
    }

    let mut promoted = 0usize;
    for path in &active_paths {
        let Some((project_id, state)) = by_path.get(path) else {
            continue;
        };
        if *state == crate::models::ActivityState::Active {
            continue;
        }
        crate::services::project::touch_activity(&conn, project_id).sanitize_err()?;
        promoted += 1;
    }

    for path in unattributed_paths {
        if active_paths.contains(&path) {
            continue;
        }

        let Some((project_id, state)) = by_path.get(&path) else {
            continue;
        };
        if matches!(
            state,
            crate::models::ActivityState::Active | crate::models::ActivityState::Recent
        ) {
            continue;
        }

        crate::db::queries::update_project(
            &conn,
            project_id,
            None,
            None,
            None,
            Some(Some(recent_floor.as_str())),
            None,
        )
        .sanitize_err()?;
        promoted += 1;
    }

    Ok(promoted)
}

pub(super) fn record_session_activity_impl(
    db: &DbState,
    project_id: String,
    cli_tool: CliTool,
    started_at: String,
    ended_at: String,
    active_duration_ms: i64,
    total_duration_ms: i64,
) -> Result<(), String> {
    let project_path = resolve_project_path(db, &project_id)?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let cli_tool = cli_tool.to_string();
    crate::db::activity_queries::insert_session_activity(
        &conn,
        &project_path,
        &cli_tool,
        &started_at,
        &ended_at,
        active_duration_ms,
        total_duration_ms,
    )
    .sanitize_err()
}

pub(super) fn get_project_activity_impl(
    db: &DbState,
    project_id: &str,
) -> Result<crate::db::activity_queries::ProjectActivityStats, String> {
    let project_path = resolve_project_path(db, project_id)?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::db::activity_queries::get_project_activity(&conn, &project_path).sanitize_err()
}
