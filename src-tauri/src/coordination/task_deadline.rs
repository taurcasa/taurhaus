//! Pure policy for deadline actions on task assignments.

use chrono::{DateTime, Duration, Utc};

pub type Timestamp = DateTime<Utc>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineInput {
    pub assigned_at: Timestamp,
    pub deadline_minutes: u32,
    pub nudged_at: Option<Timestamp>,
    pub stale_at: Option<Timestamp>,
    pub status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineAction {
    Nothing,
    Nudge,
    MarkStale,
}

pub fn decide(assignment: &DeadlineInput, now: Timestamp) -> DeadlineAction {
    if matches!(assignment.status.trim(), "completed" | "cancelled") {
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
        status: &str,
        nudged_at: Option<Timestamp>,
        stale_at: Option<Timestamp>,
    ) -> DeadlineInput {
        DeadlineInput {
            assigned_at: assigned_at(),
            deadline_minutes: 20,
            nudged_at,
            stale_at,
            status: status.to_string(),
        }
    }

    #[test]
    fn deadline_actions_follow_half_and_full_deadline_one_shot_markers() {
        let assigned = assigned_at();
        let nudged = assigned + Duration::minutes(10);
        let stale = assigned + Duration::minutes(20);
        let cases = [
            (
                "before half",
                input("in_progress", None, None),
                assigned + Duration::minutes(9),
                DeadlineAction::Nothing,
            ),
            (
                "at half first",
                input("in_progress", None, None),
                nudged,
                DeadlineAction::Nudge,
            ),
            (
                "at half repeat",
                input("in_progress", Some(nudged), None),
                nudged,
                DeadlineAction::Nothing,
            ),
            (
                "between first",
                input("in_progress", None, None),
                assigned + Duration::minutes(15),
                DeadlineAction::Nudge,
            ),
            (
                "between repeat",
                input("in_progress", Some(nudged), None),
                assigned + Duration::minutes(15),
                DeadlineAction::Nothing,
            ),
            (
                "at deadline first",
                input("in_progress", Some(nudged), None),
                stale,
                DeadlineAction::MarkStale,
            ),
            (
                "at deadline repeat",
                input("in_progress", Some(nudged), Some(stale)),
                stale,
                DeadlineAction::Nothing,
            ),
            (
                "after deadline",
                input("in_progress", Some(nudged), None),
                stale + Duration::minutes(1),
                DeadlineAction::MarkStale,
            ),
            (
                "completed",
                input("completed", None, None),
                stale,
                DeadlineAction::Nothing,
            ),
            (
                "cancelled",
                input("cancelled", None, None),
                stale,
                DeadlineAction::Nothing,
            ),
        ];

        for (name, assignment, now, expected) in cases {
            assert_eq!(decide(&assignment, now), expected, "{name}");
        }
    }
}
