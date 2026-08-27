//! Claude subscription usage, as the status line reports it.
//!
//! Claude Code hands the configured `statusLine` command a JSON payload on
//! stdin on every refresh. For a subscriber, that payload carries
//! `rate_limits.five_hour` and `rate_limits.seven_day` — the two windows the
//! product itself throttles on — as soon as the session has had one API
//! response. Verified live on 2.1.246: the block is `null` while a session is
//! still booting and appears with the first answer.
//!
//! This module is the sink side of that bridge. A short-lived process reads the
//! payload, and one record per account lands in `<app data>/claude-usage.jsonl`.
//! It is append-only and capped like `codex-notify.jsonl`, and it is written
//! from the status line of a *live TUI*, so it is written often: refreshes come
//! per keystroke. Three things keep that cheap: a throttle to one record per
//! account per 30 s, a bounded tail read for the throttle's own lookup, and a
//! lock that is never waited on — a refresh that queues is a terminal line that
//! queues with it, and a dropped record costs one keystroke.
//!
//! That lock is a sidecar, `claude-usage.jsonl.lock`, and never the sink itself:
//! the cap is enforced by publishing a compacted file over the old one with a
//! rename, so the only file everyone can agree on is one no rename ever touches.
//! The rename is also what makes the cap survive the two-second deadline the
//! status line runs under — a compaction that dies leaves the live sink whole
//! rather than holding whichever accounts had been written back so far.
//!
//! The read side takes the opposite trade. It waits a bounded moment for that
//! lock, and if it never comes it answers "unknown" rather than reading a file
//! a writer is in the middle of changing: a half-read sink reports
//! subscriptions as having no usage, and nothing on screen can tell that apart
//! from a subscription that has never reported at all.
//!
//! Nothing here reads credentials: the payload is Claude Code's own documented
//! status-line contract, and the account id comes from the config dir's
//! `.claude.json`, the same file account detection already reads.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::session_scanner::accounts::claude::ClaudeAccount;

pub const CLAUDE_USAGE_FILENAME: &str = "claude-usage.jsonl";

/// Same cap as the Codex notify sink; enforced at process startup.
const MAX_CLAUDE_USAGE_BYTES: u64 = 5 * 1024 * 1024;

/// One record per account per window. Status lines refresh per keystroke; the
/// numbers behind them move in percent, not in milliseconds.
const THROTTLE_SECONDS: i64 = 30;

/// How much of the sink's tail the *write* path looks at.
///
/// Only the throttle reads this, and only for the account about to report, so
/// a bound that never grows with the file keeps the per-keystroke path cheap.
/// The read side deliberately does not use it: see `latest_usage_records`.
const TAIL_SCAN_BYTES: u64 = 256 * 1024;

/// How long a read waits for a compacting writer before reading regardless.
const READ_LOCK_WAIT: std::time::Duration = std::time::Duration::from_millis(500);
const READ_LOCK_POLL: std::time::Duration = std::time::Duration::from_millis(10);

/// One rate-limit window, verbatim from the status-line payload.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUsageWindow {
    /// Percent of the window consumed. An integer in every payload observed on
    /// 2.1.246, typed as `number` in the schema.
    pub used_percentage: f64,
    /// Unix seconds at which the window resets, when the payload names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<i64>,
}

/// The usage of one account as the read side reports it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAccountUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<ClaudeUsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<ClaudeUsageWindow>,
    /// When the status line reported these numbers. The UI needs this: usage
    /// only flows while a session of that account is running, so a record is
    /// routinely old, and an old number presented as current is a lie.
    pub observed_at: DateTime<Utc>,
}

/// One line of `claude-usage.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUsageRecord {
    pub ts: DateTime<Utc>,
    /// The config dir whose status line reported this — baked into the script
    /// by the installer, because the payload never names it.
    pub config_dir: String,
    /// `oauthAccount.accountUuid` of that config dir when it is readable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<ClaudeUsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<ClaudeUsageWindow>,
}

/// What one append did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClaudeUsageAppendOutcome {
    pub written: bool,
    /// An earlier record for the same account is younger than the window.
    pub throttled: bool,
    pub truncated: bool,
    /// Another refresh held the sink. Nothing was written and nothing waited.
    pub contended: bool,
}

/// The status-line payload, reduced to what a status line and a record need.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatuslineInput {
    pub session_id: Option<String>,
    pub model_display: Option<String>,
    pub five_hour: Option<ClaudeUsageWindow>,
    pub seven_day: Option<ClaudeUsageWindow>,
}

impl StatuslineInput {
    /// Whether this refresh carried rate limits at all. It does not before the
    /// session's first API response, and never for a non-subscriber.
    pub fn has_usage(&self) -> bool {
        self.five_hour.is_some() || self.seven_day.is_some()
    }
}

#[derive(Debug, Deserialize)]
struct StatuslinePayload {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    model: Option<PayloadModel>,
    #[serde(default)]
    rate_limits: Option<PayloadRateLimits>,
}

#[derive(Debug, Deserialize)]
struct PayloadModel {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PayloadRateLimits {
    #[serde(default)]
    five_hour: Option<PayloadWindow>,
    #[serde(default)]
    seven_day: Option<PayloadWindow>,
}

#[derive(Debug, Deserialize)]
struct PayloadWindow {
    #[serde(default)]
    used_percentage: Option<f64>,
    #[serde(default)]
    resets_at: Option<i64>,
}

impl PayloadWindow {
    fn into_window(self) -> Option<ClaudeUsageWindow> {
        Some(ClaudeUsageWindow {
            used_percentage: self.used_percentage?,
            resets_at: self.resets_at,
        })
    }
}

/// Read one status-line payload.
pub fn parse_statusline_input(raw: &str) -> Result<StatuslineInput, String> {
    let payload: StatuslinePayload = serde_json::from_str(raw)
        .map_err(|error| format!("invalid Claude status line JSON: {error}"))?;
    let rate_limits = payload.rate_limits.unwrap_or(PayloadRateLimits {
        five_hour: None,
        seven_day: None,
    });
    Ok(StatuslineInput {
        session_id: non_empty(payload.session_id),
        model_display: payload
            .model
            .and_then(|model| non_empty(model.display_name).or_else(|| non_empty(model.id))),
        five_hour: rate_limits.five_hour.and_then(PayloadWindow::into_window),
        seven_day: rate_limits.seven_day.and_then(PayloadWindow::into_window),
    })
}

/// The status line taurhaus renders for an account that had none of its own.
///
/// Empty when the payload says nothing worth a line — verified on 2.1.246: a
/// status-line command that prints nothing leaves the row blank, it does not
/// fall back to a built-in line and it does not report an error.
pub fn render_status_line(input: &StatuslineInput) -> String {
    let mut parts = Vec::new();
    if let Some(model) = input.model_display.as_ref() {
        parts.push(model.clone());
    }
    if let Some(window) = input.five_hour.as_ref() {
        parts.push(format!("5h {}%", percent_label(window.used_percentage)));
    }
    if let Some(window) = input.seven_day.as_ref() {
        parts.push(format!("7d {}%", percent_label(window.used_percentage)));
    }
    parts.join(" · ")
}

fn percent_label(value: f64) -> String {
    format!("{}", value.round() as i64)
}

/// Append one usage record, unless an equally fresh one is already there.
///
/// The caller is a status-line subprocess, so its startup is the only place the
/// bounded file can be enforced — exactly like the Codex notify sink.
pub fn append_usage_at(
    path: &Path,
    record: &ClaudeUsageRecord,
) -> Result<ClaudeUsageAppendOutcome, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Claude usage path '{}' has no parent", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create Claude usage directory '{}': {error}",
            parent.display()
        )
    })?;

    let lock = open_sink_lock(path)?;
    // Never blocking. This runs from the status line of a live TUI, and a
    // refresh that waits is a terminal line that waits with it. Another
    // refresh holding the sink is either writing this same account's record —
    // which the throttle would have dropped anyway — or compacting, and the
    // next keystroke is a fraction of the 30 s throttle away.
    if lock.try_lock_exclusive().is_err() {
        return Ok(ClaudeUsageAppendOutcome {
            contended: true,
            ..ClaudeUsageAppendOutcome::default()
        });
    }

    let result = (|| {
        // Before the sink is opened, not after: compaction publishes a new file
        // by renaming it over this path, so a handle taken in front of it would
        // be a handle on the inode that rename retired — and every record
        // appended through it would go to a file nothing can reach.
        let truncated = compact_sink_if_needed(path)?;
        let mut file = open_sink(path)?;
        if is_throttled(&mut file, record)? {
            return Ok(ClaudeUsageAppendOutcome {
                written: false,
                throttled: true,
                truncated,
                contended: false,
            });
        }
        serde_json::to_writer(&mut file, record).map_err(|error| {
            format!(
                "failed to serialize Claude usage record '{}': {error}",
                path.display()
            )
        })?;
        file.write_all(b"\n").map_err(|error| {
            format!(
                "failed to append Claude usage record '{}': {error}",
                path.display()
            )
        })?;
        file.flush().map_err(|error| {
            format!(
                "failed to flush Claude usage sink '{}': {error}",
                path.display()
            )
        })?;
        Ok(ClaudeUsageAppendOutcome {
            written: true,
            throttled: false,
            truncated,
            contended: false,
        })
    })();

    let _ = FileExt::unlock(&lock);
    result
}

/// The sink, opened for appending and readable only by its owner.
fn open_sink(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(|error| {
        format!(
            "failed to open Claude usage sink '{}': {error}",
            path.display()
        )
    })?;
    #[cfg(unix)]
    fs::set_permissions(path, {
        use std::os::unix::fs::PermissionsExt;
        fs::Permissions::from_mode(0o600)
    })
    .map_err(|error| {
        format!(
            "failed to secure Claude usage sink '{}': {error}",
            path.display()
        )
    })?;
    Ok(file)
}

/// The file every writer and reader of the sink coordinates on.
///
/// A sidecar, deliberately, and one that is only ever created — never renamed,
/// never replaced. Compaction publishes a *new* sink by renaming one over the
/// old, and a lock held on the file that rename retires is a lock nobody else
/// can see: the next writer would take the same path's fresh inode, be told it
/// holds the sink, and append beside a compaction still in flight. One file that
/// outlives every compaction is what makes the lock mean the same thing to
/// everyone.
fn open_sink_lock(path: &Path) -> Result<File, String> {
    let lock_path = sink_lock_path(path);
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&lock_path).map_err(|error| {
        format!(
            "failed to open Claude usage lock '{}': {error}",
            lock_path.display()
        )
    })
}

fn sink_lock_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new(CLAUDE_USAGE_FILENAME))
        .to_os_string();
    name.push(".lock");
    path.with_file_name(name)
}

/// Whether this account already reported inside the throttle window.
fn is_throttled(file: &mut File, record: &ClaudeUsageRecord) -> Result<bool, String> {
    let key = config_dir_key(Path::new(&record.config_dir));
    let Some(previous) = read_tail_records(file)?
        .into_iter()
        .filter(|candidate| config_dir_key(Path::new(&candidate.config_dir)) == key)
        .max_by_key(|candidate| candidate.ts)
    else {
        return Ok(false);
    };
    let elapsed = record.ts - previous.ts;
    // A clock that jumped backwards must not silence an account forever: only
    // an earlier record inside the window throttles.
    Ok(elapsed >= ChronoDuration::zero() && elapsed < ChronoDuration::seconds(THROTTLE_SECONDS))
}

/// The records in the last `TAIL_SCAN_BYTES` of an open sink.
fn read_tail_records(file: &mut File) -> Result<Vec<ClaudeUsageRecord>, String> {
    let len = file
        .metadata()
        .map_err(|error| format!("failed to stat Claude usage sink: {error}"))?
        .len();
    let start = len.saturating_sub(TAIL_SCAN_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|error| format!("failed to seek Claude usage sink: {error}"))?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .map_err(|error| format!("failed to read Claude usage sink: {error}"))?;
    file.seek(SeekFrom::End(0))
        .map_err(|error| format!("failed to rewind Claude usage sink: {error}"))?;
    Ok(parse_records(&contents))
}

fn parse_records(contents: &[u8]) -> Vec<ClaudeUsageRecord> {
    contents
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<ClaudeUsageRecord>(line).ok())
        .collect()
}

fn compact_sink_if_needed(path: &Path) -> Result<bool, String> {
    compact_sink_if_needed_with(path, &|| Ok(()))
}

/// Cap the sink, keeping the newest record of every account it names.
///
/// The compacted file is built beside the live one and renamed over it, never
/// written into it. The process doing this is a status-line subprocess Claude
/// Code kills after two seconds, so "truncate, then write it back" has a window
/// in which the live sink holds a prefix of the accounts — and a read landing
/// after that window sees a file it can lock and parse perfectly well, and
/// reports every account missing from the prefix as never having reported at
/// all. A rename has no such window: the sink is either the whole old file or
/// the whole new one, and a compaction that dies leaves the old one exactly as
/// it was.
///
/// The caller holds the sidecar lock, which is what makes the rename safe for
/// the writers as well: nobody else is holding a handle on the inode it retires.
///
/// `before_publish` runs in the moment before that rename — the only place a
/// test can stand to be the failure. Production passes a no-op.
fn compact_sink_if_needed_with(
    path: &Path,
    before_publish: &dyn Fn() -> Result<(), String>,
) -> Result<bool, String> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(false);
    };
    if metadata.len() < MAX_CLAUDE_USAGE_BYTES {
        return Ok(false);
    }

    let contents = fs::read(path).map_err(|error| {
        format!(
            "failed to read Claude usage sink '{}': {error}",
            path.display()
        )
    })?;
    let mut retained = latest_per_account(parse_records(&contents))
        .into_values()
        .collect::<Vec<_>>();
    retained.sort_by_key(|record| record.ts);
    let mut compacted = Vec::new();
    for record in retained {
        serde_json::to_writer(&mut compacted, &record).map_err(|error| {
            format!(
                "failed to retain Claude usage record '{}': {error}",
                path.display()
            )
        })?;
        compacted.push(b'\n');
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("Claude usage path '{}' has no parent", path.display()))?;
    let temp_path = parent.join(format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(CLAUDE_USAGE_FILENAME),
        std::process::id()
    ));
    let published = (|| {
        let mut options = OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp_path)
            .map_err(|error| format!("failed to write '{}': {error}", temp_path.display()))?;
        file.write_all(&compacted)
            .map_err(|error| format!("failed to write '{}': {error}", temp_path.display()))?;
        file.flush()
            .map_err(|error| format!("failed to flush '{}': {error}", temp_path.display()))?;
        before_publish()?;
        fs::rename(&temp_path, path)
            .map_err(|error| format!("failed to replace '{}': {error}", path.display()))
    })();
    if let Err(error) = published {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    Ok(true)
}

fn latest_per_account(records: Vec<ClaudeUsageRecord>) -> HashMap<PathBuf, ClaudeUsageRecord> {
    let mut latest: HashMap<PathBuf, ClaudeUsageRecord> = HashMap::new();
    for record in records {
        let key = config_dir_key(Path::new(&record.config_dir));
        match latest.get(&key) {
            Some(existing) if existing.ts >= record.ts => {}
            _ => {
                latest.insert(key, record);
            }
        }
    }
    latest
}

/// The newest record per account in the sink, when the sink can be vouched for.
///
/// The whole file, not the tail the write path uses: one busy subscription
/// produces records for hours, and the quiet account whose last observation
/// they pushed out of view is exactly the one the user is deciding about. The
/// file is capped at 5 MB, so "the whole file" is bounded, and this runs on the
/// read side — once per `list_claude_accounts`, not once per keystroke.
///
/// `None` is "unknown", and it is not the same answer as an empty map. The
/// writer that holds the sink's lock is appending to it or compacting it, and a
/// read that goes ahead regardless can see the file mid-change and report the
/// accounts it missed as having no usage, which is indistinguishable from a
/// subscription that has never run. A missing sink, on the other hand, *is* an
/// answer: nothing has reported yet.
pub fn latest_usage_records(path: &Path) -> Option<HashMap<PathBuf, ClaudeUsageRecord>> {
    if File::open(path).is_err() {
        return Some(HashMap::new());
    }
    let lock = match open_sink_lock(path) {
        Ok(lock) => lock,
        Err(error) => {
            tracing::debug!(path = %path.display(), error, "Claude usage lock unavailable");
            return None;
        }
    };
    if !wait_for_shared_lock(&lock) {
        tracing::debug!(
            path = %path.display(),
            "Claude usage sink stayed locked; leaving the numbers as they are"
        );
        return None;
    }
    // Opened under the lock, because compaction renames a new sink over this
    // path: a handle taken before the lock could be one on the retired inode.
    let records = (|| -> Result<Vec<ClaudeUsageRecord>, String> {
        let mut file = File::open(path)
            .map_err(|error| format!("failed to open Claude usage sink: {error}"))?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .map_err(|error| format!("failed to read Claude usage sink: {error}"))?;
        Ok(parse_records(&contents))
    })();
    let _ = FileExt::unlock(&lock);
    match records {
        Ok(records) => Some(latest_per_account(records)),
        Err(error) => {
            tracing::debug!(path = %path.display(), error, "Claude usage sink unreadable");
            None
        }
    }
}

/// Wait a bounded moment for a shared lock. `false` means it never came.
///
/// A wedged writer must not stall an IPC command, and it does not get to be
/// answered for either: the caller reports "unknown" instead of reading a file
/// somebody else is in the middle of rewriting.
fn wait_for_shared_lock(file: &File) -> bool {
    let deadline = std::time::Instant::now() + READ_LOCK_WAIT;
    loop {
        if file.try_lock_shared().is_ok() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(READ_LOCK_POLL);
    }
}

/// Hang each account's latest usage off the account it belongs to.
///
/// Accounts are matched on the config dir the record names, and on the account
/// id when a dir has moved. An account with no record keeps `usage: None` —
/// nothing has reported for it yet, which is not the same as zero usage. A sink
/// that could not be read changes nothing at all: whatever the caller already
/// knew about these accounts is better than a number a busy file half-told us.
pub fn attach_usage_from(accounts: &mut [ClaudeAccount], path: &Path) {
    if accounts.is_empty() {
        return;
    }
    let Some(records) = latest_usage_records(path) else {
        return;
    };
    if records.is_empty() {
        return;
    }
    let by_account_id = newest_by_account_id(records.values());

    for account in accounts.iter_mut() {
        let record = records
            .get(&config_dir_key(&account.config_dir))
            .or_else(|| by_account_id.get(account.id.as_str()).copied());
        account.usage = record.map(|record| ClaudeAccountUsage {
            five_hour: record.five_hour,
            seven_day: record.seven_day,
            observed_at: record.ts,
        });
    }
}

/// The newest observation each account id has, whichever config dir reported it.
///
/// One account can hold records under several paths — `CLAUDE_CONFIG_DIR` moved,
/// a dir renamed — and the records above are already the newest *per path*, so
/// collecting them keeps whatever the iteration order happened to end on. That
/// order is a hash order, which makes the observation a moved account is shown
/// arbitrary rather than current. The timestamp decides instead.
fn newest_by_account_id<'a>(
    records: impl IntoIterator<Item = &'a ClaudeUsageRecord>,
) -> HashMap<&'a str, &'a ClaudeUsageRecord> {
    let mut newest: HashMap<&'a str, &'a ClaudeUsageRecord> = HashMap::new();
    for record in records {
        let Some(account_id) = record.account_id.as_deref() else {
            continue;
        };
        newest
            .entry(account_id)
            .and_modify(|known| {
                if known.ts < record.ts {
                    *known = record;
                }
            })
            .or_insert(record);
    }
    newest
}

/// Hang usage off accounts using this host's sink.
pub fn attach_usage(accounts: &mut [ClaudeAccount]) {
    attach_usage_from(
        accounts,
        &crate::provider::platform_paths::PlatformPaths::claude_usage_path(),
    );
}

/// The account id a config dir names, read the same way detection reads it.
pub fn account_id_for_config_dir(config_dir: &Path) -> Option<String> {
    let raw = fs::read_to_string(config_dir.join(".claude.json")).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    parsed
        .get("oauthAccount")?
        .get("accountUuid")?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// What the `claude-usage-sink` subcommand was told to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageSinkArgs {
    pub config_dir: PathBuf,
    /// Where to append. Baked in by the installer, because the status line runs
    /// in the user's own shell and knows nothing of `TAURHAUS_DATA_DIR`; absent
    /// only for a script written before this argument existed.
    pub sink: Option<PathBuf>,
    /// Render the status line as well: this account had none of its own.
    pub render: bool,
}

pub fn parse_usage_sink_args<I>(args: I) -> Result<UsageSinkArgs, String>
where
    I: IntoIterator<Item = String>,
{
    let mut config_dir = None;
    let mut sink = None;
    let mut render = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config-dir" => {
                config_dir =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        "--config-dir needs a directory".to_string()
                    })?));
            }
            "--sink" => {
                sink = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--sink needs a path".to_string())?,
                ));
            }
            "--render" => render = true,
            other => return Err(format!("unknown claude-usage-sink argument '{other}'")),
        }
    }
    Ok(UsageSinkArgs {
        config_dir: config_dir.ok_or_else(|| "--config-dir is required".to_string())?,
        sink,
        render,
    })
}

/// Run the sink over one status-line refresh.
///
/// Stdout is the user's status line, so it carries the rendered line and
/// nothing else — every diagnostic goes to the log. A payload this build cannot
/// read is not an error the user should see in their terminal either: the
/// record is skipped and the line stays blank.
pub fn run_usage_sink<R: Read, W: Write>(
    args: &UsageSinkArgs,
    mut stdin: R,
    mut stdout: W,
    default_sink_path: &Path,
    ts: DateTime<Utc>,
) -> Result<ClaudeUsageAppendOutcome, String> {
    let mut raw = String::new();
    stdin
        .read_to_string(&mut raw)
        .map_err(|error| format!("failed to read the Claude status line payload: {error}"))?;

    let input = parse_statusline_input(&raw)?;
    if args.render {
        let line = render_status_line(&input);
        if !line.is_empty() {
            writeln!(stdout, "{line}")
                .map_err(|error| format!("failed to write the Claude status line: {error}"))?;
        }
    }
    if !input.has_usage() {
        return Ok(ClaudeUsageAppendOutcome::default());
    }

    let record = ClaudeUsageRecord {
        ts,
        config_dir: args.config_dir.display().to_string(),
        account_id: account_id_for_config_dir(&args.config_dir),
        session_id: input.session_id,
        five_hour: input.five_hour,
        seven_day: input.seven_day,
    };
    append_usage_at(args.sink.as_deref().unwrap_or(default_sink_path), &record)
}

fn config_dir_key(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    const OBSERVED_PAYLOAD: &str = include_str!("fixtures/claude-statusline-2.1.246.json");
    const OBSERVED_PAYLOAD_BEFORE_FIRST_RESPONSE: &str =
        include_str!("fixtures/claude-statusline-no-rate-limits-2.1.246.json");

    fn ts(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_787_780_000 + seconds, 0).unwrap()
    }

    fn record(config_dir: &Path, seconds: i64, five: f64) -> ClaudeUsageRecord {
        ClaudeUsageRecord {
            ts: ts(seconds),
            config_dir: config_dir.display().to_string(),
            account_id: Some("account-1".to_string()),
            session_id: Some("session-1".to_string()),
            five_hour: Some(ClaudeUsageWindow {
                used_percentage: five,
                resets_at: Some(1_787_784_600),
            }),
            seven_day: Some(ClaudeUsageWindow {
                used_percentage: 17.0,
                resets_at: Some(1_788_300_000),
            }),
        }
    }

    #[test]
    fn reads_the_rate_limits_of_a_real_2_1_246_status_line_refresh() {
        let input = parse_statusline_input(OBSERVED_PAYLOAD).expect("payload parses");

        assert_eq!(
            input.five_hour,
            Some(ClaudeUsageWindow {
                used_percentage: 26.0,
                resets_at: Some(1_787_784_600),
            })
        );
        assert_eq!(
            input.seven_day,
            Some(ClaudeUsageWindow {
                used_percentage: 17.0,
                resets_at: Some(1_788_300_000),
            })
        );
        assert_eq!(
            input.session_id.as_deref(),
            Some("c530b681-421d-4de6-9a75-b106fd5be75d")
        );
        assert_eq!(input.model_display.as_deref(), Some("Haiku 4.5"));
        assert!(input.has_usage());
    }

    #[test]
    fn a_refresh_before_the_first_api_response_carries_no_usage() {
        // Regression: d6839a3 knew nothing of usage, and the obvious sink
        // treats every refresh as a record. Claude Code sends `rate_limits`
        // only after a session's first API response (verified live on 2.1.246,
        // where the first two refreshes of a fresh session sent `null`), so a
        // sink that records unconditionally writes empty rows for every boot.
        let input =
            parse_statusline_input(OBSERVED_PAYLOAD_BEFORE_FIRST_RESPONSE).expect("payload parses");

        assert_eq!(input.five_hour, None);
        assert_eq!(input.seven_day, None);
        assert!(!input.has_usage());
        assert_eq!(input.model_display.as_deref(), Some("Haiku 4.5"));

        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let outcome = run_usage_sink(
            &UsageSinkArgs {
                config_dir: temp.path().to_path_buf(),
                sink: None,
                render: true,
            },
            OBSERVED_PAYLOAD_BEFORE_FIRST_RESPONSE.as_bytes(),
            &mut Vec::new(),
            &sink,
            ts(0),
        )
        .expect("sink runs");

        assert_eq!(outcome, ClaudeUsageAppendOutcome::default());
        assert!(!sink.exists());
    }

    #[test]
    fn renders_a_minimal_line_for_an_account_that_had_none() {
        let input = parse_statusline_input(OBSERVED_PAYLOAD).expect("payload parses");
        assert_eq!(render_status_line(&input), "Haiku 4.5 · 5h 26% · 7d 17%");

        let booting =
            parse_statusline_input(OBSERVED_PAYLOAD_BEFORE_FIRST_RESPONSE).expect("payload parses");
        assert_eq!(render_status_line(&booting), "Haiku 4.5");

        assert_eq!(render_status_line(&StatuslineInput::default()), "");
    }

    #[test]
    fn one_account_records_at_most_once_per_throttle_window() {
        // Regression: d6839a3 had no sink at all. Claude Code re-runs the
        // status-line command on every refresh — per keystroke in a live TUI —
        // so an unthrottled sink writes thousands of identical rows a minute
        // and reaches the 5 MB cap in an afternoon.
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let config_dir = temp.path().join(".claude");
        fs::create_dir_all(&config_dir).expect("config dir");

        let first = append_usage_at(&sink, &record(&config_dir, 0, 26.0)).expect("first append");
        let inside = append_usage_at(&sink, &record(&config_dir, 5, 27.0)).expect("second append");
        let after = append_usage_at(&sink, &record(&config_dir, 31, 28.0)).expect("third append");

        assert!(first.written && !first.throttled);
        assert!(!inside.written && inside.throttled);
        assert!(after.written && !after.throttled);

        let written = fs::read_to_string(&sink).expect("sink readable");
        assert_eq!(written.lines().count(), 2);
    }

    #[test]
    fn a_second_account_is_never_throttled_by_the_first() {
        // Regression: d6839a3 had no sink; a throttle keyed on the file rather
        // than on the account silences every other subscription for 30 s, and
        // the account the user is about to switch to is exactly the one whose
        // number they need.
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let first_dir = temp.path().join(".claude");
        let second_dir = temp.path().join(".claude-account2");
        fs::create_dir_all(&first_dir).expect("first dir");
        fs::create_dir_all(&second_dir).expect("second dir");

        append_usage_at(&sink, &record(&first_dir, 0, 26.0)).expect("first append");
        let other =
            append_usage_at(&sink, &record(&second_dir, 1, 4.0)).expect("second account append");

        assert!(other.written && !other.throttled);
        assert_eq!(
            fs::read_to_string(&sink)
                .expect("sink readable")
                .lines()
                .count(),
            2
        );
    }

    #[test]
    fn the_sink_is_capped_and_keeps_the_latest_record_per_account() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let first_dir = temp.path().join(".claude");
        let second_dir = temp.path().join(".claude-account2");
        fs::create_dir_all(&first_dir).expect("first dir");
        fs::create_dir_all(&second_dir).expect("second dir");

        {
            let mut file = File::create(&sink).expect("sink");
            let mut seconds = 0;
            while file.metadata().expect("stat").len() < MAX_CLAUDE_USAGE_BYTES {
                for _ in 0..500 {
                    let line = serde_json::to_string(&record(&first_dir, seconds, 10.0))
                        .expect("serialize");
                    writeln!(file, "{line}").expect("write");
                    seconds += 60;
                }
            }
            writeln!(
                file,
                "{}",
                serde_json::to_string(&record(&second_dir, seconds, 55.0)).expect("serialize")
            )
            .expect("write");
        }

        let outcome = append_usage_at(&sink, &record(&first_dir, 10_000_000, 99.0))
            .expect("append after the cap");
        assert!(outcome.truncated && outcome.written);
        assert!(fs::metadata(&sink).expect("stat").len() < MAX_CLAUDE_USAGE_BYTES);

        let latest = latest_usage_records(&sink).expect("the sink can be read");
        assert_eq!(latest.len(), 2);
        assert_eq!(
            latest
                .get(&config_dir_key(&first_dir))
                .and_then(|record| record.five_hour)
                .map(|window| window.used_percentage),
            Some(99.0)
        );
        assert_eq!(
            latest
                .get(&config_dir_key(&second_dir))
                .and_then(|record| record.five_hour)
                .map(|window| window.used_percentage),
            Some(55.0)
        );
    }

    #[test]
    fn a_compaction_that_fails_before_it_publishes_keeps_every_accounts_last_number() {
        // Regression: c1643dc compacted the sink in place — `set_len(0)` first,
        // the records worth keeping written back after. The process doing that
        // is a status-line subprocess Claude Code kills after two seconds, so an
        // interrupted compaction leaves the live file holding a prefix of the
        // accounts, or none of them. `latest_usage_records` then reads a file it
        // can lock and parse perfectly well, and reports every account missing
        // from that prefix as never having reported at all — so the last
        // observation of a quiet subscription, which is exactly the one the user
        // is deciding about, is gone for good.
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let quiet_dir = temp.path().join(".claude-account2");
        let busy_dir = temp.path().join(".claude");
        for dir in [&quiet_dir, &busy_dir] {
            fs::create_dir_all(dir).expect("config dir");
        }
        {
            let mut file = File::create(&sink).expect("sink");
            let mut quiet = record(&quiet_dir, 0, 55.0);
            quiet.account_id = Some("account-2".to_string());
            writeln!(
                file,
                "{}",
                serde_json::to_string(&quiet).expect("serialize")
            )
            .expect("write");
            let mut seconds = 60;
            while file.metadata().expect("stat").len() < MAX_CLAUDE_USAGE_BYTES {
                for _ in 0..500 {
                    let line = serde_json::to_string(&record(&busy_dir, seconds, 10.0))
                        .expect("serialize");
                    writeln!(file, "{line}").expect("write");
                    seconds += 60;
                }
            }
        }

        let error =
            compact_sink_if_needed_with(&sink, &|| Err("the status line's deadline".to_string()))
                .expect_err("the injected failure is the compaction's answer");

        assert!(error.contains("deadline"), "{error}");
        let latest = latest_usage_records(&sink).expect("the sink can be read");
        assert_eq!(
            latest.len(),
            2,
            "a compaction that never finished emptied the live sink"
        );
        assert_eq!(
            latest
                .get(&config_dir_key(&quiet_dir))
                .and_then(|record| record.five_hour)
                .map(|window| window.used_percentage),
            Some(55.0),
            "the quiet account's only observation was lost"
        );
        let leftovers = fs::read_dir(temp.path())
            .expect("read the sink's directory")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".tmp."))
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "the failed compaction left {leftovers:?} behind"
        );

        // And the next append compacts for real, now that nothing is failing.
        let outcome = append_usage_at(&sink, &record(&busy_dir, 10_000_000, 99.0)).expect("append");
        assert!(outcome.truncated && outcome.written);
        assert!(fs::metadata(&sink).expect("stat").len() < MAX_CLAUDE_USAGE_BYTES);
        assert_eq!(
            latest_usage_records(&sink)
                .expect("the compacted sink can be read")
                .len(),
            2
        );
    }

    #[test]
    fn accounts_carry_the_latest_usage_of_their_own_config_dir() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let first_dir = temp.path().join(".claude");
        let second_dir = temp.path().join(".claude-account2");
        let silent_dir = temp.path().join(".claude-work");
        for dir in [&first_dir, &second_dir, &silent_dir] {
            fs::create_dir_all(dir).expect("config dir");
        }

        append_usage_at(&sink, &record(&first_dir, 0, 26.0)).expect("append");
        append_usage_at(&sink, &record(&first_dir, 120, 41.0)).expect("append");
        let mut second = record(&second_dir, 60, 4.0);
        second.account_id = Some("account-2".to_string());
        append_usage_at(&sink, &second).expect("append");

        let mut accounts = vec![
            account("account-1", &first_dir),
            account("account-2", &second_dir),
            account("account-3", &silent_dir),
        ];
        attach_usage_from(&mut accounts, &sink);

        assert_eq!(
            accounts[0]
                .usage
                .as_ref()
                .and_then(|usage| usage.five_hour)
                .map(|window| window.used_percentage),
            Some(41.0)
        );
        assert_eq!(
            accounts[0].usage.as_ref().map(|usage| usage.observed_at),
            Some(ts(120))
        );
        assert_eq!(
            accounts[1]
                .usage
                .as_ref()
                .and_then(|usage| usage.seven_day)
                .map(|window| window.used_percentage),
            Some(17.0)
        );
        // Regression: d6839a3 shipped accounts with no usage. An account that
        // has never run a session under taurhaus has no record, and reporting
        // that as 0 % would send the user to the subscription with the least
        // headroom.
        assert_eq!(accounts[2].usage, None);
    }

    #[test]
    fn a_moved_account_carries_the_newest_record_any_of_its_dirs_reported() {
        // Regression: 4950d00 built the account-id fallback index by collecting
        // every per-config-dir record into a `HashMap`, which keeps whichever
        // one the (unordered) iteration inserted last rather than the newest.
        // An account whose config dir has moved has records under each former
        // path, so what the moved account was shown came down to hash order: an
        // hours-old 26 % as readily as the observation from minutes ago. The
        // index has to compare timestamps.
        let temp = tempfile::tempdir().expect("temp dir");
        let former = temp.path().join(".claude");
        let also_former = temp.path().join(".claude-old");
        let moved_to = temp.path().join(".claude-moved");
        for dir in [&former, &also_former, &moved_to] {
            fs::create_dir_all(dir).expect("config dir");
        }
        let stale = record(&former, 0, 26.0);
        let newest = record(&also_former, 600, 61.0);

        // The order records arrive in is the whole bug, so both are asked for.
        for order in [[&stale, &newest], [&newest, &stale]] {
            assert_eq!(
                newest_by_account_id(order)
                    .get("account-1")
                    .map(|record| record.ts),
                Some(ts(600)),
                "the index kept whichever record it happened to read last"
            );
        }

        // And end to end, through a sink neither record's dir is the account's
        // any more: the fallback is the only thing that can answer for it.
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        append_usage_at(&sink, &stale).expect("append");
        append_usage_at(&sink, &newest).expect("append");
        let mut accounts = vec![account("account-1", &moved_to)];

        attach_usage_from(&mut accounts, &sink);

        assert_eq!(
            accounts[0].usage.as_ref().map(|usage| usage.observed_at),
            Some(ts(600)),
            "a moved account was handed an observation older than the one it has"
        );
        assert_eq!(
            accounts[0]
                .usage
                .as_ref()
                .and_then(|usage| usage.five_hour)
                .map(|window| window.used_percentage),
            Some(61.0)
        );
    }

    #[test]
    fn the_sink_renders_the_line_and_records_the_refresh_in_one_pass() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config_dir = temp.path().join(".claude");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(
            config_dir.join(".claude.json"),
            r#"{"oauthAccount":{"accountUuid":"uuid-1","emailAddress":"user@example.com"}}"#,
        )
        .expect("config file");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let mut stdout = Vec::new();

        let outcome = run_usage_sink(
            &UsageSinkArgs {
                config_dir: config_dir.clone(),
                sink: Some(sink.clone()),
                render: true,
            },
            OBSERVED_PAYLOAD.as_bytes(),
            &mut stdout,
            &sink,
            ts(0),
        )
        .expect("sink runs");

        assert!(outcome.written);
        assert_eq!(
            String::from_utf8(stdout).expect("utf8"),
            "Haiku 4.5 · 5h 26% · 7d 17%\n"
        );
        let written: ClaudeUsageRecord =
            serde_json::from_str(fs::read_to_string(&sink).expect("sink").trim())
                .expect("record parses");
        assert_eq!(written.account_id.as_deref(), Some("uuid-1"));
        assert_eq!(
            written.session_id.as_deref(),
            Some("c530b681-421d-4de6-9a75-b106fd5be75d")
        );
        assert_eq!(written.config_dir, config_dir.display().to_string());
    }

    #[test]
    fn a_wrapped_status_line_gets_no_output_of_ours() {
        // Regression: d6839a3 had no bridge. Our script pipes the payload to
        // the command the user configured, so anything the sink prints would
        // land on the same line and corrupt theirs.
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let mut stdout = Vec::new();

        run_usage_sink(
            &UsageSinkArgs {
                config_dir: temp.path().to_path_buf(),
                sink: None,
                render: false,
            },
            OBSERVED_PAYLOAD.as_bytes(),
            &mut stdout,
            &sink,
            ts(0),
        )
        .expect("sink runs");

        assert!(stdout.is_empty());
        assert!(sink.exists());
    }

    #[test]
    fn the_sink_arguments_are_the_ones_the_installer_bakes_in() {
        assert_eq!(
            parse_usage_sink_args(
                [
                    "--config-dir",
                    "/home/user/.claude",
                    "--sink",
                    "/isolated/claude-usage.jsonl",
                    "--render",
                ]
                .map(str::to_string)
            )
            .expect("args parse"),
            UsageSinkArgs {
                config_dir: PathBuf::from("/home/user/.claude"),
                sink: Some(PathBuf::from("/isolated/claude-usage.jsonl")),
                render: true,
            }
        );
        assert!(parse_usage_sink_args(Vec::new()).is_err());
        assert!(parse_usage_sink_args(["--config-dir".to_string()]).is_err());
    }

    #[test]
    fn a_locked_sink_never_holds_up_the_status_line() {
        // Regression: 79be608 took a blocking exclusive lock, and took it
        // before it knew whether the refresh was throttled at all. The status
        // line runs this on every keystroke, so a sink already held by another
        // refresh — or compacting five megabytes — stalled the terminal line
        // the user is looking at. A dropped record costs nothing: the next
        // refresh is a keystroke away. The lock is the sidecar now; what may not
        // change is that the write path never waits on it.
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let config_dir = temp.path().join(".claude");
        fs::create_dir_all(&config_dir).expect("config dir");
        let holder = open_sink_lock(&sink).expect("holder");
        holder.lock_exclusive().expect("hold the sink");

        let started = std::time::Instant::now();
        let outcome = append_usage_at(&sink, &record(&config_dir, 0, 26.0)).expect("append");

        assert!(
            started.elapsed() < std::time::Duration::from_millis(500),
            "the sink waited {:?} for a lock the status line cannot wait for",
            started.elapsed()
        );
        assert_eq!(
            outcome,
            ClaudeUsageAppendOutcome {
                written: false,
                throttled: false,
                truncated: false,
                contended: true,
            }
        );
        FileExt::unlock(&holder).expect("release");
    }

    #[test]
    fn an_account_that_stopped_reporting_keeps_its_last_number() {
        // Regression: 79be608 answered the read side from the last 256 KiB of
        // the sink. One busy subscription pushes a quiet one's last record out
        // of that window in an afternoon, and the quiet account's chip then
        // showed no usage at all — while its record was still in the file, and
        // while "last seen 6h ago" was exactly the answer the user needed.
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let quiet_dir = temp.path().join(".claude-account2");
        let busy_dir = temp.path().join(".claude");
        for dir in [&quiet_dir, &busy_dir] {
            fs::create_dir_all(dir).expect("config dir");
        }

        {
            let mut file = File::create(&sink).expect("sink");
            let mut quiet = record(&quiet_dir, 0, 55.0);
            quiet.account_id = Some("account-2".to_string());
            writeln!(
                file,
                "{}",
                serde_json::to_string(&quiet).expect("serialize")
            )
            .expect("write");
            let mut seconds = 60;
            while file.metadata().expect("stat").len() < TAIL_SCAN_BYTES + 8 * 1024 {
                writeln!(
                    file,
                    "{}",
                    serde_json::to_string(&record(&busy_dir, seconds, 10.0)).expect("serialize")
                )
                .expect("write");
                seconds += 60;
            }
        }
        // Still far below the cap, so nothing has compacted it away either.
        assert!(fs::metadata(&sink).expect("stat").len() < MAX_CLAUDE_USAGE_BYTES);

        let latest = latest_usage_records(&sink).expect("the sink can be read");

        assert_eq!(latest.len(), 2);
        assert_eq!(
            latest
                .get(&config_dir_key(&quiet_dir))
                .and_then(|record| record.five_hour)
                .map(|window| window.used_percentage),
            Some(55.0)
        );
    }

    #[test]
    fn a_read_during_compaction_waits_instead_of_seeing_half_a_file() {
        // Regression: 79be608 read the sink with no lock at all while the
        // writer truncated and rewrote it in place. A `list_claude_accounts`
        // that landed inside that window answered with healthy accounts and no
        // usage — indistinguishable from "nothing has ever reported".
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let first_dir = temp.path().join(".claude");
        let second_dir = temp.path().join(".claude-account2");
        for dir in [&first_dir, &second_dir] {
            fs::create_dir_all(dir).expect("config dir");
        }
        append_usage_at(&sink, &record(&first_dir, 0, 26.0)).expect("append");
        let mut second = record(&second_dir, 60, 4.0);
        second.account_id = Some("account-2".to_string());
        append_usage_at(&sink, &second).expect("append");
        let original = fs::read(&sink).expect("sink readable");

        let compacting = {
            let sink = sink.clone();
            std::thread::spawn(move || {
                let lock = open_sink_lock(&sink).expect("lock file");
                lock.lock_exclusive().expect("lock");
                let mut file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&sink)
                    .expect("open");
                file.set_len(0).expect("truncate");
                std::thread::sleep(std::time::Duration::from_millis(200));
                file.write_all(&original).expect("restore");
                file.flush().expect("flush");
                FileExt::unlock(&lock).expect("unlock");
            })
        };
        std::thread::sleep(std::time::Duration::from_millis(50));

        let latest = latest_usage_records(&sink).expect("the sink can be read");

        compacting.join().expect("compaction thread");
        assert_eq!(latest.len(), 2);
    }

    #[test]
    fn a_sink_held_past_the_wait_is_unknown_rather_than_half_read() {
        // Regression: a574720 waited half a second for a shared lock and then
        // read the file anyway. A read that gives up sees whatever the writer
        // holding that lock has put there so far: one account's record and not
        // the other's. Every account missing from that half then came back with
        // no usage at all, which is the answer the lock was added to stop.
        let temp = tempfile::tempdir().expect("temp dir");
        let sink = temp.path().join(CLAUDE_USAGE_FILENAME);
        let first_dir = temp.path().join(".claude");
        let second_dir = temp.path().join(".claude-account2");
        for dir in [&first_dir, &second_dir] {
            fs::create_dir_all(dir).expect("config dir");
        }
        append_usage_at(&sink, &record(&first_dir, 0, 26.0)).expect("append");
        let mut second = record(&second_dir, 60, 4.0);
        second.account_id = Some("account-2".to_string());
        append_usage_at(&sink, &second).expect("append");
        let whole = fs::read(&sink).expect("sink readable");
        let half = whole
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|end| whole[..=end].to_vec())
            .expect("two records");

        let (rewritten, wait_for_reader) = std::sync::mpsc::channel();
        let (reader_done, resume) = std::sync::mpsc::channel::<()>();
        let compacting = {
            let sink = sink.clone();
            let whole = whole.clone();
            std::thread::spawn(move || {
                let lock = open_sink_lock(&sink).expect("lock file");
                lock.lock_exclusive().expect("lock");
                let mut file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&sink)
                    .expect("open");
                file.set_len(0).expect("truncate");
                file.write_all(&half).expect("half a file");
                file.flush().expect("flush");
                rewritten.send(()).expect("announce");
                let _ = resume.recv();
                file.seek(SeekFrom::Start(0)).expect("rewind");
                file.write_all(&whole).expect("restore");
                file.flush().expect("flush");
                FileExt::unlock(&lock).expect("unlock");
            })
        };
        wait_for_reader.recv().expect("the writer got there first");

        assert_eq!(
            latest_usage_records(&sink),
            None,
            "a sink that stayed locked is unknown, not empty"
        );
        let mut accounts = vec![
            known(account("account-1", &first_dir), 55.0),
            known(account("account-2", &second_dir), 9.0),
        ];
        attach_usage_from(&mut accounts, &sink);

        reader_done.send(()).expect("release the writer");
        compacting.join().expect("compaction thread");
        assert_eq!(
            accounts
                .iter()
                .map(|account| account
                    .usage
                    .as_ref()
                    .and_then(|usage| usage.five_hour)
                    .map(|window| window.used_percentage))
                .collect::<Vec<_>>(),
            vec![Some(55.0), Some(9.0)],
            "a sink that could not be locked told us nothing, and nothing is what it may change"
        );
    }

    /// An account whose usage was read at some earlier, luckier moment.
    fn known(mut account: ClaudeAccount, five_hour: f64) -> ClaudeAccount {
        account.usage = Some(ClaudeAccountUsage {
            five_hour: Some(ClaudeUsageWindow {
                used_percentage: five_hour,
                resets_at: Some(1_787_784_600),
            }),
            seven_day: None,
            observed_at: ts(-600),
        });
        account
    }

    #[test]
    fn a_missing_sink_leaves_every_account_without_usage() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut accounts = vec![account("account-1", &temp.path().join(".claude"))];
        attach_usage_from(&mut accounts, &temp.path().join(CLAUDE_USAGE_FILENAME));
        assert_eq!(accounts[0].usage, None);
    }

    fn account(id: &str, config_dir: &Path) -> ClaudeAccount {
        ClaudeAccount {
            id: id.to_string(),
            config_dir: config_dir.to_path_buf(),
            email: format!("{id}@example.com"),
            display_name: None,
            organization: None,
            seat_tier: None,
            logged_in: true,
            is_default: false,
            is_process_default: false,
            usage: None,
        }
    }
}
