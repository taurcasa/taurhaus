//! Compaction delivery idempotency store and audit helpers.

use std::fs;
use std::path::{Path, PathBuf};

use crate::coordination::errors::CoordinationError;
use crate::coordination::mesh_cli;
use crate::session_scanner::cli_tool::CliTool;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";
const COMPACTION_SCHEMA_VERSION: u32 = 1;
pub const COMPACTION_FRESHNESS_WINDOW_SECS: i64 = 15;
const RESERVED_COMPACTION_STATE_BASENAMES: &[&str] = &["extractor-state", "signal-watcher-state"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionDeliveryResult {
    Injected,
    Skipped,
    Stale,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MemberCompactionState {
    pub version: u32,
    pub member_name: String,
    pub last_session_id: String,
    pub last_compaction_timestamp: DateTime<Utc>,
    pub last_delivery_result: CompactionDeliveryResult,
}

#[derive(Debug, Default)]
pub struct MemberCompactionStore;

impl MemberCompactionStore {
    pub fn load(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
    ) -> Result<Option<MemberCompactionState>, CoordinationError> {
        let path = compaction_state_path(teams_dir, team_name, member_name);
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(CoordinationError::Io(err)),
        };

        let state = serde_json::from_str::<MemberCompactionState>(&raw).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to parse compaction state for member '{member_name}' in team '{team_name}': {err}"
            ))
        })?;

        Ok(Some(state))
    }

    pub fn save(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
        state: &MemberCompactionState,
    ) -> Result<(), CoordinationError> {
        let _lock = super::lock::acquire_team_lock(teams_dir, team_name)?;

        let mut normalized = state.clone();
        normalized.version = COMPACTION_SCHEMA_VERSION;
        normalized.member_name = member_name.to_string();

        let dir = compaction_state_dir(teams_dir, team_name);
        fs::create_dir_all(&dir)?;

        let target_path = compaction_state_path(teams_dir, team_name, member_name);
        let tmp_path = compaction_state_tmp_path(teams_dir, team_name, member_name);
        let payload = serde_json::to_string_pretty(&normalized).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to serialize compaction state for member '{member_name}': {err}"
            ))
        })?;

        fs::write(&tmp_path, payload.as_bytes())?;
        if let Err(err) = fs::rename(&tmp_path, &target_path) {
            if is_windows_unsupported_rename_error(&err) {
                if let Err(write_err) = fs::write(&target_path, payload.as_bytes()) {
                    let _ = fs::remove_file(&tmp_path);
                    return Err(CoordinationError::Io(write_err));
                }
                let _ = fs::remove_file(&tmp_path);
                return Ok(());
            }

            let _ = fs::remove_file(&tmp_path);
            return Err(CoordinationError::Io(err));
        }

        Ok(())
    }

    pub fn load_all(
        teams_dir: &Path,
        team_name: &str,
    ) -> Result<Vec<(String, MemberCompactionState)>, CoordinationError> {
        let dir = compaction_state_dir(teams_dir, team_name);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(member_name) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            if RESERVED_COMPACTION_STATE_BASENAMES.contains(&member_name) {
                continue;
            }
            let Some(state) = Self::load(teams_dir, team_name, member_name)? else {
                continue;
            };
            results.push((member_name.to_string(), state));
        }

        results.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(results)
    }
}

pub fn is_already_handled(
    team_name: &str,
    member_name: &str,
    tool: CliTool,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
) -> bool {
    match MemberCompactionStore::load(&default_compaction_teams_dir(), team_name, member_name) {
        Ok(Some(state)) => is_already_handled_state(&state, tool, session_id, compaction_timestamp),
        Ok(None) => false,
        Err(error) => {
            tracing::warn!(
                team_name = team_name,
                member_name = member_name,
                tool = %tool,
                error = %error,
                "failed to load compaction state during idempotency check"
            );
            false
        }
    }
}

pub fn record_delivery(
    team_name: &str,
    member_name: &str,
    tool: CliTool,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
    result: CompactionDeliveryResult,
) -> Result<(), CoordinationError> {
    let state = MemberCompactionState {
        version: COMPACTION_SCHEMA_VERSION,
        member_name: member_name.to_string(),
        last_session_id: session_id.to_string(),
        last_compaction_timestamp: compaction_timestamp,
        last_delivery_result: result,
    };
    MemberCompactionStore::save(
        &default_compaction_teams_dir(),
        team_name,
        member_name,
        &state,
    )?;
    emit_compaction_delivery_event(
        team_name,
        member_name,
        tool,
        session_id,
        compaction_timestamp,
        result,
    );
    Ok(())
}

pub fn is_stale_compaction(detected_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now.signed_duration_since(detected_at) > TimeDelta::seconds(COMPACTION_FRESHNESS_WINDOW_SECS)
}

pub fn emit_compaction_detected_event(
    team_name: &str,
    member_name: &str,
    tool: CliTool,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
) {
    tracing::info!(
        team_name = team_name,
        member_name = member_name,
        tool = %tool,
        session_id = session_id,
        compaction_timestamp = %compaction_timestamp.to_rfc3339(),
        "compaction.detected"
    );
    emit_global(
        "info",
        "coordination",
        "compaction.detected",
        Some("Compaction signal detected".to_string()),
        compaction_event_fields(
            team_name,
            member_name,
            tool,
            session_id,
            compaction_timestamp,
            None,
        ),
    );
}

pub fn emit_compaction_delivery_event(
    team_name: &str,
    member_name: &str,
    tool: CliTool,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
    result: CompactionDeliveryResult,
) {
    let event = match result {
        CompactionDeliveryResult::Injected => "compaction.injected",
        CompactionDeliveryResult::Skipped => "compaction.skipped",
        CompactionDeliveryResult::Stale => "compaction.stale",
        CompactionDeliveryResult::Failed => "compaction.failed",
    };

    tracing::info!(
        team_name = team_name,
        member_name = member_name,
        tool = %tool,
        session_id = session_id,
        compaction_timestamp = %compaction_timestamp.to_rfc3339(),
        result = ?result,
        "{event}"
    );
    emit_global(
        if matches!(result, CompactionDeliveryResult::Failed) {
            "warn"
        } else {
            "info"
        },
        "coordination",
        event,
        Some("Compaction delivery outcome recorded".to_string()),
        compaction_event_fields(
            team_name,
            member_name,
            tool,
            session_id,
            compaction_timestamp,
            Some(result),
        ),
    );
}

#[cfg(not(test))]
fn emit_global(
    level: &str,
    component: &str,
    event: &str,
    message: Option<String>,
    fields: Map<String, Value>,
) {
    crate::logging::emit_global(level, component, event, message, fields);
}

#[cfg(test)]
fn emit_global(
    _level: &str,
    _component: &str,
    _event: &str,
    _message: Option<String>,
    _fields: Map<String, Value>,
) {
}

fn is_already_handled_state(
    state: &MemberCompactionState,
    _tool: CliTool,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
) -> bool {
    state.last_session_id == session_id && state.last_compaction_timestamp == compaction_timestamp
}

fn default_compaction_teams_dir() -> PathBuf {
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

fn is_windows_unsupported_rename_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

fn compaction_state_dir(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir.join(team_name).join("state").join("compaction")
}

fn compaction_state_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    compaction_state_dir(teams_dir, team_name).join(format!("{member_name}.json"))
}

fn compaction_state_tmp_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    compaction_state_dir(teams_dir, team_name).join(format!("{member_name}.json.tmp"))
}

fn compaction_event_fields(
    team_name: &str,
    member_name: &str,
    tool: CliTool,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
    result: Option<CompactionDeliveryResult>,
) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert(
        "team_name".to_string(),
        Value::String(team_name.to_string()),
    );
    fields.insert(
        "member_name".to_string(),
        Value::String(member_name.to_string()),
    );
    fields.insert("tool".to_string(), Value::String(tool.to_string()));
    fields.insert(
        "session_id".to_string(),
        Value::String(session_id.to_string()),
    );
    fields.insert(
        "compaction_timestamp".to_string(),
        Value::String(compaction_timestamp.to_rfc3339()),
    );
    if let Some(result) = result {
        fields.insert(
            "delivery_result".to_string(),
            Value::String(
                serde_json::to_value(result)
                    .ok()
                    .and_then(|value| value.as_str().map(ToOwned::to_owned))
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
        );
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs2::FileExt;
    use std::ffi::OsString;
    use std::sync::{LazyLock, Mutex, MutexGuard};

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp")
            .with_timezone(&Utc)
    }

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct EnvTestGuard {
        _in_process: MutexGuard<'static, ()>,
        lock_file: std::fs::File,
        previous_override: Option<OsString>,
    }

    impl EnvTestGuard {
        fn set_override(&self, value: impl AsRef<std::ffi::OsStr>) {
            std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, value);
        }
    }

    impl Drop for EnvTestGuard {
        fn drop(&mut self) {
            match self.previous_override.as_ref() {
                Some(previous) => std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, previous),
                None => std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV),
            }
            let _ = self.lock_file.unlock();
        }
    }

    fn acquire_env_test_guard() -> EnvTestGuard {
        let in_process = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let lock_path = std::env::temp_dir().join("taurhaus-env-tests.lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap_or_else(|e| panic!("failed to open env test lock at {:?}: {e}", lock_path));
        lock_file
            .lock_exclusive()
            .unwrap_or_else(|e| panic!("failed to lock env test lock at {:?}: {e}", lock_path));
        EnvTestGuard {
            _in_process: in_process,
            lock_file,
            previous_override: std::env::var_os(CLAUDE_DIR_OVERRIDE_ENV),
        }
    }

    fn sample_state() -> MemberCompactionState {
        MemberCompactionState {
            version: 99,
            member_name: "developer1".to_string(),
            last_session_id: "session-1".to_string(),
            last_compaction_timestamp: timestamp("2026-03-08T14:30:00Z"),
            last_delivery_result: CompactionDeliveryResult::Injected,
        }
    }

    #[test]
    fn compaction_store_round_trips_state() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let state = sample_state();

        MemberCompactionStore::save(tmp.path(), "taurhaus-team", "developer1", &state)
            .expect("save state");
        let stored = MemberCompactionStore::load(tmp.path(), "taurhaus-team", "developer1")
            .expect("load state")
            .expect("state should exist");

        assert_eq!(
            stored,
            MemberCompactionState {
                version: COMPACTION_SCHEMA_VERSION,
                member_name: "developer1".to_string(),
                ..state
            }
        );
    }

    #[test]
    fn compaction_store_load_all_returns_sorted_entries() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut state = sample_state();
        MemberCompactionStore::save(tmp.path(), "taurhaus-team", "reviewer", &state)
            .expect("save reviewer state");

        state.member_name = "architect".to_string();
        state.last_session_id = "session-2".to_string();
        MemberCompactionStore::save(tmp.path(), "taurhaus-team", "architect", &state)
            .expect("save architect state");

        let entries =
            MemberCompactionStore::load_all(tmp.path(), "taurhaus-team").expect("load all");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "architect");
        assert_eq!(entries[1].0, "reviewer");
    }

    #[test]
    fn idempotency_blocks_duplicate_session_and_timestamp() {
        let state = sample_state();

        assert!(is_already_handled_state(
            &state,
            CliTool::Codex,
            "session-1",
            timestamp("2026-03-08T14:30:00Z"),
        ));
    }

    #[test]
    fn idempotency_allows_new_session() {
        let state = sample_state();

        assert!(!is_already_handled_state(
            &state,
            CliTool::Codex,
            "session-2",
            timestamp("2026-03-08T14:30:00Z"),
        ));
    }

    #[test]
    fn idempotency_allows_same_session_with_new_timestamp() {
        let state = sample_state();

        assert!(!is_already_handled_state(
            &state,
            CliTool::Codex,
            "session-1",
            timestamp("2026-03-08T14:30:01Z"),
        ));
    }

    #[test]
    fn stale_detection_trips_after_freshness_window() {
        let detected_at = timestamp("2026-03-08T14:30:00Z");

        assert!(!is_stale_compaction(
            detected_at,
            timestamp("2026-03-08T14:30:15Z"),
        ));
        assert!(is_stale_compaction(
            detected_at,
            timestamp("2026-03-08T14:30:16Z"),
        ));
    }

    #[test]
    fn stale_detection_is_not_stale_at_exact_millisecond_boundary() {
        let detected_at = timestamp("2026-03-08T14:30:00.000Z");

        assert!(!is_stale_compaction(
            detected_at,
            timestamp("2026-03-08T14:30:15.000Z"),
        ));
        assert!(is_stale_compaction(
            detected_at,
            timestamp("2026-03-08T14:30:15.001Z"),
        ));
    }

    #[test]
    fn record_delivery_persists_stale_result_to_default_store_path() {
        let guard = acquire_env_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        guard.set_override(tmp.path());

        record_delivery(
            "taurhaus-team",
            "developer1",
            CliTool::Codex,
            "session-1",
            timestamp("2026-03-08T14:30:16Z"),
            CompactionDeliveryResult::Stale,
        )
        .expect("record delivery");

        let stored =
            MemberCompactionStore::load(&tmp.path().join("teams"), "taurhaus-team", "developer1")
                .expect("load state")
                .expect("state should exist");

        assert_eq!(stored.last_delivery_result, CompactionDeliveryResult::Stale);
        assert_eq!(stored.last_session_id, "session-1");
    }

    #[test]
    fn top_level_is_already_handled_reads_saved_default_store_state() {
        let guard = acquire_env_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        guard.set_override(tmp.path());

        let state = sample_state();
        MemberCompactionStore::save(
            &tmp.path().join("teams"),
            "taurhaus-team",
            "developer1",
            &state,
        )
        .expect("save state");

        assert!(is_already_handled(
            "taurhaus-team",
            "developer1",
            CliTool::Codex,
            "session-1",
            timestamp("2026-03-08T14:30:00Z"),
        ));
        assert!(!is_already_handled(
            "taurhaus-team",
            "developer1",
            CliTool::Codex,
            "session-1",
            timestamp("2026-03-08T14:30:01Z"),
        ));
    }
}
