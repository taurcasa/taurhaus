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
//!   only while it says exactly what an install wrote — a command that merely
//!   contains this config dir's script path is a status line like any other —
//!   and a row that *is* ours whose record cannot be read stops the removal
//!   where it stands, because the command it wraps is written down nowhere
//!   else;
//! * a row that is not ours but still *runs* our script — `/bin/bash <script>`,
//!   an edit of the command an install wrote — is neither wrapped, rewritten
//!   nor removed underneath: wrapping it would put an invocation of the script
//!   inside the script, and deleting it would leave the row naming a file that
//!   is gone. `<script>.backup` is a different word and stays unrelated;
//! * a `settings.json` the user symlinked is written *through*, never over: the
//!   rename that publishes a new file lands in the link's target directory, so
//!   a dotfiles-managed or shared settings file keeps both the link and the
//!   file it points at;
//! * neither generated file is published wider than the settings the wrapped
//!   command came out of: the script is 0700 and the record 0600, because both
//!   carry that command verbatim — checked on every install, not only on the
//!   one that writes them, so a mode widened since is narrowed again;
//! * one install at startup is not the whole story: a pass also runs whenever
//!   anything asks for the accounts, throttled to the minute the account scan
//!   is cached for, so an account signed in since — or one whose `.claude.json`
//!   was mid-rewrite and named nobody — is bridged without a restart.
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
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
/// How often the bridge is reconciled. The account scan a pass reads is cached
/// for exactly this long, so a second pass inside one would re-read the same
/// answer — and a pass probes `claude --version` before it decides anything.
const BRIDGE_PASS_INTERVAL: Duration = Duration::from_secs(60);
/// When the last reconciliation pass in this process started.
static LAST_BRIDGE_PASS: Mutex<Option<Instant>> = Mutex::new(None);

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
    /// The exact `statusLine` command this install wrote. A row is taurhaus's
    /// when it says this and nothing else — see `is_taurhaus_status_line`.
    /// Absent only in a record written before this build.
    command: Option<String>,
    /// The `statusLine` value that was configured before taurhaus wrapped it.
    wrapped: Option<Value>,
}

impl StatuslineRecord {
    fn to_value(&self) -> Value {
        json!({
            "executable": self.executable,
            "sink": self.sink,
            "command": self.command.clone().map(Value::String).unwrap_or(Value::Null),
            "wrapped": self.wrapped.clone().unwrap_or(Value::Null),
        })
    }

    fn from_value(value: &Value) -> Option<Self> {
        Some(Self {
            executable: value.get("executable")?.as_str()?.to_string(),
            sink: value.get("sink")?.as_str()?.to_string(),
            command: value
                .get("command")
                .and_then(Value::as_str)
                .map(str::to_string),
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
    let command = status_line_command(&script_command);
    for _ in 0..COMMIT_ATTEMPTS {
        let existing = load_settings(&settings_path)?.remove(STATUS_LINE_KEY);
        // Re-running against our own install must not wrap our own script: the
        // command the user actually configured is the one the record remembers.
        let wrapped =
            if is_taurhaus_status_line(existing.as_ref(), &owned_command(&hooks_dir, &command)) {
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
            } else if references_our_script(existing.as_ref(), &script_command) {
                // Not the command an install wrote, but one that still runs
                // this script — `/bin/bash <script>`, an edit of ours. Wrapping
                // it would put an invocation of this script inside the script
                // itself, and rewriting it would take a row taurhaus cannot
                // prove it owns. It renders usage as it stands; leave it.
                return Ok(StatuslineInstall {
                    skipped: Some("references_script"),
                    ..StatuslineInstall::default()
                });
            } else {
                existing.clone().filter(|value| !value.is_null())
            };

        let script = render_script(
            &executable,
            &config_dir_argument,
            &sink_argument,
            wrapped_command(wrapped.as_ref()),
        );
        // 0700, not 0755: the wrapped command is written into this script
        // verbatim, and it came out of a `settings.json` that is the user's to
        // keep private. Nobody but its owner runs a status line anyway.
        let script_changed = publish_if_changed(&script_path, script.as_bytes(), 0o700)?;

        let record = StatuslineRecord {
            executable: executable.clone(),
            sink: sink_argument.clone(),
            command: Some(command.clone()),
            wrapped: wrapped.clone(),
        };
        // And 0600 here for the same reason: the record holds that command
        // whole, which is the point of it.
        let record_changed = publish_if_changed(
            &hooks_dir.join(RECORD_FILENAME),
            serde_json::to_vec_pretty(&record.to_value())
                .map_err(|error| format!("failed to serialize the status line record: {error}"))?
                .as_slice(),
            0o600,
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
        desired.insert("command".to_string(), Value::String(command.clone()));

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
    let script_command = script_reference(&hooks_dir);
    let ours = owned_command(&hooks_dir, &status_line_command(&script_command));
    for _ in 0..COMMIT_ATTEMPTS {
        let current = load_settings(&settings_path)?.get(STATUS_LINE_KEY).cloned();
        if !is_taurhaus_status_line(current.as_ref(), &ours) {
            if references_our_script(current.as_ref(), &script_command) {
                // Not a row an install wrote, so there is nothing of the
                // user's to hand back — but it still runs this script, and
                // deleting the script under it would leave the row naming a
                // file that is gone: a blank status line on every refresh.
                // Restore nothing, remove nothing, and say nothing changed.
                return Ok(false);
            }
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
    let ours = record
        .command
        .unwrap_or_else(|| status_line_command(&script_reference(&hooks_dir)));
    load_settings(&config_dir.join(SETTINGS_FILENAME))
        .ok()
        .is_some_and(|settings| is_taurhaus_status_line(settings.get(STATUS_LINE_KEY), &ours))
}

/// Reconcile the bridge for every detected account, on a thread of its own and
/// at most once a minute.
///
/// One pass at startup is not enough. An account signed in since is one the
/// scan reports and nothing installs into; so is one whose `.claude.json` was
/// being rewritten in place when the pass ran, because a dir caught mid-write
/// names nobody and the scan caches that for a minute. Either would have gone
/// unbridged — no usage bar, and a chooser saying "no usage yet" about an
/// account in use — until the daemon happened to restart. So a pass also runs
/// whenever anything asks for the accounts: the daemon serving
/// `list_claude_accounts`, and the app answering that same command on a native
/// host, where the request never crosses the daemon.
///
/// Behind the answer, never in front of it: a pass probes two CLIs before it
/// decides anything. And the throttle is read here rather than in the thread,
/// so a burst of requests costs one pass rather than one thread each.
pub fn ensure_statusline_bridge_soon(taurhaus_exe: PathBuf) {
    if !pass_is_due(&LAST_BRIDGE_PASS, Instant::now()) {
        return;
    }
    if let Err(error) = std::thread::Builder::new()
        .name("claude-usage-statusline".to_string())
        .spawn(move || install_statusline_for_detected_accounts(&taurhaus_exe))
    {
        tracing::warn!(error = %error, "Claude usage status line pass not spawned");
    }
}

/// The same pass, on this thread — for the caller that already gave it one:
/// daemon startup, which runs the install beside the listener rather than in
/// front of it.
pub fn ensure_statusline_bridge(taurhaus_exe: &Path) {
    if !pass_is_due(&LAST_BRIDGE_PASS, Instant::now()) {
        return;
    }
    install_statusline_for_detected_accounts(taurhaus_exe);
}

/// Whether enough has passed since the last pass to run another — stamping
/// `now` when it says yes, so two callers at once produce one pass, not two.
fn pass_is_due(last: &Mutex<Option<Instant>>, now: Instant) -> bool {
    let mut last = last.lock().unwrap_or_else(|error| error.into_inner());
    if last.is_some_and(|previous| now.duration_since(previous) < BRIDGE_PASS_INTERVAL) {
        return false;
    }
    *last = Some(now);
    true
}

/// Install the bridge in every detected account, when the CLI can feed it.
///
/// Reached only through the two `ensure_statusline_bridge…` entry points above,
/// so that every caller shares one throttle.
///
/// A build older than the one this was verified against gets nothing: its
/// payload is not documented to carry `rate_limits`, and rewriting a user's
/// `statusLine` for numbers that never arrive is a bad trade. If one is already
/// installed — the user downgraded, or switched to another `claude` on their
/// PATH — it is taken back out here, because that same trade is no better for
/// having been made yesterday.
fn install_statusline_for_detected_accounts(taurhaus_exe: &Path) {
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
    install_for_detected_accounts(taurhaus_exe, &sink_path);
}

/// One pass over the accounts the scan reports *right now*.
///
/// Read fresh on every pass rather than carried over from the last one: the
/// point of running again is that the answer changes — an account signed in, or
/// a `.claude.json` that has finished being rewritten.
fn install_for_detected_accounts(taurhaus_exe: &Path, sink_path: &Path) {
    for account in detect_claude_accounts_cached() {
        let mut fields = Map::new();
        fields.insert(
            "config_dir".to_string(),
            Value::String(account.config_dir.display().to_string()),
        );
        fields.insert("account_id".to_string(), Value::String(account.id.clone()));
        match ensure_statusline_installed_at(&account.config_dir, taurhaus_exe, sink_path) {
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

/// Whether this `statusLine` is the one taurhaus wrote.
///
/// The exact command, not a path found somewhere inside it. Containment answers
/// a different question than the one being asked: `…/taurhaus-statusline.sh.backup`
/// contains this config dir's script path without being this config dir's script,
/// and would be claimed — then replaced — as taurhaus's own row. It gets the
/// opposite wrong too, because the command written here shell-quotes that path
/// and `shell_quote` breaks an apostrophe out of the quotes: under a home like
/// `/home/o'connor` the row taurhaus had just written contains no such substring
/// at all, and the next install wraps the script around itself.
///
/// So: the row is taurhaus's only while it says exactly what an install wrote —
/// the command the record remembers, or, with no record to ask, the one this
/// build would write for this config dir. A row edited by hand is the user's
/// again, which is the safe way round: it is wrapped rather than overwritten,
/// and the script under it is left where it stands rather than deleted.
fn is_taurhaus_status_line(value: Option<&Value>, ours: &str) -> bool {
    row_command(value).is_some_and(|command| command == ours)
}

/// Whether this `statusLine` runs this config dir's script without being the
/// row an install wrote.
///
/// Exact equality is the right answer to "is this ours to rewrite", and the
/// wrong answer to "is this ours to delete underneath". A row edited from
/// `bash '<script>'` to the equivalent `/bin/bash <script>` is neither: it is
/// not a command taurhaus can prove it wrote, so it is not one to replace, and
/// it is not a status line that survives its script being removed either. It
/// renders usage exactly as it stands, and so it is left exactly as it stands.
///
/// The question is asked of the command's *words*, as the shell would split
/// them — so `<script>.backup`, one word that merely starts with the script's
/// path, stays the unrelated command it is.
fn references_our_script(value: Option<&Value>, script_path: &str) -> bool {
    let Some(command) = row_command(value) else {
        return false;
    };
    shell_words(command)
        .iter()
        .any(|word| word.as_str() == script_path)
}

/// The command a `statusLine` runs, however the row spells it.
fn row_command(value: Option<&Value>) -> Option<&str> {
    match value? {
        Value::String(command) => Some(command.as_str()),
        value @ Value::Object(_) => value.get("command").and_then(Value::as_str),
        _ => None,
    }
}

/// The words a shell would split this command into.
///
/// Enough of one to answer a single question — does this command name our
/// script? — so it splits on whitespace and honours the three quotings a path
/// can be written in: single quotes take everything to the next one, double
/// quotes take everything but their own escapes, and a bare backslash escapes
/// the character after it. A quote nothing closes ends its word where the
/// command does, which is what the shell itself would have nothing to run.
fn shell_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut characters = command.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            character if character.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                for quoted in characters.by_ref() {
                    if quoted == '\'' {
                        break;
                    }
                    word.push(quoted);
                }
            }
            '"' => {
                started = true;
                while let Some(quoted) = characters.next() {
                    match quoted {
                        '"' => break,
                        // Inside double quotes a backslash escapes only these.
                        '\\' if matches!(characters.peek(), Some('"' | '\\' | '$' | '`')) => {
                            word.extend(characters.next());
                        }
                        quoted => word.push(quoted),
                    }
                }
            }
            '\\' => {
                started = true;
                word.extend(characters.next());
            }
            character => {
                started = true;
                word.push(character);
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// The `statusLine` command a row of taurhaus's says, as this install wrote it.
///
/// The record's, so that a row stays recognisable across a change to how the
/// installer renders this; this build's otherwise, so that a row whose record
/// went missing is still taurhaus's rather than something to wrap again.
fn owned_command(hooks_dir: &Path, generated: &str) -> String {
    read_record(hooks_dir)
        .and_then(|record| record.command)
        .unwrap_or_else(|| generated.to_string())
}

/// How `settings.json` invokes the generated script.
fn status_line_command(script_command: &str) -> String {
    format!("bash {}", shell_quote(script_command))
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
    let settings_path = &resolved_settings_path(settings_path);
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

/// Where a `settings.json` actually keeps its bytes.
///
/// Reading follows a symlink on its own; the rename that publishes a new file
/// does the opposite — it replaces the link itself, and a `settings.json` the
/// user symlinked into a dotfiles repo, or shared between two config dirs,
/// silently stops being either. So the write follows the link to the file it
/// points at and publishes there, leaving the link exactly as the user made it.
///
/// A relative link resolves against the link's own directory, which is what
/// `canonicalize` does and what the hand-resolution below has to do too: that
/// one is for a link whose target does not exist yet, where there is no
/// canonical path to ask for and the write is what creates it.
fn resolved_settings_path(settings_path: &Path) -> std::path::PathBuf {
    let is_symlink = fs::symlink_metadata(settings_path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);
    if !is_symlink {
        return settings_path.to_path_buf();
    }
    if let Ok(resolved) = fs::canonicalize(settings_path) {
        return resolved;
    }
    match fs::read_link(settings_path) {
        Ok(target) if target.is_absolute() => target,
        Ok(target) => settings_path
            .parent()
            .map(|parent| parent.join(&target))
            .unwrap_or(target),
        Err(_) => settings_path.to_path_buf(),
    }
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
///
/// `mode` is set before that rename, for two reasons: whatever appears under
/// `path` has to be runnable the instant it appears there, and both files carry
/// the command that was configured before the wrap — copied out of a
/// `settings.json` that may well be 0600, and may well hold a token. A rename
/// makes a *new* file, so a mode left to the umask is one taurhaus chose.
///
/// The mode is part of what "already published" means, not a side effect of
/// publishing: a file whose bytes are current but whose mode has been widened
/// since — by an upgrade from the build that wrote 0755, or by anything else on
/// the machine — is still carrying the user's command where others can read it,
/// and would never be narrowed again if the content alone decided.
fn publish_if_changed(path: &Path, payload: &[u8], mode: u32) -> Result<bool, String> {
    if fs::read(path).is_ok_and(|current| current == payload) {
        return narrow_to_mode(path, mode);
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
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = fs::set_permissions(&temp_path, fs::Permissions::from_mode(mode)) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!(
                "failed to set the mode of '{}': {error}",
                temp_path.display()
            ));
        }
    }
    #[cfg(target_os = "windows")]
    let _ = mode;
    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("failed to replace '{}': {error}", path.display()));
    }
    Ok(true)
}

/// Put an already-current artifact back to the mode it was published with.
/// `true` when that was a change.
#[cfg(not(target_os = "windows"))]
fn narrow_to_mode(path: &Path, mode: u32) -> Result<bool, String> {
    use std::os::unix::fs::PermissionsExt;
    let current = fs::metadata(path)
        .map_err(|error| format!("failed to read the mode of '{}': {error}", path.display()))?
        .permissions()
        .mode()
        & 0o777;
    if current == mode {
        return Ok(false);
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("failed to set the mode of '{}': {error}", path.display()))?;
    Ok(true)
}

#[cfg(target_os = "windows")]
fn narrow_to_mode(_path: &Path, _mode: u32) -> Result<bool, String> {
    Ok(false)
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
                & 0o700,
            0o700
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

    #[test]
    fn a_config_dir_whose_path_holds_an_apostrophe_never_wraps_its_own_script() {
        // Regression: 0ab7e1f decided whose row this is with
        // `command.contains(script_path)`, while the command it writes
        // shell-quotes that same path — and `shell_quote` breaks an embedded
        // apostrophe out of the quotes as `'"'"'`. For a home like
        // `/home/o'connor`, the row taurhaus had just written no longer
        // contained the raw path, so the next install read its own row as the
        // user's own status line and wrapped the script around an invocation of
        // itself: a sink per install, forever.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join("o'connor").join(".claude");
        let exe = temp.path().join("taurhaus-daemon");

        let first =
            ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");
        let second = ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp))
            .expect("second install");

        assert!(first.changed && !first.wrapped);
        assert_eq!(
            second,
            StatuslineInstall {
                changed: false,
                wrapped: false,
                skipped: None,
            },
            "the bridge did not recognise the row it had just written"
        );
        let script = script_of(&config_dir);
        assert_eq!(script.matches(USAGE_SINK_SUBCOMMAND).count(), 1);
        assert!(
            !script.contains(&script_filename()),
            "the bridge wrapped itself: {script}"
        );
        assert!(statusline_is_installed_at(&config_dir));
        // And the row it holds is still one removal recognises as its own.
        assert!(remove_statusline_at(&config_dir).expect("remove"));
        assert!(!settings_of(&config_dir).contains_key(STATUS_LINE_KEY));
    }

    #[test]
    fn a_command_that_merely_extends_our_script_path_is_wrapped_rather_than_claimed() {
        // Regression: 0ab7e1f claimed any command *containing* this config dir's
        // script path, so a command of the user's own that merely starts with it
        // — `…/taurhaus-statusline.sh.backup`, the copy they kept — read as
        // taurhaus's own row. With a record from an earlier install still beside
        // it, the install then replaced that command with its own and remembered
        // the record's older `wrapped` value instead: the status line the user
        // had actually configured was gone, with nothing left naming it.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        write_settings_json(
            &config_dir,
            json!({ "statusLine": { "type": "command", "command": "old-line.sh" } }),
        );
        let exe = temp.path().join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

        let backup = format!(
            "{}.backup",
            config_dir
                .join(HOOKS_DIRNAME)
                .join(script_filename())
                .display()
        );
        let theirs = json!({ "type": "command", "command": backup.clone() });
        write_settings_json(&config_dir, json!({ "statusLine": theirs.clone() }));

        let install =
            ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("reinstall");

        assert!(install.changed && install.wrapped);
        assert!(
            script_of(&config_dir).contains(&format!("{{\n{backup}\n}}\n")),
            "the command the user configured was not wrapped: {}",
            script_of(&config_dir)
        );
        assert_eq!(
            read_record(&config_dir.join(HOOKS_DIRNAME))
                .expect("record")
                .wrapped,
            Some(theirs.clone())
        );
        assert!(remove_statusline_at(&config_dir).expect("remove"));
        assert_eq!(settings_of(&config_dir)[STATUS_LINE_KEY], theirs);
    }

    #[cfg(unix)]
    #[test]
    fn the_command_taurhaus_wraps_is_never_published_wider_than_the_settings_it_came_from() {
        // Regression: 0ab7e1f published the generated script 0755 and the record
        // with nothing but the process umask. Both carry the command that was
        // configured before the wrap — verbatim in the script, whole in the
        // record — so a `statusLine` the user kept inside a 0600 `settings.json`,
        // any inline token with it, came back readable by every other account on
        // the machine the moment taurhaus wrapped it.
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        write_settings_json(
            &config_dir,
            json!({
                "statusLine": {
                    "type": "command",
                    "command": "render-line --token s3cr3t-not-for-everyone"
                }
            }),
        );
        let settings_path = config_dir.join(SETTINGS_FILENAME);
        fs::set_permissions(&settings_path, fs::Permissions::from_mode(0o600)).expect("chmod");

        ensure_statusline_installed_at(&config_dir, &temp.path().join("exe"), &sink_for(&temp))
            .expect("install");

        let script_path = config_dir.join(HOOKS_DIRNAME).join(script_filename());
        let record_path = config_dir.join(HOOKS_DIRNAME).join(RECORD_FILENAME);
        for path in [&script_path, &record_path] {
            assert!(
                fs::read_to_string(path)
                    .expect("published")
                    .contains("s3cr3t-not-for-everyone"),
                "'{}' does not carry the wrapped command at all",
                path.display()
            );
            assert_eq!(
                fs::metadata(path).expect("stat").permissions().mode() & 0o077,
                0,
                "'{}' is readable by every other account on this machine",
                path.display()
            );
        }
        // And the script is still the owner's to run.
        assert_eq!(
            fs::metadata(&script_path)
                .expect("stat")
                .permissions()
                .mode()
                & 0o700,
            0o700
        );
    }

    #[test]
    fn a_row_that_invokes_our_script_another_way_is_neither_wrapped_nor_rewritten() {
        // Regression: 6262c47 made ownership exact string equality — right for
        // `<script>.backup`, wrong for the same script invoked another way. A
        // row edited from `bash '<script>'` to `/bin/bash <script>` read as the
        // user's own status line, so the next install wrapped that command
        // inside the very script it names: a status line that runs itself,
        // once more per install, forever.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        let exe = temp.path().join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");
        let theirs = json!({
            "type": "command",
            "command": format!(
                "/bin/bash {}",
                config_dir.join(HOOKS_DIRNAME).join(script_filename()).display()
            ),
        });
        write_settings_json(&config_dir, json!({ "statusLine": theirs.clone() }));
        let script_before = script_of(&config_dir);

        let install =
            ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("reinstall");

        assert_eq!(install.skipped, Some("references_script"));
        assert!(!install.changed);
        assert_eq!(
            script_of(&config_dir),
            script_before,
            "the bridge wrapped an invocation of itself"
        );
        assert_eq!(settings_of(&config_dir)[STATUS_LINE_KEY], theirs);
    }

    #[test]
    fn removal_never_deletes_a_script_the_row_still_invokes() {
        // Regression: 6262c47 read anything but its own exact command as a
        // foreign row — nothing of taurhaus's to restore — and then deleted the
        // script and the record anyway. A row the user had edited from
        // `bash '<script>'` to `/bin/bash <script>` was left pointing at a file
        // that no longer existed: a blank status line on every refresh, with
        // the command it wrapped deleted beside it.
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        write_settings_json(
            &config_dir,
            json!({ "statusLine": { "type": "command", "command": "my-line.sh" } }),
        );
        let exe = temp.path().join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");
        let script_path = config_dir.join(HOOKS_DIRNAME).join(script_filename());
        let record_path = config_dir.join(HOOKS_DIRNAME).join(RECORD_FILENAME);
        let theirs = json!({
            "type": "command",
            "command": format!("/bin/bash {}", script_path.display()),
        });
        write_settings_json(&config_dir, json!({ "statusLine": theirs.clone() }));

        assert!(
            !remove_statusline_at(&config_dir).expect("remove"),
            "a row taurhaus did not write is not a row it restores"
        );

        assert_eq!(settings_of(&config_dir)[STATUS_LINE_KEY], theirs);
        assert!(
            script_path.exists(),
            "the script the row still invokes was deleted"
        );
        assert!(
            record_path.exists(),
            "the record of the command that script wraps was deleted"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_settings_file_the_user_symlinked_is_written_through_rather_than_replaced() {
        // Regression: 79be608 replaced `settings.json` by renaming a fresh
        // temporary file over it. A `settings.json` symlinked into a dotfiles
        // repo — or shared between two config dirs — was therefore replaced by
        // a regular file the first time taurhaus installed or removed a status
        // line, severing a link the user made on purpose without saying so.
        let temp = tempfile::tempdir().expect("temp dir");
        let exe = temp.path().join("taurhaus-daemon");
        let original = json!({ "type": "command", "command": "my-line.sh" });

        for (root, relative) in [("absolute", false), ("relative", true)] {
            let config_dir = temp.path().join(root).join(".claude");
            let target = temp
                .path()
                .join(root)
                .join("dotfiles")
                .join("settings.json");
            fs::create_dir_all(&config_dir).expect("config dir");
            fs::create_dir_all(target.parent().expect("parent")).expect("dotfiles dir");
            fs::write(
                &target,
                serde_json::to_vec_pretty(&json!({ "statusLine": original.clone() }))
                    .expect("serialize"),
            )
            .expect("write the link's target");
            let link = config_dir.join(SETTINGS_FILENAME);
            let link_value = if relative {
                std::path::PathBuf::from("../dotfiles/settings.json")
            } else {
                target.clone()
            };
            std::os::unix::fs::symlink(&link_value, &link).expect("symlink");

            ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");

            assert!(
                fs::symlink_metadata(&link)
                    .expect("stat")
                    .file_type()
                    .is_symlink(),
                "the install replaced the {root} symlink with a regular file"
            );
            assert_eq!(fs::read_link(&link).expect("read the link"), link_value);
            let through: Value =
                serde_json::from_str(&fs::read_to_string(&target).expect("the link's target"))
                    .expect("json");
            assert!(
                through[STATUS_LINE_KEY]["command"]
                    .as_str()
                    .is_some_and(|command| command.contains(SCRIPT_BASENAME)),
                "the {root} install never reached the link's target"
            );

            assert!(remove_statusline_at(&config_dir).expect("remove"));

            assert!(
                fs::symlink_metadata(&link)
                    .expect("stat")
                    .file_type()
                    .is_symlink(),
                "the removal replaced the {root} symlink with a regular file"
            );
            let restored: Value =
                serde_json::from_str(&fs::read_to_string(&target).expect("the link's target"))
                    .expect("json");
            assert_eq!(restored[STATUS_LINE_KEY], original);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_reinstall_takes_back_the_permissions_a_widened_artifact_lost() {
        // Regression: 6262c47 set 0700 and 0600 only while publishing new
        // bytes, and returned before touching the mode at all when the content
        // was already current. A script or record widened since — by an upgrade
        // from the build that published 0755, or by anything else on the
        // machine — therefore stayed readable by every other account for as
        // long as nothing about it changed, which is forever.
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        write_settings_json(
            &config_dir,
            json!({
                "statusLine": {
                    "type": "command",
                    "command": "render-line --token s3cr3t-not-for-everyone"
                }
            }),
        );
        let exe = temp.path().join("taurhaus-daemon");
        ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("install");
        let script_path = config_dir.join(HOOKS_DIRNAME).join(script_filename());
        let record_path = config_dir.join(HOOKS_DIRNAME).join(RECORD_FILENAME);
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).expect("widen");
        fs::set_permissions(&record_path, fs::Permissions::from_mode(0o644)).expect("widen");

        let again =
            ensure_statusline_installed_at(&config_dir, &exe, &sink_for(&temp)).expect("reinstall");

        assert!(
            again.changed,
            "narrowing a widened artifact is a change worth reporting"
        );
        assert_eq!(
            fs::metadata(&script_path)
                .expect("stat")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&record_path)
                .expect("stat")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
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

    /// A detected account, as the scan would report one for this config dir.
    fn account_at(config_dir: &Path) -> crate::session_scanner::claude_accounts::ClaudeAccount {
        crate::session_scanner::claude_accounts::ClaudeAccount {
            id: config_dir.display().to_string(),
            config_dir: config_dir.to_path_buf(),
            email: "user@example.com".to_string(),
            display_name: None,
            organization: None,
            seat_tier: None,
            logged_in: true,
            is_default: false,
            is_process_default: false,
            usage: None,
        }
    }

    #[test]
    fn an_account_that_signs_in_after_startup_is_bridged_by_the_next_pass() {
        // Regression: 6262c47 installed the bridge from the daemon's startup
        // and from nowhere else. A subscription the user signed in to
        // afterwards was never bridged at all, so its usage bar stayed empty —
        // and the chooser said "no usage yet" about an account that was being
        // used — until the daemon happened to be restarted. A pass reconciles
        // whatever the scan reports at the time it runs, and one runs whenever
        // anything asks for the accounts.
        use crate::session_scanner::claude_accounts::install_detection_override;

        // A pass reports what it did through the global sink, so it takes the
        // same guard every other test that emits does — in the same order as
        // the detection override, which is acquired below it everywhere.
        let _log = crate::test_support::acquire_global_log_test_guard();
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = sink_for(&temp);
        let exe = temp.path().join("taurhaus-daemon");
        let first = temp.path().join(".claude");
        let second = temp.path().join(".claude-work");

        {
            let _detected = install_detection_override(vec![account_at(&first)]);
            install_for_detected_accounts(&exe, &sink);
        }

        assert!(statusline_is_installed_at(&first));
        assert!(!statusline_is_installed_at(&second));

        let _detected = install_detection_override(vec![account_at(&first), account_at(&second)]);
        install_for_detected_accounts(&exe, &sink);

        assert!(
            statusline_is_installed_at(&second),
            "the account that appeared after startup never got a bridge"
        );
        assert!(statusline_is_installed_at(&first));
    }

    #[test]
    fn a_config_dir_that_named_nobody_is_bridged_once_it_names_an_account_again() {
        // Regression: 6262c47, same single pass. `.claude.json` is rewritten in
        // place by Claude Code, so a config dir caught mid-rewrite names no
        // account — and the scan then caches that omission for a minute. A dir
        // that happened to be mid-rewrite during the one pass at startup was
        // therefore left unbridged for the life of the daemon, on nothing worse
        // than timing.
        use crate::session_scanner::claude_accounts::{
            install_detection_override, install_scan_override, ClaudeScan,
        };

        let _log = crate::test_support::acquire_global_log_test_guard();
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = sink_for(&temp);
        let exe = temp.path().join("taurhaus-daemon");
        let config_dir = temp.path().join(".claude");

        {
            // The dir is there; the file in it names nobody yet.
            let _scanned = install_scan_override(ClaudeScan {
                config_dirs: vec![config_dir.clone()],
                accounts: Vec::new(),
            });
            install_for_detected_accounts(&exe, &sink);
        }

        assert!(!statusline_is_installed_at(&config_dir));

        let _detected = install_detection_override(vec![account_at(&config_dir)]);
        install_for_detected_accounts(&exe, &sink);

        assert!(
            statusline_is_installed_at(&config_dir),
            "the config dir that was mid-rewrite never got a bridge"
        );
    }

    #[test]
    fn a_pass_runs_again_once_the_minute_it_shares_with_the_scan_is_up() {
        // Regression: 6262c47 ran one pass per daemon start, and the fix — a
        // pass per accounts request — has to cost no more than one pass a
        // minute: it probes `claude --version` and `codex --version` before it
        // decides anything, and the scan it reads is cached for exactly that
        // minute anyway.
        let last = Mutex::new(None);
        let start = Instant::now();

        assert!(pass_is_due(&last, start));
        assert!(!pass_is_due(&last, start + Duration::from_secs(1)));
        assert!(!pass_is_due(
            &last,
            start + BRIDGE_PASS_INTERVAL - Duration::from_millis(1)
        ));
        assert!(pass_is_due(&last, start + BRIDGE_PASS_INTERVAL));
        // And the pass that just ran resets the minute for the next one.
        assert!(!pass_is_due(
            &last,
            start + BRIDGE_PASS_INTERVAL + Duration::from_secs(1)
        ));
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
