//! Pure policy for deadline actions on task assignments.

use chrono::{DateTime, Duration, Utc};

pub type Timestamp = DateTime<Utc>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineInput {
    pub assigned_at: Timestamp,
    pub deadline_minutes: u32,
    pub nudged_at: Option<Timestamp>,
    pub stale_at: Option<Timestamp>,
    /// Whether the assignment is still open. The operational layer owns the
    /// task-status vocabulary (`is_resumable_task_status`); the policy only
    /// receives its verdict, so the two can never disagree about a status.
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineAction {
    Nothing,
    Nudge,
    MarkStale,
}

/// Decide the one action due now, if any.
///
/// Stale supersedes nudge: an assignment first evaluated after its deadline
/// is marked stale and is never nudged — a nudge at that point would ask for
/// work the deadline already declared late.
pub fn decide(assignment: &DeadlineInput, now: Timestamp) -> DeadlineAction {
    if assignment.deadline_minutes == 0 || !assignment.active {
        return DeadlineAction::Nothing;
    }

    let deadline = Duration::minutes(i64::from(assignment.deadline_minutes));
    if now >= assignment.assigned_at + deadline {
        return if assignment.stale_at.is_none() {
            DeadlineAction::MarkStale
        } else {
            DeadlineAction::Nothing
        };
    }

    if now >= assignment.assigned_at + deadline / 2 && assignment.nudged_at.is_none() {
        return DeadlineAction::Nudge;
    }

    DeadlineAction::Nothing
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};

    use super::*;

    fn assigned_at() -> Timestamp {
        Utc.with_ymd_and_hms(2026, 8, 30, 12, 0, 0)
            .single()
            .expect("fixed timestamp")
    }

    fn input(
        active: bool,
        nudged_at: Option<Timestamp>,
        stale_at: Option<Timestamp>,
    ) -> DeadlineInput {
        DeadlineInput {
            assigned_at: assigned_at(),
            deadline_minutes: 20,
            nudged_at,
            stale_at,
            active,
        }
    }

    // Regression: 3ac444dd treated empty or unknown snapshot statuses as
    // active and a zero-minute deadline as stale immediately because the
    // policy used a terminal deny-list and an unguarded zero-duration window.
    #[test]
    fn deadline_actions_follow_half_and_full_deadline_one_shot_markers() {
        let assigned = assigned_at();
        let nudged = assigned + Duration::minutes(10);
        let stale = assigned + Duration::minutes(20);
        let cases = [
            (
                "before half",
                input(true, None, None),
                assigned + Duration::minutes(9),
                DeadlineAction::Nothing,
            ),
            (
                "at half first",
                input(true, None, None),
                nudged,
                DeadlineAction::Nudge,
            ),
            (
                "at half repeat",
                input(true, Some(nudged), None),
                nudged,
                DeadlineAction::Nothing,
            ),
            (
                "between first",
                input(true, None, None),
                assigned + Duration::minutes(15),
                DeadlineAction::Nudge,
            ),
            (
                "between repeat",
                input(true, Some(nudged), None),
                assigned + Duration::minutes(15),
                DeadlineAction::Nothing,
            ),
            (
                // Stale supersedes nudge: never nudge an assignment first seen late.
                "first evaluated after deadline, never nudged",
                input(true, None, None),
                assigned + Duration::minutes(25),
                DeadlineAction::MarkStale,
            ),
            (
                "at deadline first",
                input(true, Some(nudged), None),
                stale,
                DeadlineAction::MarkStale,
            ),
            (
                "at deadline repeat",
                input(true, Some(nudged), Some(stale)),
                stale,
                DeadlineAction::Nothing,
            ),
            (
                "after deadline",
                input(true, Some(nudged), None),
                stale + Duration::minutes(1),
                DeadlineAction::MarkStale,
            ),
            (
                "completed",
                input(false, None, None),
                stale,
                DeadlineAction::Nothing,
            ),
            (
                "cancelled",
                input(false, None, None),
                stale,
                DeadlineAction::Nothing,
            ),
            (
                "empty status",
                input(false, None, None),
                stale,
                DeadlineAction::Nothing,
            ),
            (
                "unknown status",
                input(false, None, None),
                stale,
                DeadlineAction::Nothing,
            ),
            (
                "zero deadline",
                DeadlineInput {
                    deadline_minutes: 0,
                    ..input(true, None, None)
                },
                assigned,
                DeadlineAction::Nothing,
            ),
        ];

        for (name, assignment, now, expected) in cases {
            assert_eq!(decide(&assignment, now), expected, "{name}");
        }
    }
}
