//! Mesh inbox file store shared with non-Claude agent delivery.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::coordination::errors::CoordinationError;
use taurhaus_lib::logging::emit_global;

const INBOXES_DIRNAME: &str = "inboxes";
pub const OPERATOR_SENDER_NAME: &str = "taurhaus";

/// Message entry stored in `teams/<team>/inboxes/<member>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshInboxMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub from: String,
    pub text: String,
    pub timestamp: String,
    #[serde(default)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_relay: Option<Value>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
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
            external_relay: None,
            extra: BTreeMap::new(),
        }
    }

    pub fn operator_originated(
        recipient_name: &str,
        text: String,
        summary: Option<String>,
        now: DateTime<Utc>,
        sender_name: Option<&str>,
    ) -> Self {
        let sender_name = sender_name
            .map(str::trim)
            .filter(|sender| !sender.is_empty() && *sender != recipient_name)
            .unwrap_or(OPERATOR_SENDER_NAME);
        Self::new(sender_name, text, summary, now)
    }

    fn remove_authored_keys_from_extra(&mut self) {
        for key in [
            "id",
            "from",
            "text",
            "timestamp",
            "read",
            "summary",
            "color",
            "priority",
            "ackedAt",
            "ackedBy",
            "externalRelay",
        ] {
            self.extra.remove(key);
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
        let raw = match super::lock::read_to_string_with_retry(&path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // A crash between a swap's two renames leaves the record only
                // at its displaced sibling; reads must still see it.
                match super::lock::read_to_string_with_retry(&super::lock::displaced_path(&path)) {
                    Ok(raw) => raw,
                    Err(_) => return Ok(Vec::new()),
                }
            }
            Err(err) => return Err(CoordinationError::Io(err)),
        };

        parse_inbox_tolerating_torn_reads(teams_dir, &path, team_name, member_name, raw, || {
            match super::lock::read_to_string_with_retry(&path) {
                Ok(raw) => Ok(raw),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
                Err(err) => Err(CoordinationError::Io(err)),
            }
        })
    }

    pub fn append(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
        message: &MeshInboxMessage,
    ) -> Result<(), CoordinationError> {
        let inbox_dir = inboxes_dir(teams_dir, team_name);
        fs::create_dir_all(&inbox_dir)?;

        let target_path = inbox_path(teams_dir, team_name, member_name);
        let target_lock = super::lock::TargetFileLock::acquire_or_create(&target_path)?;
        let raw = target_lock.read_contents()?;
        let mut messages = parse_inbox_tolerating_torn_reads(
            teams_dir,
            &target_path,
            team_name,
            member_name,
            raw,
            || target_lock.read_contents(),
        )?;
        let mut message = message.clone();
        message.remove_authored_keys_from_extra();
        for existing in &mut messages {
            existing.remove_authored_keys_from_extra();
        }
        messages.push(message);

        let payload = serde_json::to_string_pretty(&messages).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to serialize inbox for '{member_name}' in team '{team_name}': {err}"
            ))
        })?;

        let tmp_path = inbox_tmp_path(teams_dir, team_name, member_name);
        let mut tmp_file = fs::File::create(&tmp_path)?;
        tmp_file.write_all(payload.as_bytes())?;
        tmp_file.sync_all()?;
        if let Err(err) = fs::rename(&tmp_path, &target_path) {
            // Regression: initializing a team from the Windows app failed at
            // "Sending agent instructions" with os error 5 — the 9p server
            // behind the WSL-resolved teams dir refuses to rename over the
            // file our own target lock holds open. Every sibling store
            // (config, runtime, operational, mesh_task) already degrades to
            // a direct write on these volumes; the inbox was the one store
            // without the fallback.
            if super::lock::is_windows_unsupported_rename_error(&err) {
                tracing::warn!(
                    team_name,
                    member_name,
                    target = %target_path.display(),
                    raw_os_error = ?err.raw_os_error(),
                    "atomic inbox rename failed; falling back to direct write"
                );
                super::lock::report_atomic_write_degraded(
                    &target_path,
                    "inbox",
                    err.raw_os_error(),
                );
                if let Err(write_err) = super::lock::replace_via_move_aside(&tmp_path, &target_path)
                {
                    let _ = fs::remove_file(&tmp_path);
                    return Err(CoordinationError::Io(write_err));
                }
                return Ok(());
            }
            let _ = fs::remove_file(&tmp_path);
            return Err(CoordinationError::Io(err));
        }
        Ok(())
    }
}

/// Re-read a persistently unparsable inbox with backoff before letting the
/// destructive quarantine fire: on a degraded volume a writer from an older
/// build can still expose a torn state mid-write, and a torn transient must
/// never cost the unread messages.
fn parse_inbox_tolerating_torn_reads(
    teams_dir: &Path,
    path: &Path,
    team_name: &str,
    member_name: &str,
    mut raw: String,
    reread: impl Fn() -> Result<String, CoordinationError>,
) -> Result<Vec<MeshInboxMessage>, CoordinationError> {
    for backoff in super::lock::READ_RETRY_BACKOFFS {
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        if let Ok(messages) = serde_json::from_str::<Vec<MeshInboxMessage>>(&raw) {
            return Ok(messages);
        }
        thread::sleep(backoff);
        raw = reread()?;
    }
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    parse_inbox_contents(teams_dir, path, team_name, member_name, &raw)
}

fn parse_inbox_contents(
    teams_dir: &Path,
    path: &Path,
    team_name: &str,
    member_name: &str,
    raw: &str,
) -> Result<Vec<MeshInboxMessage>, CoordinationError> {
    serde_json::from_str(raw).map_err(|err| {
        handle_corrupt_inbox_file(teams_dir, path, team_name, member_name, &err.to_string())
    })
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
    use std::time::Duration;
    use tempfile::TempDir;

    use super::*;
    use taurhaus_lib::logging::{install_global_sink, LogFileState};

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
    fn mesh_message_without_read_defaults_to_unread() {
        // Regression: 2b69b9cd made taurhaus's `read` field mandatory, so a
        // mesh-written message without it quarantined the entire live inbox.
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let inbox_dir = teams_dir.join("t").join("inboxes");
        fs::create_dir_all(&inbox_dir).expect("inbox dir");
        fs::write(
            inbox_dir.join("agent.json"),
            r#"[{
  "from": "team-lead",
  "text": "new assignment",
  "timestamp": "2026-03-08T19:00:00.000Z"
}]"#,
        )
        .expect("write mesh inbox");

        let messages =
            MeshInboxStore::load(&teams_dir, "t", "agent").expect("mesh inbox should parse");

        assert_eq!(messages.len(), 1);
        assert!(!messages[0].read);
    }

    #[test]
    fn append_preserves_external_relay_and_unknown_message_fields() {
        // Regression: mesh-findings P11; taurhaus re-serialized the inbox array
        // through a closed struct and erased mesh's externalRelay metadata.
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let inbox_dir = teams_dir.join("t").join("inboxes");
        fs::create_dir_all(&inbox_dir).expect("inbox dir");
        let path = inbox_dir.join("agent.json");
        fs::write(
            &path,
            r#"[{
  "id": "relay-1",
  "from": "remote-lead",
  "text": "cross-team update",
  "timestamp": "2026-03-08T19:00:00.000Z",
  "read": false,
  "externalRelay": {
    "sourceTeam": "remote-team",
    "sourceSender": "remote-lead",
    "crossTeamMessageId": "xteam-1",
    "transport": "filesystem"
  },
  "futureMessageField": { "preserve": true }
}]"#,
        )
        .expect("write relay inbox");
        let appended = MeshInboxMessage::new(
            "taurhaus",
            "operator update".to_string(),
            Some("operator_notice".to_string()),
            DateTime::parse_from_rfc3339("2026-03-08T19:01:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        MeshInboxStore::append(&teams_dir, "t", "agent", &appended).expect("append inbox");

        let value: Value = serde_json::from_str(&fs::read_to_string(path).expect("read inbox"))
            .expect("parse inbox");
        assert_eq!(
            value[0]["externalRelay"],
            serde_json::json!({
                "sourceTeam": "remote-team",
                "sourceSender": "remote-lead",
                "crossTeamMessageId": "xteam-1",
                "transport": "filesystem"
            })
        );
        assert_eq!(
            value[0]["futureMessageField"],
            serde_json::json!({ "preserve": true })
        );
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
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
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
    fn a_torn_read_heals_before_the_inbox_is_quarantined() {
        // Regression: the direct-write fallback on lock-degraded volumes can
        // expose a torn state to a concurrent reader, and the inbox's
        // corruption handling is destructive (quarantine). A read that heals
        // within the backoff window must deliver the messages, not destroy
        // them.
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let inbox_dir = teams_dir.join("t").join("inboxes");
        fs::create_dir_all(&inbox_dir).expect("inbox dir");
        let inbox_path = inbox_dir.join("agent.json");
        fs::write(&inbox_path, "[{\"torn").expect("write torn inbox");

        let healed = serde_json::to_string(&vec![MeshInboxMessage::new(
            "taurhaus",
            "delivered".to_string(),
            None,
            DateTime::parse_from_rfc3339("2026-03-09T00:05:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        )])
        .expect("serialize healed inbox");
        let heal_path = inbox_path.clone();
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            fs::write(&heal_path, healed).expect("heal inbox");
        });

        let messages = MeshInboxStore::load(&teams_dir, "t", "agent").expect("torn read heals");
        writer.join().expect("writer thread");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "delivered");
        assert!(
            inbox_path.exists(),
            "a healed inbox must never be quarantined"
        );
    }

    // The production parse path is `append` (delivery and the compaction
    // hook), not `load`: the same healing must hold when the reread goes
    // through the still-held target lock.
    #[test]
    fn append_heals_a_torn_read_and_preserves_existing_messages() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let inbox_dir = teams_dir.join("t").join("inboxes");
        fs::create_dir_all(&inbox_dir).expect("inbox dir");
        let inbox_path = inbox_dir.join("agent.json");
        fs::write(&inbox_path, "[{\"torn").expect("write torn inbox");

        let healed = serde_json::to_string(&vec![MeshInboxMessage::new(
            "taurhaus",
            "first".to_string(),
            None,
            DateTime::parse_from_rfc3339("2026-03-09T00:05:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        )])
        .expect("serialize healed inbox");
        let heal_path = inbox_path.clone();
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            fs::write(&heal_path, healed).expect("heal inbox");
        });

        let second = MeshInboxMessage::new(
            "taurhaus",
            "second".to_string(),
            None,
            DateTime::parse_from_rfc3339("2026-03-09T00:06:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );
        MeshInboxStore::append(&teams_dir, "t", "agent", &second).expect("append heals");
        writer.join().expect("writer thread");

        let messages = MeshInboxStore::load(&teams_dir, "t", "agent").expect("load");
        assert_eq!(
            messages.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(),
            vec!["first", "second"],
            "the healed message survives and the append lands after it"
        );
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
