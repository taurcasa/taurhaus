use std::path::Path;

use chrono::Utc;
use rusqlite::Connection;

use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::OperationalContextUpdate;
use crate::coordination::stores::{
    OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
    OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
    OperationalWorkingSetSnapshot, TeamConfigStore,
};

pub fn sync_team_snapshots(
    teams_dir: &Path,
    conn: &Connection,
    team_name: &str,
) -> Result<(), CoordinationError> {
    let config = TeamConfigStore::load(teams_dir, team_name)?;
    for member in &config.members {
        sync_member_snapshot(teams_dir, conn, team_name, &member.name)?;
    }
    Ok(())
}

pub fn sync_member_snapshot(
    teams_dir: &Path,
    conn: &Connection,
    team_name: &str,
    member_name: &str,
) -> Result<(), CoordinationError> {
    let config = TeamConfigStore::load(teams_dir, team_name)?;
    let member = config
        .members
        .iter()
        .find(|member| member.name == member_name)
        .ok_or_else(|| {
            CoordinationError::NotFound(format!(
                "member '{member_name}' not found in team '{team_name}'"
            ))
        })?;
    let existing = OperationalContextSnapshotStore::load(teams_dir, team_name, member_name)?;
    let task = latest_owned_task(
        conn,
        &member.project_path.display().to_string(),
        member_name,
    )?;

    let snapshot = OperationalContextSnapshot {
        version: existing.as_ref().map_or(1, |snapshot| snapshot.version),
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
        updated_at: Utc::now(),
        task,
        assignment_footer: existing
            .as_ref()
            .map(|snapshot| snapshot.assignment_footer.clone())
            .unwrap_or_default(),
        ownership: existing
            .as_ref()
            .map(|snapshot| snapshot.ownership.clone())
            .unwrap_or_default(),
        working_set: existing
            .as_ref()
            .map(|snapshot| {
                let mut working_set = snapshot.working_set.clone();
                if working_set.project_path.trim().is_empty() {
                    working_set.project_path = member.project_path.display().to_string();
                }
                working_set
            })
            .unwrap_or_else(|| OperationalWorkingSetSnapshot {
                project_path: member.project_path.display().to_string(),
                focal_files: Vec::new(),
            }),
    };

    OperationalContextSnapshotStore::save(teams_dir, &snapshot)
}

pub fn sync_project_task_snapshots(
    teams_dir: &Path,
    conn: &Connection,
    project_path: &str,
) -> Result<(), CoordinationError> {
    for team_name in TeamConfigStore::list(teams_dir)? {
        let config = match TeamConfigStore::load(teams_dir, &team_name) {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    error = %err,
                    "skipping operational snapshot project sync for unreadable team config"
                );
                continue;
            }
        };

        for member in config
            .members
            .iter()
            .filter(|member| member.project_path == std::path::PathBuf::from(project_path))
        {
            sync_member_snapshot(teams_dir, conn, &team_name, &member.name)?;
        }
    }
    Ok(())
}

pub fn apply_delivery_context(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    context: &OperationalContextUpdate,
) -> Result<(), CoordinationError> {
    let config = TeamConfigStore::load(teams_dir, team_name)?;
    let member = config
        .members
        .iter()
        .find(|member| member.name == member_name)
        .ok_or_else(|| {
            CoordinationError::NotFound(format!(
                "member '{member_name}' not found in team '{team_name}'"
            ))
        })?;
    let existing = OperationalContextSnapshotStore::load(teams_dir, team_name, member_name)?;
    let snapshot = OperationalContextSnapshot {
        version: existing.as_ref().map_or(1, |snapshot| snapshot.version),
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
        updated_at: Utc::now(),
        task: context
            .task
            .as_ref()
            .map(|task| OperationalTaskSnapshot {
                id: task.id.clone(),
                subject: task.subject.clone(),
                status: task.status.clone(),
            })
            .or_else(|| existing.as_ref().map(|snapshot| snapshot.task.clone()))
            .unwrap_or_default(),
        assignment_footer: context
            .assignment_footer
            .as_ref()
            .map(|footer| OperationalAssignmentFooterSnapshot {
                execution_mode: footer.execution_mode.clone(),
                file_ownership_boundary: footer.file_ownership_boundary.clone(),
                adjacent_fix_policy: footer.adjacent_fix_policy.clone(),
                validation_expectation: footer.validation_expectation.clone(),
                response_expectation: footer.response_expectation.clone(),
            })
            .or_else(|| {
                existing
                    .as_ref()
                    .map(|snapshot| snapshot.assignment_footer.clone())
            })
            .unwrap_or_default(),
        ownership: context
            .ownership
            .as_ref()
            .map(|ownership| OperationalOwnershipSnapshot {
                override_allowed: ownership.override_allowed,
                active_override_reason: ownership.active_override_reason.clone(),
            })
            .or_else(|| existing.as_ref().map(|snapshot| snapshot.ownership.clone()))
            .unwrap_or_default(),
        working_set: context
            .working_set
            .as_ref()
            .map(|working_set| OperationalWorkingSetSnapshot {
                project_path: if working_set.project_path.trim().is_empty() {
                    member.project_path.display().to_string()
                } else {
                    working_set.project_path.clone()
                },
                focal_files: working_set.focal_files.clone(),
            })
            .or_else(|| {
                existing
                    .as_ref()
                    .map(|snapshot| snapshot.working_set.clone())
            })
            .unwrap_or_else(|| OperationalWorkingSetSnapshot {
                project_path: member.project_path.display().to_string(),
                focal_files: Vec::new(),
            }),
    };

    OperationalContextSnapshotStore::save(teams_dir, &snapshot)
}

fn latest_owned_task(
    conn: &Connection,
    project_path: &str,
    member_name: &str,
) -> Result<OperationalTaskSnapshot, CoordinationError> {
    let tasks = taurhaus_lib::db::task_queries::get_tasks_for_project(conn, project_path)
        .map_err(|err| CoordinationError::StoreError(err.to_string()))?;

    let task = tasks
        .into_iter()
        .filter(|task| task.owner.as_deref() == Some(member_name))
        .max_by(|left, right| {
            task_priority(left)
                .cmp(&task_priority(right))
                .then_with(|| left.updated_at.cmp(&right.updated_at))
                .then_with(|| left.state_changed_at.cmp(&right.state_changed_at))
        });

    Ok(task
        .map(|task| OperationalTaskSnapshot {
            id: task.source_task_id,
            subject: task.subject,
            status: task.status,
        })
        .unwrap_or_default())
}

fn task_priority(task: &taurhaus_lib::db::task_queries::PersistedTask) -> u8 {
    match task.status.as_str() {
        "in_progress" => 3,
        "pending" => 2,
        "completed" => 1,
        _ => 0,
    }
}

impl Default for OperationalTaskSnapshot {
    fn default() -> Self {
        Self {
            id: String::new(),
            subject: String::new(),
            status: String::new(),
        }
    }
}

impl Default for OperationalAssignmentFooterSnapshot {
    fn default() -> Self {
        Self {
            execution_mode: String::new(),
            file_ownership_boundary: Vec::new(),
            adjacent_fix_policy: String::new(),
            validation_expectation: String::new(),
            response_expectation: String::new(),
        }
    }
}

impl Default for OperationalOwnershipSnapshot {
    fn default() -> Self {
        Self {
            override_allowed: false,
            active_override_reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::{NamedTempFile, TempDir};

    use crate::coordination::domain::{Member, MemberRole};
    use crate::coordination::stores::TeamConfig;
    use crate::session_scanner::cli_tool::CliTool;

    fn test_db() -> (Connection, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("temp db");
        let conn = taurhaus_lib::db::init_db(tmp.path()).expect("init db");
        (conn, tmp)
    }

    fn write_team(teams_dir: &Path) {
        let config = TeamConfig {
            schema_version: 1,
            name: "architecture-final".to_string(),
            description: None,
            created_at: Utc::now(),
            members: vec![Member {
                name: "frontend-dev".to_string(),
                role: MemberRole::Agent,
                role_id: Some("codex-dev".to_string()),
                role_name: Some("Codex Dev".to_string()),
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                instructions: None,
                behavioral_contract: None,
                capabilities: None,
                project_path: "proj-web".into(),
                cli_tool: CliTool::Codex,
            }],
        };

        TeamConfigStore::save(teams_dir, "architecture-final", &config).expect("save team");
    }

    #[test]
    fn sync_member_snapshot_uses_latest_owned_task() {
        let teams = TempDir::new().expect("teams dir");
        let (conn, _db) = test_db();
        write_team(teams.path());

        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &taurhaus_lib::db::task_queries::PersistedTask {
                project_path: "proj-web".to_string(),
                source: "claude".to_string(),
                source_key: "session-1".to_string(),
                source_task_id: "42".to_string(),
                subject: "Fix regression".to_string(),
                description: None,
                active_form: None,
                status: "in_progress".to_string(),
                blocks: vec![],
                blocked_by: vec![],
                owner: Some("frontend-dev".to_string()),
                session_id: None,
                first_seen_at: "2026-03-08T12:00:00Z".to_string(),
                state_changed_at: Some("2026-03-08T12:00:00Z".to_string()),
                updated_at: "2026-03-08T12:00:00Z".to_string(),
                archived_at: None,
                last_status: Some("in_progress".to_string()),
                archived_reason: None,
            },
        )
        .expect("upsert task");

        sync_member_snapshot(teams.path(), &conn, "architecture-final", "frontend-dev")
            .expect("sync snapshot");

        let snapshot = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");

        assert_eq!(snapshot.task.id, "42");
        assert_eq!(snapshot.task.subject, "Fix regression");
        assert_eq!(snapshot.task.status, "in_progress");
        assert_eq!(snapshot.working_set.project_path, "proj-web");
    }

    #[test]
    fn apply_delivery_context_updates_structured_footer_without_parsing_message() {
        let teams = TempDir::new().expect("teams dir");
        write_team(teams.path());

        apply_delivery_context(
            teams.path(),
            "architecture-final",
            "frontend-dev",
            &OperationalContextUpdate {
                task: Some(crate::coordination::requests::OperationalTaskContext {
                    id: "675".to_string(),
                    subject: "Wire snapshot updates".to_string(),
                    status: "in_progress".to_string(),
                }),
                assignment_footer: Some(
                    crate::coordination::requests::OperationalAssignmentFooter {
                        execution_mode: "implement".to_string(),
                        file_ownership_boundary: vec![
                            "src-tauri/src/coordination/operational_context.rs".to_string(),
                        ],
                        adjacent_fix_policy: "no".to_string(),
                        validation_expectation: "cargo check --tests".to_string(),
                        response_expectation: "report-on-completion".to_string(),
                    },
                ),
                ownership: Some(crate::coordination::requests::OperationalOwnershipContext {
                    override_allowed: false,
                    active_override_reason: None,
                }),
                working_set: Some(
                    crate::coordination::requests::OperationalWorkingSetContext {
                        project_path: "proj-web".to_string(),
                        focal_files: vec![
                            "src-tauri/src/coordination/operational_context.rs".to_string()
                        ],
                    },
                ),
            },
        )
        .expect("apply context");

        let snapshot = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");

        assert_eq!(snapshot.task.id, "675");
        assert_eq!(snapshot.assignment_footer.execution_mode, "implement");
        assert_eq!(
            snapshot.assignment_footer.file_ownership_boundary,
            vec!["src-tauri/src/coordination/operational_context.rs".to_string()]
        );
        assert_eq!(
            snapshot.assignment_footer.validation_expectation,
            "cargo check --tests"
        );
        assert_eq!(snapshot.working_set.project_path, "proj-web");
    }
}
