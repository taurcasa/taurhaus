//! Shared managed-task deadline policy driver.

use std::fs;
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

// Reasonless activity status follows Mesh's statusState/statusSetAt expiry.
// A blocked status WITH a reason is a declared wait and never expires.
// Keep this compatibility default synchronized with Mesh; deployments can
// override it via TAURHAUS_MESH_MEMBER_STATUS_TTL_SECONDS (see data-architecture.md).
const MESH_IDLE_MONITOR_DEFAULT_STATUS_TTL: Duration = Duration::minutes(30);

fn member_status_ttl(override_seconds: Option<&str>) -> Duration {
    override_seconds
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|seconds| *seconds > 0)
        .and_then(Duration::try_seconds)
        .unwrap_or(MESH_IDLE_MONITOR_DEFAULT_STATUS_TTL)
}

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
    observe_terminal_tasks(&orchestrator.teams_dir, team_name, now);
    let config = TeamConfigStore::load(&orchestrator.teams_dir, team_name)?;
    let sender_name = config
        .members
        .iter()
        .find(|member| member.role == crate::coordination::domain::MemberRole::Lead)
        .map(|member| member.name.clone());
    let mut outcome = DeadlinePassOutcome::default();

    let ttl_override = std::env::var("TAURHAUS_MESH_MEMBER_STATUS_TTL_SECONDS").ok();
    let status_ttl = member_status_ttl(ttl_override.as_deref());
    let mut waiting_members = 0u32;
    for member in &config.members {
        // Retry roster-boot attribution even if the app's task projection has
        // not changed since the render arrived (F9c).
        if let Ok(Some(snapshot)) =
            OperationalContextSnapshotStore::load(&orchestrator.teams_dir, team_name, &member.name)
        {
            if !snapshot.task.id.trim().is_empty() {
                crate::coordination::stores::telemetry::attribute_latest_launch_to_task(
                    &orchestrator.teams_dir,
                    team_name,
                    &snapshot.task.id,
                    &member.name,
                );
            }
        }
        let blocked_is_live = member
            .extra
            .get("statusState")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|state| state.eq_ignore_ascii_case("blocked"))
            && (member
                .extra
                .get("statusReason")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|reason| !reason.trim().is_empty())
                || member
                    .extra
                    .get("statusSetAt")
                    .and_then(serde_json::Value::as_str)
                    .and_then(parse_timestamp)
                    .is_some_and(|set_at| {
                        let age = now - set_at;
                        age >= Duration::zero() && age < status_ttl
                    }));
        if blocked_is_live
            || crate::coordination::stores::mesh_task::awaiting_go(member.extra.get("metadata"))
        {
            waiting_members += 1;
            continue;
        }
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

    if waiting_members > 0 {
        // One debug summary per pass, rather than one record per waiting seat.
        taurhaus_lib::logging::emit_global(
            "debug",
            "coordination",
            "deadline.wait.skipped",
            Some("Deadline pass respected declared member waits".into()),
            serde_json::Map::from_iter([
                ("team".into(), serde_json::json!(team_name)),
                ("waiting_members".into(), serde_json::json!(waiting_members)),
            ]),
        );
    }
    Ok(outcome)
}

// Each pass scans task history and checks terminal sidecars under flock.
// Dedupe bounds appended observations, not read/lock work; a sweep cursor
// would need to preserve retries and later rulings on historical tasks.
fn observe_terminal_tasks(teams_dir: &Path, team_name: &str, now: DateTime<Utc>) {
    let Some(tasks_dir) =
        taurhaus_lib::task_scanner::claude_index::ClaudeSourceIndex::team_tasks_dir(
            teams_dir, team_name,
        )
    else {
        return;
    };
    let Ok(entries) = fs::read_dir(tasks_dir) else {
        return;
    };
    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if metadata.len() > taurhaus_lib::task_scanner::claude::MAX_FILE_SIZE {
            continue;
        }
        let Ok(Some(task)) =
            taurhaus_lib::task_scanner::claude::parse_task_file(&path, Some(team_name.to_string()))
        else {
            continue;
        };
        if !crate::coordination::operational_context::is_terminal_task_status(
            &task.status.to_string(),
        ) {
            continue;
        }
        let completion_at = task
            .state_changed_at
            .as_deref()
            .and_then(parse_timestamp)
            .or_else(|| task.updated_at.as_deref().and_then(parse_timestamp))
            .or_else(|| metadata.modified().ok().map(DateTime::<Utc>::from))
            .unwrap_or(now);
        crate::coordination::stores::telemetry::record_completion_observed(
            teams_dir,
            team_name,
            &task.id,
            &task.status.to_string(),
            task.has_review_ruling,
            completion_at,
        );
    }
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
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
        DeadlineAction::Nudge => {
            let mut recovery_facts = crate::coordination::recovery_delivery::assignment_facts(
                &orchestrator.teams_dir,
                team_name,
                member_name,
                Some(&snapshot),
            );
            recovery_facts.task_id = snapshot.task.id.clone();
            orchestrator
                .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                    journal_links: Some(crate::coordination::journal::JournalLinks {
                        task: recovery_facts.task_id.clone(),
                        assignment: recovery_facts.assignment_token.clone(),
                    }),
                    recovery_card: None,
                    team_name: team_name.to_string(),
                    member_name: member_name.to_string(),
                    message: render_deadline_nudge(&recovery_facts, deadline_minutes),
                    sender_name: sender_name.map(ToString::to_string),
                    operational_context: None,
                }))
                .map(|_| ())
        }
        DeadlineAction::MarkStale => {
            crate::coordination::stores::mesh_task::commit_status_if_unchanged(
                &orchestrator.teams_dir,
                team_name,
                member_name,
                &snapshot.task.id,
                "in_progress",
                "stale",
            )
        }
    };

    if let Err(error) = action_result {
        // Canonical acceptance can have committed before a lost reply. Keep the
        // one-shot claim: only Mesh can reconcile that outcome, never resend.
        if action != DeadlineAction::Nudge
            || !crate::coordination::journal::canonical(&orchestrator.teams_dir, team_name)?
        {
            rollback_claim(&orchestrator.teams_dir, &claimed, action, now)?;
        } else {
            taurhaus_lib::logging::emit_global(
                "warn",
                "coordination",
                "deadline.nudge.unconfirmed",
                Some("Nudge claim retained without confirmed journal acceptance".into()),
                deadline_event_fields(team_name, member_name, &snapshot.task.id, deadline_minutes),
            );
        }
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
        &orchestrator.teams_dir,
        action,
        team_name,
        member_name,
        &snapshot.task.id,
        deadline_minutes,
    );
    Ok(())
}

fn emit_deadline_action(
    teams_dir: &Path,
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
    crate::coordination::stores::telemetry::record_deadline_action(
        teams_dir,
        team_name,
        task_id,
        member_name,
        deadline_minutes,
        action == DeadlineAction::MarkStale,
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

fn render_deadline_nudge(
    facts: &crate::coordination::recovery_card::AssignmentFacts,
    minutes: u32,
) -> String {
    let value = |s: &str| {
        if s.is_empty() {
            "unavailable".to_string()
        } else {
            s.to_string()
        }
    };
    format!("ACTION REQUIRED: Task #{} — half the deadline is gone ({minutes} minutes total); assignment={}; stage={}; wait/release={}. Next action: report progress or BLOCKED; preserve any existing wait. This reminder does not release GO.", facts.task_id, value(&facts.assignment_token), value(&facts.stage_id), value(&facts.wait))
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    use crate::coordination::stores::{
        OperationalAssignmentFooterSnapshot, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot,
    };

    // Regression: 51923397 copied Mesh IdleMonitor's expiry with no way to
    // follow a deployment whose mesh-owned TTL differs from the default.
    #[test]
    fn wave2_review_mesh_status_ttl_can_follow_the_monitor_policy() {
        assert_eq!(member_status_ttl(Some("3600")), Duration::hours(1));
        for value in [
            None,
            Some("bad"),
            Some("0"),
            Some("-1"),
            Some("9223372036854775807"),
        ] {
            assert_eq!(member_status_ttl(value), Duration::minutes(30));
        }
    }

    // Regression: c9c6c49b required a pre-existing launch sidecar, so F9c
    // terminal tasks missed between daemon snapshots never got an observation.
    #[test]
    fn wave2_terminal_observer_creates_missing_sidecars_once() {
        let root = TempDir::new().unwrap();
        let teams = root.path().join("teams");
        let tasks = root.path().join("tasks/completion-team");
        std::fs::create_dir_all(&tasks).unwrap();
        for (id, status) in [("3", "completed"), ("4", "stale"), ("5", "pending")] {
            std::fs::write(
                tasks.join(format!("{id}.json")),
                serde_json::json!({"id":id,"subject":"Observed task","status":status,
                    "stateChangedAt":"2026-09-03T10:10:00Z"})
                .to_string(),
            )
            .unwrap();
        }
        super::observe_terminal_tasks(&teams, "completion-team", Utc::now());
        super::observe_terminal_tasks(&teams, "completion-team", Utc::now());
        for id in ["3", "4"] {
            let events = crate::coordination::stores::telemetry::read_task_telemetry(
                &teams.join(format!("completion-team/state/telemetry/{id}.jsonl")),
            );
            assert_eq!(events.len(), 1, "terminal task {id}");
        }
        assert!(!teams
            .join("completion-team/state/telemetry/5.jsonl")
            .exists());
    }

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
            recovery_card: None,
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
    #[test]
    fn recovery_deadline_nudge_contains_identity_wait_and_action_without_card() {
        let facts = crate::coordination::recovery_card::AssignmentFacts {
            task_id: "17".into(),
            assignment_token: "token-1".into(),
            stage_id: "review".into(),
            wait: "awaiting_go".into(),
            ..Default::default()
        };
        let text = render_deadline_nudge(&facts, 20);
        for required in ["17", "token-1", "review", "awaiting_go", "Next action:"] {
            assert!(text.contains(required));
        }
        assert!(!text.contains("recovery_card"));
        assert!(!text.contains("continue immediately"));
    }
}
