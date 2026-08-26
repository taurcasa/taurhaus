use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

pub const CODEX_NOTIFY_FILENAME: &str = "codex-notify.jsonl";
const MAX_CODEX_NOTIFY_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct CodexNotifyEvent {
    value: Value,
}

impl CodexNotifyEvent {
    pub fn event_type(&self) -> Option<&str> {
        self.value.get("type").and_then(Value::as_str)
    }

    pub fn session_id(&self) -> Option<&str> {
        self.value
            .get("session_id")
            .or_else(|| self.value.get("session-id"))
            .or_else(|| self.value.get("thread_id"))
            .or_else(|| self.value.get("thread-id"))
            .and_then(Value::as_str)
    }

    pub fn turn_id(&self) -> Option<&str> {
        self.value
            .get("turn_id")
            .or_else(|| self.value.get("turn-id"))
            .and_then(Value::as_str)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodexNotifyRecord {
    pub ts: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(deserialize_with = "deserialize_event_name")]
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexNotifyAppendOutcome {
    pub truncated: bool,
}

fn deserialize_event_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(event) => Ok(event),
        Value::Object(event) => event
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| serde::de::Error::custom("notify event object has no string type")),
        _ => Err(serde::de::Error::custom(
            "notify event must be a string or event object",
        )),
    }
}

#[derive(Debug)]
struct CachedNotifyRecords {
    len: u64,
    modified: SystemTime,
    records: HashMap<(String, String), CodexNotifyRecord>,
}

static RECORD_CACHE: LazyLock<Mutex<HashMap<PathBuf, CachedNotifyRecords>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(test)]
static RECORD_CACHE_PARSE_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

pub fn parse_event(raw: &str) -> Result<CodexNotifyEvent, serde_json::Error> {
    serde_json::from_str(raw).map(|value| CodexNotifyEvent { value })
}

/// Append a Codex notification as one JSONL record.
///
/// The notifier is a short-lived subprocess, so its startup is the natural
/// place to enforce the bounded append-only file. An exclusive file lock keeps
/// simultaneous turn completions from racing a cap truncation.
pub fn append_event_at(
    path: &Path,
    raw_event: &str,
    ts: DateTime<Utc>,
) -> Result<CodexNotifyAppendOutcome, String> {
    let parsed =
        parse_event(raw_event).map_err(|error| format!("invalid Codex notify JSON: {error}"))?;
    let record = CodexNotifyRecord {
        ts,
        session_id: parsed.session_id().map(str::to_string),
        event: parsed.event_type().unwrap_or("unknown").to_string(),
        turn_id: parsed.turn_id().map(str::to_string),
    };
    let parent = path
        .parent()
        .ok_or_else(|| format!("Codex notify path '{}' has no parent", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create Codex notify directory '{}': {error}",
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
            "failed to open Codex notify sink '{}': {error}",
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
            "failed to secure Codex notify sink '{}': {error}",
            path.display()
        )
    })?;
    file.lock_exclusive().map_err(|error| {
        format!(
            "failed to lock Codex notify sink '{}': {error}",
            path.display()
        )
    })?;

    let result = (|| {
        let truncated = compact_sink_if_needed(&mut file, path)?;
        serde_json::to_writer(&mut file, &record).map_err(|error| {
            format!(
                "failed to serialize Codex notify record '{}': {error}",
                path.display()
            )
        })?;
        file.write_all(b"\n").map_err(|error| {
            format!(
                "failed to append Codex notify record '{}': {error}",
                path.display()
            )
        })?;
        file.flush().map_err(|error| {
            format!(
                "failed to flush Codex notify sink '{}': {error}",
                path.display()
            )
        })?;
        Ok(CodexNotifyAppendOutcome { truncated })
    })();

    let _ = FileExt::unlock(&file);
    if result.is_ok() {
        invalidate_record_cache(path);
    }
    result
}

fn compact_sink_if_needed(file: &mut File, path: &Path) -> Result<bool, String> {
    let len = file
        .metadata()
        .map_err(|error| {
            format!(
                "failed to stat Codex notify sink '{}': {error}",
                path.display()
            )
        })?
        .len();
    if len < MAX_CODEX_NOTIFY_BYTES {
        return Ok(false);
    }

    file.seek(SeekFrom::Start(0)).map_err(|error| {
        format!(
            "failed to seek Codex notify sink '{}': {error}",
            path.display()
        )
    })?;
    let mut contents = Vec::with_capacity(len.min(MAX_CODEX_NOTIFY_BYTES) as usize);
    file.read_to_end(&mut contents).map_err(|error| {
        format!(
            "failed to read Codex notify sink '{}': {error}",
            path.display()
        )
    })?;
    let mut latest = HashMap::<(Option<String>, String), CodexNotifyRecord>::new();
    for line in contents.split(|byte| *byte == b'\n') {
        if let Ok(record) = serde_json::from_slice::<CodexNotifyRecord>(line) {
            latest.insert((record.session_id.clone(), record.event.clone()), record);
        }
    }
    let mut retained = latest.into_values().collect::<Vec<_>>();
    retained.sort_by_key(|record| record.ts);

    file.set_len(0).map_err(|error| {
        format!(
            "failed to cap Codex notify sink '{}': {error}",
            path.display()
        )
    })?;
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        format!(
            "failed to rewind Codex notify sink '{}': {error}",
            path.display()
        )
    })?;
    for record in retained {
        serde_json::to_writer(&mut *file, &record).map_err(|error| {
            format!(
                "failed to retain Codex notify record '{}': {error}",
                path.display()
            )
        })?;
        file.write_all(b"\n").map_err(|error| {
            format!(
                "failed to retain Codex notify record '{}': {error}",
                path.display()
            )
        })?;
    }
    Ok(true)
}

pub(crate) fn latest_record_for_session_after(
    path: &Path,
    session_id: &str,
    event: &str,
    not_before: SystemTime,
) -> Option<CodexNotifyRecord> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    if modified < not_before {
        return None;
    }
    let len = metadata.len();
    let key = (session_id.to_string(), event.to_string());
    if let Some(record) = cached_record(path, len, modified, &key) {
        return record;
    }

    let contents = fs::read(path).ok()?;
    #[cfg(test)]
    RECORD_CACHE_PARSE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut records = HashMap::new();
    for line in contents.split(|byte| *byte == b'\n') {
        let Ok(record) = serde_json::from_slice::<CodexNotifyRecord>(line) else {
            continue;
        };
        let Some(record_session_id) = record.session_id.as_ref() else {
            continue;
        };
        records.insert((record_session_id.clone(), record.event.clone()), record);
    }
    let result = records.get(&key).cloned();
    let mut cache = RECORD_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    cache.clear();
    cache.insert(
        path.to_path_buf(),
        CachedNotifyRecords {
            len,
            modified,
            records,
        },
    );
    result
}

fn cached_record(
    path: &Path,
    len: u64,
    modified: SystemTime,
    key: &(String, String),
) -> Option<Option<CodexNotifyRecord>> {
    let cache = RECORD_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let cached = cache.get(path)?;
    if cached.len != len || cached.modified != modified {
        return None;
    }
    Some(cached.records.get(key).cloned())
}

fn invalidate_record_cache(path: &Path) {
    RECORD_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(path);
}

#[cfg(test)]
fn clear_record_cache_for_test() {
    RECORD_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();
    RECORD_CACHE_PARSE_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
fn record_cache_parse_count_for_test() -> usize {
    RECORD_CACHE_PARSE_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::io::Write;

    static CACHE_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    const OBSERVED_CODEX_0_149_PAYLOAD: &str =
        include_str!("../session_scanner/idle/fixtures/codex-agent-turn-complete-0.149.0.json");

    // Regression: 791f6be centralized Codex launch rendering without a native
    // turn-complete sink, leaving managed sessions on delayed rchar hysteresis.
    #[test]
    fn observed_codex_0_149_payload_maps_thread_id() {
        let event = parse_event(OBSERVED_CODEX_0_149_PAYLOAD).expect("observed payload");

        assert_eq!(event.event_type(), Some("agent-turn-complete"));
        assert_eq!(
            event.session_id(),
            Some("01a03e54-7a7a-7fb3-85f5-24dfa739a2e1")
        );
    }

    #[test]
    fn sink_appends_one_enveloped_json_line() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        let now = Utc
            .with_ymd_and_hms(2026, 8, 26, 12, 0, 0)
            .single()
            .expect("timestamp");

        let outcome =
            append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, now).expect("append notify event");
        let lines = std::fs::read_to_string(&path).expect("notify sink");
        let records = lines.lines().collect::<Vec<_>>();

        assert!(!outcome.truncated);
        assert_eq!(records.len(), 1);
        let record: CodexNotifyRecord = serde_json::from_str(records[0]).expect("record");
        assert_eq!(record.ts, now);
        assert_eq!(
            record.session_id.as_deref(),
            Some("01a03e54-7a7a-7fb3-85f5-24dfa739a2e1")
        );
        assert_eq!(record.event, "agent-turn-complete");
        assert_eq!(
            record.turn_id.as_deref(),
            Some("01a03e54-7bbf-74b2-ac52-2cfc3b0688cc")
        );
    }

    #[test]
    fn sink_truncates_at_five_megabytes_before_append() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        let mut file = std::fs::File::create(&path).expect("oversized sink");
        file.set_len(MAX_CODEX_NOTIFY_BYTES + 1)
            .expect("extend fixture");
        file.flush().expect("flush fixture");

        let outcome = append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, Utc::now())
            .expect("append after cap");
        let contents = std::fs::read_to_string(&path).expect("capped sink");

        assert!(outcome.truncated);
        assert_eq!(contents.lines().count(), 1);
        assert!(contents.len() < 4096);
    }

    // Regression: 61e9a24 read and parsed up to 5 MB once per Codex process on
    // every scanner tick, making the authoritative edge a scan-hot-path cost.
    #[test]
    fn unchanged_notify_sink_is_parsed_only_once() {
        let _guard = CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_record_cache_for_test();
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, Utc::now())
            .expect("append notify event");

        let first = latest_record_for_session_after(
            &path,
            "01a03e54-7a7a-7fb3-85f5-24dfa739a2e1",
            "agent-turn-complete",
            std::time::UNIX_EPOCH,
        );
        let second = latest_record_for_session_after(
            &path,
            "01a03e54-7a7a-7fb3-85f5-24dfa739a2e1",
            "agent-turn-complete",
            std::time::UNIX_EPOCH,
        );

        assert!(first.is_some());
        assert_eq!(second, first);
        assert_eq!(record_cache_parse_count_for_test(), 1);
    }

    // Regression: 61e9a24 made misses the most expensive path; unmanaged or
    // not-yet-complete sessions reparsed every record on every scanner tick.
    #[test]
    fn unchanged_notify_sink_miss_is_cached() {
        let _guard = CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_record_cache_for_test();
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, Utc::now())
            .expect("append notify event");

        for _ in 0..2 {
            assert!(latest_record_for_session_after(
                &path,
                "session-with-no-record",
                "agent-turn-complete",
                std::time::UNIX_EPOCH,
            )
            .is_none());
        }
        assert_eq!(record_cache_parse_count_for_test(), 1);
    }

    // Regression: 61e9a24 parsed the sink before comparing it with the newer
    // transcript, even though the sink mtime proves no record can be fresh.
    #[test]
    fn sink_older_than_transcript_short_circuits_before_parse() {
        let _guard = CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_record_cache_for_test();
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, Utc::now())
            .expect("append notify event");
        let sink_mtime = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .expect("sink mtime");
        let transcript_mtime = sink_mtime + std::time::Duration::from_secs(1);

        assert!(latest_record_for_session_after(
            &path,
            "01a03e54-7a7a-7fb3-85f5-24dfa739a2e1",
            "agent-turn-complete",
            transcript_mtime,
        )
        .is_none());
        assert_eq!(record_cache_parse_count_for_test(), 0);
    }

    // Regression: 61e9a24 persisted full prompts and replies even though the
    // scanner only consumes the event type, thread id, turn id, and timestamp.
    #[test]
    fn sink_projects_private_notify_payload_to_edge_fields() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, Utc::now())
            .expect("append notify event");
        let contents = std::fs::read_to_string(&path).expect("notify sink");

        assert!(!contents.contains("Reply only OK"));
        assert!(!contents.contains("last-assistant-message"));
        let record: CodexNotifyRecord =
            serde_json::from_str(contents.trim()).expect("projected record");
        assert_eq!(record.event, "agent-turn-complete");
        assert_eq!(
            record.turn_id.as_deref(),
            Some("01a03e54-7bbf-74b2-ac52-2cfc3b0688cc")
        );
    }

    // Regression: 61e9a24 returned the newest event of any type, so a future
    // notify kind could mask the last authoritative turn-complete edge.
    #[test]
    fn lookup_filters_by_event_type_before_selecting_latest_record() {
        let _guard = CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_record_cache_for_test();
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, Utc::now())
            .expect("append turn complete");
        append_event_at(
            &path,
            r#"{"type":"future-event","thread-id":"01a03e54-7a7a-7fb3-85f5-24dfa739a2e1"}"#,
            Utc::now(),
        )
        .expect("append future event");

        let record = latest_record_for_session_after(
            &path,
            "01a03e54-7a7a-7fb3-85f5-24dfa739a2e1",
            "agent-turn-complete",
            std::time::UNIX_EPOCH,
        )
        .expect("turn-complete record");
        assert_eq!(record.event, "agent-turn-complete");
    }

    // Regression: 61e9a24 wiped every session edge when the sink crossed the
    // cap, causing all still-live sessions to fall back until their next turn.
    #[test]
    fn sink_cap_preserves_each_sessions_latest_edge() {
        let _guard = CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_record_cache_for_test();
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        let foreign = OBSERVED_CODEX_0_149_PAYLOAD
            .replace("01a03e54-7a7a-7fb3-85f5-24dfa739a2e1", "foreign-session");
        append_event_at(&path, &foreign, Utc::now()).expect("append foreign edge");
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open sink")
            .set_len(MAX_CODEX_NOTIFY_BYTES + 1)
            .expect("extend sink past cap");

        append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, Utc::now())
            .expect("append edge after cap");

        assert!(latest_record_for_session_after(
            &path,
            "foreign-session",
            "agent-turn-complete",
            std::time::UNIX_EPOCH,
        )
        .is_some());
    }

    #[cfg(unix)]
    // Regression: 61e9a24 created a world-readable sink containing complete
    // prompts and replies instead of private per-user activity metadata.
    #[test]
    fn notify_sink_permissions_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, Utc::now())
            .expect("append notify event");
        let mode = std::fs::metadata(&path)
            .expect("sink metadata")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(mode, 0o600);
    }
}
