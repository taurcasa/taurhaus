//! The status-line bridge that feeds `claude-usage.jsonl`.
//!
//! Claude Code has exactly one documented place where a subscription's usage
//! leaves the product: the `statusLine` command, which receives the session's
//! JSON — `rate_limits` included — on stdin at every refresh. Taking that seat
//! means writing into a config dir the user also owns, so the rule here is that
//! nothing the user configured may stop rendering:
//!
//! * a config dir with no `statusLine` gets ours, and ours *always* prints a
//!   line (`<model> · 5h n% · 7d n%`, or a fallback when the sink cannot run or
//!   the payload says nothing) — verified live on 2.1.246, a status-line command
//!   that prints nothing leaves the row blank rather than falling back to a
//!   built-in line, so an empty print is a row taurhaus emptied;
//! * a config dir that already has one gets **wrapped**: our script tees the
//!   payload to the sink and then pipes the very same JSON to the command that
//!   was configured before — as one shell group, so that a command reading
//!   stdin into a variable before rendering it still sees the payload — and its
//!   stdout and exit code are the status line, its empty output staying empty
//!   because that row is theirs;
//! * a `statusLine` configured *while* an install is running is wrapped like
//!   any other: the commit only lands while the value it was decided from is
//!   still the one on disk, and rebuilds itself around a newer one;
//! * every option beside `type` and `command` is read off the row as it stands
//!   and written back untouched — the `padding` the user set before the wrap,
//!   and equally the one they set on our row afterwards;
//! * no sink call may outlive its deadline: each one is given a couple of
//!   seconds — by `timeout` where there is one, and by a watchdog in a process
//!   group of its own where there is not — so a wedged daemon costs a record
//!   and never the row, and a call that finished leaves nothing running;
//! * removal puts the original `statusLine` value back exactly as it was,
//!   extra keys and all, and takes the script out only once the row has stopped
//!   naming it — and a CLI that is *known* to be older than the build that
//!   sends `rate_limits` gets that removal, rather than keeping a bridge
//!   nothing can feed;
//! * nothing is restored, removed or deleted on a guess: the row is taurhaus's
//!   only while it names this config dir's own script by path — a user's script
//!   that merely shares that basename is a status line like any other — and a
//!   row that *is* ours whose record cannot be read stops the removal where it
//!   stands, because the command it wraps is written down nowhere else.
//!
//! Installation is idempotent and mirrors the compaction hook installer: a
//! generated script under `<config dir>/hooks`, a record naming the executable
//! and the sink it was generated for (so an app that moved, or a run under an
//! isolated `TAURHAUS_DATA_DIR`, reinstalls itself) and one entry in
//! `settings.json`. All three are published by renaming a finished file over
//! the old one, because the row points Claude Code at that script the whole
//! time and the record is the only copy of the command it wraps — one that
//! cannot be read leaves the install exactly as it stands. Both paths are baked
//! into the script rather than resolved when it runs: it runs in the user's own
//! shell, which knows nothing of taurhaus's environment.

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
/// What the row says when the sink has nothing to put in it.
const FALLBACK_LINE: &str = "taurhaus · no usage yet";
/// How long the generated script gives one sink call before killing it. Claude
/// Code refreshes the status line several times a second; a record is worth a
/// couple of seconds of waiting at the very most, and the row is worth none.
const SINK_DEADLINE_SECONDS: u32 = 2;
/// How many times an install rebuilds itself around a `statusLine` that changed
/// underneath it. The other writers here are a person and Claude Code; one
/// retry is already more than this has ever needed.
const COMMIT_ATTEMPTS: usize = 3;

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
    install_statusline_at(config_dir, taurhaus_exe, sink_path, &|| {})
}

/// The install itself.
///
/// `before_commit` runs in the window between reading `statusLine` and writing
/// it back — the window a concurrent editor lands in, and the only place a test
/// can stand to be that editor. Production passes a no-op.
fn install_statusline_at(
    config_dir: &Path,
    taurhaus_exe: &Path,
    sink_path: &Path,
    before_commit: &dyn Fn(),
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
    if !is_posix_executable(taurhaus_exe) {
        // A Windows-native executable reached through `/mnt/c/…` is not the
        // binary this script can drive: it would receive `/home/…` arguments it
        // resolves as Windows paths. Only the daemon that lives in the same
        // namespace as the config dir may take this seat.
        return Ok(StatuslineInstall {
            skipped: Some("cross_namespace_executable"),
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
    fs::create_dir_all(&hooks_dir)
        .map_err(|error| format!("failed to create '{}': {error}", hooks_dir.display()))?;

    // Everything below is decided from one reading of `statusLine` and only
    // committed while that reading still holds. Writing the script and the
    // record takes long enough for a person — or Claude Code — to configure a
    // status line in between, and that command has to be wrapped like any
    // other, not overwritten by a decision that predates it.
    for _ in 0..COMMIT_ATTEMPTS {
        let existing = load_settings(&settings_path)?.remove(STATUS_LINE_KEY);
        // Re-running against our own install must not wrap our own script: the
        // command the user actually configured is the one the record remembers.
        let wrapped = if is_taurhaus_status_line(existing.as_ref(), &script_command) {
            let Some(record) = read_record(&hooks_dir) else {
                // The row is already ours, so that record is the only place the
                // command it wraps is written down. Rebuilding the bridge from
                // a record we cannot read would publish a renderer over a
                // command nobody can name any more; leaving the install exactly
                // as it stands costs at most a refreshed script.
                return Ok(StatuslineInstall {
                    skipped: Some("unreadable_record"),
                    ..StatuslineInstall::default()
                });
            };
            record.wrapped
        } else {
            existing.clone().filter(|value| !value.is_null())
        };

        let script = render_script(
            &executable,
            &config_dir_argument,
            &sink_argument,
            wrapped_command(wrapped.as_ref()),
        );
        let script_changed = publish_if_changed(&script_path, script.as_bytes(), true)?;

        let record = StatuslineRecord {
            executable: executable.clone(),
            sink: sink_argument.clone(),
            wrapped: wrapped.clone(),
        };
        let record_changed = publish_if_changed(
            &hooks_dir.join(RECORD_FILENAME),
            serde_json::to_vec_pretty(&record.to_value())
                .map_err(|error| format!("failed to serialize the status line record: {error}"))?
                .as_slice(),
            false,
        )?;

        // Only `type` and `command` are taurhaus's. Everything else on the row
        // as it stands right now stays exactly as it stands: the `padding` the
        // user set on their own status line before it was wrapped, and equally
        // the one they set on ours after it took the seat. Rebuilding this from
        // the record instead would hand back options the user has since edited.
        let mut desired = match existing.as_ref() {
            Some(Value::Object(row)) => row.clone(),
            _ => Map::new(),
        };
        desired.insert("type".to_string(), Value::String("command".to_string()));
        desired.insert(
            "command".to_string(),
            Value::String(format!("bash {}", shell_quote(&script_command))),
        );

        before_commit();
        match commit_status_line(
            &settings_path,
            existing.as_ref(),
            Some(Value::Object(desired)),
        )? {
            // Somebody configured a status line while this one was being
            // written. Read it again and wrap what is actually there.
            StatusLineCommit::Stale => continue,
            StatusLineCommit::Written => {
                return Ok(StatuslineInstall {
                    changed: true,
                    wrapped: wrapped.is_some(),
                    skipped: None,
                })
            }
            StatusLineCommit::Unchanged => {
                return Ok(StatuslineInstall {
                    changed: script_changed || record_changed,
                    wrapped: wrapped.is_some(),
                    skipped: None,
                })
            }
        }
    }

    Err(format!(
        "'{}' kept changing its status line while taurhaus installed one",
        settings_path.display()
    ))
}

/// Take the bridge back out, restoring whatever it wrapped.
pub fn remove_statusline_at(config_dir: &Path) -> Result<bool, String> {
    remove_statusline_with(config_dir, &|| {})
}

/// The removal itself.
///
/// `before_commit` runs in the window between reading `statusLine` and putting
/// back what the bridge wrapped — the window a concurrent editor lands in, and
/// the only place a test can stand to be that editor. Production passes a no-op.
fn remove_statusline_with(config_dir: &Path, before_commit: &dyn Fn()) -> Result<bool, String> {
    let hooks_dir = config_dir.join(HOOKS_DIRNAME);
    let settings_path = config_dir.join(SETTINGS_FILENAME);
    let mut changed = false;
    // Nothing is deleted while the row still names it. An edit to a field
    // taurhaus does not own — the `padding` on the row it holds — makes the
    // guarded restore stale without making it wrong, and stopping there would
    // leave the user pointed at a script that is about to be removed, with the
    // only record of what it wrapped removed beside it. So: read again, and
    // hand the command back to the row as it now stands.
    let mut released = false;
    let script_reference = script_reference(&hooks_dir);
    for _ in 0..COMMIT_ATTEMPTS {
        let current = load_settings(&settings_path)?.get(STATUS_LINE_KEY).cloned();
        if !is_taurhaus_status_line(current.as_ref(), &script_reference) {
            // A `statusLine` that does not name our script is somebody else's,
            // and giving them a command they replaced would be the same
            // overwrite this bridge exists to avoid. Nothing points at the
            // script either, so the files below are free to go.
            released = true;
            break;
        }
        // The row is ours, so the record is the only place the command it wraps
        // is written down. A removal that cannot read it cannot restore
        // anything, and taking the row out anyway would delete the user's own
        // status line — the exact loss wrapping exists to prevent. Leave every
        // part of the install exactly as it stands and say so.
        let Some(record) = read_record(&hooks_dir) else {
            return Err(format!(
                "'{}' still runs taurhaus's status line, but '{}' cannot be read: \
                 the command it wraps is written down nowhere else",
                settings_path.display(),
                hooks_dir.join(RECORD_FILENAME).display()
            ));
        };
        before_commit();
        match commit_status_line(&settings_path, current.as_ref(), record.wrapped)? {
            StatusLineCommit::Stale => continue,
            StatusLineCommit::Written => {
                changed = true;
                released = true;
                break;
            }
            StatusLineCommit::Unchanged => {
                released = true;
                break;
            }
        }
    }
    if !released {
        return Err(format!(
            "'{}' kept changing its status line while taurhaus removed one",
            settings_path.display()
        ));
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
        .is_some_and(|settings| {
            is_taurhaus_status_line(settings.get(STATUS_LINE_KEY), &script_reference(&hooks_dir))
        })
}

/// Install the bridge in every detected account, when the CLI can feed it.
///
/// Called by the daemon and only by the daemon. It is the process that lives in
/// the same namespace as the config dirs on every platform, and a second
/// installer would only bake its own executable path into the same script.
///
/// A build older than the one this was verified against gets nothing: its
/// payload is not documented to carry `rate_limits`, and rewriting a user's
/// `statusLine` for numbers that never arrive is a bad trade. If one is already
/// installed — the user downgraded, or switched to another `claude` on their
/// PATH — it is taken back out here, because that same trade is no better for
/// having been made yesterday.
pub fn install_statusline_for_detected_accounts(taurhaus_exe: &Path) {
    let versions = CliVersions::current();
    match statusline_bridge_action(versions) {
        BridgeAction::Install => {}
        BridgeAction::Remove => {
            remove_statusline_from_detected_accounts(versions.claude.as_deref());
            return;
        }
        BridgeAction::Leave => {
            emit_skipped_run(versions.claude.as_deref());
            return;
        }
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

/// Take the bridge out of every detected account, and say why.
///
/// The CLI in front of these config dirs cannot feed the sink any more, so the
/// script would wrap the user's status line for numbers that never arrive.
fn remove_statusline_from_detected_accounts(claude_version: Option<&str>) {
    for account in detect_claude_accounts_cached() {
        let mut fields = Map::new();
        fields.insert(
            "config_dir".to_string(),
            Value::String(account.config_dir.display().to_string()),
        );
        fields.insert("account_id".to_string(), Value::String(account.id.clone()));
        if let Some(version) = claude_version {
            fields.insert("claude_version".to_string(), Value::String(version.into()));
        }
        match remove_statusline_at(&account.config_dir) {
            Ok(false) => {}
            Ok(true) => emit_global(
                "info",
                "claude_usage",
                "claude.usage.statusline.removed",
                Some(format!(
                    "Claude usage status line removed for {}: this CLI does not report usage",
                    account.email
                )),
                fields,
            ),
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

/// What one run should do about the bridge, given what the CLI probe said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BridgeAction {
    Install,
    /// Take an installed bridge back out, restoring what it wrapped.
    Remove,
    /// Touch nothing at all.
    Leave,
}

fn statusline_bridge_action(versions: &CliVersions) -> BridgeAction {
    match (
        versions.claude_statusline_usage_supported,
        versions.claude.as_deref(),
    ) {
        (true, _) => BridgeAction::Install,
        // A version we read, and it is older than the one that sends
        // `rate_limits`. A bridge installed by a newer CLI is now wrapping a
        // status line for nothing, so it goes back out.
        (false, Some(_)) => BridgeAction::Remove,
        // A probe that could not answer — no `claude` on this PATH, a timeout,
        // a shell that failed to start — says nothing about the CLI the user
        // runs. Tearing down a working bridge on that would be worse than
        // leaving one that has nothing to feed it.
        (false, None) => BridgeAction::Leave,
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
    let mut script = String::from(
        "#!/usr/bin/env bash\n\
         # taurhaus status line bridge — generated file, rewritten on every install.\n\
         # Records this subscription's 5-hour and 7-day rate limits.\n",
    );
    // No `set -e`: a sink that cannot run must cost the user a record, never a
    // status line.
    script.push_str("payload=\"$(cat)\"\n");
    // And a sink that never *finishes* must cost no more than one either. Every
    // call goes through here, where it is given a deadline and then killed and
    // reaped: a wedged daemon, or a filesystem that stopped answering, can lose
    // the user a record and never the row.
    //
    // `timeout` does that with nothing left over. Where there is none — a Mac
    // without coreutils — a watchdog does it instead, started under `set -m` so
    // that it has a process group of its own: cancelling it has to take the
    // `sleep` it is blocked on with it, and this runs on every keystroke.
    push_lines(
        &mut script,
        [
            format!(
                "taurhaus_sink_cmd=( {} {USAGE_SINK_SUBCOMMAND} --config-dir {} --sink {} )",
                shell_quote(executable),
                shell_quote(config_dir),
                shell_quote(sink_path)
            ),
            "taurhaus_timeout=\"$(command -v timeout 2>/dev/null \
             || command -v gtimeout 2>/dev/null)\""
                .to_string(),
            "taurhaus_sink() {".to_string(),
            "  local out=$1".to_string(),
            "  shift".to_string(),
            "  if [ -n \"$taurhaus_timeout\" ]; then".to_string(),
            format!(
                "    printf '%s' \"$payload\" | \"$taurhaus_timeout\" -k 1 \
                 {SINK_DEADLINE_SECONDS} \"${{taurhaus_sink_cmd[@]}}\" \"$@\" >\"$out\" 2>/dev/null"
            ),
            "    return 0".to_string(),
            "  fi".to_string(),
            "  set -m".to_string(),
            "  printf '%s' \"$payload\" | \"${taurhaus_sink_cmd[@]}\" \"$@\" >\"$out\" 2>/dev/null &"
                .to_string(),
            "  local sink_pid=$!".to_string(),
            format!(
                "  ( sleep {SINK_DEADLINE_SECONDS}; kill \"$sink_pid\"; \
                 sleep 1; kill -9 \"$sink_pid\" ) >/dev/null 2>&1 &"
            ),
            "  local deadline=$!".to_string(),
            "  set +m".to_string(),
            "  wait \"$sink_pid\" 2>/dev/null".to_string(),
            "  kill -- -\"$deadline\" 2>/dev/null || kill \"$deadline\" 2>/dev/null".to_string(),
            "  wait \"$deadline\" 2>/dev/null".to_string(),
            "} 2>/dev/null".to_string(),
        ],
    );
    match wrapped {
        Some(command) => {
            script.push_str(
                "# The status line below was configured before taurhaus wrapped it;\n\
                 # it receives the same payload and owns the rendered line. The\n\
                 # record is taken beside it, never in front of it: a sink that\n\
                 # waits on the sink file — or never returns at all — must not\n\
                 # delay the user's own line.\n",
            );
            script.push_str("taurhaus_sink /dev/null >/dev/null 2>&1 &\n");
            // As one group, because a bare `… | command` binds the pipe to that
            // command's first pipeline only: the ordinary way to write a status
            // line — `input="$(cat)"; render "$input"` — would read the payload
            // in the pipe's subshell and render from an empty variable outside
            // it. The group's stdout and exit status are still the command's.
            script.push_str("printf '%s' \"$payload\" | {\n");
            script.push_str(&command);
            script.push_str("\n}\n");
        }
        None => {
            script.push_str(
                "# This account had no status line, so this row is taurhaus's to\n\
                 # fill — and a row taurhaus installed may never come back empty.\n\
                 # A sink that cannot be executed, one that never answers, a\n\
                 # payload it cannot read and a refresh with nothing to report all\n\
                 # print nothing at all, and a blank row is the one outcome the\n\
                 # install promised to avoid.\n",
            );
            push_lines(
                &mut script,
                [
                    // A pipe would mean waiting for the sink to close it, which
                    // is the wait this deadline exists to bound.
                    "rendered=\"$(mktemp 2>/dev/null)\" || rendered=''".to_string(),
                    "line=''".to_string(),
                    "if [ -n \"$rendered\" ]; then".to_string(),
                    "  taurhaus_sink \"$rendered\" --render".to_string(),
                    "  line=\"$(cat \"$rendered\" 2>/dev/null)\"".to_string(),
                    "  rm -f \"$rendered\"".to_string(),
                    "fi".to_string(),
                    format!("[ -n \"$line\" ] || line={}", shell_quote(FALLBACK_LINE)),
                    "printf '%s\\n' \"$line\"".to_string(),
                    "exit 0".to_string(),
                ],
            );
        }
    }
    script
}

fn push_lines(script: &mut String, lines: impl IntoIterator<Item = String>) {
    for line in lines {
        script.push_str(&line);
        script.push('\n');
    }
}

/// Whether this `statusLine` runs the script in *this* config dir's hooks.
///
/// The whole path, not the basename: a user's own `taurhaus-statusline-mine.sh`
/// shares that name, and both the ownership this asks about and the deletion it
/// gates are questions about one exact file. Containment rather than equality,
/// because the row stays ours through an edit to how it is invoked — `bash` for
/// `/bin/bash`, an added flag — and a script the row still names in any form is
/// one removal may not delete underneath it.
fn is_taurhaus_status_line(value: Option<&Value>, script_path: &str) -> bool {
    let Some(value) = value else {
        return false;
    };
    let command = match value {
        Value::String(command) => command.as_str(),
        Value::Object(_) => value.get("command").and_then(Value::as_str).unwrap_or(""),
        _ => "",
    };
    command.contains(script_path)
}

/// The script's path as a `statusLine` in this config dir would name it.
fn script_reference(hooks_dir: &Path) -> String {
    let script_path = hooks_dir.join(script_filename());
    linux_path_string(&script_path).unwrap_or_else(|| script_path.display().to_string())
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

/// What one commit did to `statusLine`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusLineCommit {
    /// The file already said what this commit wanted it to say.
    Unchanged,
    Written,
    /// `statusLine` is no longer what the caller decided against. The caller
    /// owns the retry, because the new value changes what it would write.
    Stale,
}

/// Replace exactly `statusLine`, and only while it still says `expected`.
///
/// The value being written is decided from a reading taken before the script
/// and the record were written, and this file has other writers: a person with
/// an editor, and Claude Code itself. Committing against that reading is what
/// makes a status line configured in between something to wrap rather than
/// something to overwrite — and everything but `statusLine` stays out of
/// taurhaus's hands either way.
fn commit_status_line(
    settings_path: &Path,
    expected: Option<&Value>,
    value: Option<Value>,
) -> Result<StatusLineCommit, String> {
    let mut settings = load_settings(settings_path)?;
    if settings.get(STATUS_LINE_KEY) != expected {
        return Ok(StatusLineCommit::Stale);
    }
    match value {
        Some(desired) => {
            if settings.get(STATUS_LINE_KEY) == Some(&desired) {
                return Ok(StatusLineCommit::Unchanged);
            }
            settings.insert(STATUS_LINE_KEY.to_string(), desired);
        }
        None => {
            if settings.remove(STATUS_LINE_KEY).is_none() {
                return Ok(StatusLineCommit::Unchanged);
            }
        }
    }
    write_settings(settings_path, &settings)?;
    Ok(StatusLineCommit::Written)
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
    #[cfg(not(target_os = "windows"))]
    {
        // A rename puts a *new* file in the user's place, so it has to be given
        // the mode the old one had: `settings.json` carries permission rules and
        // can carry an API key, and one the user locked to 0600 may not come
        // back 0644 because taurhaus touched it. One taurhaus creates is private
        // from the start rather than as wide as the umask allows.
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(settings_path)
            .map(|metadata| metadata.permissions().mode() & 0o777)
            .unwrap_or(0o600);
        if let Err(error) = fs::set_permissions(&temp_path, fs::Permissions::from_mode(mode)) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!(
                "failed to set the mode of '{}': {error}",
                temp_path.display()
            ));
        }
    }
    if let Err(error) = fs::rename(&temp_path, settings_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "failed to replace '{}': {error}",
            settings_path.display()
        ));
    }
    Ok(())
}

/// Publish one generated file whole, or not at all.
///
/// `settings.json` names the script for as long as the bridge is installed, and
/// Claude Code refreshes several times a second: a truncating write leaves a
/// window where a refresh runs an empty file. The record has the same problem
/// with worse consequences — it is the only copy of the command the script
/// wraps, and a half-written one reads as "there was nothing to wrap". So both
/// are filled beside their final name and renamed over it, which no reader can
/// land inside of.
fn publish_if_changed(path: &Path, payload: &[u8], executable: bool) -> Result<bool, String> {
    if fs::read(path).is_ok_and(|current| current == payload) {
        return Ok(false);
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("'{}' has no parent", path.display()))?;
    let temp_path = parent.join(format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(SCRIPT_BASENAME),
        std::process::id()
    ));
    fs::write(&temp_path, payload)
        .map_err(|error| format!("failed to write '{}': {error}", temp_path.display()))?;
    #[cfg(not(target_os = "windows"))]
    if executable {
        // Before the rename, so that whatever appears under `path` is runnable
        // the instant it appears there.
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755)) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!(
                "failed to make '{}' executable: {error}",
                temp_path.display()
            ));
        }
    }
    #[cfg(target_os = "windows")]
    let _ = executable;
    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("failed to replace '{}': {error}", path.display()));
    }
    Ok(true)
}

fn is_posix_config_dir(config_dir: &Path) -> bool {
    let value = config_dir.display().to_string();
    value.starts_with('/') || path::is_wsl_path(&value)
}

/// Whether this executable lives where the generated bash script runs.
///
/// A Windows drive path is reachable from WSL as `/mnt/c/…`, which is exactly
/// what makes this worth checking: the script would run, and hand a Windows
/// binary Linux paths it cannot resolve. The bridge belongs to the process in
/// the same namespace as the config dir.
fn is_posix_executable(taurhaus_exe: &Path) -> bool {
    let value = taurhaus_exe.display().to_string();
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
            "taurhaus_sink_cmd=( '{}' {USAGE_SINK_SUBCOMMAND} --config-dir '{}' --sink '{}' )",
            exe.display(),
            config_dir.display(),
            sink_for(&temp).display()
        )));
        // The row is ours to fill here, so the sink is asked for a line.
        assert!(script.contains("--render"));
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
            script.contains("printf '%s' \"$payload\" | {\n/home/user/zq/statusline-zq.sh\n}\n"),
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
        assert!(script.contains("{\nmy-line.sh\n}\n"));
        assert!(!script.contains(
            &config_dir
                .join(HOOKS_DIRNAME)
                .join(script_filename())
                .display()
                .to_string()
        ));
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
    fn a_windows_executable_never_takes_the_seat_of_a_wsl_config_dir() {
        // Regression: 79be608 ran this installer from the app's own startup on
        // every platform. On Windows account detection reaches the WSL home
        // through its UNC path, and the installer accepted that dir: it wrote a
        // *bash* script that invoked the Windows executable through `/mnt/c/…`
        // and handed it `/home/…` arguments, which Windows `PathBuf`s cannot
        // resolve — overwriting the working script the WSL daemon had just
        // installed for that same account.
        let install = ensure_statusline_installed_at(
            Path::new(r"\\wsl.localhost\Ubuntu\home\mstie\.claude"),
            Path::new(r"C:\Program Files\taurhaus\taurhaus.exe"),
            Path::new(r"C:\Users\mstie\AppData\Roaming\com.taurhaus.dev\claude-usage.jsonl"),
        )
        .expect("install");

        assert_eq!(install.skipped, Some("cross_namespace_executable"));
        assert!(!install.changed);
    }

    #[test]
    fn wrapping_keeps_the_options_the_user_set_on_their_status_line() {
        // Regression: 79be608 replaced the active `statusLine` with a bare
        // `{type, command}` object. Everything else Claude Code reads there —
        // `padding`, and any option a later build adds — stopped applying for
        // as long as taurhaus was installed, even though the record remembered
        // it for removal.
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

        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        let active = settings_of(&config_dir)[STATUS_LINE_KEY].clone();
        assert_eq!(active["padding"], json!(0));
        assert_eq!(active["type"].as_str(), Some("command"));
        assert!(active["command"]
            .as_str()
            .is_some_and(|command| command.contains(SCRIPT_BASENAME)));

        // The preserved options must not make the next run look different.
        let second = ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp))
            .expect("second install");
        assert!(!second.changed);
    }

    #[test]
    fn an_option_set_on_the_row_taurhaus_holds_survives_the_next_install() {
        // Regression: 984218c rebuilt the active `statusLine` object from the
        // *record's* remembered original on every install, so an option the
        // user set on the row while taurhaus held it — `padding`, or whatever a
        // later Claude Code reads there — was reverted at the next daemon
        // start, silently, for as long as the bridge stayed installed.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let original = json!({
            "type": "command",
            "command": "/home/user/zq/statusline-zq.sh",
            "padding": 0
        });
        write_settings_json(&config_dir, json!({ "statusLine": original.clone() }));
        let exe = temp.path().join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        // The user tunes the row while taurhaus is holding it.
        let mut active = settings_of(&config_dir)[STATUS_LINE_KEY]
            .as_object()
            .expect("an object")
            .clone();
        active.insert("padding".to_string(), json!(2));
        active.insert("paddingTop".to_string(), json!(1));
        write_settings_json(
            &config_dir,
            json!({ "statusLine": Value::Object(active.clone()) }),
        );

        let again = ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp))
            .expect("second install");

        let row = settings_of(&config_dir)[STATUS_LINE_KEY].clone();
        assert_eq!(row["padding"], json!(2), "the edited option was reverted");
        assert_eq!(
            row["paddingTop"],
            json!(1),
            "an option we do not know was dropped"
        );
        assert!(row["command"]
            .as_str()
            .is_some_and(|command| command.contains(SCRIPT_BASENAME)));
        assert!(!again.changed, "keeping the row as it is is not a change");
        // Removal still hands back what was there before taurhaus took the seat.
        assert!(remove_statusline_at(&config_dir).expect("remove"));
        assert_eq!(settings_of(&config_dir)[STATUS_LINE_KEY], original);
    }

    #[test]
    fn a_claude_that_stopped_sending_rate_limits_gets_the_bridge_taken_back_out() {
        // Regression: 79be608 returned from the install run the moment the
        // version gate said no. A bridge installed under 2.1.246 therefore
        // stayed in `settings.json` after the user went back to an older CLI —
        // wrapping their status line, and running a sink for numbers that build
        // never sends. The gate has to be able to say "take it out", and only a
        // version we actually read may say it: a probe that could not answer is
        // no reason to tear down a working bridge.
        let supported = CliVersions {
            claude: Some("2.1.246".to_string()),
            claude_statusline_usage_supported: true,
            ..CliVersions::default()
        };
        let older = CliVersions {
            claude: Some("2.1.238".to_string()),
            claude_statusline_usage_supported: false,
            ..CliVersions::default()
        };
        let unknown = CliVersions::default();

        assert_eq!(statusline_bridge_action(&supported), BridgeAction::Install);
        assert_eq!(statusline_bridge_action(&older), BridgeAction::Remove);
        assert_eq!(statusline_bridge_action(&unknown), BridgeAction::Leave);

        // And the removal that decision asks for gives the row back whole.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let original = json!({ "type": "command", "command": "my-line.sh", "padding": 0 });
        write_settings_json(&config_dir, json!({ "statusLine": original.clone() }));
        ensure_statusline_installed_at(&config_dir, &temp.path().join("exe"), &sink_for(&temp))
            .expect("install");
        assert!(statusline_is_installed_at(&config_dir));

        assert!(remove_statusline_at(&config_dir).expect("remove"));

        assert_eq!(settings_of(&config_dir)[STATUS_LINE_KEY], original);
        assert!(!statusline_is_installed_at(&config_dir));
    }

    #[cfg(unix)]
    #[test]
    fn a_wrapped_status_line_renders_even_when_the_sink_stalls() {
        // Regression: 79be608 piped the payload to the sink synchronously and
        // only then ran the command the user had configured. A sink waiting on
        // the sink file's lock therefore delayed — and a wedged one blocked —
        // the status line taurhaus promised never to disturb.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let theirs = temp.path().join("statusline-zq.sh");
        write_stub(&theirs, "cat >/dev/null\necho 'theirs'");
        write_settings_json(
            &config_dir,
            json!({ "statusLine": { "type": "command", "command": theirs.display().to_string() } }),
        );
        let exe = temp.path().join("bin").join("taurhaus-daemon");
        // A sink that will never answer.
        write_stub(&exe, "sleep 30");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        let (ok, line) =
            run_script_within(&config_dir, OBSERVED_PAYLOAD_FOR_SCRIPT, SINK_STALL_LIMIT)
                .expect("a stalled sink must not hold the user's status line");

        assert!(ok);
        assert_eq!(line, "theirs\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_sink_that_never_answers_still_leaves_a_line() {
        // Regression: 984218c rendered the row from `$(… | sink --render)`,
        // which waits for the sink to exit. A sink that hangs — a wedged
        // daemon, a filesystem that stopped answering — therefore printed
        // neither its line nor the fallback, and the row taurhaus had taken
        // over stayed blank for as long as the process lived.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let exe = temp.path().join("bin").join("taurhaus-daemon");
        write_stub(&exe, "sleep 30\necho 'far too late'");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        let (ok, line) =
            run_script_within(&config_dir, OBSERVED_PAYLOAD_FOR_SCRIPT, SINK_STALL_LIMIT)
                .expect("a stalled sink must not hold the status line taurhaus renders");

        assert!(ok, "a stalled sink must not fail the status line");
        assert_eq!(line.trim(), FALLBACK_LINE);
    }

    #[test]
    fn an_install_replaces_the_status_line_and_nothing_else() {
        // Regression: 79be608 read the whole settings object at the top of the
        // install and wrote that same object back after generating the script,
        // so anything Claude Code (or a second installer) wrote in between was
        // reverted. Only `statusLine` is taurhaus's to change.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let settings_path = config_dir.join(SETTINGS_FILENAME);
        write_settings_json(&config_dir, json!({ "model": "old" }));

        // The value the installer decided on, from settings as they were then —
        // which had no status line at all.
        let desired = json!({ "type": "command", "command": "bash /tmp/taurhaus-statusline.sh" });
        // Claude Code writes the file while the script and record are written.
        write_settings_json(&config_dir, json!({ "model": "new", "theme": "dark" }));

        assert_eq!(
            commit_status_line(&settings_path, None, Some(desired.clone())).expect("commit"),
            StatusLineCommit::Written
        );

        let settings = settings_of(&config_dir);
        assert_eq!(settings["model"].as_str(), Some("new"));
        assert_eq!(settings["theme"].as_str(), Some("dark"));
        assert_eq!(settings[STATUS_LINE_KEY], desired);
        // Committing the same value again is not a change.
        assert_eq!(
            commit_status_line(&settings_path, Some(&desired), Some(desired.clone()))
                .expect("second commit"),
            StatusLineCommit::Unchanged
        );
        // And a status line that moved on since the decision is not ours to
        // replace: the caller has to look again.
        write_settings_json(&config_dir, json!({ "statusLine": "their-line.sh" }));
        assert_eq!(
            commit_status_line(&settings_path, Some(&desired.clone()), Some(desired))
                .expect("third"),
            StatusLineCommit::Stale
        );
    }

    #[test]
    fn a_status_line_configured_while_installing_is_wrapped_rather_than_lost() {
        // Regression: a574720 decided what to wrap from a snapshot taken at the
        // top of the install and committed that decision after the script and
        // the record were written. A `statusLine` configured inside that window
        // was overwritten: it was neither wrapped — so it stopped rendering —
        // nor remembered, so removal handed back a command the user had already
        // replaced.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        write_settings_json(&config_dir, json!({ "model": "claude-fable-5" }));
        let exe = temp.path().join("taurhaus-daemon");
        let configured = json!({ "type": "command", "command": "my-line.sh" });

        let raced = std::sync::atomic::AtomicBool::new(false);
        let install = install_statusline_at(&config_dir, &exe, &sink_for(&temp), &|| {
            if raced.swap(true, std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            write_settings_json(
                &config_dir,
                json!({ "model": "claude-fable-5", "statusLine": configured.clone() }),
            );
        })
        .expect("install");

        assert!(install.changed && install.wrapped);
        assert!(
            script_of(&config_dir).contains("{\nmy-line.sh\n}\n"),
            "the command configured mid-install must be wrapped, not dropped: {}",
            script_of(&config_dir)
        );
        assert_eq!(
            read_record(&config_dir.join(HOOKS_DIRNAME))
                .expect("record")
                .wrapped,
            Some(configured.clone())
        );
        assert!(statusline_is_installed_at(&config_dir));

        // And removal gives back the command that was actually configured.
        assert!(remove_statusline_at(&config_dir).expect("remove"));
        assert_eq!(settings_of(&config_dir)[STATUS_LINE_KEY], configured);
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

    #[test]
    fn a_record_taurhaus_cannot_read_never_costs_the_user_their_wrapped_command() {
        // Regression: 79be608 recovered the command it had wrapped from the
        // record, and read a record it could not parse as "there was nothing to
        // wrap". A record left half-written — by an interrupted install, or by
        // any writer that truncates before it fills — therefore turned the next
        // install into a renderer over the user's own status line, and
        // overwrote the only copy of their command with `null`.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        write_settings_json(
            &config_dir,
            json!({ "statusLine": { "type": "command", "command": "my-line.sh" } }),
        );
        let exe = temp.path().join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");
        let record_path = config_dir.join(HOOKS_DIRNAME).join(RECORD_FILENAME);
        let whole = fs::read_to_string(&record_path).expect("record");
        let half = whole[..whole.len() / 2].to_string();
        let script_before = script_of(&config_dir);
        fs::write(&record_path, &half).expect("truncate the record");

        let install =
            ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        assert_eq!(
            script_of(&config_dir),
            script_before,
            "the bridge was rebuilt without the command it wraps"
        );
        assert_eq!(
            fs::read_to_string(&record_path).expect("record"),
            half,
            "the only record of the wrapped command was overwritten"
        );
        assert_eq!(install.skipped, Some("unreadable_record"));
        assert!(!install.changed);
    }

    #[cfg(unix)]
    #[test]
    fn the_script_a_status_line_is_running_is_never_rewritten_underneath_it() {
        // Regression: 79be608 published the script and the record with a
        // truncating `fs::write`. `settings.json` names that script for as long
        // as the bridge is installed and Claude Code refreshes several times a
        // second, so a refresh landing inside a reinstall could run an empty or
        // half-written file — and a record read in that same window is the
        // unreadable record above.
        use std::io::Read as _;
        use std::os::unix::fs::MetadataExt as _;
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let old_exe = temp.path().join("old").join("taurhaus-daemon");
        let new_exe = temp.path().join("new").join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &old_exe, &sink_for(&temp)).expect("install");
        let script_path = config_dir.join(HOOKS_DIRNAME).join(script_filename());
        let record_path = config_dir.join(HOOKS_DIRNAME).join(RECORD_FILENAME);

        // The status line opens the script the instant before the reinstall.
        let mut running = fs::File::open(&script_path).expect("open the script");
        let was = script_of(&config_dir);
        let script_inode = fs::metadata(&script_path).expect("stat").ino();
        let record_inode = fs::metadata(&record_path).expect("stat").ino();

        ensure_statusline_installed_at(&config_dir, &new_exe, &sink_for(&temp)).expect("reinstall");

        let mut still_reading = String::new();
        running
            .read_to_string(&mut still_reading)
            .expect("read the script that was already running");
        assert_eq!(
            still_reading, was,
            "the script changed under a reader that had already opened it"
        );
        assert_ne!(
            fs::metadata(&script_path).expect("stat").ino(),
            script_inode,
            "the script was rewritten in place"
        );
        assert_ne!(
            fs::metadata(&record_path).expect("stat").ino(),
            record_inode,
            "the record was rewritten in place"
        );
        // And what appears under that name is runnable the moment it appears.
        assert_eq!(
            fs::metadata(&script_path)
                .expect("stat")
                .permissions()
                .mode()
                & 0o111,
            0o111
        );
        assert!(script_of(&config_dir).contains(&new_exe.display().to_string()));
    }

    #[test]
    fn a_row_edited_while_the_bridge_comes_out_never_points_at_a_deleted_script() {
        // Regression: 984218c guarded the restore with a compare-and-set but
        // left the deletion below it unconditional. Editing a field taurhaus
        // does not own — the `padding` on the row it holds — made that restore
        // stale without making it wrong, and removal then deleted both the
        // script the row still named and the only record of what it wrapped.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let original = json!({ "type": "command", "command": "my-line.sh", "padding": 0 });
        write_settings_json(&config_dir, json!({ "statusLine": original.clone() }));
        let exe = temp.path().join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        let raced = std::sync::atomic::AtomicBool::new(false);
        let removed = remove_statusline_with(&config_dir, &|| {
            if raced.swap(true, std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            // The user tunes the row taurhaus holds, mid-removal.
            let mut row = settings_of(&config_dir)[STATUS_LINE_KEY]
                .as_object()
                .expect("an object")
                .clone();
            row.insert("padding".to_string(), json!(3));
            write_settings_json(&config_dir, json!({ "statusLine": Value::Object(row) }));
        })
        .expect("remove");

        assert!(removed);
        assert_eq!(
            settings_of(&config_dir)[STATUS_LINE_KEY],
            original,
            "the row still names a script removal deleted"
        );
        assert!(!config_dir
            .join(HOOKS_DIRNAME)
            .join(script_filename())
            .exists());
        assert!(!statusline_is_installed_at(&config_dir));
    }

    #[test]
    fn removing_never_takes_a_row_whose_record_it_cannot_read() {
        // Regression: 79be608 recovered the wrapped command during removal with
        // `read_record(…).and_then(…)`, so a record that was missing or
        // half-written read as "there was nothing to wrap" — and removal then
        // deleted the active `statusLine` outright. The command that row wrapped
        // is written down in exactly one place, and this is the path that
        // removes both in a single write.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let original = json!({ "type": "command", "command": "my-line.sh", "padding": 0 });
        write_settings_json(&config_dir, json!({ "statusLine": original.clone() }));
        let exe = temp.path().join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");
        let ours = settings_of(&config_dir)[STATUS_LINE_KEY].clone();
        let record_path = config_dir.join(HOOKS_DIRNAME).join(RECORD_FILENAME);
        let whole = fs::read_to_string(&record_path).expect("record");
        let half = whole[..whole.len() / 2].to_string();
        fs::write(&record_path, &half).expect("truncate the record");

        let error = remove_statusline_at(&config_dir)
            .expect_err("a removal that cannot name what it restores is not a removal");

        assert!(error.contains("cannot be read"), "{error}");
        assert_eq!(
            settings_of(&config_dir)[STATUS_LINE_KEY],
            ours,
            "the row was taken out with no way to say what it wrapped"
        );
        assert!(
            config_dir
                .join(HOOKS_DIRNAME)
                .join(script_filename())
                .exists(),
            "the script the row still names was deleted"
        );
        assert_eq!(fs::read_to_string(&record_path).expect("record"), half);
    }

    #[test]
    fn a_status_line_that_only_shares_our_name_is_wrapped_rather_than_claimed() {
        // Regression: 79be608 claimed any command merely *containing*
        // `taurhaus-statusline` as taurhaus's own. A user's own script named for
        // this integration — `~/bin/taurhaus-statusline-mine.sh` — was therefore
        // deleted outright by the downgrade path, which takes the row out when
        // no record says what it wrapped; and installing over it never wrapped
        // it, because the installer read it as its own seat.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let theirs = json!({
            "type": "command",
            "command": "/home/user/bin/taurhaus-statusline-mine.sh"
        });
        write_settings_json(&config_dir, json!({ "statusLine": theirs.clone() }));

        assert!(
            !remove_statusline_at(&config_dir).expect("remove"),
            "a row that is not ours is not ours to take out"
        );
        assert_eq!(settings_of(&config_dir)[STATUS_LINE_KEY], theirs);

        // And it is a status line like any other: it gets wrapped, and removal
        // hands it straight back.
        let install =
            ensure_statusline_installed_at(&config_dir, &temp.path().join("exe"), &sink_for(&temp))
                .expect("install");
        assert!(install.changed && install.wrapped);
        assert!(script_of(&config_dir).contains("/home/user/bin/taurhaus-statusline-mine.sh"));
        assert!(remove_statusline_at(&config_dir).expect("remove"));
        assert_eq!(settings_of(&config_dir)[STATUS_LINE_KEY], theirs);
    }

    #[cfg(unix)]
    #[test]
    fn the_settings_file_keeps_the_mode_the_user_gave_it() {
        // Regression: 79be608 replaced `settings.json` by renaming a fresh
        // temporary file over it and never carried the destination's mode
        // across. A settings file the user had locked to 0600 — it holds
        // permission rules, and can hold credentials — came back 0644 the first
        // time taurhaus installed a status line into it.
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        write_settings_json(&config_dir, json!({ "model": "claude-fable-5" }));
        let settings_path = config_dir.join(SETTINGS_FILENAME);
        fs::set_permissions(&settings_path, fs::Permissions::from_mode(0o600)).expect("chmod");

        ensure_statusline_installed_at(&config_dir, &temp.path().join("exe"), &sink_for(&temp))
            .expect("install");

        assert_eq!(
            fs::metadata(&settings_path)
                .expect("stat")
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "the user's mode was replaced by the process umask's"
        );

        // And a settings file taurhaus creates is private from the start.
        let fresh = temp.path().join(".claude-account2");
        ensure_statusline_installed_at(&fresh, &temp.path().join("exe"), &sink_for(&temp))
            .expect("install");
        assert_eq!(
            fs::metadata(fresh.join(SETTINGS_FILENAME))
                .expect("stat")
                .permissions()
                .mode()
                & 0o077,
            0,
            "a settings file taurhaus created is readable by others"
        );
    }

    /// A stand-in for the sink, or for the command a user had configured.
    #[cfg(unix)]
    fn write_stub(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(path.parent().expect("parent")).expect("stub dir");
        fs::write(path, format!("#!/usr/bin/env bash\n{body}\n")).expect("write stub");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
    }

    /// How long a test waits for a status line that should be bounded by the
    /// script itself. Comfortably above the script's own deadline and far below
    /// the 30 seconds a stalling stub sink would take.
    #[cfg(unix)]
    const SINK_STALL_LIMIT: std::time::Duration = std::time::Duration::from_secs(12);

    /// The generated script, run the way Claude Code runs it.
    #[cfg(unix)]
    fn run_script(config_dir: &Path, payload: &str) -> (bool, String) {
        run_script_within(config_dir, payload, std::time::Duration::from_secs(30))
            .expect("status line finishes")
    }

    /// The same, but never hanging the test suite: `None` when the script was
    /// still running at `limit`, which is itself the failure worth reporting.
    #[cfg(unix)]
    fn run_script_within(
        config_dir: &Path,
        payload: &str,
        limit: std::time::Duration,
    ) -> Option<(bool, String)> {
        use std::io::Write as _;
        use std::process::{Command, Stdio};
        use std::time::Instant;

        let script = config_dir.join(HOOKS_DIRNAME).join(script_filename());
        // Captured through a file rather than a pipe: reading a pipe means
        // waiting for the child, which is the very thing under test.
        let captured = config_dir.with_extension("status-line.out");
        let mut child = Command::new("bash")
            .arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(
                fs::File::create(&captured).expect("capture file"),
            ))
            .stderr(Stdio::null())
            .spawn()
            .expect("run the status line");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(payload.as_bytes())
            .expect("feed the payload");

        let started = Instant::now();
        loop {
            match child.try_wait().expect("status line") {
                Some(status) => {
                    return Some((
                        status.success(),
                        fs::read_to_string(&captured).unwrap_or_default(),
                    ))
                }
                None if started.elapsed() >= limit => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                None => std::thread::sleep(std::time::Duration::from_millis(25)),
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_status_line_taurhaus_installs_never_renders_a_blank_row() {
        // Regression: a574720 printed whatever the sink printed and nothing
        // else. The sink prints nothing for a payload with no model and no
        // windows, and nothing for one it cannot parse — so the row taurhaus
        // took over from an account that had no status line of its own went
        // blank, which is exactly what installing a renderer was meant to
        // prevent.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let exe = temp.path().join("bin").join("taurhaus-daemon");
        // A sink that reads the payload and finds nothing worth a line.
        write_stub(&exe, "cat >/dev/null\nexit 0");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        for payload in ["{}", "not json at all", "", r#"{"model":{}}"#] {
            let (ok, line) = run_script(&config_dir, payload);
            assert!(ok, "the status line must not fail on '{payload}'");
            assert!(
                !line.trim().is_empty(),
                "'{payload}' left the user's status line blank"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_sink_that_cannot_run_still_leaves_a_line() {
        // Regression: a574720 sent the sink's stderr to `/dev/null` and printed
        // its (empty) stdout. A daemon that had been moved or removed since the
        // install therefore cost the user the whole row, silently.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let missing = temp.path().join("bin").join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &missing, &sink_for(&temp)).expect("install");

        let (ok, line) = run_script(&config_dir, OBSERVED_PAYLOAD_FOR_SCRIPT);
        assert!(ok, "a missing sink must not fail the status line");
        assert!(!line.trim().is_empty(), "a missing sink blanked the row");
    }

    #[cfg(unix)]
    #[test]
    fn the_line_is_the_sink_s_whenever_the_sink_has_one() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let exe = temp.path().join("bin").join("taurhaus-daemon");
        write_stub(&exe, "cat >/dev/null\necho 'Haiku 4.5 · 5h 26% · 7d 17%'");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        let (ok, line) = run_script(&config_dir, OBSERVED_PAYLOAD_FOR_SCRIPT);

        assert!(ok);
        assert_eq!(line, "Haiku 4.5 · 5h 26% · 7d 17%\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_wrapped_status_line_is_never_given_a_fallback_of_ours() {
        // The fallback belongs to the row taurhaus took over. A user whose own
        // command chooses to print nothing has chosen an empty row, and putting
        // taurhaus's text there would be the disturbance wrapping exists to
        // avoid.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let theirs = temp.path().join("statusline-zq.sh");
        write_stub(&theirs, "cat >/dev/null");
        write_settings_json(
            &config_dir,
            json!({ "statusLine": { "type": "command", "command": theirs.display().to_string() } }),
        );
        let exe = temp.path().join("bin").join("taurhaus-daemon");
        write_stub(&exe, "cat >/dev/null");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        let (ok, line) = run_script(&config_dir, OBSERVED_PAYLOAD_FOR_SCRIPT);

        assert!(ok);
        assert_eq!(line, "", "their empty line is theirs to keep: {line:?}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_finished_sink_call_leaves_nothing_running_behind_it() {
        // Regression: c91a158 bounded every sink call with a watchdog subshell
        // whose first command is a child `sleep`. On the path taken several
        // times a second — the sink answers, the watchdog is killed — killing
        // that shell did not kill the `sleep` it was blocked on, which stayed
        // behind, reparented, for the rest of the deadline. A status line may
        // not litter the user's machine with processes.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let exe = temp.path().join("bin").join("taurhaus-daemon");
        write_stub(&exe, "cat >/dev/null\necho 'Haiku 4.5 · 5h 26% · 7d 17%'");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        // Both ways the script can bound a call: with `timeout` on the PATH,
        // and on a machine that has none.
        for path_env in [
            std::env::var("PATH").unwrap_or_default(),
            coreutils_without_timeout(&temp),
        ] {
            let (line, running) =
                run_script_in_its_own_session(&config_dir, OBSERVED_PAYLOAD_FOR_SCRIPT, &path_env);

            assert_eq!(line, "Haiku 4.5 · 5h 26% · 7d 17%\n");
            assert!(
                running.is_empty(),
                "the status line left {running:?} running (PATH={path_env})"
            );
        }
    }

    /// The generated script, run as the leader of its own session — so that
    /// anything it leaves behind can still be found once it has exited.
    ///
    /// Returns the rendered line and whatever was still running afterwards.
    #[cfg(target_os = "linux")]
    fn run_script_in_its_own_session(
        config_dir: &Path,
        payload: &str,
        path_env: &str,
    ) -> (String, Vec<String>) {
        use std::io::Write as _;
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let script = config_dir.join(HOOKS_DIRNAME).join(script_filename());
        let session_file = config_dir.with_extension("session");
        let captured = config_dir.with_extension("status-line.out");
        let mut child = Command::new("setsid")
            .arg("--wait")
            .arg("bash")
            .arg("-c")
            .arg(format!(
                "printf '%s' \"$$\" >{}; exec bash {}",
                shell_quote(&session_file.display().to_string()),
                shell_quote(&script.display().to_string())
            ))
            .env("PATH", path_env)
            .stdin(Stdio::piped())
            // Captured through a file: an orphaned process would inherit a
            // pipe and hold it open, which is the very thing under test.
            .stdout(Stdio::from(
                fs::File::create(&captured).expect("capture file"),
            ))
            .stderr(Stdio::null())
            .spawn()
            .expect("setsid, to prove what the status line left running");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(payload.as_bytes())
            .expect("feed the payload");
        assert!(
            child.wait().expect("status line").success(),
            "the status line failed"
        );

        let session: u32 = fs::read_to_string(&session_file)
            .expect("session id")
            .trim()
            .parse()
            .expect("a pid");
        // A process the script killed on its way out is reaped by init rather
        // than by us, so give the session a moment to empty — far less than
        // the deadline a leaked timer would sit out.
        let started = Instant::now();
        loop {
            let running = processes_in_session(session);
            if running.is_empty() || started.elapsed() >= Duration::from_secs(1) {
                return (fs::read_to_string(&captured).unwrap_or_default(), running);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// Every process still alive in one session, named for the failure message.
    #[cfg(target_os = "linux")]
    fn processes_in_session(session: u32) -> Vec<String> {
        let mut running = Vec::new();
        for entry in fs::read_dir("/proc").expect("read /proc").flatten() {
            let name = entry.file_name();
            let Some(pid) = name.to_str().and_then(|name| name.parse::<u32>().ok()) else {
                continue;
            };
            let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };
            // `comm` can hold spaces and parentheses; the numeric fields start
            // after its closing one — state, ppid, pgrp, session.
            let Some(end) = stat.rfind(')') else { continue };
            let fields: Vec<&str> = stat[end + 1..].split_whitespace().collect();
            if fields.get(3).and_then(|field| field.parse::<u32>().ok()) != Some(session) {
                continue;
            }
            let command = fs::read_to_string(entry.path().join("cmdline")).unwrap_or_default();
            running.push(format!("{pid} {}", command.replace('\0', " ").trim()));
        }
        running
    }

    /// A PATH with everything the script needs on it except `timeout`.
    #[cfg(target_os = "linux")]
    fn coreutils_without_timeout(temp: &tempfile::TempDir) -> String {
        let bin = temp.path().join("no-timeout-bin");
        fs::create_dir_all(&bin).expect("bin dir");
        for tool in ["setsid", "bash", "cat", "mktemp", "rm", "sleep"] {
            let output = std::process::Command::new("bash")
                .arg("-c")
                .arg(format!("command -v {tool}"))
                .output()
                .expect("command -v");
            let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
            assert!(!resolved.is_empty(), "this host has no '{tool}'");
            let link = bin.join(tool);
            if !link.exists() {
                std::os::unix::fs::symlink(&resolved, &link).expect("link the tool");
            }
        }
        bin.display().to_string()
    }

    #[cfg(unix)]
    #[test]
    fn a_wrapped_command_that_reads_stdin_into_a_variable_still_gets_the_payload() {
        // Regression: 79be608 rendered the wrap as `printf … | <command>`, where
        // the pipe binds to that command's *first* pipeline only. A status line
        // written the ordinary way — `input="$(cat)"; render "$input"` — read the
        // payload inside the pipe's subshell and rendered from an empty variable
        // outside it. Wrapped, still running, and blind: the one thing wrapping
        // promised not to do.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        write_settings_json(
            &config_dir,
            json!({
                "statusLine": {
                    "type": "command",
                    "command": "input=\"$(cat)\"; printf 'theirs: %s\\n' \"$input\""
                }
            }),
        );
        let exe = temp.path().join("bin").join("taurhaus-daemon");
        write_stub(&exe, "cat >/dev/null");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        let (ok, line) = run_script(&config_dir, OBSERVED_PAYLOAD_FOR_SCRIPT);

        assert!(ok);
        assert_eq!(line, format!("theirs: {OBSERVED_PAYLOAD_FOR_SCRIPT}\n"));

        // And the exit code is still theirs, which is what the row is judged by.
        let failing = temp.path().join(".claude-account2");
        write_settings_json(
            &failing,
            json!({
                "statusLine": {
                    "type": "command",
                    "command": "input=\"$(cat)\"; [ -n \"$input\" ] && exit 3"
                }
            }),
        );
        ensure_statusline_installed_at(&failing, &exe, &sink_for(&temp)).expect("install");
        assert!(!run_script(&failing, OBSERVED_PAYLOAD_FOR_SCRIPT).0);
    }

    #[cfg(unix)]
    const OBSERVED_PAYLOAD_FOR_SCRIPT: &str =
        r#"{"session_id":"c530b681","model":{"display_name":"Haiku 4.5"}}"#;
}
