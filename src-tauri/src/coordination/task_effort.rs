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

use crate::coordination::stores::OperationalContextSnapshot;
use crate::session_scanner::cli_tool::{spec, CliTool, EffortFlag, RuntimeEffort};
use crate::session_scanner::launch::command_contains_flag;

/// The effort a lead attached to an assignment, with the reason for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentEffort {
    /// Level as the lead wrote it, trimmed and lowercased.
    pub level: String,
    /// Why the lead chose that level. Absent on a record written before the
    /// field was required, never on one mesh 0.2.22 wrote.
    pub why: Option<String>,
}

/// The effort the assignment a member is currently on carries.
///
/// Read off the member's operational snapshot, which pairs the task taurhaus
/// selected as active with the level and reason mesh persists on that task
/// record. Correlating on the task is the whole point: an inbox keeps every
/// assignment ever delivered, so its newest effort-bearing message outlives the
/// task it was asked for and would pair one task with another's level. A member
/// with no active task has no assignment effort — the level of finished work is
/// not what it is working under now.
pub fn active_task_effort(snapshot: &OperationalContextSnapshot) -> Option<AssignmentEffort> {
    if snapshot.task.id.trim().is_empty() {
        return None;
    }
    let level = trimmed(Some(&snapshot.assignment_footer.task_effort))?;
    Some(AssignmentEffort {
        level: level.to_ascii_lowercase(),
        why: trimmed(Some(&snapshot.assignment_footer.task_effort_why)),
    })
}

/// What a run of the effort pass is allowed to start.
///
/// A relaunch takes a session down, so the pass that starts one has to be the
/// event that made the assignment visible — not a timer, which would let a
/// member work at the wrong level for a whole interval and then stop it
/// mid-turn. The timer's job is only to pick up a switch that already tried and
/// failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffortPassScope {
    /// A task event: start any switch the member owes.
    TaskChanged,
    /// A background sweep: retry only a switch already recorded as failed.
    RetryPending,
}

/// Whether this harness changes effort by being relaunched.
///
/// The only runtime effort path taurhaus owns. A `SlashCommand` harness takes
/// the level in its own prompt, and mesh types it there before it delivers the
/// assignment notice — taurhaus must never send it a second time.
pub fn relaunches_for_effort(tool: CliTool) -> bool {
    spec(tool).capabilities.runtime_effort == RuntimeEffort::ResumeWithFlag
}

/// Whether the operator's own base command already pins the effort.
///
/// The launch renderer leaves a configured base alone: an effort the base
/// already carries is kept and the requested one is dropped with a note. So a
/// relaunch that has to put an assignment's level into force rewrites the
/// pinned value rather than appending beside it.
pub fn base_pins_effort(tool: CliTool, base: &str) -> bool {
    effort_key(tool).is_some_and(|key| command_contains_flag(base, key))
}

/// The same base command with the effort it pins replaced by `level`.
///
/// `None` when the base pins nothing to replace — the renderer appends the
/// requested level itself — and when the pin names no value token the rewrite
/// could take over, which is the one shape a relaunch cannot make carry the
/// level.
pub fn base_with_effort(tool: CliTool, base: &str, level: &str) -> Option<String> {
    match spec(tool).capabilities.effort_flag? {
        EffortFlag::Config { key, .. } => config_base_with_effort(base, key, level),
        EffortFlag::Argument { flag } => argument_base_with_effort(base, flag, level),
    }
}

/// Rewrite every `key = value` config assignment in `base` to carry `level`.
///
/// The whole assignment is replaced as one span, so the spaced spelling the
/// shell hands the harness inside a single quoted argument survives the
/// rewrite. The value is always written quoted: that is the form the harness
/// parses whatever the original spelling was.
fn config_base_with_effort(base: &str, key: &str, level: &str) -> Option<String> {
    let spans = config_assignment_spans(base, key);
    if spans.is_empty() {
        return None;
    }
    let mut rewritten = base.to_string();
    for (span, _) in spans.into_iter().rev() {
        rewritten.replace_range(span, &format!("{key}=\"{level}\""));
    }
    Some(rewritten)
}

/// Rewrite a plain `--flag value` / `--flag=value` effort argument.
fn argument_base_with_effort(base: &str, flag: &str, level: &str) -> Option<String> {
    let mut tokens: Vec<String> = base.split_whitespace().map(ToString::to_string).collect();
    let mut rewrote = false;
    for index in 0..tokens.len() {
        let bare = tokens[index].trim_start_matches(['\'', '"']).to_string();
        if bare.starts_with(&format!("{flag}=")) {
            tokens[index] = format!("{flag}={level}");
            rewrote = true;
        } else if bare == flag {
            // `--effort high`: the level is the token after the flag.
            if index + 1 >= tokens.len() {
                return None;
            }
            tokens[index + 1] = level.to_string();
            rewrote = true;
        }
    }
    rewrote.then(|| tokens.join(" "))
}

/// Every `key <ws> = <ws> value` assignment in `command`, as a byte span
/// covering the whole assignment and the value the harness would read.
///
/// Found on the raw string rather than on whitespace-separated tokens: the
/// shell strips the quoting, so `-c 'model_reasoning_effort = "low"'` is one
/// argument to the harness and three tokens to a naive split.
fn config_assignment_spans(command: &str, key: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let mut spans: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for (index, _) in command.match_indices(key) {
        if spans.last().is_some_and(|(span, _)| index < span.end) {
            continue;
        }
        let before = command[..index].chars().next_back();
        let starts_key = before.is_none_or(|character| {
            character.is_whitespace() || matches!(character, '\'' | '"' | '=')
        });
        if !starts_key {
            continue;
        }
        let after = &command[index + key.len()..];
        let equals_at = after.len() - after.trim_start().len();
        let Some(value_offset) = after[equals_at..]
            .strip_prefix('=')
            .map(|rest| equals_at + 1 + (rest.len() - rest.trim_start().len()))
        else {
            continue;
        };
        let Some((raw_len, value)) = read_config_value(&after[value_offset..]) else {
            continue;
        };
        let end = index + key.len() + value_offset + raw_len;
        spans.push((index..end, value));
    }
    spans
}

/// The value token at the start of `rest`, as its raw length and its content.
///
/// A quoted value ends at its closing quote; a bare one ends at the first
/// whitespace or quote, which is where the shell would end it too.
fn read_config_value(rest: &str) -> Option<(usize, String)> {
    let mut characters = rest.char_indices();
    let (_, first) = characters.next()?;
    if matches!(first, '\'' | '"') {
        let close = rest[first.len_utf8()..].find(first)?;
        let value = rest[first.len_utf8()..first.len_utf8() + close].to_string();
        return Some((first.len_utf8() * 2 + close, value));
    }
    let end = rest
        .find(|character: char| character.is_whitespace() || matches!(character, '\'' | '"'))
        .unwrap_or(rest.len());
    (end > 0).then(|| (end, rest[..end].to_string()))
}

/// The level a base command pins, read back the way the harness would.
///
/// The relaunch checks its own rewrite with this before it stops anything: a
/// member is only taken down for a command that demonstrably carries the level.
pub fn pinned_base_effort(tool: CliTool, base: &str) -> Option<String> {
    match spec(tool).capabilities.effort_flag? {
        EffortFlag::Config { key, .. } => config_assignment_spans(base, key)
            .into_iter()
            .find_map(|(_, value)| trimmed(Some(value.trim_matches(['\'', '"'])))),
        EffortFlag::Argument { flag } => {
            let tokens: Vec<&str> = base.split_whitespace().collect();
            for (index, token) in tokens.iter().enumerate() {
                let bare = token.trim_matches(['\'', '"']);
                if let Some(value) = bare.strip_prefix(&format!("{flag}=")) {
                    return trimmed(Some(value.trim_matches(['\'', '"'])));
                }
                if bare == flag {
                    return tokens
                        .get(index + 1)
                        .and_then(|value| trimmed(Some(value.trim_matches(['\'', '"']))));
                }
            }
            None
        }
    }
}

/// The token a harness's base command pins its effort with.
fn effort_key(tool: CliTool) -> Option<&'static str> {
    match spec(tool).capabilities.effort_flag? {
        EffortFlag::Argument { flag } => Some(flag),
        EffortFlag::Config { key, .. } => Some(key),
    }
}

/// The effort level taurhaus must put into force for a member, if any.
///
/// `None` for every harness whose level mesh already owns, for a member already
/// at the requested level, and for an assignment that carries no effort. The
/// comparison is the same one mesh makes before it submits the slash command,
/// so the two owners never both act on one assignment.
pub fn resume_effort_target(
    tool: CliTool,
    requested: Option<&str>,
    applied: Option<&str>,
) -> Option<String> {
    if !relaunches_for_effort(tool) {
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
    task_id: &str,
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
        "task_id".to_string(),
        serde_json::Value::String(task_id.to_string()),
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
    use chrono::{TimeZone, Utc};
    use serde_json::Value;

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

    fn snapshot(task_id: &str, level: &str, why: &str) -> OperationalContextSnapshot {
        use crate::coordination::stores::{
            OperationalAssignmentFooterSnapshot, OperationalOwnershipSnapshot,
            OperationalTaskSnapshot, OperationalWorkingSetSnapshot,
        };

        OperationalContextSnapshot {
            version: 1,
            team_name: "architecture-final".to_string(),
            member_name: "codex-reviewer".to_string(),
            updated_at: Utc.with_ymd_and_hms(2026, 8, 29, 9, 0, 0).unwrap(),
            task: OperationalTaskSnapshot {
                id: task_id.to_string(),
                subject: "Run the migration".to_string(),
                status: "in_progress".to_string(),
                ..Default::default()
            },
            assignment_footer: OperationalAssignmentFooterSnapshot {
                task_effort: level.to_string(),
                task_effort_why: why.to_string(),
                ..Default::default()
            },
            ownership: OperationalOwnershipSnapshot::default(),
            working_set: OperationalWorkingSetSnapshot {
                project_path: String::new(),
                focal_files: vec![],
            },
        }
    }

    #[test]
    fn an_active_task_carries_the_level_and_the_reason() {
        let effort = active_task_effort(&snapshot("42", "High", "the migration is irreversible"))
            .expect("the task carries an effort");

        assert_eq!(effort.level, "high");
        assert_eq!(effort.why.as_deref(), Some("the migration is irreversible"));
    }

    #[test]
    fn a_member_with_no_active_task_owes_no_level() {
        // The snapshot clears the pair with the task, but a record written
        // before that rule would still carry a level beside an empty task.
        assert_eq!(active_task_effort(&snapshot("", "high", "stale")), None);
    }

    #[test]
    fn a_task_without_an_effort_carries_none() {
        assert_eq!(active_task_effort(&snapshot("42", "   ", "")), None);
    }

    #[test]
    fn a_level_without_a_reason_still_reads_back() {
        let effort = active_task_effort(&snapshot("42", "medium", "  "))
            .expect("the task carries an effort");
        assert_eq!(effort.level, "medium");
        assert_eq!(effort.why, None);
    }

    #[test]
    fn only_a_relaunching_harness_is_taurhauss_to_switch() {
        // Every other harness takes the level in its own prompt, and mesh
        // types it there before the notice. A second submission from taurhaus
        // would arrive after the member could already read the assignment.
        for entry in crate::session_scanner::cli_tool::all() {
            let relaunches = entry.capabilities.runtime_effort == RuntimeEffort::ResumeWithFlag;
            assert_eq!(
                relaunches_for_effort(entry.tool),
                relaunches,
                "{} declares the wrong owner for a runtime effort change",
                entry.name
            );
            assert_eq!(
                resume_effort_target(entry.tool, Some("high"), None).as_deref(),
                relaunches.then_some("high"),
                "{} must only be switched by the owner that declares it",
                entry.name
            );
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

    #[test]
    fn a_pinned_config_value_is_replaced_by_the_requested_level() {
        let tool = resume_with_flag_tool();
        let base = "codex resume --last -c model_reasoning_effort=\"low\" --yolo";

        assert!(base_pins_effort(tool, base));
        assert_eq!(pinned_base_effort(tool, base).as_deref(), Some("low"));

        let rewritten = base_with_effort(tool, base, "high").expect("the pin is rewritable");
        assert_eq!(
            rewritten,
            "codex resume --last -c model_reasoning_effort=\"high\" --yolo"
        );
        assert_eq!(
            pinned_base_effort(tool, &rewritten).as_deref(),
            Some("high")
        );
    }

    // Regression: 0abb2e4 rewrote a pinned effort by splitting the base on
    // whitespace, so the spaced form Codex accepts inside one quoted argument
    // — `-c 'model_reasoning_effort = "low"'` — had its bare `=` token
    // replaced and the old value left standing beside it.
    #[test]
    fn a_spaced_config_pin_is_rewritten_as_one_assignment() {
        let tool = resume_with_flag_tool();
        let base = "codex resume --last -c 'model_reasoning_effort = \"low\"' --yolo";

        assert!(base_pins_effort(tool, base));
        assert_eq!(pinned_base_effort(tool, base).as_deref(), Some("low"));

        let rewritten = base_with_effort(tool, base, "high").expect("the pin is rewritable");
        assert_eq!(
            rewritten,
            "codex resume --last -c 'model_reasoning_effort=\"high\"' --yolo"
        );
        assert_eq!(
            pinned_base_effort(tool, &rewritten).as_deref(),
            Some("high"),
            "the rewrite has to read back as the level it was asked for"
        );
    }

    #[test]
    fn an_unquoted_config_pin_is_rewritten_into_a_quoted_one() {
        let tool = resume_with_flag_tool();
        let rewritten =
            base_with_effort(tool, "codex resume -c model_reasoning_effort=low", "high")
                .expect("the pin is rewritable");

        assert_eq!(rewritten, "codex resume -c model_reasoning_effort=\"high\"");
        assert_eq!(
            pinned_base_effort(tool, &rewritten).as_deref(),
            Some("high")
        );
    }

    #[test]
    fn a_base_that_pins_nothing_has_nothing_to_rewrite() {
        let tool = resume_with_flag_tool();
        assert_eq!(
            base_with_effort(tool, "codex resume --last --yolo", "high"),
            None
        );
    }

    #[test]
    fn a_pin_with_no_value_token_cannot_be_rewritten() {
        // The renderer would keep this pin and drop the requested level, so
        // the relaunch has no way to put the assignment's level into force.
        let tool = resume_with_flag_tool();
        assert!(base_pins_effort(
            tool,
            "codex resume -c model_reasoning_effort"
        ));
        assert_eq!(
            base_with_effort(tool, "codex resume -c model_reasoning_effort", "high"),
            None
        );
    }

    #[test]
    fn a_pinned_argument_flag_is_replaced_in_either_shape() {
        let tool = crate::session_scanner::cli_tool::all()
            .iter()
            .find(|entry| {
                matches!(
                    entry.capabilities.effort_flag,
                    Some(EffortFlag::Argument { .. })
                )
            })
            .expect("one harness pins effort with a plain argument")
            .tool;

        assert_eq!(
            base_with_effort(tool, "claude --effort low --yolo", "high").as_deref(),
            Some("claude --effort high --yolo")
        );
        assert_eq!(
            base_with_effort(tool, "claude --effort=low", "high").as_deref(),
            Some("claude --effort=high")
        );
    }

    fn resume_with_flag_tool() -> CliTool {
        crate::session_scanner::cli_tool::all()
            .iter()
            .find(|entry| entry.capabilities.runtime_effort == RuntimeEffort::ResumeWithFlag)
            .expect("one harness applies effort by relaunching")
            .tool
    }
}
