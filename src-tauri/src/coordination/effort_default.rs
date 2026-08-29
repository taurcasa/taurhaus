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
use std::io::Write;
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
    /// Left alone: the value on disk is not one the harness is known to have
    /// written, so it is the user's — either changed by hand after the team
    /// started, or never touched by the harness at all.
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

/// Serializes taurhaus's own read-compare-write of a settings file.
///
/// Two members of one team stop at the same moment; without this they could
/// both read the file and the second rename would drop the first's edit. It
/// cannot lock the harness out — nothing here can — which is why the ownership
/// check below refuses to write over a value taurhaus cannot prove it caused.
static RESTORE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Put the user's default back.
///
/// `written_by_harness` is the level taurhaus knows the harness was asked to
/// run at — mesh types the assignment's level, so that is the value that
/// reached the file. The field is only touched while it still holds that
/// level: a user who changed it by hand since keeps their change, and with no
/// such proof at all the value on disk is the user's and is left alone.
/// Writing the same value twice is a no-op, so a repeated teardown is safe.
pub fn restore(
    sink: EffortDefaultSink,
    recorded: &RecordedEffortDefault,
    written_by_harness: Option<&str>,
) -> RestoreOutcome {
    let _guard = RESTORE_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let Some(mut settings) = read_settings(&recorded.settings_path) else {
        return emit_restore(recorded, RestoreOutcome::Unavailable);
    };
    let current = current_level(&settings, sink, &recorded.model);

    if current.as_deref() == recorded.level.as_deref() {
        return emit_restore(recorded, RestoreOutcome::AlreadyRestored);
    }
    let written_by_harness = written_by_harness.map(str::trim).filter(|l| !l.is_empty());
    let harness_owns_current = written_by_harness.is_some_and(|expected| {
        current
            .as_deref()
            .is_some_and(|level| level.eq_ignore_ascii_case(expected))
    });
    if !harness_owns_current {
        return emit_restore(recorded, RestoreOutcome::UserChanged);
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

    // A restore that could not read or write the file has not run at all.
    // Forgetting the record here would leave the harness's level in the
    // operator's settings with nothing left to put back.
    let written = harness_written_level(&record);
    if restore(sink, recorded, written.as_deref()) == RestoreOutcome::Unavailable {
        return;
    }
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

/// The level taurhaus knows the harness was actually asked to run at.
///
/// The only evidence taurhaus has is the level in force in the member's own
/// runtime record diverging from the one taurhaus itself launched at: mesh
/// writes `appliedEffort` there before it types `/effort` into the pane, and
/// nothing else moves that field. An assignment is not evidence — mesh sends
/// no command at all when the assignment matches the level already in force —
/// so the inbox is never consulted, and a member still running at the level
/// taurhaus launched it at yields no proof and no write.
fn harness_written_level(
    record: &crate::coordination::stores::MemberRuntimeRecord,
) -> Option<String> {
    let applied = record
        .applied_effort
        .as_deref()
        .map(str::trim)
        .filter(|level| !level.is_empty())?;
    let launched = record
        .launch_effort
        .as_deref()
        .map(str::trim)
        .filter(|level| !level.is_empty());
    if launched.is_some_and(|level| level.eq_ignore_ascii_case(applied)) {
        return None;
    }
    Some(applied.to_string())
}

/// Same, for the member that owns `pane_id`.
///
/// The Stop control knows only the pane it is stopping, so the member is found
/// by it. An unmanaged pane matches nothing and nothing is written.
/// Put a pane's recorded default back, but only once its stop has succeeded.
///
/// Restoring first and stopping second gives the operator's value back to a
/// session that is still running and still able to rewrite it, and a stop that
/// then fails has already discarded the record a later stop would need. Taking
/// the stop's own outcome makes that ordering impossible to get wrong: the
/// value cannot be read before the stop has produced one.
pub fn restore_effort_default_for_stopped_pane(
    teams_dir: &Path,
    pane_id: &str,
    stop_outcome: &Result<(), String>,
) {
    if stop_outcome.is_err() {
        tracing::debug!(
            pane_id = %pane_id,
            "keeping the recorded effort default: the session was not stopped"
        );
        return;
    }
    restore_effort_default_for_pane(teams_dir, pane_id);
}

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

/// Serial number for this process's temp files, so two members restoring at
/// once never write through the same one.
static TEMP_FILE_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Write through a temp file beside the target so a crash cannot leave a
/// half-written settings file behind.
///
/// The target is the link's destination where the operator linked their
/// settings into a dotfiles repo — renaming over the link would replace it
/// with a regular file and every later edit would go somewhere else. The temp
/// file carries this process and a serial in its name so concurrent restores
/// cannot race through one path, takes the target's own permissions rather
/// than the default ones, and is flushed to disk before the rename.
fn write_settings(path: &Path, settings: &Value) -> std::io::Result<()> {
    let payload = serde_json::to_string_pretty(settings)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = target.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "settings path has no directory",
        )
    })?;
    let serial = TEMP_FILE_SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp_path = dir.join(format!(
        ".taurhaus-effort-{}-{serial}.tmp",
        std::process::id()
    ));

    let write = || -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(payload.as_bytes())?;
        file.sync_all()?;
        drop(file);
        if let Ok(metadata) = fs::metadata(&target) {
            fs::set_permissions(&tmp_path, metadata.permissions())?;
        }
        fs::rename(&tmp_path, &target)
    };
    if let Err(err) = write() {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }
    // The rename is only durable once the directory entry is.
    if let Ok(handle) = fs::File::open(dir) {
        let _ = handle.sync_all();
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
        // What a member launched with no declared level looks like once mesh
        // has typed `/effort high` into its pane and recorded the level.
        seed_member_with_recorded_default_and_effort(
            teams_dir,
            pane_id,
            recorded,
            Some("high".to_string()),
            None,
        )
    }

    fn seed_member_with_recorded_default_and_effort(
        teams_dir: &Path,
        pane_id: &str,
        recorded: RecordedEffortDefault,
        applied_effort: Option<String>,
        launch_effort: Option<String>,
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
            applied_effort,
            launch_effort,
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

    // Regression: 45cd190 wrote a sibling temp file and renamed it over the
    // configured path, which replaces a symlink with a regular file. An
    // operator whose `settings.json` links into a dotfiles repo lost the link
    // and every later edit went to the wrong file.
    #[cfg(unix)]
    #[test]
    fn a_settings_symlink_is_written_through_rather_than_replaced() {
        let dir = account_dir(None);
        let real = dir.path().join("dotfiles-settings.json");
        fs::write(&real, r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#).expect("write");
        std::os::unix::fs::symlink(&real, dir.path().join("settings.json")).expect("symlink");

        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            &real,
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");

        assert_eq!(
            restore(SINK, &recorded, Some("high")),
            RestoreOutcome::Restored
        );
        assert!(
            fs::symlink_metadata(dir.path().join("settings.json"))
                .expect("settings metadata")
                .file_type()
                .is_symlink(),
            "the operator's link must survive the write"
        );
        let written: Value =
            serde_json::from_str(&fs::read_to_string(&real).expect("read target")).expect("json");
        assert_eq!(written["modelSettings"]["opus"]["effortLevel"], "low");
    }

    // Regression: the same write created the replacement with default
    // permissions, so a settings file the operator had locked down came back
    // world-readable.
    #[cfg(unix)]
    #[test]
    fn a_restore_keeps_the_settings_files_own_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let path = dir.path().join("settings.json");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("chmod");
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            &path,
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("chmod");

        restore(SINK, &recorded, Some("high"));

        let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the operator's own file mode must survive");
    }

    #[test]
    fn a_restore_leaves_no_temp_file_behind() {
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");

        restore(SINK, &recorded, Some("high"));

        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "settings.json")
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    // Regression: 45cd190 cleared the recorded default whatever the restore
    // reported. A settings file that was momentarily unreadable — locked,
    // half-written, on a disconnected share — therefore threw away the only
    // record of the operator's own level, and mesh's value stayed forever.
    #[test]
    fn a_restore_that_could_not_run_keeps_the_record_for_the_next_try() {
        let teams = TempDir::new().expect("teams dir");
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");
        seed_member_with_recorded_default(teams.path(), "%42", recorded);

        // Unreadable right now: the restore cannot know what it would replace.
        fs::write(dir.path().join("settings.json"), "{ not json").expect("corrupt settings");
        restore_member_effort_default(teams.path(), "effort-team", "lead-dev");

        let record = crate::coordination::stores::MemberRuntimeStore::load(
            teams.path(),
            "effort-team",
            "lead-dev",
        )
        .expect("runtime record");
        assert!(
            record.effort_default.is_some(),
            "a restore that never ran must stay pending"
        );

        // The next stop finds the file readable again and puts the level back.
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("readable again");
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
        assert_eq!(record.effort_default, None);
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

    // Regression: 53b2e63 restored the operator's own default from the Stop
    // command before the stop request was issued, and the restore cleared the
    // record for every outcome but an unreadable settings file. A stop that
    // then failed left the session live and still able to rewrite the level,
    // with nothing left for a later stop to put back.
    #[test]
    fn a_stop_that_failed_keeps_the_recorded_default() {
        let teams = TempDir::new().expect("teams dir");
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");
        seed_member_with_recorded_default(teams.path(), "%42", recorded);

        restore_effort_default_for_stopped_pane(
            teams.path(),
            "%42",
            &Err("Failed to stop session".to_string()),
        );

        assert_eq!(
            settings_json(&dir)["modelSettings"]["opus"]["effortLevel"],
            "high",
            "a session that is still running keeps the level it is running at"
        );
        let record = crate::coordination::stores::MemberRuntimeStore::load(
            teams.path(),
            "effort-team",
            "lead-dev",
        )
        .expect("runtime record");
        assert!(
            record.effort_default.is_some(),
            "the operator's own value is still there for a later stop to put back"
        );
    }

    #[test]
    fn a_stop_that_succeeded_restores_and_forgets() {
        let teams = TempDir::new().expect("teams dir");
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");
        seed_member_with_recorded_default(teams.path(), "%42", recorded);

        restore_effort_default_for_stopped_pane(teams.path(), "%42", &Ok(()));

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
        assert!(record.effort_default.is_none());
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

    fn assignment_message(level: &str) -> crate::coordination::stores::MeshInboxMessage {
        let mut message = crate::coordination::stores::MeshInboxMessage::new(
            "team-lead",
            format!("Effort: {level} — the migration is irreversible"),
            None,
            chrono::Utc::now(),
        );
        message
            .extra
            .insert("effort".to_string(), serde_json::json!(level));
        message
    }

    // Regression: 45cd190 ran the ownership check only when it was handed a
    // level the harness had been asked for. A launch that declares no effort
    // records none, so the common case wrote unconditionally: an operator who
    // changed their own default while such a team ran had that change reverted
    // on stop, with nothing to say the harness had ever touched the file.
    #[test]
    fn a_value_the_harness_never_wrote_is_not_reverted() {
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        // The operator's own change, made while the team ran.
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"xhigh"}}}"#,
        )
        .expect("user write");

        assert_eq!(restore(SINK, &recorded, None), RestoreOutcome::UserChanged);
        assert_eq!(
            settings_json(&dir)["modelSettings"]["opus"]["effortLevel"],
            "xhigh",
            "an unproven value on disk belongs to the user"
        );
    }

    // Regression: d0f1ff8 took the newest effort-bearing message in a member's
    // inbox as proof that mesh had rewritten the operator's settings. An inbox
    // record proves only that an assignment exists: mesh skips `/effort`
    // entirely when the assignment matches the level the member already runs
    // at, and its pane delivery can fail. A member launched at `high` whose
    // assignment is also `high` never gets the command — and an operator who
    // then set their own default to `high` had it replaced with the captured
    // `low` when the member stopped.
    #[test]
    fn an_assignment_mesh_never_had_to_type_is_not_proof_of_a_harness_write() {
        let teams = TempDir::new().expect("teams dir");
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        // Launched at `high`, and still at `high`: taurhaus put that level
        // there itself, so nothing has asked the harness to change it.
        seed_member_with_recorded_default_and_effort(
            teams.path(),
            "%42",
            recorded,
            Some("high".to_string()),
            Some("high".to_string()),
        );
        crate::coordination::stores::MeshInboxStore::append(
            teams.path(),
            "effort-team",
            "lead-dev",
            &assignment_message("high"),
        )
        .expect("append assignment");
        // The operator's own change, made while the team ran.
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("user write");

        restore_member_effort_default(teams.path(), "effort-team", "lead-dev");

        assert_eq!(
            settings_json(&dir)["modelSettings"]["opus"]["effortLevel"],
            "high",
            "an assignment is not evidence that anything was written"
        );
    }

    // The other side of the same rule: mesh records the level in the member's
    // runtime record before it types `/effort`, so a level in force that
    // taurhaus did not launch at is the proof — and here the member was
    // launched declaring none at all.
    #[test]
    fn a_level_taurhaus_never_launched_at_is_proof_enough_to_put_the_users_back() {
        let teams = TempDir::new().expect("teams dir");
        let dir = account_dir(Some(r#"{"modelSettings":{"opus":{"effortLevel":"low"}}}"#));
        let recorded = record(SINK, dir.path(), "opus").expect("recorded");
        seed_member_with_recorded_default_and_effort(
            teams.path(),
            "%42",
            recorded,
            Some("high".to_string()),
            None,
        );
        // What `/effort high` left in the operator's own settings.
        fs::write(
            dir.path().join("settings.json"),
            r#"{"modelSettings":{"opus":{"effortLevel":"high"}}}"#,
        )
        .expect("harness write");

        restore_member_effort_default(teams.path(), "effort-team", "lead-dev");

        assert_eq!(
            settings_json(&dir)["modelSettings"]["opus"]["effortLevel"],
            "low"
        );
    }
}
