//! Task-level effort: the level a lead attached to an assignment, and how it
//! reaches a member that is already running.
//!
//! mesh owns the assignment. `mesh task assign` requires `--effort` and
//! `--why`, persists both on the task record and on the inbox message the
//! assignee receives, and — for a harness whose registry row declares
//! [`RuntimeEffort::SlashCommand`] — types `/effort <level>` into the pane
//! before it delivers the notice. taurhaus reads the pair back for its own
//! surfaces and owns the one path mesh cannot take: relaunching a
//! [`RuntimeEffort::ResumeWithFlag`] member with the effort flag.

use chrono::{DateTime, Utc};

use crate::coordination::stores::MeshInboxMessage;
use crate::session_scanner::cli_tool::{spec, CliTool, RuntimeEffort};

/// The effort a lead attached to an assignment, with the reason for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentEffort {
    /// Level as the lead wrote it, trimmed and lowercased.
    pub level: String,
    /// Why the lead chose that level. Absent on a record written before the
    /// field was required, never on one mesh 0.2.22 wrote.
    pub why: Option<String>,
}

/// Effort carried by one inbox message, if it carries one.
///
/// mesh serializes its inbox messages in camelCase (`effortWhy`) and its task
/// metadata in snake_case (`effort_why`); both spellings are read so the same
/// helper works on either record.
pub fn message_effort(message: &MeshInboxMessage) -> Option<AssignmentEffort> {
    let level = trimmed(message.extra.get("effort").and_then(|value| value.as_str()))?;
    let why = ["effortWhy", "effort_why"]
        .iter()
        .find_map(|key| trimmed(message.extra.get(*key).and_then(|value| value.as_str())));
    Some(AssignmentEffort {
        level: level.to_ascii_lowercase(),
        why,
    })
}

/// The newest assignment effort in a member's inbox.
///
/// mesh appends, so the last message carrying an effort is the current
/// assignment. Messages after it — a nudge, a question — carry no effort and
/// must not clear the level the member is working under.
pub fn latest_assignment_effort(messages: &[MeshInboxMessage]) -> Option<AssignmentEffort> {
    messages.iter().rev().find_map(message_effort)
}

/// The newest assignment effort delivered since `since`.
///
/// A relaunch takes a member's session down, so it may only answer an
/// assignment the running session has not already been through. An older
/// record carries no applied level, and an inbox keeps every assignment ever
/// delivered: without this an upgrade — or an operator restarting a member by
/// hand — would take the pane straight back down for work that is long done.
/// The timestamp mesh wrote is the only thing that separates the two.
pub fn assignment_effort_since(
    messages: &[MeshInboxMessage],
    since: DateTime<Utc>,
) -> Option<AssignmentEffort> {
    messages
        .iter()
        .rev()
        .find(|message| message_effort(message).is_some())
        .filter(|message| {
            DateTime::parse_from_rfc3339(&message.timestamp)
                .is_ok_and(|delivered| delivered.with_timezone(&Utc) >= since)
        })
        .and_then(message_effort)
}

/// The effort level a member must be relaunched to reach, if any.
///
/// `None` for every harness that takes the level through its own prompt, for a
/// member already at the requested level, and for an assignment that carries no
/// effort. The comparison is the same one mesh makes before it submits the
/// slash command, so the two owners never both act on one assignment.
pub fn resume_effort_target(
    tool: CliTool,
    requested: Option<&str>,
    applied: Option<&str>,
) -> Option<String> {
    if spec(tool).capabilities.runtime_effort != RuntimeEffort::ResumeWithFlag {
        return None;
    }
    let requested = trimmed(requested)?;
    if applied
        .map(str::trim)
        .is_some_and(|level| level.eq_ignore_ascii_case(&requested))
    {
        return None;
    }
    Some(requested.to_ascii_lowercase())
}

/// Report that a member is being relaunched to reach an assignment's effort.
pub fn emit_effort_resume(
    event_name: &str,
    team_name: &str,
    member_name: &str,
    level: &str,
    previous: Option<&str>,
    failure: Option<&str>,
) {
    let mut fields = serde_json::Map::new();
    fields.insert(
        "team_name".to_string(),
        serde_json::Value::String(team_name.to_string()),
    );
    fields.insert(
        "member_name".to_string(),
        serde_json::Value::String(member_name.to_string()),
    );
    fields.insert(
        "effort".to_string(),
        serde_json::Value::String(level.to_string()),
    );
    fields.insert(
        "previous_effort".to_string(),
        previous
            .map(|level| serde_json::Value::String(level.to_string()))
            .unwrap_or(serde_json::Value::Null),
    );
    if let Some(failure) = failure {
        fields.insert(
            "fail_reason".to_string(),
            serde_json::Value::String(failure.to_string()),
        );
    }
    taurhaus_lib::logging::emit_global(
        if failure.is_some() { "warn" } else { "info" },
        "coordination",
        event_name,
        Some("Task-level effort resume".to_string()),
        fields,
    );
}

fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::{json, Value};

    use super::*;

    /// The first mesh release whose `task assign` carries `--effort` and
    /// `--why`. Everything in this module reads a pair only that release
    /// writes.
    const ASSIGNMENT_EFFORT_MESH_VERSION: (u32, u32, u32) = (0, 2, 22);

    // Regression: the W5b read-back shipped on top of bundled mesh 0.2.21,
    // whose `mesh task assign` has neither `--effort` nor `--why`, so the two
    // bundled lead roles instructed a command the bundled binary rejects.
    #[test]
    fn the_bundled_mesh_carries_the_assignment_effort_flags() {
        let lock =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/mesh.lock.json");
        let raw = std::fs::read_to_string(&lock).expect("bundled mesh lock manifest");
        let pinned: Value = serde_json::from_str(&raw).expect("lock manifest is json");
        let version = pinned["version"].as_str().expect("pinned mesh version");
        let parts: Vec<u32> = version
            .split('.')
            .map(|part| part.parse().expect("numeric mesh version part"))
            .collect();
        let pinned = (parts[0], parts[1], parts[2]);

        assert!(
            pinned >= ASSIGNMENT_EFFORT_MESH_VERSION,
            "bundled mesh {version} predates the assignment effort contract \
             ({ASSIGNMENT_EFFORT_MESH_VERSION:?}); `mesh task assign --effort/--why` would fail"
        );
    }

    fn message(from: &str, text: &str, effort: Option<(&str, Option<&str>)>) -> MeshInboxMessage {
        let now = Utc.with_ymd_and_hms(2026, 8, 29, 9, 0, 0).unwrap();
        let mut message = MeshInboxMessage::new(from, text.to_string(), None, now);
        if let Some((level, why)) = effort {
            message
                .extra
                .insert("effort".to_string(), json!(level.to_string()));
            if let Some(why) = why {
                message
                    .extra
                    .insert("effortWhy".to_string(), json!(why.to_string()));
            }
        }
        message
    }

    #[test]
    fn an_assignment_message_carries_the_level_and_the_reason() {
        let effort = message_effort(&message(
            "lead",
            "Effort: high — the migration is irreversible",
            Some(("high", Some("the migration is irreversible"))),
        ))
        .expect("assignment carries effort");

        assert_eq!(effort.level, "high");
        assert_eq!(effort.why.as_deref(), Some("the migration is irreversible"));
    }

    #[test]
    fn a_snake_case_reason_reads_the_same_as_the_camel_case_one() {
        // The inbox message spells it `effortWhy`; the task record mesh writes
        // alongside it spells the same value `effort_why`.
        let mut message = message("lead", "assignment", Some(("medium", None)));
        message.extra.insert(
            "effort_why".to_string(),
            Value::String("routine lane work".to_string()),
        );

        let effort = message_effort(&message).expect("assignment carries effort");
        assert_eq!(effort.why.as_deref(), Some("routine lane work"));
    }

    #[test]
    fn a_message_without_an_effort_carries_none() {
        assert_eq!(
            message_effort(&message("lead", "any progress?", None)),
            None
        );
    }

    #[test]
    fn a_blank_level_is_not_an_effort() {
        assert_eq!(
            message_effort(&message("lead", "assignment", Some(("   ", None)))),
            None
        );
    }

    #[test]
    fn the_newest_assignment_wins_and_a_later_nudge_does_not_clear_it() {
        let messages = vec![
            message("lead", "first", Some(("low", Some("trivial")))),
            message("lead", "second", Some(("high", Some("irreversible")))),
            message("lead", "any progress?", None),
        ];

        let effort = latest_assignment_effort(&messages).expect("latest assignment");
        assert_eq!(effort.level, "high");
        assert_eq!(effort.why.as_deref(), Some("irreversible"));
    }

    #[test]
    fn an_assignment_delivered_before_the_session_started_is_not_current() {
        let attached = Utc.with_ymd_and_hms(2026, 8, 29, 12, 0, 0).unwrap();
        let mut old = message("lead", "first", Some(("high", Some("irreversible"))));
        old.timestamp = Utc
            .with_ymd_and_hms(2026, 8, 29, 9, 0, 0)
            .unwrap()
            .to_rfc3339();

        assert_eq!(assignment_effort_since(&[old.clone()], attached), None);

        let mut fresh = old;
        fresh.timestamp = Utc
            .with_ymd_and_hms(2026, 8, 29, 12, 30, 0)
            .unwrap()
            .to_rfc3339();
        assert_eq!(
            assignment_effort_since(&[fresh], attached)
                .expect("a fresh assignment counts")
                .level,
            "high"
        );
    }

    #[test]
    fn an_unreadable_timestamp_is_not_treated_as_current() {
        // A record taurhaus cannot date is not a reason to take a pane down.
        let mut message = message("lead", "first", Some(("high", None)));
        message.timestamp = "not a timestamp".to_string();

        assert_eq!(
            assignment_effort_since(
                &[message],
                Utc.with_ymd_and_hms(2026, 8, 29, 12, 0, 0).unwrap()
            ),
            None
        );
    }

    #[test]
    fn an_empty_inbox_carries_no_effort() {
        assert_eq!(latest_assignment_effort(&[]), None);
    }

    #[test]
    fn only_a_resume_with_flag_harness_is_relaunched_for_effort() {
        for entry in crate::session_scanner::cli_tool::all() {
            let target = resume_effort_target(entry.tool, Some("high"), None);
            match entry.capabilities.runtime_effort {
                RuntimeEffort::ResumeWithFlag => {
                    assert_eq!(
                        target.as_deref(),
                        Some("high"),
                        "{} has no other way to change effort",
                        entry.name
                    );
                }
                RuntimeEffort::SlashCommand | RuntimeEffort::None => {
                    assert_eq!(
                        target, None,
                        "{} must not be relaunched for an effort change",
                        entry.name
                    );
                }
            }
        }
    }

    #[test]
    fn a_member_already_at_the_level_is_not_relaunched() {
        let tool = resume_with_flag_tool();
        assert_eq!(resume_effort_target(tool, Some("high"), Some("high")), None);
        assert_eq!(
            resume_effort_target(tool, Some("high"), Some(" High ")),
            None,
            "the level is compared the way mesh compares it"
        );
    }

    #[test]
    fn a_different_level_relaunches_and_normalizes_the_target() {
        let tool = resume_with_flag_tool();
        assert_eq!(
            resume_effort_target(tool, Some("XHigh"), Some("medium")).as_deref(),
            Some("xhigh")
        );
    }

    #[test]
    fn an_assignment_without_an_effort_never_relaunches() {
        let tool = resume_with_flag_tool();
        assert_eq!(resume_effort_target(tool, None, Some("medium")), None);
        assert_eq!(resume_effort_target(tool, Some("  "), None), None);
    }

    fn resume_with_flag_tool() -> CliTool {
        crate::session_scanner::cli_tool::all()
            .iter()
            .find(|entry| entry.capabilities.runtime_effort == RuntimeEffort::ResumeWithFlag)
            .expect("one harness applies effort by relaunching")
            .tool
    }
}
