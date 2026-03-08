//! Mesh inbox file store shared with non-Claude agent delivery.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::coordination::errors::CoordinationError;

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
            Err(err) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "mesh inbox file was corrupt; treating as empty during load"
                );
                Ok(Vec::new())
            }
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

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

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
}
