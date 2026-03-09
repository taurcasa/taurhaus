//! Mesh inbox file store shared with non-Claude agent delivery.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::coordination::errors::CoordinationError;
use taurhaus_lib::logging::emit_global;

const INBOXES_DIRNAME: &str = "inboxes";

/// Message entry stored in `teams/<team>/inboxes/<member>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshInboxMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub from: String,
    pub text: String,
    pub timestamp: String,
    pub read: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acked_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acked_by: Option<String>,
}

impl MeshInboxMessage {
    pub fn new(from: &str, text: String, summary: Option<String>, now: DateTime<Utc>) -> Self {
        Self {
            id: Some(uuid::Uuid::new_v4().to_string()),
            from: from.to_string(),
            text,
            timestamp: now.to_rfc3339_opts(SecondsFormat::Millis, true),
            read: false,
            summary,
            color: None,
            priority: None,
            acked_at: None,
            acked_by: None,
        }
    }
}

/// Filesystem-backed append/load helper for mesh inbox files.
#[derive(Debug, Default)]
pub struct MeshInboxStore;

impl MeshInboxStore {
    pub fn load(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
    ) -> Result<Vec<MeshInboxMessage>, CoordinationError> {
        let path = inbox_path(teams_dir, team_name, member_name);
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(CoordinationError::Io(err)),
        };

        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }

        match serde_json::from_str(&raw) {
            Ok(messages) => Ok(messages),
            Err(err) => Err(handle_corrupt_inbox_file(
                teams_dir,
                &path,
                team_name,
                member_name,
                &err.to_string(),
            )),
        }
    }

    pub fn append(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
        message: &MeshInboxMessage,
    ) -> Result<(), CoordinationError> {
        let _lock = super::lock::acquire_team_lock(teams_dir, team_name)?;

        let inbox_dir = inboxes_dir(teams_dir, team_name);
        fs::create_dir_all(&inbox_dir)?;

        let mut messages = Self::load(teams_dir, team_name, member_name)?;
        messages.push(message.clone());

        let payload = serde_json::to_string_pretty(&messages).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to serialize inbox for '{member_name}' in team '{team_name}': {err}"
            ))
        })?;

        let target_path = inbox_path(teams_dir, team_name, member_name);
        let tmp_path = inbox_tmp_path(teams_dir, team_name, member_name);
        fs::write(&tmp_path, payload)?;
        if let Err(err) = fs::rename(&tmp_path, &target_path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(CoordinationError::Io(err));
        }
        Ok(())
    }
}

fn inboxes_dir(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir.join(team_name).join(INBOXES_DIRNAME)
}

fn inbox_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    inboxes_dir(teams_dir, team_name).join(format!("{member_name}.json"))
}

fn inbox_tmp_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    inboxes_dir(teams_dir, team_name).join(format!("{member_name}.json.tmp"))
}

fn inbox_corrupt_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    inboxes_dir(teams_dir, team_name).join(format!("{member_name}.json.corrupt.{timestamp}"))
}

fn handle_corrupt_inbox_file(
    teams_dir: &Path,
    path: &Path,
    team_name: &str,
    member_name: &str,
    parse_error: &str,
) -> CoordinationError {
    let quarantine_path = inbox_corrupt_path(teams_dir, team_name, member_name);

    let quarantine_result = fs::rename(path, &quarantine_path);
    emit_inbox_corruption_event(
        team_name,
        member_name,
        path,
        quarantine_result
            .as_ref()
            .ok()
            .map(|_| quarantine_path.as_path()),
        parse_error,
        quarantine_result
            .as_ref()
            .err()
            .map(|error| error.to_string()),
    );

    match quarantine_result {
        Ok(()) => CoordinationError::StoreError(format!(
            "mesh inbox for '{member_name}' in team '{team_name}' is corrupt at '{}'; quarantined to '{}': {parse_error}",
            path.display(),
            quarantine_path.display(),
        )),
        Err(rename_error) => CoordinationError::StoreError(format!(
            "mesh inbox for '{member_name}' in team '{team_name}' is corrupt at '{}': {parse_error}; quarantine failed: {rename_error}",
            path.display(),
        )),
    }
}

fn emit_inbox_corruption_event(
    team_name: &str,
    member_name: &str,
    path: &Path,
    quarantine_path: Option<&Path>,
    parse_error: &str,
    quarantine_error: Option<String>,
) {
    let mut fields = Map::new();
    fields.insert(
        "team_name".to_string(),
        Value::String(team_name.to_string()),
    );
    fields.insert(
        "member_name".to_string(),
        Value::String(member_name.to_string()),
    );
    fields.insert(
        "path".to_string(),
        Value::String(path.display().to_string()),
    );
    fields.insert(
        "error.message".to_string(),
        Value::String(parse_error.to_string()),
    );
    if let Some(quarantine_path) = quarantine_path {
        fields.insert(
            "quarantine_path".to_string(),
            Value::String(quarantine_path.display().to_string()),
        );
    }
    if let Some(quarantine_error) = quarantine_error {
        fields.insert(
            "quarantine_error".to_string(),
            Value::String(quarantine_error),
        );
    }
    emit_global(
        "warn",
        "coordination",
        "mesh.inbox.corrupt",
        Some("Mesh inbox file is corrupt".to_string()),
        fields,
    );
}

#[cfg(test)]
mod tests {
    use std::sync::{LazyLock, Mutex};

    use tempfile::TempDir;

    use super::*;
    use taurhaus_lib::logging::{install_global_sink, LogFileState};

    static LOG_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn append_and_load_round_trip() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let message = MeshInboxMessage::new(
            "taurhaus",
            "{\"kind\":\"post_compaction_context\"}".to_string(),
            Some("post_compaction_context".to_string()),
            DateTime::parse_from_rfc3339("2026-03-08T19:00:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        MeshInboxStore::append(&teams_dir, "t", "agent", &message).expect("append inbox");
        let loaded = MeshInboxStore::load(&teams_dir, "t", "agent").expect("load inbox");

        assert_eq!(loaded, vec![message]);
    }

    #[test]
    fn append_creates_inboxes_directory_lazily() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let message = MeshInboxMessage::new(
            "taurhaus",
            "payload".to_string(),
            None,
            DateTime::parse_from_rfc3339("2026-03-08T19:00:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        MeshInboxStore::append(&teams_dir, "t", "agent", &message).expect("append inbox");

        assert!(teams_dir
            .join("t")
            .join("inboxes")
            .join("agent.json")
            .exists());
    }

    #[test]
    fn load_returns_empty_when_inbox_missing() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let loaded = MeshInboxStore::load(&teams_dir, "t", "missing").expect("load inbox");
        assert!(loaded.is_empty());
    }

    #[test]
    fn load_returns_error_and_quarantines_corrupt_inbox() {
        let _log_guard = LOG_LOCK.lock().expect("log lock");
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let inbox_dir = teams_dir.join("t").join("inboxes");
        fs::create_dir_all(&inbox_dir).expect("inbox dir");
        let inbox_path = inbox_dir.join("agent.json");
        fs::write(&inbox_path, "{bad json").expect("write corrupt inbox");

        let log_path = tmp.path().join("inbox.log.jsonl");
        let log_state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&log_state);

        let error = MeshInboxStore::load(&teams_dir, "t", "agent").expect_err("corrupt inbox");
        assert!(error.to_string().contains("is corrupt"));
        assert!(
            !inbox_path.exists(),
            "corrupt inbox should be quarantined away"
        );

        let quarantine_files = inbox_dir
            .read_dir()
            .expect("read quarantine dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("agent.json.corrupt."))
            })
            .collect::<Vec<_>>();
        assert_eq!(quarantine_files.len(), 1, "one quarantined inbox expected");
        assert_eq!(
            fs::read_to_string(&quarantine_files[0]).expect("read quarantine file"),
            "{bad json"
        );

        let contents = wait_for_log_contains(&log_path, "\"event\":\"mesh.inbox.corrupt\"");
        assert!(contents.contains("\"team_name\":\"t\""));
        assert!(contents.contains("\"member_name\":\"agent\""));
    }

    #[test]
    fn append_fails_closed_when_existing_inbox_is_corrupt() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let inbox_dir = teams_dir.join("t").join("inboxes");
        fs::create_dir_all(&inbox_dir).expect("inbox dir");
        let inbox_path = inbox_dir.join("agent.json");
        fs::write(&inbox_path, "{bad json").expect("write corrupt inbox");

        let message = MeshInboxMessage::new(
            "taurhaus",
            "payload".to_string(),
            None,
            DateTime::parse_from_rfc3339("2026-03-09T00:05:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        let error = MeshInboxStore::append(&teams_dir, "t", "agent", &message)
            .expect_err("append should fail on corrupt inbox");
        assert!(error.to_string().contains("is corrupt"));
        assert!(
            !inbox_path.exists(),
            "append must not recreate the inbox after quarantining corruption"
        );
        let quarantine_files = inbox_dir
            .read_dir()
            .expect("read quarantine dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("agent.json.corrupt."))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            quarantine_files.len(),
            1,
            "quarantined inbox should be preserved"
        );
    }

    fn wait_for_log_contains(path: &Path, needle: &str) -> String {
        for _ in 0..50 {
            if let Ok(contents) = fs::read_to_string(path) {
                if contents.contains(needle) {
                    return contents;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        fs::read_to_string(path).unwrap_or_default()
    }
}
