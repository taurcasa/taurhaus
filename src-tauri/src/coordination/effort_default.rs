//! Keeping a harness's runtime effort change out of the user's own settings.
//!
//! Claude Code's `/effort <level>` does two things: it changes the running
//! session, and it saves the level as the user's default for that model. mesh
//! types that command for every assignment, so a team run would quietly leave
//! the operator's own default rewritten long after the team stopped.
//!
//! taurhaus records the user's value before a managed member's first launch and
//! puts it back when the member — or the team — stops. Which harnesses have the
//! side effect, and where they save it, is declared in the registry
//! (`CliCapabilities::runtime_effort_default_sink`), so nothing here branches on
//! tool identity.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::session_scanner::cli_tool::EffortDefaultSink;
use taurhaus_lib::logging::emit_global;

/// The user's own saved default, captured before a harness overwrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedEffortDefault {
    /// Settings file it was read from, so a restore never has to re-resolve an
    /// account that may have moved since.
    pub settings_path: PathBuf,
    /// Model the default belongs to.
    pub model: String,
    /// The user's level. `None` means the harness had no saved default and the
    /// field must be removed again, not set to something.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
}

/// What a restore actually did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreOutcome {
    /// The user's value is back in the file.
    Restored,
    /// The file already carries the user's value.
    AlreadyRestored,
    /// Left alone: the value on disk is not the one the harness wrote, so the
    /// user changed it themselves after the team started.
    UserChanged,
    /// The settings file could not be read or written.
    Unavailable,
}

/// Read the user's saved default for `model` before a managed launch runs.
///
/// A missing or unreadable settings file still records — with no level — so the
/// restore removes whatever the harness later writes rather than leaving it.
pub fn record(
    sink: EffortDefaultSink,
    account_dir: &Path,
    model: &str,
) -> Option<RecordedEffortDefault> {
    let model = model.trim();
    if model.is_empty() {
        return None;
    }
    let settings_path = account_dir.join(sink.file);
    let level = read_settings(&settings_path)
        .as_ref()
        .and_then(|settings| current_level(settings, sink, model));

    let recorded = RecordedEffortDefault {
        settings_path,
        model: model.to_string(),
        level,
    };
    emit_effort_default(
        "effort.user_default.recorded",
        &recorded,
        recorded.level.as_deref(),
        None,
    );
    Some(recorded)
}

/// Put the user's default back.
///
/// `written_by_harness` is the level the harness was last asked to run at. The
/// field is only touched while it still holds that level: a user who changed it
/// by hand since keeps their change. Writing the same value twice is a no-op,
/// so a repeated teardown is safe.
pub fn restore(
    sink: EffortDefaultSink,
    recorded: &RecordedEffortDefault,
    written_by_harness: Option<&str>,
) -> RestoreOutcome {
    let Some(mut settings) = read_settings(&recorded.settings_path) else {
        return emit_restore(recorded, RestoreOutcome::Unavailable);
    };
    let current = current_level(&settings, sink, &recorded.model);

    if current.as_deref() == recorded.level.as_deref() {
        return emit_restore(recorded, RestoreOutcome::AlreadyRestored);
    }
    let written_by_harness = written_by_harness.map(str::trim).filter(|l| !l.is_empty());
    if let Some(expected) = written_by_harness {
        if !current
            .as_deref()
            .is_some_and(|level| level.eq_ignore_ascii_case(expected))
        {
            return emit_restore(recorded, RestoreOutcome::UserChanged);
        }
    }

    apply_level(
        &mut settings,
        sink,
        &recorded.model,
        recorded.level.as_deref(),
    );
    match write_settings(&recorded.settings_path, &settings) {
        Ok(()) => emit_restore(recorded, RestoreOutcome::Restored),
        Err(err) => {
            tracing::warn!(
                path = %recorded.settings_path.display(),
                error = %err,
                "failed to restore the user's saved effort default"
            );
            emit_restore(recorded, RestoreOutcome::Unavailable)
        }
    }
}

/// Put a member's recorded user default back, then forget it.
///
/// Forgetting matters: the next launch captures whatever the operator's value
/// is then, rather than restoring a level from an earlier run.
pub fn restore_member_effort_default(teams_dir: &Path, team_name: &str, member_name: &str) {
    let Ok(record) =
        crate::coordination::stores::MemberRuntimeStore::load(teams_dir, team_name, member_name)
    else {
        return;
    };
    let Some(recorded) = record.effort_default.as_ref() else {
        return;
    };
    let Some(sink) = record.cli_tool.and_then(|tool| {
        crate::session_scanner::cli_tool::spec(tool)
            .capabilities
            .runtime_effort_default_sink
    }) else {
        return;
    };

    restore(sink, recorded, record.applied_effort.as_deref());
    if let Err(err) = crate::coordination::stores::MemberRuntimeStore::update(
        teams_dir,
        team_name,
        member_name,
        |record| {
            record.effort_default = None;
        },
    ) {
        tracing::warn!(
            team = %team_name,
            member = %member_name,
            error = %err,
            "failed to clear the recorded effort default after restoring it"
        );
    }
}

/// Same, for the member that owns `pane_id`.
///
/// The Stop control knows only the pane it is stopping, so the member is found
/// by it. An unmanaged pane matches nothing and nothing is written.
pub fn restore_effort_default_for_pane(teams_dir: &Path, pane_id: &str) {
    let Ok(team_names) = crate::coordination::stores::TeamConfigStore::list(teams_dir) else {
        return;
    };
    for team_name in team_names {
        let Ok(records) =
            crate::coordination::stores::MemberRuntimeStore::load_all(teams_dir, &team_name)
        else {
            continue;
        };
        for (member_name, record) in records {
            if record.pane_id.as_deref() == Some(pane_id) && record.effort_default.is_some() {
                restore_member_effort_default(teams_dir, &team_name, &member_name);
                return;
            }
        }
    }
}

fn read_settings(path: &Path) -> Option<Value> {
    let raw = fs::read_to_string(path).ok()?;
    if raw.trim().is_empty() {
        return Some(Value::Object(Map::new()));
    }
    serde_json::from_str::<Value>(&raw)
        .ok()
        .filter(Value::is_object)
}

fn current_level(settings: &Value, sink: EffortDefaultSink, model: &str) -> Option<String> {
    settings
        .get(sink.section)?
        .get(model)?
        .get(sink.field)?
        .as_str()
        .map(str::trim)
        .filter(|level| !level.is_empty())
        .map(ToString::to_string)
}

/// Set the model's field to `level`, or remove it when the user had none.
///
/// Only the one field is touched: every other setting in the file, and every
/// other model's entry, is left exactly as it was.
fn apply_level(settings: &mut Value, sink: EffortDefaultSink, model: &str, level: Option<&str>) {
    let Some(root) = settings.as_object_mut() else {
        return;
    };
    match level {
        Some(level) => {
            root.entry(sink.section)
                .or_insert_with(|| Value::Object(Map::new()));
            let Some(section) = root.get_mut(sink.section).and_then(Value::as_object_mut) else {
                return;
            };
            section
                .entry(model)
                .or_insert_with(|| Value::Object(Map::new()));
            if let Some(entry) = section.get_mut(model).and_then(Value::as_object_mut) {
                entry.insert(sink.field.to_string(), Value::String(level.to_string()));
            }
        }
        None => {
            let Some(section) = root.get_mut(sink.section).and_then(Value::as_object_mut) else {
                return;
            };
            let model_became_empty = match section.get_mut(model).and_then(Value::as_object_mut) {
                Some(entry) => {
                    entry.remove(sink.field);
                    entry.is_empty()
                }
                None => false,
            };
            if model_became_empty {
                section.remove(model);
            }
            if section.is_empty() {
                root.remove(sink.section);
            }
        }
    }
}

/// Write through a sibling temp file so a crash cannot leave a half-written
/// settings file behind.
fn write_settings(path: &Path, settings: &Value) -> std::io::Result<()> {
    let payload = serde_json::to_string_pretty(settings)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    let tmp_path = path.with_extension("json.taurhaus-tmp");
    fs::write(&tmp_path, payload.as_bytes())?;
    if let Err(err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }
    Ok(())
}

fn emit_restore(recorded: &RecordedEffortDefault, outcome: RestoreOutcome) -> RestoreOutcome {
    emit_effort_default(
        "effort.user_default.restored",
        recorded,
        recorded.level.as_deref(),
        Some(match outcome {
            RestoreOutcome::Restored => "restored",
            RestoreOutcome::AlreadyRestored => "already_restored",
            RestoreOutcome::UserChanged => "user_changed",
            RestoreOutcome::Unavailable => "unavailable",
        }),
    );
    outcome
}

fn emit_effort_default(
    event_name: &str,
    recorded: &RecordedEffortDefault,
    level: Option<&str>,
    outcome: Option<&str>,
) {
    let mut fields = Map::new();
    fields.insert(
        "settings_path".to_string(),
        Value::String(recorded.settings_path.display().to_string()),
    );
    fields.insert("model".to_string(), Value::String(recorded.model.clone()));
    fields.insert(
        "user_default".to_string(),
        level
            .map(|level| Value::String(level.to_string()))
            .unwrap_or(Value::Null),
    );
    if let Some(outcome) = outcome {
        fields.insert("outcome".to_string(), Value::String(outcome.to_string()));
    }
    emit_global(
        if outcome == Some("unavailable") {
            "warn"
        } else {
            "info"
        },
        "coordination",
        event_name,
        Some("Harness effort user-default".to_string()),
        fields,
    );
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    const SINK: EffortDefaultSink = EffortDefaultSink {
        file: "settings.json",
        section: "modelSettings",
        field: "effortLevel",
    };

    /// A scratch account dir. Tests never read or write a real harness home.
    fn account_dir(contents: Option<&str>) -> TempDir {
        let tmp = TempDir::new().expect("tempdir");
        if let Some(contents) = contents {
            fs::write(tmp.path().join("settings.json"), contents).expect("write settings");
        }
        tmp
    }

    fn settings_json(dir: &TempDir) -> Value {
        serde_json::from_str(
            &fs::read_to_string(dir.path().join("settings.json")).expect("read settings"),
        )
        .expect("settings json")
    }

    #[test]
    fn the_users_own_level_is_recorded_before_the_harness_overwrites_it() {
        let dir = account_dir(Some(
            r#"{"theme":"dark","modelSettings":{"opus":{"effortLevel":"low"}}}"#,
        ));

        let recorded = record(SINK, dir.path(), "opus").expect("recorded");

        assert_eq!(recorded.level.as_deref(), Some("low"));
        assert_eq!(recorded.model, "opus");
        assert_eq!(recorded.settings_path, dir.path().join("settings.json"));
    }

    #[test]
    fn a_user_with_no_saved_default_records_the_absence() {
        let dir = account_dir(Some(r#"{"theme":"dark"}"#));

        let recorded = record(SINK, dir.path(), "opus").expect("recorded");

        assert_eq!(recorded.level, None);
    }

    #[test]
    fn a_missing_settings_file_still_records_so_the_write_is_undone() {
        let dir = account_dir(None);

        let recorded = record(SINK, dir.path(), "opus").expect("recorded");

        assert_eq!(recorded.level, None);
    }

    #[test]
    fn the_users_level_comes_back_when_the_team_stops() {
        let dir = account_dir(Some(
            r#"{"theme":"dark","modelSettings":{"opus":{"effortLevel":"low"}}}"#,
        ));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");

        // What mesh's `/effort high` leaves behind.
        fs::write(
            dir.path().join("settings.json"),
            r#"{"theme":"dark","modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");

        assert_eq!(
            restore(SINK, &recorded, Some("high")),
            RestoreOutcome::Restored
        );

        let settings = settings_json(&dir);
        assert_eq!(settings["modelSettings"]["opus"]["effortLevel"], "low");
        assert_eq!(settings["theme"], "dark", "nothing else is touched");
    }

    #[test]
    fn a_field_the_harness_invented_is_removed_again() {
        let dir = account_dir(Some(r#"{"theme":"dark"}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"theme":"dark","modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");

        assert_eq!(
            restore(SINK, &recorded, Some("high")),
            RestoreOutcome::Restored
        );

        let settings = settings_json(&dir);
        assert!(settings.get("modelSettings").is_none());
        assert_eq!(settings["theme"], "dark");
    }

    #[test]
    fn another_models_saved_default_is_left_alone() {
        let dir = account_dir(Some(
            r#"{"modelSettings":{"opus":{"effortLevel":"low"},"sonnet":{"effortLevel":"medium"}}}"#,
        ));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"},"sonnet":{"effortLevel":"medium"}}}"#,
        )
        .expect("harness write");

        restore(SINK, &recorded, Some("high"));

        let settings = settings_json(&dir);
        assert_eq!(settings["modelSettings"]["opus"]["effortLevel"], "low");
        assert_eq!(settings["modelSettings"]["sonnet"]["effortLevel"], "medium");
    }

    #[test]
    fn a_value_the_user_changed_since_is_kept() {
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"xhigh"}}}"#,
        )
        .expect("user write");

        assert_eq!(
            restore(SINK, &recorded, Some("high")),
            RestoreOutcome::UserChanged
        );
        assert_eq!(
            settings_json(&dir)["modelSettings"]["opus"]["effortLevel"],
            "xhigh"
        );
    }

    #[test]
    fn restoring_twice_changes_nothing_the_second_time() {
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");

        assert_eq!(
            restore(SINK, &recorded, Some("high")),
            RestoreOutcome::Restored
        );
        assert_eq!(
            restore(SINK, &recorded, Some("high")),
            RestoreOutcome::AlreadyRestored
        );
        assert_eq!(
            settings_json(&dir)["modelSettings"]["opus"]["effortLevel"],
            "low"
        );
    }

    #[test]
    fn a_settings_file_that_disappeared_is_reported_not_recreated() {
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::remove_file(dir.path().join("settings.json")).expect("remove settings");

        assert_eq!(
            restore(SINK, &recorded, Some("high")),
            RestoreOutcome::Unavailable
        );
        assert!(!dir.path().join("settings.json").exists());
    }

    fn seed_member_with_recorded_default(
        teams_dir: &Path,
        pane_id: &str,
        recorded: RecordedEffortDefault,
    ) {
        use crate::coordination::domain::{HealthState, Member, MemberRole};
        use crate::coordination::stores::{MemberRuntimeStore, TeamConfig, TeamConfigStore};

        let mut member = Member {
            name: "lead-dev".to_string(),
            role: MemberRole::Agent,
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
            model: Some("opus".to_string()),
            reasoning_effort: None,
            project_path: PathBuf::from("/tmp/app"),
            cli_tool: slash_command_tool_with_sink(),
            extra: Default::default(),
        };
        member.name = "lead-dev".to_string();
        TeamConfigStore::save(
            teams_dir,
            "effort-team",
            &TeamConfig {
                schema_version: 1,
                name: "effort-team".to_string(),
                description: None,
                created_at: chrono::Utc::now(),
                members: vec![member],
                extra: Default::default(),
            },
        )
        .expect("save team");

        let record = crate::coordination::stores::MemberRuntimeRecord {
            schema_version: 3,
            member_name: "lead-dev".to_string(),
            cli_tool: Some(slash_command_tool_with_sink()),
            project_path: Some(PathBuf::from("/tmp/app")),
            pane_id: Some(pane_id.to_string()),
            pane_pid: None,
            pane_start_time: None,
            session_id: None,
            jsonl_path: None,
            daemon_pid: None,
            health: HealthState::Healthy,
            delivery_lease: None,
            attached_at: None,
            last_seen_at: None,
            applied_effort: Some("high".to_string()),
            effort_default: Some(recorded),
            effort_resume_failure: None,
        };
        MemberRuntimeStore::save(teams_dir, "effort-team", "lead-dev", &record)
            .expect("save runtime");
    }

    fn slash_command_tool_with_sink() -> crate::session_scanner::cli_tool::CliTool {
        crate::session_scanner::cli_tool::all()
            .iter()
            .find(|entry| entry.capabilities.runtime_effort_default_sink.is_some())
            .expect("one harness saves the level as the user's default")
            .tool
    }

    #[test]
    fn stopping_a_member_puts_the_users_level_back_and_forgets_it() {
        let teams = TempDir::new().expect("teams dir");
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");
        seed_member_with_recorded_default(teams.path(), "%42", recorded);

        restore_member_effort_default(teams.path(), "effort-team", "lead-dev");

        assert_eq!(
            settings_json(&dir)["modelSettings"]["opus"]["effortLevel"],
            "low"
        );
        let record = crate::coordination::stores::MemberRuntimeStore::load(
            teams.path(),
            "effort-team",
            "lead-dev",
        )
        .expect("runtime record");
        assert_eq!(
            record.effort_default, None,
            "a restored default is not restored again on the next stop"
        );
    }

    #[test]
    fn stopping_a_pane_restores_the_member_that_owns_it() {
        // The Stop button knows only the pane, so the member is found by it.
        let teams = TempDir::new().expect("teams dir");
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");
        seed_member_with_recorded_default(teams.path(), "%42", recorded);

        restore_effort_default_for_pane(teams.path(), "%42");

        assert_eq!(
            settings_json(&dir)["modelSettings"]["opus"]["effortLevel"],
            "low"
        );
    }

    #[test]
    fn stopping_an_unmanaged_pane_touches_nothing() {
        let teams = TempDir::new().expect("teams dir");
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");
        seed_member_with_recorded_default(teams.path(), "%42", recorded);

        restore_effort_default_for_pane(teams.path(), "%99");

        assert_eq!(
            settings_json(&dir)["modelSettings"]["opus"]["effortLevel"],
            "high"
        );
    }

    #[test]
    fn a_member_with_no_model_records_nothing() {
        let dir = account_dir(Some(r#"{}"#));

        assert_eq!(record(SINK, dir.path(), "  "), None);
    }
}
