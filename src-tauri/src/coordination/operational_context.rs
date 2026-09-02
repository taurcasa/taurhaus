use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::OperationalContextUpdate;
use crate::coordination::stores::{
    OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
    OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
    OperationalWorkingSetSnapshot, TeamConfigStore,
};
use crate::coordination::task_effort::AssignmentEffort;

type PreparedSnapshot = (OperationalContextSnapshot, Option<DateTime<Utc>>);

#[cfg(test)]
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

/// Build the app-DB-derived snapshots carried by a daemon initialization
/// intent. This performs no team-store writes; the daemon publishes the
/// snapshots only after the pipeline succeeds.
pub(crate) fn prepare_initialize_snapshots(
    conn: &Connection,
    request: &crate::coordination::requests::InitializeTeamRequest,
) -> Result<Vec<OperationalContextSnapshot>, CoordinationError> {
    std::iter::once(&request.lead)
        .chain(request.agents.iter())
        .map(|member| {
            let tasks = load_project_tasks(conn, &member.project_id)?;
            let (task, effort, task_state_changed_at) =
                latest_owned_task_from_tasks(&tasks, &member.name);
            Ok(build_member_snapshot(
                None,
                &request.team_name,
                &member.name,
                &member.project_id,
                task,
                effort,
                task_state_changed_at,
            ))
        })
        .collect()
}

/// Build the app-DB-derived snapshot carried by a daemon member-operation
/// intent. This performs no team-store writes.
#[cfg(test)]
pub(crate) fn prepare_member_snapshot(
    teams_dir: &Path,
    conn: &Connection,
    team_name: &str,
    member_name: &str,
    project_path: &str,
) -> Result<OperationalContextSnapshot, CoordinationError> {
    prepare_member_snapshot_with_task_timestamp(
        teams_dir,
        conn,
        team_name,
        member_name,
        project_path,
    )
    .map(|(snapshot, _)| snapshot)
}

pub(crate) fn prepare_member_snapshot_with_task_timestamp(
    teams_dir: &Path,
    conn: &Connection,
    team_name: &str,
    member_name: &str,
    project_path: &str,
) -> Result<(OperationalContextSnapshot, Option<DateTime<Utc>>), CoordinationError> {
    let existing = OperationalContextSnapshotStore::load(teams_dir, team_name, member_name)?;
    let tasks = load_project_tasks(conn, project_path)?;
    let (task, effort, task_state_changed_at) = latest_owned_task_from_tasks(&tasks, member_name);
    Ok((
        build_member_snapshot(
            existing.as_ref(),
            team_name,
            member_name,
            project_path,
            task,
            effort,
            task_state_changed_at,
        ),
        task_state_changed_at,
    ))
}

/// Publish a pre-pipeline initialize snapshot without replacing context that
/// was refreshed while the daemon pipeline was running.
pub(crate) fn publish_initialize_snapshot(
    teams_dir: &Path,
    snapshot: &OperationalContextSnapshot,
) -> Result<(), CoordinationError> {
    publish_snapshot(teams_dir, snapshot, None)
}

pub(crate) fn publish_member_operation_snapshot(
    teams_dir: &Path,
    snapshot: &OperationalContextSnapshot,
    task_state_changed_at: Option<DateTime<Utc>>,
) -> Result<(), CoordinationError> {
    publish_snapshot(teams_dir, snapshot, task_state_changed_at)
}

fn publish_snapshot(
    teams_dir: &Path,
    snapshot: &OperationalContextSnapshot,
    task_state_changed_at: Option<DateTime<Utc>>,
) -> Result<(), CoordinationError> {
    let guard =
        crate::coordination::stores::lock::acquire_team_lock(teams_dir, &snapshot.team_name)?;
    let current = OperationalContextSnapshotStore::load(
        teams_dir,
        &snapshot.team_name,
        &snapshot.member_name,
    )?;
    if current
        .as_ref()
        .is_some_and(|current| current.updated_at >= snapshot.updated_at)
    {
        return Ok(());
    }

    let mut candidate = snapshot.clone();
    if let Some(current) = current {
        candidate.version = current.version;
        candidate.task = preserve_task_deadline_markers(
            Some(&current.task),
            candidate.task,
            task_state_changed_at,
        );
        let task_effort = candidate.assignment_footer.task_effort.clone();
        let task_effort_why = candidate.assignment_footer.task_effort_why.clone();
        candidate.assignment_footer = current.assignment_footer;
        candidate.assignment_footer.task_effort = task_effort;
        candidate.assignment_footer.task_effort_why = task_effort_why;
        candidate.ownership = current.ownership;
        candidate.working_set = current.working_set;
        if candidate.working_set.project_path.trim().is_empty() {
            candidate.working_set.project_path = snapshot.working_set.project_path.clone();
        }
    }

    OperationalContextSnapshotStore::save_locked(&guard, teams_dir, &candidate)
}

#[cfg(test)]
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
    let project_path = member.project_path.display().to_string();
    let (snapshot, task_state_changed_at) = prepare_member_snapshot_with_task_timestamp(
        teams_dir,
        conn,
        team_name,
        member_name,
        &project_path,
    )?;

    save_snapshot_if_changed(teams_dir, snapshot, task_state_changed_at)
}

/// Prepare changed app-DB-derived snapshots for daemon publication.
///
/// Team config and current snapshots remain legal app-side reads. The returned
/// values are the complete write intent; this function never mutates team state.
pub(crate) fn prepare_project_task_snapshots(
    teams_dir: &Path,
    conn: &Connection,
    project_path: &str,
) -> Result<Vec<PreparedSnapshot>, CoordinationError> {
    let tasks = load_project_tasks(conn, project_path)?;
    let mut prepared = Vec::new();
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
            .filter(|member| member.project_path == Path::new(project_path))
        {
            let existing =
                OperationalContextSnapshotStore::load(teams_dir, &team_name, &member.name)?;
            let (task, effort, task_state_changed_at) =
                latest_owned_task_from_tasks(&tasks, &member.name);
            let snapshot = build_member_snapshot(
                existing.as_ref(),
                &team_name,
                &member.name,
                project_path,
                task,
                effort,
                task_state_changed_at,
            );
            let unchanged = existing.as_ref().is_some_and(|existing| {
                let mut candidate = snapshot.clone();
                candidate.updated_at = existing.updated_at;
                candidate == *existing
            });
            if !unchanged {
                prepared.push((snapshot, task_state_changed_at));
            }
        }
    }
    Ok(prepared)
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
            .map(|task| {
                preserve_task_deadline_markers(
                    existing.as_ref().map(|snapshot| &snapshot.task),
                    OperationalTaskSnapshot {
                        id: task.id.clone(),
                        subject: task.subject.clone(),
                        status: task.status.clone(),
                        ..Default::default()
                    },
                    None,
                )
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
                task_effort: footer.task_effort.clone(),
                task_effort_why: footer.task_effort_why.clone(),
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

    save_snapshot_if_changed(teams_dir, snapshot, None)
}

fn load_project_tasks(
    conn: &Connection,
    project_path: &str,
) -> Result<Vec<taurhaus_lib::db::task_queries::PersistedTask>, CoordinationError> {
    taurhaus_lib::db::task_queries::get_tasks_for_project(conn, project_path)
        .map_err(|err| CoordinationError::StoreError(err.to_string()))
}

/// The task a member is on, and the effort its lead attached to it.
///
/// The two travel together on purpose: an effort read from anywhere else —
/// the newest message in an inbox that keeps every assignment ever delivered —
/// would outlive the task it was asked for and pair one task with another
/// assignment's level.
fn latest_owned_task_from_tasks(
    tasks: &[taurhaus_lib::db::task_queries::PersistedTask],
    member_name: &str,
) -> (
    OperationalTaskSnapshot,
    Option<AssignmentEffort>,
    Option<DateTime<Utc>>,
) {
    let task = tasks
        .iter()
        .filter(|task| task.owner.as_deref() == Some(member_name))
        .filter(|task| is_resumable_task_status(&task.status))
        .max_by(|left, right| {
            task_priority(left)
                .cmp(&task_priority(right))
                .then_with(|| left.updated_at.cmp(&right.updated_at))
                .then_with(|| left.state_changed_at.cmp(&right.state_changed_at))
        });

    let effort = task.and_then(|task| {
        Some(AssignmentEffort {
            level: trimmed(task.effort.as_deref())?.to_ascii_lowercase(),
            why: trimmed(task.effort_why.as_deref()),
        })
    });
    let state_changed_at = task
        .and_then(|task| task.state_changed_at.as_deref())
        .and_then(|timestamp| DateTime::parse_from_rfc3339(timestamp).ok())
        .map(|timestamp| timestamp.with_timezone(&Utc));
    let snapshot = task
        .map(|task| OperationalTaskSnapshot {
            id: task.source_task_id.clone(),
            subject: task.subject.clone(),
            status: task.status.clone(),
            deadline_minutes: task.deadline_minutes,
            ..Default::default()
        })
        .unwrap_or_default();

    (snapshot, effort, state_changed_at)
}

fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn build_member_snapshot(
    existing: Option<&OperationalContextSnapshot>,
    team_name: &str,
    member_name: &str,
    project_path: &str,
    task: OperationalTaskSnapshot,
    effort: Option<AssignmentEffort>,
    task_state_changed_at: Option<DateTime<Utc>>,
) -> OperationalContextSnapshot {
    OperationalContextSnapshot {
        version: existing.map_or(1, |snapshot| snapshot.version),
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
        updated_at: Utc::now(),
        task: preserve_task_deadline_markers(
            existing.map(|snapshot| &snapshot.task),
            task,
            task_state_changed_at,
        ),
        assignment_footer: {
            let mut footer = existing
                .map(|snapshot| snapshot.assignment_footer.clone())
                .unwrap_or_default();
            // Written and cleared together with the task: a member with
            // nothing assigned has no task effort, and the level of a finished
            // assignment is not what it is working under now.
            let (level, why) = match effort {
                Some(effort) => (effort.level, effort.why.unwrap_or_default()),
                None => (String::new(), String::new()),
            };
            footer.task_effort = level;
            footer.task_effort_why = why;
            footer
        },
        ownership: existing
            .map(|snapshot| snapshot.ownership.clone())
            .unwrap_or_default(),
        working_set: existing
            .map(|snapshot| {
                let mut working_set = snapshot.working_set.clone();
                if working_set.project_path.trim().is_empty() {
                    working_set.project_path = project_path.to_string();
                }
                working_set
            })
            .unwrap_or_else(|| OperationalWorkingSetSnapshot {
                project_path: project_path.to_string(),
                focal_files: Vec::new(),
            }),
    }
}

fn preserve_task_deadline_markers(
    existing: Option<&OperationalTaskSnapshot>,
    mut task: OperationalTaskSnapshot,
    task_state_changed_at: Option<DateTime<Utc>>,
) -> OperationalTaskSnapshot {
    let observed_at = task_state_changed_at.unwrap_or_else(Utc::now);
    if !task.id.is_empty() {
        if let Some(existing) = existing.filter(|existing| existing.id == task.id) {
            if task.deadline_minutes.is_none() {
                task.deadline_minutes = existing.deadline_minutes;
            }
            let reopened_after_stale = existing.status.trim() == "stale"
                && is_resumable_task_status(&task.status)
                && existing
                    .stale_at
                    .zip(task_state_changed_at)
                    .is_some_and(|(stale_at, state_changed_at)| state_changed_at > stale_at);
            if reopened_after_stale {
                task.assigned_at = deadline_assignment_started(&task, None).then_some(observed_at);
                task.nudged_at = None;
                task.stale_at = None;
                return task;
            }
            if task.assigned_at.is_none() {
                task.assigned_at = if deadline_assignment_started(&task, Some(existing)) {
                    Some(observed_at)
                } else {
                    existing.assigned_at
                };
            }
            task.nudged_at = existing.nudged_at;
            task.stale_at = existing.stale_at;
            if existing.status == "stale" {
                task.status = existing.status.clone();
            }
        }
        if task.assigned_at.is_none() && deadline_assignment_started(&task, None) {
            task.assigned_at = Some(observed_at);
        }
    }

    task
}

fn deadline_assignment_started(
    task: &OperationalTaskSnapshot,
    existing: Option<&OperationalTaskSnapshot>,
) -> bool {
    task.deadline_minutes.is_some()
        && existing.is_none_or(|existing| {
            existing.deadline_minutes.is_none()
                || (!is_deadline_eligible_task_status(&existing.status)
                    && is_deadline_eligible_task_status(&task.status))
        })
}

fn save_snapshot_if_changed(
    teams_dir: &Path,
    mut snapshot: OperationalContextSnapshot,
    task_state_changed_at: Option<DateTime<Utc>>,
) -> Result<(), CoordinationError> {
    let guard =
        crate::coordination::stores::lock::acquire_team_lock(teams_dir, &snapshot.team_name)?;
    let current = OperationalContextSnapshotStore::load(
        teams_dir,
        &snapshot.team_name,
        &snapshot.member_name,
    )?;
    snapshot.task = preserve_task_deadline_markers(
        current.as_ref().map(|current| &current.task),
        snapshot.task,
        task_state_changed_at,
    );

    if let Some(existing_snapshot) = current.as_ref() {
        let mut candidate = snapshot.clone();
        candidate.updated_at = existing_snapshot.updated_at;
        if candidate == *existing_snapshot {
            return Ok(());
        }
    }

    OperationalContextSnapshotStore::save_locked(&guard, teams_dir, &snapshot)
}

/// The task statuses an assignment is still open in. This is the one place
/// the vocabulary is read for assignment selection and deadline policy; the
/// policy module takes this verdict as input rather than re-deriving it.
pub(crate) fn is_resumable_task_status(status: &str) -> bool {
    matches!(status.trim(), "pending" | "in_progress")
}

/// The narrower gate the deadline pass applies: deadlines run only after work
/// has started. A sibling of `is_resumable_task_status`, owned here so the
/// status vocabulary never gets re-derived in a consumer.
pub(crate) fn is_deadline_eligible_task_status(status: &str) -> bool {
    status.trim() == "in_progress"
}

fn task_priority(task: &taurhaus_lib::db::task_queries::PersistedTask) -> u8 {
    match task.status.as_str() {
        "in_progress" => 3,
        "pending" => 2,
        _ => 0,
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
                project_path: "proj-web".into(),
                cli_tool: CliTool::Codex,
                extra: Default::default(),
            }],
            extra: Default::default(),
        };

        TeamConfigStore::save(teams_dir, "architecture-final", &config).expect("save team");
    }

    #[test]
    fn prepare_initialize_snapshots_derives_lead_and_agent_context_from_the_db() {
        let (conn, _db) = test_db();
        let mut lead_task = owned_task(
            "lead-task",
            "Coordinate rollout",
            "in_progress",
            Some(("high", "cross-project coordination")),
        );
        lead_task.project_path = "proj-lead".to_string();
        lead_task.owner = Some("team-lead".to_string());
        taurhaus_lib::db::task_queries::upsert_task(&conn, &lead_task).expect("upsert lead task");
        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &owned_task(
                "agent-task",
                "Build frontend",
                "pending",
                Some(("medium", "bounded UI work")),
            ),
        )
        .expect("upsert agent task");

        let agent = |name: &str, project_id: &str| crate::coordination::requests::AgentDefinition {
            name: name.to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning_effort: None,
            project_id: project_id.to_string(),
            description: None,
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
        };
        let request = crate::coordination::requests::InitializeTeamRequest {
            team_name: "architecture-final".to_string(),
            team_description: None,
            lead_mode: crate::coordination::requests::LeadMode::LaunchNew,
            lead: agent("team-lead", "proj-lead"),
            agents: vec![agent("frontend-dev", "proj-web")],
        };

        let snapshots = prepare_initialize_snapshots(&conn, &request).expect("prepare snapshots");
        assert_eq!(snapshots.len(), 2);
        let lead = snapshots
            .iter()
            .find(|snapshot| snapshot.member_name == "team-lead")
            .expect("lead snapshot");
        assert_eq!(lead.task.id, "lead-task");
        assert_eq!(lead.assignment_footer.task_effort, "high");
        assert_eq!(lead.working_set.project_path, "proj-lead");
        let agent = snapshots
            .iter()
            .find(|snapshot| snapshot.member_name == "frontend-dev")
            .expect("agent snapshot");
        assert_eq!(agent.task.id, "agent-task");
        assert_eq!(agent.assignment_footer.task_effort, "medium");
        assert_eq!(agent.working_set.project_path, "proj-web");
    }

    #[test]
    fn prepare_member_snapshot_derives_context_without_writing_team_state() {
        let teams = TempDir::new().expect("teams dir");
        let (conn, _db) = test_db();
        write_team(teams.path());

        let snapshot = prepare_member_snapshot(
            teams.path(),
            &conn,
            "architecture-final",
            "frontend-dev",
            "proj-web",
        )
        .expect("prepare snapshot");

        assert_eq!(snapshot.team_name, "architecture-final");
        assert_eq!(snapshot.member_name, "frontend-dev");
        assert_eq!(snapshot.working_set.project_path, "proj-web");
        assert!(OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .is_none());
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
                effort: None,
                effort_why: None,
                deadline_minutes: None,
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
        assert_eq!(snapshot.task.deadline_minutes, None);
        assert_eq!(snapshot.task.assigned_at, None);
        assert_eq!(snapshot.working_set.project_path, "proj-web");
    }

    #[test]
    fn a_member_with_no_assignment_effort_keeps_an_empty_footer_pair() {
        let teams = TempDir::new().expect("teams dir");
        let (conn, _db) = test_db();
        write_team(teams.path());

        sync_member_snapshot(teams.path(), &conn, "architecture-final", "frontend-dev")
            .expect("sync snapshot");

        let snapshot = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");

        assert!(snapshot.assignment_footer.task_effort.is_empty());
        assert!(snapshot.assignment_footer.task_effort_why.is_empty());
    }

    fn owned_task(
        source_task_id: &str,
        subject: &str,
        status: &str,
        effort: Option<(&str, &str)>,
    ) -> taurhaus_lib::db::task_queries::PersistedTask {
        taurhaus_lib::db::task_queries::PersistedTask {
            project_path: "proj-web".to_string(),
            source: "claude".to_string(),
            source_key: "session-1".to_string(),
            source_task_id: source_task_id.to_string(),
            subject: subject.to_string(),
            description: None,
            active_form: None,
            status: status.to_string(),
            blocks: vec![],
            blocked_by: vec![],
            owner: Some("frontend-dev".to_string()),
            session_id: None,
            first_seen_at: "2026-03-08T12:00:00Z".to_string(),
            state_changed_at: Some("2026-03-08T12:00:00Z".to_string()),
            updated_at: "2026-03-08T12:00:00Z".to_string(),
            archived_at: None,
            last_status: Some(status.to_string()),
            archived_reason: None,
            effort: effort.map(|(level, _)| level.to_string()),
            effort_why: effort.map(|(_, why)| why.to_string()),
            deadline_minutes: None,
        }
    }

    fn footer_effort(teams_dir: &Path) -> (String, String) {
        let snapshot =
            OperationalContextSnapshotStore::load(teams_dir, "architecture-final", "frontend-dev")
                .expect("load snapshot")
                .expect("snapshot exists");
        (
            snapshot.assignment_footer.task_effort,
            snapshot.assignment_footer.task_effort_why,
        )
    }

    // Regression: 5384985 took the newest effort-bearing message in the
    // member's inbox whatever task the snapshot had selected, so a member
    // owning two tasks showed one task's subject beside the other's level.
    #[test]
    fn the_footer_carries_the_effort_of_the_task_the_member_is_on() {
        let teams = TempDir::new().expect("teams dir");
        let (conn, _db) = test_db();
        write_team(teams.path());

        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &owned_task(
                "41",
                "Queued cleanup",
                "pending",
                Some(("low", "mechanical")),
            ),
        )
        .expect("upsert queued task");
        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &owned_task(
                "42",
                "Run the migration",
                "in_progress",
                Some(("high", "the migration is irreversible")),
            ),
        )
        .expect("upsert active task");

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
        assert_eq!(
            (
                snapshot.assignment_footer.task_effort,
                snapshot.assignment_footer.task_effort_why
            ),
            (
                "high".to_string(),
                "the migration is irreversible".to_string()
            )
        );
    }

    // Regression: the same commit never cleared the pair, so the level of a
    // finished assignment stayed on the node as the member's current task
    // effort until another assignment arrived.
    #[test]
    fn a_finished_assignment_stops_showing_its_effort() {
        let teams = TempDir::new().expect("teams dir");
        let (conn, _db) = test_db();
        write_team(teams.path());

        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &owned_task(
                "42",
                "Run the migration",
                "in_progress",
                Some(("high", "the migration is irreversible")),
            ),
        )
        .expect("upsert task");
        sync_member_snapshot(teams.path(), &conn, "architecture-final", "frontend-dev")
            .expect("sync snapshot");
        assert_eq!(footer_effort(teams.path()).0, "high");

        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &owned_task(
                "42",
                "Run the migration",
                "completed",
                Some(("high", "the migration is irreversible")),
            ),
        )
        .expect("complete task");
        sync_member_snapshot(teams.path(), &conn, "architecture-final", "frontend-dev")
            .expect("sync snapshot");

        assert_eq!(
            footer_effort(teams.path()),
            (String::new(), String::new()),
            "a member with nothing assigned has no task effort"
        );
    }

    #[test]
    fn sync_member_snapshot_clears_completed_task_when_no_resumable_task_exists() {
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
                subject: "Document completed change".to_string(),
                description: None,
                active_form: None,
                status: "completed".to_string(),
                blocks: vec![],
                blocked_by: vec![],
                owner: Some("frontend-dev".to_string()),
                session_id: None,
                first_seen_at: "2026-03-08T12:00:00Z".to_string(),
                state_changed_at: Some("2026-03-08T12:00:00Z".to_string()),
                updated_at: "2026-03-08T12:00:00Z".to_string(),
                archived_at: None,
                last_status: Some("completed".to_string()),
                archived_reason: None,
                effort: None,
                effort_why: None,
                deadline_minutes: None,
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

        assert!(snapshot.task.id.is_empty());
        assert!(snapshot.task.subject.is_empty());
        assert!(snapshot.task.status.is_empty());
        assert_eq!(snapshot.working_set.project_path, "proj-web");
    }

    #[test]
    fn sync_member_snapshot_preserves_timestamp_when_task_context_is_unchanged() {
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
                effort: None,
                effort_why: None,
                deadline_minutes: None,
            },
        )
        .expect("upsert task");

        let seeded_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T13:00:00Z")
            .expect("timestamp")
            .with_timezone(&Utc);
        OperationalContextSnapshotStore::save(
            teams.path(),
            &OperationalContextSnapshot {
                version: 1,
                team_name: "architecture-final".to_string(),
                member_name: "frontend-dev".to_string(),
                updated_at: seeded_at,
                task: OperationalTaskSnapshot {
                    id: "42".to_string(),
                    subject: "Fix regression".to_string(),
                    status: "in_progress".to_string(),
                    ..Default::default()
                },
                assignment_footer: OperationalAssignmentFooterSnapshot::default(),
                ownership: OperationalOwnershipSnapshot::default(),
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "proj-web".to_string(),
                    focal_files: Vec::new(),
                },
            },
        )
        .expect("seed snapshot");

        sync_member_snapshot(teams.path(), &conn, "architecture-final", "frontend-dev")
            .expect("sync snapshot");

        let snapshot = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");

        assert_eq!(snapshot.updated_at, seeded_at);
        assert_eq!(snapshot.task.id, "42");
        assert_eq!(snapshot.task.status, "in_progress");
    }

    // Regression: e5d0935a added deadline markers to the operational record,
    // but task scans rebuilt that record from defaults and erased the markers
    // even while the member remained on the same assignment.
    #[test]
    fn sync_member_snapshot_preserves_deadline_markers_only_for_same_task() {
        let teams = TempDir::new().expect("teams dir");
        let (conn, _db) = test_db();
        write_team(teams.path());

        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &owned_task("42", "Fix regression", "in_progress", None),
        )
        .expect("upsert task");

        let nudged_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T12:10:00Z")
            .expect("nudge timestamp")
            .with_timezone(&Utc);
        let stale_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T12:20:00Z")
            .expect("stale timestamp")
            .with_timezone(&Utc);
        let assigned_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T12:00:00Z")
            .expect("assigned timestamp")
            .with_timezone(&Utc);
        OperationalContextSnapshotStore::save(
            teams.path(),
            &OperationalContextSnapshot {
                version: 1,
                team_name: "architecture-final".to_string(),
                member_name: "frontend-dev".to_string(),
                updated_at: Utc::now(),
                task: OperationalTaskSnapshot {
                    id: "42".to_string(),
                    subject: "Fix regression".to_string(),
                    status: "in_progress".to_string(),
                    deadline_minutes: Some(20),
                    assigned_at: Some(assigned_at),
                    nudged_at: Some(nudged_at),
                    stale_at: Some(stale_at),
                },
                assignment_footer: OperationalAssignmentFooterSnapshot::default(),
                ownership: OperationalOwnershipSnapshot::default(),
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "proj-web".to_string(),
                    focal_files: Vec::new(),
                },
            },
        )
        .expect("seed snapshot");

        sync_member_snapshot(teams.path(), &conn, "architecture-final", "frontend-dev")
            .expect("sync same task");
        let same_task = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");
        assert_eq!(same_task.task.deadline_minutes, Some(20));
        assert_eq!(same_task.task.assigned_at, Some(assigned_at));
        assert_eq!(same_task.task.nudged_at, Some(nudged_at));
        assert_eq!(same_task.task.stale_at, Some(stale_at));

        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &owned_task("42", "Fix regression", "completed", None),
        )
        .expect("complete old task");
        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &owned_task("43", "Fix another regression", "in_progress", None),
        )
        .expect("upsert replacement task");

        sync_member_snapshot(teams.path(), &conn, "architecture-final", "frontend-dev")
            .expect("sync replacement task");
        let replacement = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load replacement snapshot")
        .expect("replacement snapshot exists");
        assert_eq!(replacement.task.id, "43");
        assert_eq!(replacement.task.deadline_minutes, None);
        assert_eq!(replacement.task.assigned_at, None);
        assert_eq!(replacement.task.nudged_at, None);
        assert_eq!(replacement.task.stale_at, None);
    }

    #[test]
    fn a_deadline_assignment_is_stamped_when_in_progress_first_appears() {
        let mut pending = owned_task("42", "Fix regression", "pending", None);
        pending.deadline_minutes = Some(20);
        let (pending_task, _, _) = latest_owned_task_from_tasks(&[pending], "frontend-dev");
        let pending_task = preserve_task_deadline_markers(None, pending_task, None);
        let first_seen_at = pending_task.assigned_at.expect("first task observation");

        let mut in_progress = owned_task("42", "Fix regression", "in_progress", None);
        in_progress.deadline_minutes = Some(20);
        in_progress.state_changed_at = Some("2026-03-08T12:30:00Z".to_string());
        let (in_progress_task, _, _) = latest_owned_task_from_tasks(&[in_progress], "frontend-dev");
        let in_progress_at = DateTime::parse_from_rfc3339("2026-03-08T12:30:00Z")
            .expect("in-progress timestamp")
            .with_timezone(&Utc);
        let stamped = preserve_task_deadline_markers(
            Some(&pending_task),
            in_progress_task,
            Some(in_progress_at),
        );

        assert_eq!(stamped.assigned_at, Some(in_progress_at));
        assert_ne!(stamped.assigned_at, Some(first_seen_at));
    }

    // Regression: 04bda5ec made a stale snapshot sticky by task id alone, so
    // a later mesh transition reopening that task could never reach the
    // operational snapshot or start a fresh deadline window.
    #[test]
    fn sync_member_snapshot_clears_stale_markers_for_a_reopened_task() {
        let teams = TempDir::new().expect("teams dir");
        let (conn, _db) = test_db();
        write_team(teams.path());

        let nudged_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T12:10:00Z")
            .expect("nudge timestamp")
            .with_timezone(&Utc);
        let stale_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T12:20:00Z")
            .expect("stale timestamp")
            .with_timezone(&Utc);
        let mut reopened = owned_task("42", "Fix regression", "in_progress", None);
        reopened.state_changed_at = Some("2026-03-08T12:30:00Z".to_string());
        reopened.updated_at = "2026-03-08T12:30:00Z".to_string();
        taurhaus_lib::db::task_queries::upsert_task(&conn, &reopened)
            .expect("upsert reopened task");
        OperationalContextSnapshotStore::save(
            teams.path(),
            &OperationalContextSnapshot {
                version: 1,
                team_name: "architecture-final".to_string(),
                member_name: "frontend-dev".to_string(),
                updated_at: stale_at,
                task: OperationalTaskSnapshot {
                    id: "42".to_string(),
                    subject: "Fix regression".to_string(),
                    status: "stale".to_string(),
                    deadline_minutes: Some(20),
                    assigned_at: Some(stale_at - chrono::Duration::minutes(20)),
                    nudged_at: Some(nudged_at),
                    stale_at: Some(stale_at),
                },
                assignment_footer: OperationalAssignmentFooterSnapshot::default(),
                ownership: OperationalOwnershipSnapshot::default(),
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "proj-web".to_string(),
                    focal_files: Vec::new(),
                },
            },
        )
        .expect("seed stale snapshot");

        sync_member_snapshot(teams.path(), &conn, "architecture-final", "frontend-dev")
            .expect("sync reopened task");

        let snapshot = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");
        assert_eq!(snapshot.task.status, "in_progress");
        assert_eq!(snapshot.task.deadline_minutes, Some(20));
        assert_eq!(snapshot.task.nudged_at, None);
        assert_eq!(snapshot.task.stale_at, None);
    }

    // Regression: 639b340e published daemon member-operation snapshots without
    // the task transition timestamp, so a reopened task was forced back stale.
    #[test]
    fn daemon_member_snapshot_publish_clears_stale_markers_for_a_reopened_task() {
        let teams = TempDir::new().expect("teams dir");
        write_team(teams.path());
        let stale_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T12:20:00Z")
            .expect("stale timestamp")
            .with_timezone(&Utc);
        let state_changed_at = stale_at + chrono::Duration::minutes(10);
        let existing = OperationalContextSnapshot {
            version: 1,
            team_name: "architecture-final".to_string(),
            member_name: "frontend-dev".to_string(),
            updated_at: stale_at,
            task: OperationalTaskSnapshot {
                id: "42".to_string(),
                subject: "Fix regression".to_string(),
                status: "stale".to_string(),
                deadline_minutes: Some(20),
                assigned_at: Some(stale_at - chrono::Duration::minutes(20)),
                nudged_at: Some(stale_at - chrono::Duration::minutes(10)),
                stale_at: Some(stale_at),
            },
            assignment_footer: OperationalAssignmentFooterSnapshot::default(),
            ownership: OperationalOwnershipSnapshot::default(),
            working_set: OperationalWorkingSetSnapshot {
                project_path: "proj-web".to_string(),
                focal_files: Vec::new(),
            },
        };
        OperationalContextSnapshotStore::save(teams.path(), &existing)
            .expect("seed stale snapshot");
        let mut reopened = existing.clone();
        reopened.updated_at = state_changed_at;
        reopened.task.status = "in_progress".to_string();

        publish_member_operation_snapshot(teams.path(), &reopened, Some(state_changed_at))
            .expect("publish reopened snapshot");

        let stored = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");
        assert_eq!(stored.task.status, "in_progress");
        assert_eq!(stored.task.nudged_at, None);
        assert_eq!(stored.task.stale_at, None);
    }

    // Regression: 04bda5ec must keep the deadline pass's stale marker while
    // the task importer still carries the older in-progress transition.
    #[test]
    fn sync_member_snapshot_preserves_stale_until_task_sync_catches_up() {
        let teams = TempDir::new().expect("teams dir");
        let (conn, _db) = test_db();
        write_team(teams.path());

        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &owned_task("42", "Fix regression", "in_progress", None),
        )
        .expect("upsert pre-deadline task");
        let stale_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T12:20:00Z")
            .expect("stale timestamp")
            .with_timezone(&Utc);
        OperationalContextSnapshotStore::save(
            teams.path(),
            &OperationalContextSnapshot {
                version: 1,
                team_name: "architecture-final".to_string(),
                member_name: "frontend-dev".to_string(),
                updated_at: stale_at,
                task: OperationalTaskSnapshot {
                    id: "42".to_string(),
                    subject: "Fix regression".to_string(),
                    status: "stale".to_string(),
                    deadline_minutes: Some(20),
                    assigned_at: Some(stale_at - chrono::Duration::minutes(20)),
                    nudged_at: None,
                    stale_at: Some(stale_at),
                },
                assignment_footer: OperationalAssignmentFooterSnapshot::default(),
                ownership: OperationalOwnershipSnapshot::default(),
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "proj-web".to_string(),
                    focal_files: Vec::new(),
                },
            },
        )
        .expect("seed stale snapshot");

        sync_member_snapshot(teams.path(), &conn, "architecture-final", "frontend-dev")
            .expect("sync lagging task");

        let snapshot = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");
        assert_eq!(snapshot.task.status, "stale");
        assert_eq!(snapshot.task.stale_at, Some(stale_at));
    }

    // Regression: 04bda5ec must also let the normal stale task import close
    // and remove the operational assignment after the deadline write lands.
    #[test]
    fn sync_member_snapshot_clears_task_after_stale_import_round_trip() {
        let teams = TempDir::new().expect("teams dir");
        let (conn, _db) = test_db();
        write_team(teams.path());

        taurhaus_lib::db::task_queries::upsert_task(
            &conn,
            &owned_task("42", "Fix regression", "stale", None),
        )
        .expect("upsert stale task");
        let stale_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T12:20:00Z")
            .expect("stale timestamp")
            .with_timezone(&Utc);
        OperationalContextSnapshotStore::save(
            teams.path(),
            &OperationalContextSnapshot {
                version: 1,
                team_name: "architecture-final".to_string(),
                member_name: "frontend-dev".to_string(),
                updated_at: stale_at,
                task: OperationalTaskSnapshot {
                    id: "42".to_string(),
                    subject: "Fix regression".to_string(),
                    status: "stale".to_string(),
                    deadline_minutes: Some(20),
                    assigned_at: Some(stale_at - chrono::Duration::minutes(20)),
                    nudged_at: None,
                    stale_at: Some(stale_at),
                },
                assignment_footer: OperationalAssignmentFooterSnapshot::default(),
                ownership: OperationalOwnershipSnapshot::default(),
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "proj-web".to_string(),
                    focal_files: Vec::new(),
                },
            },
        )
        .expect("seed stale snapshot");

        sync_member_snapshot(teams.path(), &conn, "architecture-final", "frontend-dev")
            .expect("sync imported stale task");

        let snapshot = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");
        assert_eq!(snapshot.task, OperationalTaskSnapshot::default());
    }

    // Regression: 1bb8668e let an operational refresh loaded before the
    // deadline pass overwrite a one-shot marker the pass committed while the
    // refresh was building its replacement snapshot.
    #[test]
    fn snapshot_refresh_preserves_a_marker_committed_after_its_load() {
        let teams = TempDir::new().expect("teams dir");
        write_team(teams.path());
        let assigned_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T12:00:00Z")
            .expect("assigned timestamp")
            .with_timezone(&Utc);
        let nudged_at = assigned_at + chrono::Duration::minutes(10);
        let original = OperationalContextSnapshot {
            version: 1,
            team_name: "architecture-final".to_string(),
            member_name: "frontend-dev".to_string(),
            updated_at: assigned_at,
            task: OperationalTaskSnapshot {
                id: "42".to_string(),
                subject: "Original subject".to_string(),
                status: "in_progress".to_string(),
                deadline_minutes: Some(20),
                ..Default::default()
            },
            assignment_footer: OperationalAssignmentFooterSnapshot::default(),
            ownership: OperationalOwnershipSnapshot::default(),
            working_set: OperationalWorkingSetSnapshot {
                project_path: "proj-web".to_string(),
                focal_files: Vec::new(),
            },
        };
        OperationalContextSnapshotStore::save(teams.path(), &original)
            .expect("seed original snapshot");

        let refresh = build_member_snapshot(
            Some(&original),
            "architecture-final",
            "frontend-dev",
            "proj-web",
            OperationalTaskSnapshot {
                id: "42".to_string(),
                subject: "Refreshed subject".to_string(),
                status: "in_progress".to_string(),
                ..Default::default()
            },
            None,
            None,
        );
        let mut concurrent = original.clone();
        concurrent.task.nudged_at = Some(nudged_at);
        OperationalContextSnapshotStore::save(teams.path(), &concurrent)
            .expect("commit concurrent deadline marker");

        save_snapshot_if_changed(teams.path(), refresh, None).expect("save refreshed snapshot");

        let stored = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load refreshed snapshot")
        .expect("refreshed snapshot exists");
        assert_eq!(stored.task.subject, "Refreshed subject");
        assert_eq!(stored.task.nudged_at, Some(nudged_at));
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
                        task_effort: String::new(),
                        task_effort_why: String::new(),
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

    // Regression: e5d0935a added deadline markers to the operational record,
    // but delivery updates rebuilt a supplied task from defaults and erased
    // the same assignment's one-shot markers.
    #[test]
    fn apply_delivery_context_preserves_deadline_markers_only_for_same_task() {
        let teams = TempDir::new().expect("teams dir");
        write_team(teams.path());

        let nudged_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T12:10:00Z")
            .expect("nudge timestamp")
            .with_timezone(&Utc);
        OperationalContextSnapshotStore::save(
            teams.path(),
            &OperationalContextSnapshot {
                version: 1,
                team_name: "architecture-final".to_string(),
                member_name: "frontend-dev".to_string(),
                updated_at: Utc::now(),
                task: OperationalTaskSnapshot {
                    id: "42".to_string(),
                    subject: "Fix regression".to_string(),
                    status: "in_progress".to_string(),
                    deadline_minutes: Some(20),
                    assigned_at: Some(nudged_at - chrono::Duration::minutes(10)),
                    nudged_at: Some(nudged_at),
                    stale_at: None,
                },
                assignment_footer: OperationalAssignmentFooterSnapshot::default(),
                ownership: OperationalOwnershipSnapshot::default(),
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "proj-web".to_string(),
                    focal_files: Vec::new(),
                },
            },
        )
        .expect("seed snapshot");

        let context_for = |id: &str| OperationalContextUpdate {
            task: Some(crate::coordination::requests::OperationalTaskContext {
                id: id.to_string(),
                subject: "Delivered assignment".to_string(),
                status: "in_progress".to_string(),
            }),
            assignment_footer: None,
            ownership: None,
            working_set: None,
        };

        apply_delivery_context(
            teams.path(),
            "architecture-final",
            "frontend-dev",
            &context_for("42"),
        )
        .expect("apply same task");
        let same_task = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");
        assert_eq!(same_task.task.deadline_minutes, Some(20));
        assert_eq!(same_task.task.nudged_at, Some(nudged_at));

        apply_delivery_context(
            teams.path(),
            "architecture-final",
            "frontend-dev",
            &context_for("43"),
        )
        .expect("apply replacement task");
        let replacement = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load replacement snapshot")
        .expect("replacement snapshot exists");
        assert_eq!(replacement.task.id, "43");
        assert_eq!(replacement.task.deadline_minutes, None);
        assert_eq!(replacement.task.nudged_at, None);
        assert_eq!(replacement.task.stale_at, None);
    }
}
