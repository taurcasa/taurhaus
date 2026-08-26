//! The status-line bridge that feeds `claude-usage.jsonl`.
//!
//! Claude Code has exactly one documented place where a subscription's usage
//! leaves the product: the `statusLine` command, which receives the session's
//! JSON — `rate_limits` included — on stdin at every refresh. Taking that seat
//! means writing into a config dir the user also owns, so the rule here is that
//! nothing the user configured may stop rendering:
//!
//! * a config dir with no `statusLine` gets ours, and ours prints a line
//!   (`<model> · 5h n% · 7d n%`) — verified live on 2.1.246, a status-line
//!   command that prints nothing leaves the row blank rather than falling back
//!   to a built-in line;
//! * a config dir that already has one gets **wrapped**: our script tees the
//!   payload to the sink and then pipes the very same JSON to the command that
//!   was configured before, whose stdout and exit code are the status line;
//! * removal puts the original `statusLine` value back exactly as it was,
//!   extra keys and all.
//!
//! Installation is idempotent and mirrors the compaction hook installer: a
//! generated script under `<config dir>/hooks`, a record naming the executable
//! and the sink it was generated for (so an app that moved, or a run under an
//! isolated `TAURHAUS_DATA_DIR`, reinstalls itself) and one entry in
//! `settings.json`, written atomically. Both paths are baked into the script
//! rather than resolved when it runs: it runs in the user's own shell, which
//! knows nothing of taurhaus's environment.

use std::fs;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::models::CliVersions;
use crate::provider::path;
use crate::session_scanner::claude_accounts::detect_claude_accounts_cached;
use taurhaus_lib::logging::emit_global;

const SETTINGS_FILENAME: &str = "settings.json";
const HOOKS_DIRNAME: &str = "hooks";
const SCRIPT_BASENAME: &str = "taurhaus-statusline";
const RECORD_FILENAME: &str = "taurhaus-statusline.json";
const STATUS_LINE_KEY: &str = "statusLine";
/// The subcommand the generated script calls.
pub const USAGE_SINK_SUBCOMMAND: &str = "claude-usage-sink";

/// What one install did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatuslineInstall {
    /// Anything on disk changed. `false` on a re-run against a current install.
    pub changed: bool,
    /// A status line the user had configured is being kept alive by our script.
    pub wrapped: bool,
    /// Nothing was installed, and why.
    pub skipped: Option<&'static str>,
}

/// The record that lets a later run recognise its own install.
#[derive(Debug, Clone, PartialEq)]
struct StatuslineRecord {
    executable: String,
    /// Where the script this record describes appends its records.
    sink: String,
    /// The `statusLine` value that was configured before taurhaus wrapped it.
    wrapped: Option<Value>,
}

impl StatuslineRecord {
    fn to_value(&self) -> Value {
        json!({
            "executable": self.executable,
            "sink": self.sink,
            "wrapped": self.wrapped.clone().unwrap_or(Value::Null),
        })
    }

    fn from_value(value: &Value) -> Option<Self> {
        Some(Self {
            executable: value.get("executable")?.as_str()?.to_string(),
            sink: value.get("sink")?.as_str()?.to_string(),
            wrapped: value
                .get("wrapped")
                .filter(|wrapped| !wrapped.is_null())
                .cloned(),
        })
    }
}

/// Install (or refresh) the bridge in one config dir.
pub fn ensure_statusline_installed_at(
    config_dir: &Path,
    taurhaus_exe: &Path,
    sink_path: &Path,
) -> Result<StatuslineInstall, String> {
    if !is_posix_config_dir(config_dir) {
        // The sink is executed by the status line, from inside the shell that
        // runs Claude Code. A Windows-native config dir would need a Windows
        // runner for a Linux daemon binary; taurhaus does not have one, and a
        // half-installed bridge would cost the user their status line.
        return Ok(StatuslineInstall {
            skipped: Some("non_posix_config_dir"),
            ..StatuslineInstall::default()
        });
    }
    let Some(executable) = linux_path_string(taurhaus_exe) else {
        return Ok(StatuslineInstall {
            skipped: Some("executable_not_reachable"),
            ..StatuslineInstall::default()
        });
    };

    let hooks_dir = config_dir.join(HOOKS_DIRNAME);
    let script_path = hooks_dir.join(script_filename());
    let Some(script_command) = linux_path_string(&script_path) else {
        return Ok(StatuslineInstall {
            skipped: Some("script_not_reachable"),
            ..StatuslineInstall::default()
        });
    };
    let Some(config_dir_argument) = linux_path_string(config_dir) else {
        return Ok(StatuslineInstall {
            skipped: Some("config_dir_not_reachable"),
            ..StatuslineInstall::default()
        });
    };
    let Some(sink_argument) = linux_path_string(sink_path) else {
        return Ok(StatuslineInstall {
            skipped: Some("sink_not_reachable"),
            ..StatuslineInstall::default()
        });
    };

    let settings_path = config_dir.join(SETTINGS_FILENAME);
    let mut settings = load_settings(&settings_path)?;
    let existing = settings.get(STATUS_LINE_KEY).cloned();
    // Re-running against our own install must not wrap our own script: the
    // command the user actually configured is the one the record remembers.
    let wrapped = if is_taurhaus_status_line(existing.as_ref()) {
        read_record(&hooks_dir).and_then(|record| record.wrapped)
    } else {
        existing.filter(|value| !value.is_null())
    };

    fs::create_dir_all(&hooks_dir)
        .map_err(|error| format!("failed to create '{}': {error}", hooks_dir.display()))?;

    let script = render_script(
        &executable,
        &config_dir_argument,
        &sink_argument,
        wrapped_command(wrapped.as_ref()),
    );
    let script_changed = write_if_changed(&script_path, script.as_bytes())?;
    #[cfg(not(target_os = "windows"))]
    if script_changed {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script_path)
            .map_err(|error| format!("failed to stat '{}': {error}", script_path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).map_err(|error| {
            format!(
                "failed to make '{}' executable: {error}",
                script_path.display()
            )
        })?;
    }

    let record = StatuslineRecord {
        executable,
        sink: sink_argument,
        wrapped: wrapped.clone(),
    };
    let record_changed = write_if_changed(
        &hooks_dir.join(RECORD_FILENAME),
        serde_json::to_vec_pretty(&record.to_value())
            .map_err(|error| format!("failed to serialize the status line record: {error}"))?
            .as_slice(),
    )?;

    let desired = json!({
        "type": "command",
        "command": format!("bash {}", shell_quote(&script_command)),
    });
    let settings_changed = settings.get(STATUS_LINE_KEY) != Some(&desired);
    if settings_changed {
        settings.insert(STATUS_LINE_KEY.to_string(), desired);
        write_settings(&settings_path, &settings)?;
    }

    Ok(StatuslineInstall {
        changed: script_changed || record_changed || settings_changed,
        wrapped: wrapped.is_some(),
        skipped: None,
    })
}

/// Take the bridge back out, restoring whatever it wrapped.
pub fn remove_statusline_at(config_dir: &Path) -> Result<bool, String> {
    let hooks_dir = config_dir.join(HOOKS_DIRNAME);
    let settings_path = config_dir.join(SETTINGS_FILENAME);
    let mut settings = load_settings(&settings_path)?;
    let mut changed = false;

    if is_taurhaus_status_line(settings.get(STATUS_LINE_KEY)) {
        match read_record(&hooks_dir).and_then(|record| record.wrapped) {
            Some(original) => {
                settings.insert(STATUS_LINE_KEY.to_string(), original);
            }
            None => {
                settings.remove(STATUS_LINE_KEY);
            }
        }
        write_settings(&settings_path, &settings)?;
        changed = true;
    }

    for path in [
        hooks_dir.join(script_filename()),
        hooks_dir.join(RECORD_FILENAME),
    ] {
        match fs::remove_file(&path) {
            Ok(()) => changed = true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("failed to remove '{}': {error}", path.display())),
        }
    }

    Ok(changed)
}

/// Whether this config dir is currently bridged by this build.
pub fn statusline_is_installed_at(config_dir: &Path) -> bool {
    let hooks_dir = config_dir.join(HOOKS_DIRNAME);
    let Some(record) = read_record(&hooks_dir) else {
        return false;
    };
    let script_path = hooks_dir.join(script_filename());
    let Some(config_dir_argument) = linux_path_string(config_dir) else {
        return false;
    };
    let expected = render_script(
        &record.executable,
        &config_dir_argument,
        &record.sink,
        wrapped_command(record.wrapped.as_ref()),
    );
    if fs::read_to_string(&script_path).ok().as_deref() != Some(expected.as_str()) {
        return false;
    }
    load_settings(&config_dir.join(SETTINGS_FILENAME))
        .ok()
        .is_some_and(|settings| is_taurhaus_status_line(settings.get(STATUS_LINE_KEY)))
}

/// Install the bridge in every detected account, when the CLI can feed it.
///
/// A build older than the one this was verified against gets nothing: its
/// payload is not documented to carry `rate_limits`, and rewriting a user's
/// `statusLine` for numbers that never arrive is a bad trade.
pub fn install_statusline_for_detected_accounts(taurhaus_exe: &Path) {
    let versions = CliVersions::current();
    if !versions.claude_statusline_usage_supported {
        emit_skipped_run(versions.claude.as_deref());
        return;
    }

    // The path is resolved here, in the process that also reads the sink, and
    // baked into every script: the status line runs in the user's own shell,
    // which knows nothing of `TAURHAUS_DATA_DIR`.
    let sink_path = crate::provider::platform_paths::PlatformPaths::claude_usage_path();
    for account in detect_claude_accounts_cached() {
        let mut fields = Map::new();
        fields.insert(
            "config_dir".to_string(),
            Value::String(account.config_dir.display().to_string()),
        );
        fields.insert("account_id".to_string(), Value::String(account.id.clone()));
        match ensure_statusline_installed_at(&account.config_dir, taurhaus_exe, &sink_path) {
            Ok(install) => {
                if let Some(reason) = install.skipped {
                    fields.insert("reason".to_string(), Value::String(reason.to_string()));
                    emit_global(
                        "debug",
                        "claude_usage",
                        "claude.usage.statusline.skipped",
                        None,
                        fields,
                    );
                    continue;
                }
                if !install.changed {
                    continue;
                }
                fields.insert("wrapped".to_string(), Value::Bool(install.wrapped));
                // The sink is baked in, so two processes with different data
                // roots would each rewrite the script with their own. Logging
                // it makes that visible instead of silently empty usage.
                fields.insert(
                    "sink".to_string(),
                    Value::String(sink_path.display().to_string()),
                );
                emit_global(
                    "info",
                    "claude_usage",
                    "claude.usage.statusline.installed",
                    Some(format!(
                        "Claude usage status line installed for {}",
                        account.email
                    )),
                    fields,
                );
            }
            Err(error) => {
                fields.insert("error".to_string(), Value::String(error.clone()));
                emit_global(
                    "warn",
                    "claude_usage",
                    "claude.usage.statusline.failed",
                    Some(error),
                    fields,
                );
            }
        }
    }
}

fn emit_skipped_run(claude_version: Option<&str>) {
    let mut fields = Map::new();
    fields.insert(
        "reason".to_string(),
        Value::String("claude_version_without_rate_limits".to_string()),
    );
    if let Some(version) = claude_version {
        fields.insert("claude_version".to_string(), Value::String(version.into()));
    }
    emit_global(
        "debug",
        "claude_usage",
        "claude.usage.statusline.skipped",
        None,
        fields,
    );
}

/// The shell command that renders the wrapped status line, if there is one.
fn wrapped_command(wrapped: Option<&Value>) -> Option<String> {
    let value = wrapped?;
    let command = match value {
        Value::String(command) => command.as_str(),
        Value::Object(_) => value.get("command")?.as_str()?,
        _ => return None,
    };
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    Some(command.to_string())
}

fn render_script(
    executable: &str,
    config_dir: &str,
    sink_path: &str,
    wrapped: Option<String>,
) -> String {
    let sink = format!(
        "{} {USAGE_SINK_SUBCOMMAND} --config-dir {} --sink {}",
        shell_quote(executable),
        shell_quote(config_dir),
        shell_quote(sink_path)
    );
    let mut script = String::from(
        "#!/usr/bin/env bash\n\
         # taurhaus status line bridge — generated file, rewritten on every install.\n\
         # Records this subscription's 5-hour and 7-day rate limits.\n",
    );
    // No `set -e`: a sink that cannot run must cost the user a record, never a
    // status line.
    script.push_str("payload=\"$(cat)\"\n");
    match wrapped {
        Some(command) => {
            script.push_str(
                "# The status line below was configured before taurhaus wrapped it;\n\
                 # it receives the same payload and owns the rendered line.\n",
            );
            script.push_str(&format!(
                "printf '%s' \"$payload\" | {sink} >/dev/null 2>&1\n"
            ));
            script.push_str(&format!("printf '%s' \"$payload\" | {command}\n"));
        }
        None => {
            script.push_str(&format!(
                "printf '%s' \"$payload\" | {sink} --render 2>/dev/null\n"
            ));
            script.push_str("exit 0\n");
        }
    }
    script
}

fn is_taurhaus_status_line(value: Option<&Value>) -> bool {
    let Some(value) = value else {
        return false;
    };
    let command = match value {
        Value::String(command) => command.as_str(),
        Value::Object(_) => value.get("command").and_then(Value::as_str).unwrap_or(""),
        _ => "",
    };
    command.contains(SCRIPT_BASENAME)
}

fn read_record(hooks_dir: &Path) -> Option<StatuslineRecord> {
    let raw = fs::read_to_string(hooks_dir.join(RECORD_FILENAME)).ok()?;
    StatuslineRecord::from_value(&serde_json::from_str::<Value>(&raw).ok()?)
}

fn script_filename() -> String {
    format!("{SCRIPT_BASENAME}.sh")
}

fn load_settings(settings_path: &Path) -> Result<Map<String, Value>, String> {
    let raw = match fs::read_to_string(settings_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => {
            return Err(format!(
                "failed to read '{}': {error}",
                settings_path.display()
            ))
        }
    };
    if raw.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Object(settings)) => Ok(settings),
        Ok(_) => Err(format!(
            "Claude settings at '{}' are not a JSON object",
            settings_path.display()
        )),
        Err(error) => Err(format!(
            "failed to parse '{}': {error}",
            settings_path.display()
        )),
    }
}

/// Write settings the way Claude Code's own writer does: never in place, so a
/// reader never sees half a file.
fn write_settings(settings_path: &Path, settings: &Map<String, Value>) -> Result<(), String> {
    let payload = serde_json::to_vec_pretty(&Value::Object(settings.clone()))
        .map_err(|error| format!("failed to serialize Claude settings: {error}"))?;
    let parent = settings_path
        .parent()
        .ok_or_else(|| format!("'{}' has no parent", settings_path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
    let temp_path = settings_path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&temp_path, &payload)
        .map_err(|error| format!("failed to write '{}': {error}", temp_path.display()))?;
    if let Err(error) = fs::rename(&temp_path, settings_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "failed to replace '{}': {error}",
            settings_path.display()
        ));
    }
    Ok(())
}

fn write_if_changed(path: &Path, payload: &[u8]) -> Result<bool, String> {
    if fs::read(path).is_ok_and(|current| current == payload) {
        return Ok(false);
    }
    fs::write(path, payload)
        .map_err(|error| format!("failed to write '{}': {error}", path.display()))?;
    Ok(true)
}

fn is_posix_config_dir(config_dir: &Path) -> bool {
    let value = config_dir.display().to_string();
    value.starts_with('/') || path::is_wsl_path(&value)
}

/// The path as the shell that runs Claude Code sees it.
fn linux_path_string(value: &Path) -> Option<String> {
    let value = value.display().to_string();
    if value.starts_with('/') {
        return Some(value);
    }
    path::to_linux(&value)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn sink_for(temp: &tempfile::TempDir) -> std::path::PathBuf {
        temp.path().join("app-data").join("claude-usage.jsonl")
    }

    fn settings_of(config_dir: &Path) -> Map<String, Value> {
        load_settings(&config_dir.join(SETTINGS_FILENAME)).expect("settings readable")
    }

    fn script_of(config_dir: &Path) -> String {
        fs::read_to_string(config_dir.join(HOOKS_DIRNAME).join(script_filename()))
            .expect("script exists")
    }

    fn write_settings_json(config_dir: &Path, value: Value) {
        fs::create_dir_all(config_dir).expect("config dir");
        fs::write(
            config_dir.join(SETTINGS_FILENAME),
            serde_json::to_vec_pretty(&value).expect("serialize"),
        )
        .expect("write settings");
    }

    #[test]
    fn installs_a_status_line_for_an_account_that_had_none() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude-account2");
        write_settings_json(&config_dir, json!({ "model": "claude-fable-5" }));
        let exe = temp.path().join("bin").join("taurhaus-daemon");

        let install =
            ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        assert_eq!(
            install,
            StatuslineInstall {
                changed: true,
                wrapped: false,
                skipped: None,
            }
        );
        let settings = settings_of(&config_dir);
        assert_eq!(
            settings[STATUS_LINE_KEY]["command"].as_str(),
            Some(
                format!(
                    "bash '{}'",
                    config_dir
                        .join(HOOKS_DIRNAME)
                        .join(script_filename())
                        .display()
                )
                .as_str()
            )
        );
        // Untouched settings stay untouched.
        assert_eq!(settings["model"].as_str(), Some("claude-fable-5"));

        let script = script_of(&config_dir);
        assert!(script.contains(&format!(
            "'{}' {USAGE_SINK_SUBCOMMAND} --config-dir '{}' --sink '{}' --render",
            exe.display(),
            config_dir.display(),
            sink_for(&temp).display()
        )));
        assert!(statusline_is_installed_at(&config_dir));
    }

    #[test]
    fn wrapping_keeps_the_status_line_the_user_already_had() {
        // Regression: d6839a3 had no installer, and the obvious one sets
        // `statusLine` to taurhaus's script — which silently deletes the line
        // the user was already running (this host's `~/.claude` points at
        // `statusline-zq.sh`). Wrapping is the contract: the sink runs, their
        // command still renders, and their exit code is still the line's.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        write_settings_json(
            &config_dir,
            json!({
                "statusLine": {
                    "type": "command",
                    "command": "/home/user/zq/statusline-zq.sh",
                    "padding": 0
                }
            }),
        );
        let exe = temp.path().join("taurhaus-daemon");

        let install =
            ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        assert!(install.changed && install.wrapped);
        let script = script_of(&config_dir);
        assert!(
            script.contains("printf '%s' \"$payload\" | /home/user/zq/statusline-zq.sh\n"),
            "the wrapped command must receive the same payload: {script}"
        );
        assert!(!script.contains("--render"));
        // The whole original value is remembered, `padding` included, because
        // removal has to give it back exactly.
        let record = read_record(&config_dir.join(HOOKS_DIRNAME)).expect("record");
        assert_eq!(
            record.wrapped,
            Some(json!({
                "type": "command",
                "command": "/home/user/zq/statusline-zq.sh",
                "padding": 0
            }))
        );
    }

    #[test]
    fn installing_twice_changes_nothing_and_never_wraps_our_own_script() {
        // Regression: d6839a3 had no installer. An installer that reads the
        // current `statusLine` as "the user's command" wraps its own script on
        // the second run, and the user's real command is lost one app start
        // after it was preserved.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        write_settings_json(
            &config_dir,
            json!({ "statusLine": { "type": "command", "command": "my-line.sh" } }),
        );
        let exe = temp.path().join("taurhaus-daemon");

        let first = ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp))
            .expect("first install");
        let second = ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp))
            .expect("second install");

        assert!(first.changed);
        assert_eq!(
            second,
            StatuslineInstall {
                changed: false,
                wrapped: true,
                skipped: None,
            }
        );
        let script = script_of(&config_dir);
        assert_eq!(script.matches(USAGE_SINK_SUBCOMMAND).count(), 1);
        assert!(script.contains("| my-line.sh\n"));
        assert!(!script.contains(&format!("| bash '{}", config_dir.display())));
    }

    #[test]
    fn the_script_names_the_sink_the_installer_itself_will_read() {
        // Regression: d6839a3 had no bridge. The generated script runs in the
        // user's own shell, where `TAURHAUS_DATA_DIR` is not set — so a sink
        // resolved at run time always lands in the default app-data root while
        // an app or daemon started with an isolated root reads a different file
        // and reports no usage at all. The path is the installer's answer, and
        // it belongs in the script.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let sink = temp.path().join("isolated").join("claude-usage.jsonl");

        ensure_statusline_installed_at(&config_dir, &temp.path().join("exe"), &sink)
            .expect("install");

        assert!(script_of(&config_dir).contains(&format!("--sink '{}'", sink.display())));
        assert!(statusline_is_installed_at(&config_dir));
    }

    #[test]
    fn a_moved_executable_is_reinstalled_rather_than_left_dangling() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let old_exe = temp.path().join("old").join("taurhaus-daemon");
        let new_exe = temp.path().join("new").join("taurhaus-daemon");

        ensure_statusline_installed_at(&config_dir, &old_exe, &sink_for(&temp)).expect("install");
        let moved = ensure_statusline_installed_at(&config_dir, &new_exe, &sink_for(&temp))
            .expect("reinstall");

        assert!(moved.changed);
        assert!(script_of(&config_dir).contains(&new_exe.display().to_string()));
        assert!(!script_of(&config_dir).contains(&old_exe.display().to_string()));
    }

    #[test]
    fn removing_restores_the_status_line_it_wrapped() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let original = json!({
            "type": "command",
            "command": "/home/user/zq/statusline-zq.sh",
            "padding": 0
        });
        write_settings_json(
            &config_dir,
            json!({ "model": "claude-fable-5", "statusLine": original.clone() }),
        );
        let exe = temp.path().join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        assert!(remove_statusline_at(&config_dir).expect("remove"));

        let settings = settings_of(&config_dir);
        assert_eq!(settings[STATUS_LINE_KEY], original);
        assert_eq!(settings["model"].as_str(), Some("claude-fable-5"));
        assert!(!config_dir
            .join(HOOKS_DIRNAME)
            .join(script_filename())
            .exists());
        assert!(!statusline_is_installed_at(&config_dir));
        // Removing twice is not an error, and does not resurrect anything.
        assert!(!remove_statusline_at(&config_dir).expect("second remove"));
    }

    #[test]
    fn removing_takes_the_status_line_out_when_there_was_none() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude-account2");
        write_settings_json(&config_dir, json!({ "theme": "dark" }));
        let exe = temp.path().join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        assert!(remove_statusline_at(&config_dir).expect("remove"));

        let settings = settings_of(&config_dir);
        assert!(!settings.contains_key(STATUS_LINE_KEY));
        assert_eq!(settings["theme"].as_str(), Some("dark"));
    }

    #[test]
    fn a_config_dir_the_status_line_cannot_reach_is_skipped_whole() {
        let install = ensure_statusline_installed_at(
            Path::new(r"C:\Users\mstie\.claude"),
            Path::new("/home/user/.local/bin/taurhaus-daemon"),
            Path::new("/home/user/.local/share/com.taurhaus.dev/claude-usage.jsonl"),
        )
        .expect("install");

        assert_eq!(install.skipped, Some("non_posix_config_dir"));
        assert!(!install.changed);
    }

    #[test]
    fn an_unreadable_settings_file_never_costs_the_user_their_status_line() {
        // Regression: d6839a3 had no installer. One that treats an unparseable
        // `settings.json` as "no settings" writes a fresh file over the user's
        // own — hooks, model, permissions and all.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(config_dir.join(SETTINGS_FILENAME), "{ not json").expect("write");

        let error =
            ensure_statusline_installed_at(&config_dir, &temp.path().join("exe"), &sink_for(&temp))
                .expect_err("unparseable settings are an error, not an empty object");

        assert!(error.contains("failed to parse"), "{error}");
        assert_eq!(
            fs::read_to_string(config_dir.join(SETTINGS_FILENAME)).expect("settings"),
            "{ not json"
        );
    }
}
