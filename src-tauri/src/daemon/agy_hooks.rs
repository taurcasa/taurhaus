use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

pub const AGY_HOOKS_FILENAME: &str = "agy-hooks.jsonl";
const MAX_AGY_HOOK_BYTES: u64 = 5 * 1024 * 1024;
const DUPLICATE_THROTTLE_MS: i64 = 250;
const DUPLICATE_TAIL_BYTES: u64 = 64 * 1024;

struct CachedRecords {
    len: u64,
    modified: SystemTime,
    records: HashMap<String, AgyHookRecord>,
}

static RECORD_CACHE: LazyLock<Mutex<HashMap<PathBuf, CachedRecords>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(test)]
pub(crate) static AGY_HOOK_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[cfg(test)]
static APPEND_READ_BYTES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static RECORD_PARSE_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgyHookEvent {
    Busy,
    Idle,
}

impl AgyHookEvent {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "busy" => Some(Self::Busy),
            "idle" => Some(Self::Idle),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyHookState {
    Busy,
    Idle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgyHookRecord {
    pub ts: DateTime<Utc>,
    pub conversation_id: String,
    pub state: AgyHookState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgyHookAppendOutcome {
    pub recorded: bool,
    pub throttled: bool,
    pub truncated: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookPayload {
    conversation_id: Option<String>,
    #[serde(default)]
    fully_idle: bool,
}

/// Append one native Antigravity activity edge to the bounded JSONL sink.
pub fn append_event_at(
    path: &Path,
    event: AgyHookEvent,
    raw_payload: &str,
    ts: DateTime<Utc>,
) -> Result<AgyHookAppendOutcome, String> {
    let payload = serde_json::from_str::<HookPayload>(raw_payload)
        .map_err(|error| format!("invalid Antigravity hook JSON: {error}"))?;
    let conversation_id = payload
        .conversation_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Antigravity hook payload has no conversationId".to_string())?;
    if event == AgyHookEvent::Idle && !payload.fully_idle {
        return Ok(AgyHookAppendOutcome {
            recorded: false,
            throttled: false,
            truncated: false,
        });
    }
    let record = AgyHookRecord {
        ts,
        conversation_id,
        state: match event {
            AgyHookEvent::Busy => AgyHookState::Busy,
            AgyHookEvent::Idle => AgyHookState::Idle,
        },
    };
    let parent = path
        .parent()
        .ok_or_else(|| format!("Antigravity hook sink '{}' has no parent", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create Antigravity hook sink directory '{}': {error}",
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
    let mut file = options
        .open(path)
        .map_err(|error| format!("failed to open Antigravity hook sink: {error}"))?;
    #[cfg(unix)]
    fs::set_permissions(path, {
        use std::os::unix::fs::PermissionsExt;
        fs::Permissions::from_mode(0o600)
    })
    .map_err(|error| format!("failed to secure Antigravity hook sink: {error}"))?;
    file.lock_exclusive()
        .map_err(|error| format!("failed to lock Antigravity hook sink: {error}"))?;
    let result = append_locked(&mut file, &record);
    let _ = FileExt::unlock(&file);
    if result.as_ref().is_ok_and(|outcome| outcome.recorded) {
        RECORD_CACHE
            .lock()
            .expect("agy hook record cache lock")
            .remove(path);
    }
    result
}

fn append_locked(file: &mut File, record: &AgyHookRecord) -> Result<AgyHookAppendOutcome, String> {
    let len = file
        .metadata()
        .map_err(|error| format!("failed to inspect Antigravity hook sink: {error}"))?
        .len();
    let truncated = len >= MAX_AGY_HOOK_BYTES;
    let read_start = if truncated {
        0
    } else {
        len.saturating_sub(DUPLICATE_TAIL_BYTES)
    };
    file.seek(SeekFrom::Start(read_start))
        .map_err(|error| format!("failed to seek Antigravity hook sink: {error}"))?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .map_err(|error| format!("failed to read Antigravity hook sink: {error}"))?;
    #[cfg(test)]
    APPEND_READ_BYTES.fetch_add(contents.len(), std::sync::atomic::Ordering::Relaxed);
    let parseable = if read_start == 0 {
        contents.as_slice()
    } else {
        contents
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(&[][..], |newline| &contents[newline + 1..])
    };
    let mut latest = latest_records(parseable);
    if latest.get(&record.conversation_id).is_some_and(|previous| {
        previous.state == record.state
            && record
                .ts
                .signed_duration_since(previous.ts)
                .num_milliseconds()
                < DUPLICATE_THROTTLE_MS
    }) {
        return Ok(AgyHookAppendOutcome {
            recorded: false,
            throttled: true,
            truncated: false,
        });
    }

    if truncated {
        latest.insert(record.conversation_id.clone(), record.clone());
        let mut retained = latest.into_values().collect::<Vec<_>>();
        retained.sort_by_key(|entry| entry.ts);
        file.set_len(0)
            .map_err(|error| format!("failed to cap Antigravity hook sink: {error}"))?;
        for retained_record in retained {
            serde_json::to_writer(&mut *file, &retained_record)
                .map_err(|error| format!("failed to serialize Antigravity hook record: {error}"))?;
            file.write_all(b"\n")
                .map_err(|error| format!("failed to retain Antigravity hook record: {error}"))?;
        }
    } else {
        file.seek(SeekFrom::End(0))
            .map_err(|error| format!("failed to seek Antigravity hook sink: {error}"))?;
        serde_json::to_writer(&mut *file, record)
            .map_err(|error| format!("failed to serialize Antigravity hook record: {error}"))?;
        file.write_all(b"\n")
            .map_err(|error| format!("failed to append Antigravity hook record: {error}"))?;
    }
    file.flush()
        .map_err(|error| format!("failed to flush Antigravity hook sink: {error}"))?;
    Ok(AgyHookAppendOutcome {
        recorded: true,
        throttled: false,
        truncated,
    })
}

fn latest_records(contents: &[u8]) -> HashMap<String, AgyHookRecord> {
    contents
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<AgyHookRecord>(line).ok())
        .map(|record| (record.conversation_id.clone(), record))
        .collect()
}

pub fn latest_record_for_session_after(
    path: &Path,
    session_id: &str,
    not_before: SystemTime,
) -> Option<AgyHookRecord> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    {
        let cache = RECORD_CACHE.lock().ok()?;
        if let Some(cached) = cache
            .get(path)
            .filter(|cached| cached.len == metadata.len() && cached.modified == modified)
        {
            return cached.records.get(session_id).cloned().and_then(|record| {
                (record.ts >= DateTime::<Utc>::from(not_before)).then_some(record)
            });
        }
    }

    let contents = fs::read(path).ok()?;
    #[cfg(test)]
    RECORD_PARSE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let records = latest_records(&contents);
    let record = records.get(session_id).cloned();
    RECORD_CACHE.lock().ok()?.insert(
        path.to_path_buf(),
        CachedRecords {
            len: metadata.len(),
            modified,
            records,
        },
    );
    let record = record?;
    (record.ts >= DateTime::<Utc>::from(not_before)).then_some(record)
}

pub fn latest_record_for_session(path: &Path, session_id: &str) -> Option<AgyHookRecord> {
    latest_record_for_session_after(path, session_id, SystemTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn agy_hook_sink_parses_busy_and_fully_idle() {
        // Regression: commit c0aa59a only consumed Codex's notifier, so agy's
        // verified PreInvocation/Stop edges had no bounded native sink.
        let _guard = AGY_HOOK_TEST_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("agy-hooks.jsonl");
        let busy_at = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
        append_event_at(
            &path,
            AgyHookEvent::Busy,
            r#"{"conversationId":"conversation-1"}"#,
            busy_at,
        )
        .unwrap();
        append_event_at(
            &path,
            AgyHookEvent::Idle,
            r#"{"conversationId":"conversation-1","fullyIdle":true}"#,
            busy_at + chrono::Duration::seconds(1),
        )
        .unwrap();

        let latest = latest_record_for_session(&path, "conversation-1").unwrap();
        assert_eq!(latest.state, AgyHookState::Idle);
    }

    #[test]
    fn agy_hook_sink_throttles_duplicates_and_caps_at_five_megabytes() {
        // Regression: commit c0aa59a's native edge path had no agy-specific
        // hook-rate protection; synchronous hooks must remain millisecond work.
        let _guard = AGY_HOOK_TEST_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("agy-hooks.jsonl");
        let ts = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
        append_event_at(
            &path,
            AgyHookEvent::Busy,
            r#"{"conversationId":"conversation-1"}"#,
            ts,
        )
        .unwrap();
        let duplicate = append_event_at(
            &path,
            AgyHookEvent::Busy,
            r#"{"conversationId":"conversation-1"}"#,
            ts + chrono::Duration::milliseconds(100),
        )
        .unwrap();
        assert!(duplicate.throttled);

        std::fs::write(&path, vec![b'x'; MAX_AGY_HOOK_BYTES as usize]).unwrap();
        let capped = append_event_at(
            &path,
            AgyHookEvent::Busy,
            r#"{"conversationId":"conversation-2"}"#,
            ts + chrono::Duration::seconds(1),
        )
        .unwrap();
        assert!(capped.truncated);
        assert!(std::fs::metadata(path).unwrap().len() < MAX_AGY_HOOK_BYTES);
    }

    #[test]
    fn duplicate_throttle_reads_only_a_bounded_tail() {
        // Regression: commit 4e9e2c5 fully read and parsed a sink up to 5 MB
        // inside every synchronous PreInvocation hook.
        let _guard = AGY_HOOK_TEST_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("agy-hooks.jsonl");
        let ts = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
        let mut fixture = Vec::new();
        for index in 0..8_000 {
            serde_json::to_writer(
                &mut fixture,
                &AgyHookRecord {
                    ts,
                    conversation_id: format!("conversation-{index}"),
                    state: AgyHookState::Idle,
                },
            )
            .unwrap();
            fixture.push(b'\n');
        }
        std::fs::write(&path, fixture).unwrap();
        APPEND_READ_BYTES.store(0, std::sync::atomic::Ordering::Relaxed);

        append_event_at(
            &path,
            AgyHookEvent::Busy,
            r#"{"conversationId":"target"}"#,
            ts + chrono::Duration::seconds(1),
        )
        .unwrap();

        assert!(
            APPEND_READ_BYTES.load(std::sync::atomic::Ordering::Relaxed) <= 64 * 1024,
            "duplicate throttle must not scan the whole sink"
        );
    }

    #[test]
    fn unchanged_sink_is_parsed_once_across_scan_polls() {
        // Regression: commit 4e9e2c5 reparsed the full hook sink for every
        // process on every scanner poll even when its metadata was unchanged.
        let _guard = AGY_HOOK_TEST_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("agy-hooks.jsonl");
        let ts = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
        append_event_at(
            &path,
            AgyHookEvent::Busy,
            r#"{"conversationId":"conversation-1"}"#,
            ts,
        )
        .unwrap();
        RECORD_PARSE_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);

        assert!(latest_record_for_session(&path, "conversation-1").is_some());
        assert!(latest_record_for_session(&path, "conversation-1").is_some());
        assert_eq!(
            RECORD_PARSE_COUNT.load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }
}
