//! Deadline side effects owned by the background self-heal pass.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};

use crate::coordination::errors::CoordinationError;
use crate::coordination::operational_context::is_resumable_task_status;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{DeliveryRequest, OperatorNoticeDelivery};
use crate::coordination::stores::{
    OperationalContextSnapshot, OperationalContextSnapshotStore, OperationalSnapshotCommitOutcome,
    TeamConfigStore,
};
use crate::coordination::task_deadline::{decide, DeadlineAction, DeadlineInput, Timestamp};

const ACTIVITY_FRESHNESS: Duration = Duration::seconds(120);
const MAX_ACTIVITY_SNAPSHOT_BYTES: u64 = 1_048_576;
const MAX_TASK_RECORD_BYTES: usize = 1_048_576;

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct DeadlinePassOutcome {
    pub failures: Vec<(String, String)>,
}

pub(crate) fn apply_task_deadlines(
    orchestrator: &mut CoordinationOrchestrator,
    team_name: &str,
    now: Timestamp,
) -> Result<DeadlinePassOutcome, CoordinationError> {
    let config = TeamConfigStore::load(&orchestrator.teams_dir, team_name)?;
    let sender_name = config
        .members
        .iter()
        .find(|member| member.role == crate::coordination::domain::MemberRole::Lead)
        .map(|member| member.name.clone());
    let mut outcome = DeadlinePassOutcome::default();

    for member in &config.members {
        let result = apply_member_deadline(
            orchestrator,
            team_name,
            &member.name,
            sender_name.as_deref(),
            now,
        );
        if let Err(error) = result {
            outcome
                .failures
                .push((member.name.clone(), error.to_string()));
        }
    }

    Ok(outcome)
}

fn apply_member_deadline(
    orchestrator: &mut CoordinationOrchestrator,
    team_name: &str,
    member_name: &str,
    sender_name: Option<&str>,
    now: Timestamp,
) -> Result<(), CoordinationError> {
    let Some(snapshot) =
        OperationalContextSnapshotStore::load(&orchestrator.teams_dir, team_name, member_name)?
    else {
        return Ok(());
    };
    let Some(deadline_minutes) = snapshot.task.deadline_minutes else {
        return Ok(());
    };

    // W4 deadlines apply only after work has started. The shared predicate
    // remains the status authority passed into the pure policy; the stricter
    // in-progress gate is this pass's scope, not a second vocabulary.
    if snapshot.task.status.trim() != "in_progress" {
        return Ok(());
    }
    let action = decide(
        &DeadlineInput {
            assigned_at: snapshot.updated_at,
            deadline_minutes,
            nudged_at: snapshot.task.nudged_at,
            stale_at: snapshot.task.stale_at,
            active: is_resumable_task_status(&snapshot.task.status),
        },
        now,
    );
    if action == DeadlineAction::Nothing
        || (action == DeadlineAction::Nudge
            && member_has_fresh_active_signal(&orchestrator.teams_dir, team_name, member_name, now))
    {
        return Ok(());
    }

    let Some(claimed) = claim_action(&orchestrator.teams_dir, &snapshot, action, now)? else {
        return Ok(());
    };

    let action_result = match action {
        DeadlineAction::Nothing => Ok(()),
        DeadlineAction::Nudge => orchestrator
            .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                team_name: team_name.to_string(),
                member_name: member_name.to_string(),
                message: format!(
                    "ACTION REQUIRED: Task #{} is halfway through its {}-minute deadline; half the deadline is gone; report progress or BLOCKED.",
                    snapshot.task.id, deadline_minutes
                ),
                sender_name: sender_name.map(ToString::to_string),
                operational_context: None,
            }))
            .map(|_| ()),
        DeadlineAction::MarkStale => mark_mesh_task_stale(
            &orchestrator.teams_dir,
            team_name,
            member_name,
            &snapshot.task.id,
        ),
    };

    if let Err(error) = action_result {
        rollback_claim(&orchestrator.teams_dir, &claimed, action, now)?;
        return Err(error);
    }

    emit_deadline_action(
        action,
        team_name,
        member_name,
        &snapshot.task.id,
        deadline_minutes,
    );
    Ok(())
}

fn emit_deadline_action(
    action: DeadlineAction,
    team_name: &str,
    member_name: &str,
    task_id: &str,
    deadline_minutes: u32,
) {
    let (event_name, message) = match action {
        DeadlineAction::Nothing => return,
        DeadlineAction::Nudge => ("deadline.nudge.sent", "Task deadline nudge sent"),
        DeadlineAction::MarkStale => ("deadline.task.staled", "Task deadline marked stale"),
    };
    taurhaus_lib::logging::emit_global(
        "info",
        "coordination",
        event_name,
        Some(message.to_string()),
        deadline_event_fields(team_name, member_name, task_id, deadline_minutes),
    );
}

fn deadline_event_fields(
    team_name: &str,
    member_name: &str,
    task_id: &str,
    deadline_minutes: u32,
) -> serde_json::Map<String, serde_json::Value> {
    let mut fields = serde_json::Map::new();
    fields.insert(
        "team".to_string(),
        serde_json::Value::String(team_name.to_string()),
    );
    fields.insert(
        "member".to_string(),
        serde_json::Value::String(member_name.to_string()),
    );
    fields.insert(
        "task_id".to_string(),
        serde_json::Value::String(task_id.to_string()),
    );
    fields.insert(
        "deadline_minutes".to_string(),
        serde_json::Value::Number(deadline_minutes.into()),
    );
    fields
}

fn claim_action(
    teams_dir: &Path,
    expected: &OperationalContextSnapshot,
    action: DeadlineAction,
    now: Timestamp,
) -> Result<Option<OperationalContextSnapshot>, CoordinationError> {
    let mut claimed = expected.clone();
    set_action_marker(&mut claimed, action, Some(now));
    let outcome =
        OperationalContextSnapshotStore::commit_if_unchanged(teams_dir, expected, |snapshot| {
            set_action_marker(snapshot, action, Some(now))
        })?;
    Ok((outcome == OperationalSnapshotCommitOutcome::Committed).then_some(claimed))
}

fn rollback_claim(
    teams_dir: &Path,
    claimed: &OperationalContextSnapshot,
    action: DeadlineAction,
    now: Timestamp,
) -> Result<(), CoordinationError> {
    let marker_still_owned = match action {
        DeadlineAction::Nothing => false,
        DeadlineAction::Nudge => claimed.task.nudged_at == Some(now),
        DeadlineAction::MarkStale => claimed.task.stale_at == Some(now),
    };
    if !marker_still_owned {
        return Ok(());
    }
    let _ = OperationalContextSnapshotStore::commit_if_unchanged(teams_dir, claimed, |snapshot| {
        set_action_marker(snapshot, action, None)
    })?;
    Ok(())
}

fn set_action_marker(
    snapshot: &mut OperationalContextSnapshot,
    action: DeadlineAction,
    value: Option<Timestamp>,
) {
    match action {
        DeadlineAction::Nothing => {}
        DeadlineAction::Nudge => snapshot.task.nudged_at = value,
        DeadlineAction::MarkStale => snapshot.task.stale_at = value,
    }
}

fn member_has_fresh_active_signal(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    now: Timestamp,
) -> bool {
    let path = teams_dir
        .join(team_name)
        .join("state/activity")
        .join(format!("{member_name}.json"));
    if fs::metadata(&path)
        .ok()
        .is_none_or(|metadata| metadata.len() > MAX_ACTIVITY_SNAPSHOT_BYTES)
    {
        return false;
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(snapshot) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    if snapshot.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
        return false;
    }
    let Some(observed_at) = snapshot
        .get("observed_at")
        .and_then(serde_json::Value::as_str)
        .and_then(|timestamp| DateTime::parse_from_rfc3339(timestamp).ok())
        .map(|timestamp| timestamp.with_timezone(&Utc))
    else {
        return false;
    };
    let age = now.signed_duration_since(observed_at);
    let fresh = age <= ACTIVITY_FRESHNESS && age >= -ACTIVITY_FRESHNESS;
    let active = snapshot
        .get("stall_recent_activity")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
        || snapshot
            .get("activity_confidence")
            .and_then(serde_json::Value::as_str)
            == Some("active");
    fresh && active
}

fn mark_mesh_task_stale(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    task_id: &str,
) -> Result<(), CoordinationError> {
    // The mesh task file remains the source of truth. Its normal watcher/task
    // sync path will carry this status into derived views; the deadline pass
    // must not invent a second database write path.
    let path = mesh_task_path(teams_dir, team_name, task_id)?;
    let target_lock = crate::coordination::stores::lock::TargetFileLock::acquire_if_exists(&path)?
        .ok_or_else(|| {
            CoordinationError::NotFound(format!(
                "mesh task '{task_id}' not found for team '{team_name}'"
            ))
        })?;
    let raw = target_lock.read_contents()?;
    if raw.len() > MAX_TASK_RECORD_BYTES {
        return Err(CoordinationError::Validation(format!(
            "mesh task '{task_id}' exceeds the 1 MiB record limit"
        )));
    }
    let mut task: serde_json::Value = serde_json::from_str(&raw).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to parse mesh task '{task_id}' for team '{team_name}': {error}"
        ))
    })?;
    if task.get("id").and_then(serde_json::Value::as_str) != Some(task_id)
        || task.get("owner").and_then(serde_json::Value::as_str) != Some(member_name)
        || task.get("status").and_then(serde_json::Value::as_str) != Some("in_progress")
    {
        return Err(CoordinationError::Conflict(format!(
            "mesh task '{task_id}' changed before its deadline action committed"
        )));
    }
    task["status"] = serde_json::Value::String("stale".to_string());

    let payload = serde_json::to_vec_pretty(&task).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to serialize stale mesh task '{task_id}': {error}"
        ))
    })?;
    let tmp_path = deadline_task_tmp_path(&path);
    fs::write(&tmp_path, payload)?;
    if let Err(error) = fs::rename(&tmp_path, &path) {
        if cfg!(target_os = "windows") && error.raw_os_error() == Some(1) {
            fs::write(
                &path,
                serde_json::to_vec_pretty(&task).map_err(|serialize_error| {
                    CoordinationError::StoreError(format!(
                        "failed to serialize stale mesh task '{task_id}': {serialize_error}"
                    ))
                })?,
            )?;
            let _ = fs::remove_file(&tmp_path);
        } else {
            let _ = fs::remove_file(&tmp_path);
            return Err(CoordinationError::Io(error));
        }
    }

    Ok(())
}

fn mesh_task_path(
    teams_dir: &Path,
    team_name: &str,
    task_id: &str,
) -> Result<PathBuf, CoordinationError> {
    if teams_dir.file_name().and_then(|name| name.to_str()) != Some("teams") {
        return Err(CoordinationError::StoreError(format!(
            "coordination teams root is not canonical: {}",
            teams_dir.display()
        )));
    }
    if task_id.is_empty()
        || !task_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(CoordinationError::Validation(format!(
            "invalid mesh task id '{task_id}'"
        )));
    }
    let tasks_root = teams_dir.parent().ok_or_else(|| {
        CoordinationError::StoreError("coordination teams root has no parent".to_string())
    })?;
    Ok(tasks_root
        .join("tasks")
        .join(team_name)
        .join(format!("{task_id}.json")))
}

fn deadline_task_tmp_path(path: &Path) -> PathBuf {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".deadline.tmp");
    PathBuf::from(tmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadline_events_carry_the_bounded_action_context() {
        assert_eq!(
            deadline_event_fields("deadline-team", "builder", "42", 20),
            serde_json::Map::from_iter([
                (
                    "team".to_string(),
                    serde_json::Value::String("deadline-team".to_string()),
                ),
                (
                    "member".to_string(),
                    serde_json::Value::String("builder".to_string()),
                ),
                (
                    "task_id".to_string(),
                    serde_json::Value::String("42".to_string()),
                ),
                (
                    "deadline_minutes".to_string(),
                    serde_json::Value::Number(20.into()),
                ),
            ])
        );
    }
}
