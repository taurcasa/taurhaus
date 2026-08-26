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
//! per keystroke. Two things keep that cheap — a throttle to one record per
//! account per 30 s, and a bounded tail read instead of a full parse.
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

use crate::session_scanner::claude_accounts::ClaudeAccount;

pub const CLAUDE_USAGE_FILENAME: &str = "claude-usage.jsonl";

/// Same cap as the Codex notify sink; enforced at process startup.
const MAX_CLAUDE_USAGE_BYTES: u64 = 5 * 1024 * 1024;

/// One record per account per window. Status lines refresh per keystroke; the
/// numbers behind them move in percent, not in milliseconds.
const THROTTLE_SECONDS: i64 = 30;

/// How much of the sink's tail one read looks at.
///
/// Records are ~200 bytes and throttled to two a minute per account, so this
/// covers many hours of every account on the host — while never growing with
/// the file. Nothing here needs the sink's history; only its last word per
/// account.
const TAIL_SCAN_BYTES: u64 = 256 * 1024;

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

    let mut options = OpenOptions::new();
    options.create(true).read(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
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
    file.lock_exclusive().map_err(|error| {
        format!(
            "failed to lock Claude usage sink '{}': {error}",
            path.display()
        )
    })?;

    let result = (|| {
        let truncated = compact_sink_if_needed(&mut file, path)?;
        if is_throttled(&mut file, record)? {
            return Ok(ClaudeUsageAppendOutcome {
                written: false,
                throttled: true,
                truncated,
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
        })
    })();

    let _ = FileExt::unlock(&file);
    result
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

fn compact_sink_if_needed(file: &mut File, path: &Path) -> Result<bool, String> {
    let len = file
        .metadata()
        .map_err(|error| {
            format!(
                "failed to stat Claude usage sink '{}': {error}",
                path.display()
            )
        })?
        .len();
    if len < MAX_CLAUDE_USAGE_BYTES {
        return Ok(false);
    }

    file.seek(SeekFrom::Start(0)).map_err(|error| {
        format!(
            "failed to seek Claude usage sink '{}': {error}",
            path.display()
        )
    })?;
    let mut contents = Vec::with_capacity(len.min(MAX_CLAUDE_USAGE_BYTES) as usize);
    file.read_to_end(&mut contents).map_err(|error| {
        format!(
            "failed to read Claude usage sink '{}': {error}",
            path.display()
        )
    })?;
    let mut retained = latest_per_account(parse_records(&contents))
        .into_values()
        .collect::<Vec<_>>();
    retained.sort_by_key(|record| record.ts);

    file.set_len(0).map_err(|error| {
        format!(
            "failed to cap Claude usage sink '{}': {error}",
            path.display()
        )
    })?;
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        format!(
            "failed to rewind Claude usage sink '{}': {error}",
            path.display()
        )
    })?;
    for record in retained {
        serde_json::to_writer(&mut *file, &record).map_err(|error| {
            format!(
                "failed to retain Claude usage record '{}': {error}",
                path.display()
            )
        })?;
        file.write_all(b"\n").map_err(|error| {
            format!(
                "failed to retain Claude usage record '{}': {error}",
                path.display()
            )
        })?;
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

/// The newest record per account in the sink's tail.
pub fn latest_usage_records(path: &Path) -> HashMap<PathBuf, ClaudeUsageRecord> {
    let Ok(mut file) = File::open(path) else {
        return HashMap::new();
    };
    match read_tail_records(&mut file) {
        Ok(records) => latest_per_account(records),
        Err(error) => {
            tracing::debug!(path = %path.display(), error, "Claude usage sink unreadable");
            HashMap::new()
        }
    }
}

/// Hang each account's latest usage off the account it belongs to.
///
/// Accounts are matched on the config dir the record names, and on the account
/// id when a dir has moved. An account with no record keeps `usage: None` —
/// nothing has reported for it yet, which is not the same as zero usage.
pub fn attach_usage_from(accounts: &mut [ClaudeAccount], path: &Path) {
    if accounts.is_empty() {
        return;
    }
    let records = latest_usage_records(path);
    if records.is_empty() {
        return;
    }
    let by_account_id: HashMap<&str, &ClaudeUsageRecord> = records
        .values()
        .filter_map(|record| Some((record.account_id.as_deref()?, record)))
        .collect();

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

        let latest = latest_usage_records(&sink);
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
