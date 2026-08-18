//! Append-only compaction signal log store.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::coordination::errors::CoordinationError;
use crate::coordination::mesh_cli;
use crate::session_scanner::cli_tool::CliTool;

const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";
const COMPACTION_SIGNAL_SCHEMA_VERSION: u32 = 1;
const COMPACTION_SIGNAL_FILENAME: &str = "codex-compaction-signals.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionSignalKind {
    Compacted,
    ContextCompacted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CompactionSignalRecord {
    pub version: u32,
    pub signal_id: String,
    pub emitted_at: DateTime<Utc>,
    pub tool: CliTool,
    pub session_id: String,
    pub pane_id: String,
    pub project_path: String,
    pub jsonl_path: String,
    pub jsonl_offset: u64,
    pub transcript_timestamp: DateTime<Utc>,
    pub signal_kind: CompactionSignalKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionSignalReadItem {
    pub record: CompactionSignalRecord,
    pub next_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionSignalLogInspection {
    pub signal_log_path: PathBuf,
    pub file_size_bytes: u64,
    pub total_signals: usize,
    pub last_consumed_offset: u64,
    pub unconsumed_count: usize,
    pub recent_signals: Vec<CompactionSignalRecord>,
}

#[derive(Debug, Default)]
pub struct CompactionSignalLog;

impl CompactionSignalLog {
    pub fn append(
        teams_dir: &Path,
        team_name: &str,
        record: &CompactionSignalRecord,
    ) -> Result<u64, CoordinationError> {
        let _lock = super::lock::acquire_team_lock(teams_dir, team_name)?;

        let mut normalized = record.clone();
        normalized.version = COMPACTION_SIGNAL_SCHEMA_VERSION;

        let signal_dir = compaction_signal_dir(teams_dir, team_name);
        fs::create_dir_all(&signal_dir)?;

        let signal_path = compaction_signal_path(teams_dir, team_name);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&signal_path)?;
        let byte_offset = file.metadata()?.len();
        let payload = serde_json::to_string(&normalized).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to serialize compaction signal for team '{team_name}': {err}"
            ))
        })?;

        file.write_all(payload.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_data()?;

        Ok(byte_offset)
    }

    pub fn read_from_offset(
        teams_dir: &Path,
        team_name: &str,
        byte_offset: u64,
    ) -> Result<Vec<CompactionSignalRecord>, CoordinationError> {
        let items = Self::read_items_from_offset(teams_dir, team_name, byte_offset)?;
        Ok(items.into_iter().map(|item| item.record).collect())
    }

    pub fn read_items_from_offset(
        teams_dir: &Path,
        team_name: &str,
        byte_offset: u64,
    ) -> Result<Vec<CompactionSignalReadItem>, CoordinationError> {
        let signal_path = compaction_signal_path(teams_dir, team_name);
        let file = match File::open(&signal_path) {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(CoordinationError::Io(err)),
        };

        let file_len = file.metadata()?.len();
        if byte_offset >= file_len {
            return Ok(Vec::new());
        }

        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(byte_offset))?;

        let mut records = Vec::new();
        let mut line = String::new();
        let mut committed_offset = byte_offset;
        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line)?;
            if bytes_read == 0 {
                break;
            }
            if !line.ends_with('\n') {
                break;
            }
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                committed_offset += bytes_read as u64;
                continue;
            }

            let record = match serde_json::from_str::<CompactionSignalRecord>(trimmed) {
                Ok(record) => record,
                Err(_) => break,
            };
            committed_offset += bytes_read as u64;
            records.push(CompactionSignalReadItem {
                record,
                next_offset: committed_offset,
            });
        }

        Ok(records)
    }
}

pub fn append_signal(
    team_name: &str,
    record: &CompactionSignalRecord,
) -> Result<u64, CoordinationError> {
    CompactionSignalLog::append(&default_compaction_signal_teams_dir(), team_name, record)
}

pub fn read_signals_from_offset(
    team_name: &str,
    byte_offset: u64,
) -> Result<Vec<CompactionSignalRecord>, CoordinationError> {
    CompactionSignalLog::read_from_offset(
        &default_compaction_signal_teams_dir(),
        team_name,
        byte_offset,
    )
}

pub fn read_signal_items_from_offset(
    team_name: &str,
    byte_offset: u64,
) -> Result<Vec<CompactionSignalReadItem>, CoordinationError> {
    CompactionSignalLog::read_items_from_offset(
        &default_compaction_signal_teams_dir(),
        team_name,
        byte_offset,
    )
}

pub(crate) fn default_compaction_signal_teams_dir() -> PathBuf {
    if let Some(path) = std::env::var_os(CLAUDE_DIR_OVERRIDE_ENV) {
        if !path.is_empty() {
            return PathBuf::from(path).join("teams");
        }
    }
    if let Some(path) = mesh_cli::resolve_windows_mesh_teams_dir() {
        return path;
    }
    dirs::home_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("taurhaus-home"))
        .join(".claude")
        .join("teams")
}

fn compaction_signal_dir(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir
        .join(team_name)
        .join("state")
        .join("compaction")
        .join("signals")
}

fn compaction_signal_path(teams_dir: &Path, team_name: &str) -> PathBuf {
    compaction_signal_dir(teams_dir, team_name).join(COMPACTION_SIGNAL_FILENAME)
}

pub fn signal_log_path_for_team(teams_dir: &Path, team_name: &str) -> PathBuf {
    compaction_signal_path(teams_dir, team_name)
}

pub fn inspect_signal_log_at(
    teams_dir: &Path,
    team_name: &str,
    last_consumed_offset: u64,
    recent_limit: usize,
) -> Result<CompactionSignalLogInspection, CoordinationError> {
    let signal_log_path = signal_log_path_for_team(teams_dir, team_name);
    let file_size_bytes = match fs::metadata(&signal_log_path) {
        Ok(metadata) => metadata.len(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CompactionSignalLogInspection {
                signal_log_path,
                file_size_bytes: 0,
                total_signals: 0,
                last_consumed_offset,
                unconsumed_count: 0,
                recent_signals: Vec::new(),
            });
        }
        Err(err) => return Err(CoordinationError::Io(err)),
    };

    let all_items = CompactionSignalLog::read_items_from_offset(teams_dir, team_name, 0)?;
    let unconsumed_items =
        CompactionSignalLog::read_items_from_offset(teams_dir, team_name, last_consumed_offset)?;
    let mut recent_signals = all_items
        .iter()
        .rev()
        .take(recent_limit)
        .map(|item| item.record.clone())
        .collect::<Vec<_>>();
    recent_signals.reverse();

    Ok(CompactionSignalLogInspection {
        signal_log_path,
        file_size_bytes,
        total_signals: all_items.len(),
        last_consumed_offset,
        unconsumed_count: unconsumed_items.len(),
        recent_signals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::TimeZone;
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    fn sample_signal() -> CompactionSignalRecord {
        CompactionSignalRecord {
            version: 99,
            signal_id: Uuid::new_v4().to_string(),
            emitted_at: Utc
                .with_ymd_and_hms(2026, 3, 8, 20, 0, 0)
                .single()
                .expect("datetime"),
            tool: CliTool::Codex,
            session_id: "sess-123".to_string(),
            pane_id: "%217".to_string(),
            project_path: "/home/user/projects/taurhaus".to_string(),
            jsonl_path: "/home/user/.codex/sessions/2026/03/08/rollout.jsonl".to_string(),
            jsonl_offset: 18_423,
            transcript_timestamp: Utc
                .with_ymd_and_hms(2026, 3, 8, 19, 59, 59)
                .single()
                .expect("datetime"),
            signal_kind: CompactionSignalKind::ContextCompacted,
        }
    }

    #[test]
    fn compaction_signal_round_trips_through_json() {
        let signal = sample_signal();

        let json = serde_json::to_string(&signal).expect("serialize");
        let parsed: CompactionSignalRecord = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed, signal);
    }

    #[test]
    fn append_and_read_returns_appended_records() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let team_name = "taurhaus-team";
        let first = sample_signal();
        let second = CompactionSignalRecord {
            signal_id: Uuid::new_v4().to_string(),
            jsonl_offset: 19_001,
            signal_kind: CompactionSignalKind::Compacted,
            ..sample_signal()
        };

        let first_offset =
            CompactionSignalLog::append(tmp.path(), team_name, &first).expect("append first");
        let second_offset =
            CompactionSignalLog::append(tmp.path(), team_name, &second).expect("append second");

        assert_eq!(first_offset, 0);
        assert!(second_offset > first_offset);

        let records =
            CompactionSignalLog::read_from_offset(tmp.path(), team_name, 0).expect("read");

        assert_eq!(
            records,
            vec![
                CompactionSignalRecord {
                    version: COMPACTION_SIGNAL_SCHEMA_VERSION,
                    ..first
                },
                CompactionSignalRecord {
                    version: COMPACTION_SIGNAL_SCHEMA_VERSION,
                    ..second
                }
            ]
        );
    }

    #[test]
    fn read_from_offset_only_returns_newer_records() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let team_name = "taurhaus-team";
        let first = sample_signal();
        let second = CompactionSignalRecord {
            signal_id: Uuid::new_v4().to_string(),
            jsonl_offset: 19_777,
            ..sample_signal()
        };

        CompactionSignalLog::append(tmp.path(), team_name, &first).expect("append first");
        let second_offset =
            CompactionSignalLog::append(tmp.path(), team_name, &second).expect("append second");

        let records = CompactionSignalLog::read_from_offset(tmp.path(), team_name, second_offset)
            .expect("read from second offset");

        assert_eq!(
            records,
            vec![CompactionSignalRecord {
                version: COMPACTION_SIGNAL_SCHEMA_VERSION,
                ..second
            }]
        );
    }

    #[test]
    fn read_items_from_offset_stops_before_partial_trailing_line() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let team_name = "taurhaus-team";
        let first = sample_signal();

        CompactionSignalLog::append(tmp.path(), team_name, &first).expect("append first");

        let signal_path = signal_log_path_for_team(tmp.path(), team_name);
        let mut file = OpenOptions::new()
            .append(true)
            .open(&signal_path)
            .expect("open signal log");
        write!(file, "{{\"version\":1").expect("write partial line");

        let items =
            CompactionSignalLog::read_items_from_offset(tmp.path(), team_name, 0).expect("read");

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].record,
            CompactionSignalRecord {
                version: COMPACTION_SIGNAL_SCHEMA_VERSION,
                ..first
            }
        );
        let raw = std::fs::read_to_string(&signal_path).expect("read signal file");
        assert!(items[0].next_offset < raw.len() as u64);
    }
}
