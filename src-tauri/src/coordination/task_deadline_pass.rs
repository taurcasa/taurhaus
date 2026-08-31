//! Deadline side effects owned by the background self-heal pass.

use std::path::Path;

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
    let Some(assigned_at) = snapshot.task.assigned_at else {
        return Ok(());
    };

    // W4 deadlines apply only after work has started. The stricter
    // in-progress gate is this pass's scope; every other status is inert.
    if !crate::coordination::operational_context::is_deadline_eligible_task_status(
        &snapshot.task.status,
    ) {
        return Ok(());
    }
    let action = decide(
        &DeadlineInput {
            assigned_at,
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

    if action == DeadlineAction::Nudge
        && !crate::coordination::stores::mesh_task::is_still_open(
            &orchestrator.teams_dir,
            team_name,
            member_name,
            &snapshot.task.id,
        )
    {
        rollback_claim(&orchestrator.teams_dir, &claimed, action, now)?;
        return Ok(());
    }
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
        DeadlineAction::MarkStale => crate::coordination::stores::mesh_task::commit_status_if_unchanged(
            &orchestrator.teams_dir,
            team_name,
            member_name,
            &snapshot.task.id,
            "in_progress",
            "stale",
        ),
    };

    if let Err(error) = action_result {
        rollback_claim(&orchestrator.teams_dir, &claimed, action, now)?;
        if action == DeadlineAction::MarkStale
            && matches!(
                &error,
                CoordinationError::Conflict(_) | CoordinationError::NotFound(_)
            )
        {
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
    // Ownership is proven by the compare-and-commit below: it rolls the
    // marker back only if the stored snapshot is still exactly the one this
    // pass wrote (`claimed.current`); any concurrent movement skips.
    let _ = now;
    if action == DeadlineAction::Nothing {
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
                assigned_at: Some(now - Duration::minutes(20)),
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
