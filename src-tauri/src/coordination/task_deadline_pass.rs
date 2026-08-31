//! Deadline side effects owned by the background self-heal pass.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};

use crate::coordination::activity_export::read_member_activity_snapshot;
use crate::coordination::activity_schema::SnapshotActivityConfidence;
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{DeliveryRequest, OperatorNoticeDelivery};
use crate::coordination::stores::{
    OperationalContextSnapshot, OperationalContextSnapshotStore, OperationalSnapshotCommitOutcome,
    TeamConfigStore,
};
use crate::coordination::task_deadline::{decide, DeadlineAction, DeadlineInput, Timestamp};

const ACTIVITY_FRESHNESS: Duration = Duration::seconds(120);
const MAX_TASK_RECORD_BYTES: u64 = 1_048_576;

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

    // W4 deadlines apply only after work has started. The stricter
    // in-progress gate is this pass's scope; every other status is inert.
    if snapshot.task.status.trim() != "in_progress" {
        return Ok(());
    }
    let action = decide(
        &DeadlineInput {
            // Until mesh supplies an assignment timestamp, the latest saved
            // operational snapshot write is the deadline clock origin. Any
            // snapshot content refresh therefore restarts this clock.
            assigned_at: snapshot.updated_at,
            deadline_minutes,
            nudged_at: snapshot.task.nudged_at,
            stale_at: snapshot.task.stale_at,
            // `DeadlineInput::active` means the assignment remains open. The
            // in-progress scope gate above proves that verdict here; member
            // activity suppresses only Nudge in the pass below.
            active: true,
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
                    "ACTION REQUIRED: Task #{} — half the deadline is gone ({} minutes total); report progress or BLOCKED.",
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
        if action == DeadlineAction::MarkStale && matches!(&error, CoordinationError::Conflict(_)) {
            return Ok(());
        }
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
) -> Result<Option<ClaimedDeadlineAction>, CoordinationError> {
    let mut current = expected.clone();
    set_action_marker(&mut current, action, now);
    let outcome =
        OperationalContextSnapshotStore::commit_if_unchanged(teams_dir, expected, |snapshot| {
            set_action_marker(snapshot, action, now)
        })?;
    Ok(
        (outcome == OperationalSnapshotCommitOutcome::Committed).then_some(ClaimedDeadlineAction {
            current,
            previous_task_status: expected.task.status.clone(),
        }),
    )
}

struct ClaimedDeadlineAction {
    current: OperationalContextSnapshot,
    previous_task_status: String,
}

fn rollback_claim(
    teams_dir: &Path,
    claimed: &ClaimedDeadlineAction,
    action: DeadlineAction,
    now: Timestamp,
) -> Result<(), CoordinationError> {
    let marker_still_owned = match action {
        DeadlineAction::Nothing => false,
        DeadlineAction::Nudge => claimed.current.task.nudged_at == Some(now),
        DeadlineAction::MarkStale => claimed.current.task.stale_at == Some(now),
    };
    if !marker_still_owned {
        return Ok(());
    }
    let outcome = OperationalContextSnapshotStore::commit_if_unchanged(
        teams_dir,
        &claimed.current,
        |snapshot| clear_action_marker(snapshot, action, &claimed.previous_task_status),
    )?;
    if outcome == OperationalSnapshotCommitOutcome::Skipped {
        tracing::warn!(
            team = %claimed.current.team_name,
            member = %claimed.current.member_name,
            task_id = %claimed.current.task.id,
            ?action,
            "deadline action marker rollback skipped because the operational snapshot changed"
        );
    }
    Ok(())
}

fn set_action_marker(
    snapshot: &mut OperationalContextSnapshot,
    action: DeadlineAction,
    now: Timestamp,
) {
    match action {
        DeadlineAction::Nothing => {}
        DeadlineAction::Nudge => snapshot.task.nudged_at = Some(now),
        DeadlineAction::MarkStale => {
            snapshot.task.stale_at = Some(now);
            snapshot.task.status = "stale".to_string();
        }
    }
}

fn clear_action_marker(
    snapshot: &mut OperationalContextSnapshot,
    action: DeadlineAction,
    previous_task_status: &str,
) {
    match action {
        DeadlineAction::Nothing => {}
        DeadlineAction::Nudge => snapshot.task.nudged_at = None,
        DeadlineAction::MarkStale => {
            snapshot.task.stale_at = None;
            snapshot.task.status = previous_task_status.to_string();
        }
    }
}

fn member_has_fresh_active_signal(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    now: Timestamp,
) -> bool {
    let Some(snapshot) = read_member_activity_snapshot(teams_dir, team_name, member_name) else {
        return false;
    };
    let Some(observed_at) = DateTime::parse_from_rfc3339(&snapshot.observed_at)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
    else {
        return false;
    };
    let age = now.signed_duration_since(observed_at);
    let fresh = age <= ACTIVITY_FRESHNESS && age >= -ACTIVITY_FRESHNESS;
    let active = matches!(
        snapshot.activity_confidence,
        SnapshotActivityConfidence::Active | SnapshotActivityConfidence::LikelyWorking
    );
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
    if fs::metadata(&path)?.len() > MAX_TASK_RECORD_BYTES {
        return Err(CoordinationError::Validation(format!(
            "mesh task '{task_id}' exceeds the 1 MiB record limit"
        )));
    }
    let raw = target_lock.read_contents()?;
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

    let payload = serde_json::to_string_pretty(&task).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to serialize stale mesh task '{task_id}': {error}"
        ))
    })?;
    let tmp_path = deadline_task_tmp_path(&path);
    write_file_synced(&tmp_path, &payload)?;
    if let Err(error) = fs::rename(&tmp_path, &path) {
        if crate::coordination::stores::operational::is_windows_unsupported_rename_error(&error) {
            write_file_synced(&path, &payload)?;
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
    if task_id.is_empty()
        || !task_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(CoordinationError::Validation(format!(
            "invalid mesh task id '{task_id}'"
        )));
    }
    let tasks_dir = crate::coordination::pipelines::mesh_tasks_dir(teams_dir, team_name)
        .ok_or_else(|| {
            CoordinationError::StoreError(format!(
                "coordination teams root is not canonical: {}",
                teams_dir.display()
            ))
        })?;
    Ok(tasks_dir.join(format!("{task_id}.json")))
}

fn deadline_task_tmp_path(path: &Path) -> PathBuf {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".deadline.tmp");
    PathBuf::from(tmp)
}

fn write_file_synced(path: &Path, payload: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(payload.as_bytes())?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    use crate::coordination::stores::{
        OperationalAssignmentFooterSnapshot, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot,
    };

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

    // Regression: 04bda5ec hardcoded the rollback status instead of restoring
    // the exact pre-claim value, so a failed action could rewrite task state.
    #[test]
    fn stale_claim_rollback_restores_the_captured_status() {
        let teams = TempDir::new().expect("teams dir");
        let now = DateTime::parse_from_rfc3339("2026-03-08T12:20:00Z")
            .expect("deadline timestamp")
            .with_timezone(&Utc);
        let before = OperationalContextSnapshot {
            version: 1,
            team_name: "deadline-team".to_string(),
            member_name: "builder".to_string(),
            updated_at: now - Duration::minutes(20),
            task: OperationalTaskSnapshot {
                id: "42".to_string(),
                subject: "Fix regression".to_string(),
                status: "in_progress ".to_string(),
                deadline_minutes: Some(20),
                nudged_at: None,
                stale_at: None,
            },
            assignment_footer: OperationalAssignmentFooterSnapshot::default(),
            ownership: OperationalOwnershipSnapshot::default(),
            working_set: OperationalWorkingSetSnapshot {
                project_path: "proj-web".to_string(),
                focal_files: Vec::new(),
            },
        };
        OperationalContextSnapshotStore::save(teams.path(), &before)
            .expect("seed operational snapshot");
        let claimed = claim_action(teams.path(), &before, DeadlineAction::MarkStale, now)
            .expect("claim stale action")
            .expect("claim committed");

        rollback_claim(teams.path(), &claimed, DeadlineAction::MarkStale, now)
            .expect("rollback stale claim");

        let stored =
            OperationalContextSnapshotStore::load(teams.path(), "deadline-team", "builder")
                .expect("load operational snapshot")
                .expect("snapshot exists");
        assert_eq!(stored.task.status, "in_progress ");
        assert_eq!(stored.task.stale_at, None);
    }
}
