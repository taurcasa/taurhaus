use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{Map, Value};

pub const JSONL_LOG_FILE_NAME: &str = "taurhaus.log.jsonl";
const ROTATION_BYTES: u64 = 20 * 1024 * 1024;
const RETENTION_DAYS: i64 = 7;
const LOG_WRITE_WARN_THROTTLE_MS: u64 = 5_000;

static LAST_LOG_WRITE_WARN_MS: AtomicU64 = AtomicU64::new(0);
static GLOBAL_LOG_EMITTER: OnceLock<LogEmitter> = OnceLock::new();

#[derive(Clone)]
struct LogEmitter {
    sender: Sender<LogRecord>,
    run_id: Arc<str>,
}

/// Managed state: async JSONL sink for frontend + backend log events.
pub struct LogFileState {
    emitter: LogEmitter,
}

#[derive(Debug, Clone, Serialize)]
struct LogRecord {
    ts: String,
    level: String,
    component: String,
    event: String,
    run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(flatten)]
    fields: Map<String, Value>,
}

struct JsonlFileWriter {
    base_path: PathBuf,
    file: std::fs::File,
    current_size: u64,
    rotate_bytes: u64,
    retention_days: i64,
}

impl LogFileState {
    pub fn new(log_path: PathBuf) -> std::io::Result<Self> {
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        let emitter = spawn_writer(log_path, run_id)?;
        Ok(Self { emitter })
    }

    #[cfg(test)]
    pub fn run_id(&self) -> &str {
        self.emitter.run_id.as_ref()
    }

    pub fn emit(
        &self,
        level: &str,
        component: &str,
        event: &str,
        message: Option<String>,
        fields: Map<String, Value>,
    ) {
        self.emitter.emit(level, component, event, message, fields);
    }
}

impl LogEmitter {
    fn emit(
        &self,
        level: &str,
        component: &str,
        event: &str,
        message: Option<String>,
        mut fields: Map<String, Value>,
    ) {
        // Guard canonical top-level keys from caller-provided field collisions.
        for reserved in ["ts", "level", "component", "event", "run_id", "message"] {
            fields.remove(reserved);
        }

        let record = LogRecord {
            ts: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            level: normalize_level(level).to_string(),
            component: component.to_string(),
            event: event.to_string(),
            run_id: self.run_id.to_string(),
            message,
            fields,
        };

        if let Err(error) = self.sender.send(record) {
            if should_emit_write_warning(now_millis()) {
                tracing::warn!(error = %error, "failed to enqueue structured log event");
            }
        }
    }
}

impl JsonlFileWriter {
    fn new(base_path: PathBuf, rotate_bytes: u64, retention_days: i64) -> std::io::Result<Self> {
        if let Some(parent) = base_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&base_path)?;
        let current_size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            base_path,
            file,
            current_size,
            rotate_bytes,
            retention_days,
        })
    }

    fn write_record(&mut self, record: &LogRecord) -> std::io::Result<()> {
        let payload = serde_json::to_vec(record).map_err(|error| {
            std::io::Error::other(format!("serialize log record failed: {error}"))
        })?;
        let line_len = payload.len() as u64 + 1;
        if self.current_size > 0 && self.current_size.saturating_add(line_len) > self.rotate_bytes {
            self.rotate(Utc::now())?;
        }

        self.file.write_all(&payload)?;
        self.file.write_all(b"\n")?;
        self.file.flush()?;
        self.current_size = self.current_size.saturating_add(line_len);
        Ok(())
    }

    fn rotate(&mut self, now: DateTime<Utc>) -> std::io::Result<()> {
        let segment_path = next_rotation_segment_path(&self.base_path, now);
        if self.base_path.exists() {
            std::fs::rename(&self.base_path, &segment_path)?;
        }

        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.base_path)?;
        self.current_size = self.file.metadata().map(|m| m.len()).unwrap_or(0);
        self.prune_old_segments(now)?;
        Ok(())
    }

    fn prune_old_segments(&self, now: DateTime<Utc>) -> std::io::Result<()> {
        let Some(dir) = self.base_path.parent() else {
            return Ok(());
        };
        let Some(base_name) = self.base_path.file_name().and_then(|name| name.to_str()) else {
            return Ok(());
        };
        let Some(prefix_without_suffix) = base_name.strip_suffix(".jsonl") else {
            return Ok(());
        };
        let prefix = format!("{prefix_without_suffix}.");
        let cutoff = now - chrono::Duration::days(self.retention_days);

        for entry in std::fs::read_dir(dir)? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if file_name == base_name {
                continue;
            }
            let Some(timestamp) = parse_rotation_timestamp(file_name, &prefix) else {
                continue;
            };
            if timestamp < cutoff {
                let _ = std::fs::remove_file(path);
            }
        }

        Ok(())
    }
}

pub fn jsonl_log_path(data_dir: &Path) -> PathBuf {
    data_dir.join(JSONL_LOG_FILE_NAME)
}

pub fn install_global_sink(state: &LogFileState) {
    let _ = GLOBAL_LOG_EMITTER.set(state.emitter.clone());
}

pub fn emit_global(
    level: &str,
    component: &str,
    event: &str,
    message: Option<String>,
    fields: Map<String, Value>,
) {
    if let Some(emitter) = GLOBAL_LOG_EMITTER.get() {
        emitter.emit(level, component, event, message, fields);
    }
}

#[tauri::command]
pub fn frontend_log(level: String, message: String, log_file: tauri::State<LogFileState>) {
    frontend_log_impl(&level, &message, log_file.inner());
}

fn frontend_log_impl(level: &str, message: &str, log_file: &LogFileState) {
    let mut fields = Map::new();
    fields.insert("source".to_string(), Value::String("frontend".to_string()));
    log_file.emit(
        level,
        "frontend",
        "frontend.log",
        Some(message.to_string()),
        fields,
    );
}

fn spawn_writer(log_path: PathBuf, run_id: String) -> std::io::Result<LogEmitter> {
    let writer = JsonlFileWriter::new(log_path, ROTATION_BYTES, RETENTION_DAYS)?;
    let (sender, receiver) = mpsc::channel::<LogRecord>();

    std::thread::Builder::new()
        .name("taurhaus-jsonl-log-writer".to_string())
        .spawn(move || {
            let mut writer = writer;
            while let Ok(record) = receiver.recv() {
                if let Err(error) = writer.write_record(&record) {
                    if should_emit_write_warning(now_millis()) {
                        tracing::warn!(error = %error, "failed to write structured log event");
                    }
                }
            }
        })?;

    Ok(LogEmitter {
        sender,
        run_id: Arc::from(run_id),
    })
}

fn normalize_level(level: &str) -> &'static str {
    if level.eq_ignore_ascii_case("error") {
        "ERROR"
    } else if level.eq_ignore_ascii_case("warn") || level.eq_ignore_ascii_case("warning") {
        "WARN"
    } else if level.eq_ignore_ascii_case("debug") {
        "DEBUG"
    } else {
        "INFO"
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn should_emit_write_warning(now_ms: u64) -> bool {
    LAST_LOG_WRITE_WARN_MS
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |last| {
            if now_ms.saturating_sub(last) >= LOG_WRITE_WARN_THROTTLE_MS {
                Some(now_ms)
            } else {
                None
            }
        })
        .is_ok()
}

fn next_rotation_segment_path(base_path: &Path, now: DateTime<Utc>) -> PathBuf {
    for attempt in 0u32..1000 {
        let candidate = rotation_segment_path(base_path, now, attempt);
        if !candidate.exists() {
            return candidate;
        }
    }

    rotation_segment_path(base_path, now, 1001)
}

fn rotation_segment_path(base_path: &Path, now: DateTime<Utc>, attempt: u32) -> PathBuf {
    let parent = base_path.parent().unwrap_or_else(|| Path::new("."));
    let base_name = base_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("taurhaus.log.jsonl");
    let base_prefix = base_name.strip_suffix(".jsonl").unwrap_or(base_name);
    let stamp = now.format("%Y%m%dT%H%M%SZ");
    let file_name = if attempt == 0 {
        format!("{base_prefix}.{stamp}.jsonl")
    } else {
        format!("{base_prefix}.{stamp}.{attempt}.jsonl")
    };
    parent.join(file_name)
}

fn parse_rotation_timestamp(file_name: &str, prefix: &str) -> Option<DateTime<Utc>> {
    if !file_name.starts_with(prefix) || !file_name.ends_with(".jsonl") {
        return None;
    }
    let body = &file_name[prefix.len()..file_name.len() - ".jsonl".len()];
    let timestamp_token = body.split('.').next()?;
    let naive = NaiveDateTime::parse_from_str(timestamp_token, "%Y%m%dT%H%M%SZ").ok()?;
    Some(DateTime::from_naive_utc_and_offset(naive, Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::time::Duration;

    fn read_lines(path: &Path) -> Vec<String> {
        let mut file = std::fs::File::open(path).expect("open log for read");
        let mut content = String::new();
        file.read_to_string(&mut content).expect("read log file");
        content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    fn wait_for_lines(path: &Path, expected_minimum: usize) -> Vec<String> {
        for _ in 0..50 {
            let lines = if path.exists() {
                read_lines(path)
            } else {
                Vec::new()
            };
            if lines.len() >= expected_minimum {
                return lines;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        read_lines(path)
    }

    #[test]
    fn frontend_log_writes_valid_jsonl_with_required_schema() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join(JSONL_LOG_FILE_NAME);
        let state = LogFileState::new(log_path.clone()).expect("create log state");

        frontend_log_impl("warn", "hello from ui", &state);
        let lines = wait_for_lines(&log_path, 1);
        assert_eq!(lines.len(), 1);

        let value: Value = serde_json::from_str(&lines[0]).expect("valid json");
        for key in ["ts", "level", "component", "event", "run_id"] {
            assert!(value.get(key).is_some(), "missing required key: {key}");
        }
        assert_eq!(value["level"], "WARN");
        assert_eq!(value["component"], "frontend");
        assert_eq!(value["event"], "frontend.log");
        assert_eq!(value["message"], "hello from ui");
    }

    #[test]
    fn run_id_is_reused_across_events_from_same_state() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join(JSONL_LOG_FILE_NAME);
        let state = LogFileState::new(log_path.clone()).expect("create log state");

        frontend_log_impl("info", "first", &state);
        frontend_log_impl("error", "second", &state);
        let lines = wait_for_lines(&log_path, 2);
        assert_eq!(lines.len(), 2);

        let first: Value = serde_json::from_str(&lines[0]).expect("first json");
        let second: Value = serde_json::from_str(&lines[1]).expect("second json");
        assert_eq!(first["run_id"], second["run_id"]);
        assert_eq!(first["run_id"], state.run_id());
    }

    #[test]
    fn rotation_segment_path_uses_expected_naming() {
        let now = DateTime::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("20260305T231530Z", "%Y%m%dT%H%M%SZ").unwrap(),
            Utc,
        );
        let base = PathBuf::from("/tmp/taurhaus.log.jsonl");
        let first = rotation_segment_path(&base, now, 0);
        let retry = rotation_segment_path(&base, now, 2);

        assert_eq!(
            first.file_name().and_then(|name| name.to_str()),
            Some("taurhaus.log.20260305T231530Z.jsonl")
        );
        assert_eq!(
            retry.file_name().and_then(|name| name.to_str()),
            Some("taurhaus.log.20260305T231530Z.2.jsonl")
        );
    }

    #[test]
    fn rotation_happens_when_size_threshold_is_exceeded() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let base_path = dir.path().join(JSONL_LOG_FILE_NAME);
        let mut writer = JsonlFileWriter::new(base_path.clone(), 240, RETENTION_DAYS).unwrap();

        let record = LogRecord {
            ts: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            level: "INFO".to_string(),
            component: "test".to_string(),
            event: "rotation.check".to_string(),
            run_id: "run_test".to_string(),
            message: Some("x".repeat(150)),
            fields: Map::new(),
        };

        writer.write_record(&record).expect("first write");
        writer
            .write_record(&record)
            .expect("second write triggers rotation");

        let entries = std::fs::read_dir(dir.path())
            .expect("read log dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(entries.iter().any(|name| name == JSONL_LOG_FILE_NAME));
        assert!(entries
            .iter()
            .any(|name| name.starts_with("taurhaus.log.") && name.ends_with(".jsonl")));
    }

    #[test]
    fn prune_removes_segments_older_than_retention_window() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let base_path = dir.path().join(JSONL_LOG_FILE_NAME);
        let writer = JsonlFileWriter::new(base_path.clone(), ROTATION_BYTES, 7).unwrap();

        let old_name = "taurhaus.log.20260220T010203Z.jsonl";
        let fresh_name = "taurhaus.log.20260305T010203Z.jsonl";
        std::fs::write(dir.path().join(old_name), b"{}\n").unwrap();
        std::fs::write(dir.path().join(fresh_name), b"{}\n").unwrap();

        let now = DateTime::from_naive_utc_and_offset(
            NaiveDateTime::parse_from_str("20260306T000000Z", "%Y%m%dT%H%M%SZ").unwrap(),
            Utc,
        );
        writer
            .prune_old_segments(now)
            .expect("prune should succeed");

        assert!(
            !dir.path().join(old_name).exists(),
            "old segment should be removed"
        );
        assert!(
            dir.path().join(fresh_name).exists(),
            "fresh segment should be retained"
        );
    }

    #[test]
    fn parse_rotation_timestamp_accepts_suffix_attempt_format() {
        let parsed =
            parse_rotation_timestamp("taurhaus.log.20260305T231530Z.4.jsonl", "taurhaus.log.")
                .expect("parse timestamp");

        assert_eq!(
            parsed,
            DateTime::<Utc>::from_naive_utc_and_offset(
                NaiveDateTime::parse_from_str("20260305T231530Z", "%Y%m%dT%H%M%SZ").unwrap(),
                Utc,
            )
        );
    }
}
