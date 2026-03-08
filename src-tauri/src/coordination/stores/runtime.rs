//! Member runtime state store.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::coordination::domain::{DeliveryLease, HealthState};
use crate::coordination::errors::CoordinationError;
use crate::session_scanner::cli_tool::CliTool;

const RUNTIME_DIRNAME: &str = "runtime";
const RUNTIME_SCHEMA_VERSION: u32 = 3;

/// Runtime record persisted at `teams/<team>/runtime/<member>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRuntimeRecord {
    pub schema_version: u32,
    pub member_name: String,
    pub cli_tool: Option<CliTool>,
    pub project_path: Option<PathBuf>,
    pub pane_id: Option<String>,
    pub session_id: Option<String>,
    pub jsonl_path: Option<PathBuf>,
    pub daemon_pid: Option<u32>,
    pub health: HealthState,
    pub delivery_lease: Option<DeliveryLease>,
    pub attached_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// Stateless filesystem-backed store for member runtime documents.
#[derive(Debug, Default)]
pub struct MemberRuntimeStore;

impl MemberRuntimeStore {
    /// Load runtime state from `<teams_dir>/<team_name>/runtime/<member_name>.json`.
    pub fn load(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
    ) -> Result<MemberRuntimeRecord, CoordinationError> {
        let path = runtime_record_path(teams_dir, team_name, member_name);
        let raw = fs::read_to_string(&path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => CoordinationError::NotFound(format!(
                "runtime state not found for member '{member_name}' in team '{team_name}'"
            )),
            _ => CoordinationError::Io(err),
        })?;

        parse_runtime_record(&raw, team_name, member_name)
    }

    /// Save runtime state atomically via advisory lock + `<member>.json.tmp` + rename.
    pub fn save(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
        record: &MemberRuntimeRecord,
    ) -> Result<(), CoordinationError> {
        let _lock = super::lock::acquire_team_lock(teams_dir, team_name)?;

        let mut normalized = record.clone();
        normalized.schema_version = RUNTIME_SCHEMA_VERSION;
        normalized.member_name = member_name.to_string();

        let runtime_dir = runtime_dir_path(teams_dir, team_name);
        fs::create_dir_all(&runtime_dir)?;

        let target_path = runtime_record_path(teams_dir, team_name, member_name);
        let tmp_path = runtime_tmp_path(teams_dir, team_name, member_name);
        let payload = serde_json::to_string_pretty(&normalized).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to serialize runtime record for '{member_name}': {err}"
            ))
        })?;

        fs::write(&tmp_path, payload)?;
        if let Err(err) = fs::rename(&tmp_path, &target_path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(CoordinationError::Io(err));
        }

        Ok(())
    }

    /// List runtime member names from `<teams_dir>/<team_name>/runtime/*.json`.
    pub fn list(teams_dir: &Path, team_name: &str) -> Result<Vec<String>, CoordinationError> {
        let runtime_dir = runtime_dir_path(teams_dir, team_name);
        if !runtime_dir.exists() {
            return Ok(Vec::new());
        }

        let mut members = Vec::new();
        for entry in fs::read_dir(runtime_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                members.push(name.to_string());
            }
        }

        members.sort();
        Ok(members)
    }

    /// Load all runtime records for a team. Skips corrupt files.
    pub fn load_all(
        teams_dir: &Path,
        team_name: &str,
    ) -> Result<Vec<(String, MemberRuntimeRecord)>, CoordinationError> {
        let runtime_dir = runtime_dir_path(teams_dir, team_name);
        if !runtime_dir.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for entry in fs::read_dir(runtime_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let member_name = match path.file_stem().and_then(|s| s.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };
            let raw = match fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(_) => continue,
            };
            match parse_runtime_record(&raw, team_name, &member_name) {
                Ok(record) => results.push((member_name, record)),
                Err(_) => continue, // skip corrupt
            }
        }

        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }

    /// Delete a member runtime record if present.
    pub fn delete(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
    ) -> Result<(), CoordinationError> {
        let path = runtime_record_path(teams_dir, team_name, member_name);
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(CoordinationError::Io(err)),
        }
    }

    /// Remove stale runtime entries based on TTL and explicit `now` timestamp.
    ///
    /// Returns the member names that were pruned.
    pub fn cleanup_stale(
        teams_dir: &Path,
        team_name: &str,
        ttl: Duration,
        now: DateTime<Utc>,
    ) -> Result<Vec<String>, CoordinationError> {
        let runtime_dir = runtime_dir_path(teams_dir, team_name);
        if !runtime_dir.exists() {
            return Ok(Vec::new());
        }

        let ttl = chrono::Duration::from_std(ttl).map_err(|err| {
            CoordinationError::Validation(format!("invalid stale-cleanup TTL duration: {err}"))
        })?;
        let cutoff = now - ttl;

        let mut removed = Vec::new();
        for entry in fs::read_dir(runtime_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let member_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let should_remove = match fs::read_to_string(&path) {
                Ok(raw) => match parse_runtime_record(&raw, team_name, &member_name) {
                    Ok(record) => is_stale(&record, cutoff),
                    Err(_) => true,
                },
                Err(err) => return Err(CoordinationError::Io(err)),
            };

            if should_remove {
                fs::remove_file(&path)?;
                removed.push(member_name);
            }
        }

        removed.sort();
        Ok(removed)
    }
}

fn parse_runtime_record(
    raw: &str,
    team_name: &str,
    member_name: &str,
) -> Result<MemberRuntimeRecord, CoordinationError> {
    #[derive(Debug, Deserialize)]
    struct RuntimeRecordWire {
        #[serde(default = "schema_version_one")]
        schema_version: u32,
        #[serde(default)]
        member_name: Option<String>,
        #[serde(default, alias = "cliTool")]
        cli_tool: Option<CliTool>,
        #[serde(default, alias = "projectPath", alias = "cwd")]
        project_path: Option<PathBuf>,
        #[serde(default, alias = "paneId")]
        pane_id: Option<String>,
        #[serde(default, alias = "sessionId")]
        session_id: Option<String>,
        #[serde(default, alias = "jsonlPath")]
        jsonl_path: Option<PathBuf>,
        #[serde(default, alias = "daemonPid")]
        daemon_pid: Option<u32>,
        health: HealthState,
        #[serde(default)]
        delivery_lease: Option<DeliveryLease>,
        #[serde(default)]
        attached_at: Option<DateTime<Utc>>,
        #[serde(default)]
        last_seen_at: Option<DateTime<Utc>>,
    }

    let wire: RuntimeRecordWire = serde_json::from_str(raw).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to parse runtime/{member_name}.json for team '{team_name}': {err}"
        ))
    })?;

    Ok(MemberRuntimeRecord {
        schema_version: wire.schema_version,
        member_name: wire.member_name.unwrap_or_else(|| member_name.to_string()),
        cli_tool: wire.cli_tool,
        project_path: wire.project_path,
        pane_id: wire.pane_id,
        session_id: wire.session_id,
        jsonl_path: wire.jsonl_path,
        daemon_pid: wire.daemon_pid,
        health: wire.health,
        delivery_lease: wire.delivery_lease,
        attached_at: wire.attached_at,
        last_seen_at: wire.last_seen_at,
    })
}

fn is_stale(record: &MemberRuntimeRecord, cutoff: DateTime<Utc>) -> bool {
    latest_activity(record).is_none_or(|ts| ts <= cutoff)
}

fn latest_activity(record: &MemberRuntimeRecord) -> Option<DateTime<Utc>> {
    let mut latest = record.last_seen_at;
    if let Some(attached_at) = record.attached_at {
        latest = Some(latest.map_or(attached_at, |ts| ts.max(attached_at)));
    }
    if let Some(lease) = &record.delivery_lease {
        latest = Some(latest.map_or(lease.heartbeat_at, |ts| ts.max(lease.heartbeat_at)));
    }
    latest
}

fn team_dir(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir.join(team_name)
}

fn runtime_dir_path(teams_dir: &Path, team_name: &str) -> PathBuf {
    team_dir(teams_dir, team_name).join(RUNTIME_DIRNAME)
}

fn runtime_record_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    runtime_dir_path(teams_dir, team_name).join(format!("{member_name}.json"))
}

fn runtime_tmp_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    runtime_dir_path(teams_dir, team_name).join(format!("{member_name}.json.tmp"))
}

const fn schema_version_one() -> u32 {
    RUNTIME_SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::TimeZone;
    use tempfile::TempDir;

    use super::*;

    fn ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid RFC3339 timestamp")
            .with_timezone(&Utc)
    }

    fn sample_record(member_name: &str) -> MemberRuntimeRecord {
        MemberRuntimeRecord {
            schema_version: 3,
            member_name: member_name.to_string(),
            cli_tool: Some(CliTool::Codex),
            project_path: Some(PathBuf::from("/tmp/taurhaus")),
            pane_id: Some("%12".to_string()),
            session_id: Some("session-123".to_string()),
            jsonl_path: Some(PathBuf::from("/tmp/taurhaus/.codex/session.jsonl")),
            daemon_pid: Some(4242),
            health: HealthState::Healthy,
            delivery_lease: Some(DeliveryLease {
                owner_pid: 4242,
                instance_uuid: "instance-1".to_string(),
                hostname: "devbox".to_string(),
                heartbeat_at: ts("2026-03-01T21:05:00Z"),
                started_at: ts("2026-03-01T21:00:00Z"),
            }),
            attached_at: Some(ts("2026-03-01T21:00:10Z")),
            last_seen_at: Some(ts("2026-03-01T21:05:10Z")),
        }
    }

    #[test]
    fn save_then_load_round_trip() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";
        let record = sample_record(member_name);

        MemberRuntimeStore::save(teams_dir, team_name, member_name, &record)
            .expect("save should succeed");
        let loaded = MemberRuntimeStore::load(teams_dir, team_name, member_name)
            .expect("load should succeed");

        assert_eq!(loaded, record);
    }

    #[test]
    fn load_legacy_runtime_without_metadata_defaults_new_fields_to_none() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let member_name = "legacy-agent";
        let runtime_dir = teams_dir.join(team_name).join("runtime");
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        fs::write(
            runtime_dir.join(format!("{member_name}.json")),
            r#"{
  "schema_version": 1,
  "member_name": "legacy-agent",
  "pane_id": "%7",
  "session_id": "session-123",
  "daemon_pid": 1234,
  "health": "healthy",
  "delivery_lease": null,
  "attached_at": null,
  "last_seen_at": null
}"#,
        )
        .expect("legacy runtime");

        let loaded =
            MemberRuntimeStore::load(teams_dir, team_name, member_name).expect("load runtime");

        assert_eq!(loaded.cli_tool, None);
        assert_eq!(loaded.project_path, None);
        assert_eq!(loaded.session_id.as_deref(), Some("session-123"));
        assert_eq!(loaded.jsonl_path, None);
    }

    #[test]
    fn stale_logic_uses_latest_activity_timestamp() {
        let mut record = sample_record("agent-1");
        record.delivery_lease.as_mut().expect("lease").heartbeat_at = ts("2026-03-01T21:07:00Z");
        record.last_seen_at = Some(ts("2026-03-01T21:06:30Z"));

        let cutoff = ts("2026-03-01T21:06:00Z");
        assert!(
            !is_stale(&record, cutoff),
            "fresh heartbeat should keep record"
        );

        let cutoff_late = ts("2026-03-01T21:08:00Z");
        assert!(
            is_stale(&record, cutoff_late),
            "record should become stale after cutoff"
        );
    }

    #[test]
    fn cleanup_stale_prunes_old_records() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";

        let stale = MemberRuntimeRecord {
            last_seen_at: Some(ts("2026-03-01T20:00:00Z")),
            attached_at: Some(ts("2026-03-01T20:00:00Z")),
            delivery_lease: None,
            ..sample_record("stale-agent")
        };
        let fresh = MemberRuntimeRecord {
            last_seen_at: Some(ts("2026-03-01T21:05:00Z")),
            attached_at: Some(ts("2026-03-01T21:05:00Z")),
            delivery_lease: None,
            ..sample_record("fresh-agent")
        };

        MemberRuntimeStore::save(teams_dir, team_name, "stale-agent", &stale).expect("save stale");
        MemberRuntimeStore::save(teams_dir, team_name, "fresh-agent", &fresh).expect("save fresh");

        let removed = MemberRuntimeStore::cleanup_stale(
            teams_dir,
            team_name,
            Duration::from_secs(60 * 30),
            ts("2026-03-01T21:10:00Z"),
        )
        .expect("cleanup should succeed");

        assert_eq!(removed, vec!["stale-agent".to_string()]);
        assert!(
            !runtime_record_path(teams_dir, team_name, "stale-agent").exists(),
            "stale file should be removed"
        );
        assert!(
            runtime_record_path(teams_dir, team_name, "fresh-agent").exists(),
            "fresh file should remain"
        );
    }

    #[test]
    fn cleanup_stale_removes_corrupt_runtime_file() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let runtime_dir = runtime_dir_path(teams_dir, team_name);
        fs::create_dir_all(&runtime_dir).expect("create runtime dir");
        fs::write(runtime_dir.join("broken.json"), "{not-json").expect("write broken json");

        let removed = MemberRuntimeStore::cleanup_stale(
            teams_dir,
            team_name,
            Duration::from_secs(60),
            Utc.timestamp_opt(0, 0).single().expect("epoch"),
        )
        .expect("cleanup should succeed");

        assert_eq!(removed, vec!["broken".to_string()]);
        assert!(
            !runtime_dir.join("broken.json").exists(),
            "broken runtime file should be pruned during cleanup"
        );
    }

    #[test]
    fn delete_missing_member_is_ok() {
        let tmp = TempDir::new().expect("tempdir");
        MemberRuntimeStore::delete(tmp.path(), "nonexistent-team", "ghost-agent")
            .expect("deleting missing member should not error");
    }

    #[test]
    fn cleanup_stale_treats_none_last_seen_as_stale() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";

        let no_timestamps = MemberRuntimeRecord {
            schema_version: 3,
            member_name: "no-heartbeat".to_string(),
            cli_tool: None,
            project_path: None,
            pane_id: None,
            session_id: None,
            jsonl_path: None,
            daemon_pid: None,
            health: HealthState::SessionDead,
            delivery_lease: None,
            attached_at: None,
            last_seen_at: None,
        };

        MemberRuntimeStore::save(teams_dir, team_name, "no-heartbeat", &no_timestamps)
            .expect("save");

        let removed = MemberRuntimeStore::cleanup_stale(
            teams_dir,
            team_name,
            Duration::from_secs(60),
            ts("2026-03-01T21:10:00Z"),
        )
        .expect("cleanup should succeed");

        assert_eq!(removed, vec!["no-heartbeat".to_string()]);
    }

    #[test]
    fn duplicate_save_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";
        let record = sample_record(member_name);

        MemberRuntimeStore::save(teams_dir, team_name, member_name, &record).expect("first save");
        MemberRuntimeStore::save(teams_dir, team_name, member_name, &record)
            .expect("second save should succeed");

        let loaded = MemberRuntimeStore::load(teams_dir, team_name, member_name)
            .expect("load after double save");
        assert_eq!(loaded, record);
    }

    #[test]
    fn duplicate_delete_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";

        MemberRuntimeStore::save(
            teams_dir,
            team_name,
            member_name,
            &sample_record(member_name),
        )
        .expect("save");
        MemberRuntimeStore::delete(teams_dir, team_name, member_name).expect("first delete");
        MemberRuntimeStore::delete(teams_dir, team_name, member_name)
            .expect("second delete should not error");
    }

    #[test]
    fn concurrent_runtime_saves_are_safe() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = Arc::new(tmp.path().to_path_buf());
        let team_name = "concurrent-team";
        let member_name = "contested-agent";
        let barrier = Arc::new(Barrier::new(8));

        // Pre-create with initial save.
        MemberRuntimeStore::save(
            &teams_dir,
            team_name,
            member_name,
            &sample_record(member_name),
        )
        .expect("initial save");

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let dir = Arc::clone(&teams_dir);
                let bar = Arc::clone(&barrier);
                let tname = team_name.to_string();
                let mname = member_name.to_string();
                thread::spawn(move || {
                    bar.wait();
                    let mut record = sample_record(&mname);
                    record.pane_id = Some(format!("%{i}"));
                    MemberRuntimeStore::save(&dir, &tname, &mname, &record)
                        .expect("concurrent save");
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread should not panic");
        }

        // Final state should be a valid record from one of the threads.
        let loaded = MemberRuntimeStore::load(&teams_dir, team_name, member_name)
            .expect("load after concurrent saves");
        assert_eq!(loaded.member_name, member_name);
        assert!(
            loaded.pane_id.as_ref().unwrap().starts_with('%'),
            "pane_id should be from one of the concurrent writers"
        );
    }

    #[test]
    fn concurrent_save_and_load_no_corruption() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = Arc::new(tmp.path().to_path_buf());
        let team_name = "mixed-team";
        let member_name = "mixed-agent";
        let barrier = Arc::new(Barrier::new(6));

        MemberRuntimeStore::save(
            &teams_dir,
            team_name,
            member_name,
            &sample_record(member_name),
        )
        .expect("initial save");

        let mut handles = Vec::new();

        // 3 writers
        for i in 0..3 {
            let dir = Arc::clone(&teams_dir);
            let bar = Arc::clone(&barrier);
            let tname = team_name.to_string();
            let mname = member_name.to_string();
            handles.push(thread::spawn(move || {
                bar.wait();
                let mut record = sample_record(&mname);
                record.pane_id = Some(format!("%{}", 10 + i));
                MemberRuntimeStore::save(&dir, &tname, &mname, &record)
                    .expect("writer should succeed");
            }));
        }

        // 3 readers
        for _ in 0..3 {
            let dir = Arc::clone(&teams_dir);
            let bar = Arc::clone(&barrier);
            let tname = team_name.to_string();
            let mname = member_name.to_string();
            handles.push(thread::spawn(move || {
                bar.wait();
                // Load may see any valid state, but should never get corrupt JSON.
                let result = MemberRuntimeStore::load(&dir, &tname, &mname);
                if let Ok(record) = result {
                    assert_eq!(record.member_name, mname);
                }
            }));
        }

        for h in handles {
            h.join().expect("thread should not panic");
        }
    }

    #[test]
    fn load_all_skips_corrupt_files() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";

        let valid = sample_record("valid-agent");
        MemberRuntimeStore::save(teams_dir, team_name, "valid-agent", &valid).expect("save");

        let runtime_dir = runtime_dir_path(teams_dir, team_name);
        fs::write(runtime_dir.join("corrupt-agent.json"), "{{bad json").expect("write corrupt");

        let results =
            MemberRuntimeStore::load_all(teams_dir, team_name).expect("load_all should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "valid-agent");
        assert_eq!(results[0].1, valid);
    }

    #[test]
    fn load_missing_runtime_returns_not_found() {
        let tmp = TempDir::new().expect("tempdir");
        let err = MemberRuntimeStore::load(tmp.path(), "architecture-final", "ghost")
            .expect_err("missing runtime");
        match err {
            CoordinationError::NotFound(message) => assert!(message.contains("ghost")),
            other => panic!("expected not found, got {other:?}"),
        }
    }

    #[test]
    fn list_errors_when_runtime_path_is_file() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "architecture-final";
        let runtime_path = runtime_dir_path(tmp.path(), team_name);
        fs::create_dir_all(team_dir(tmp.path(), team_name)).expect("create team dir");
        fs::write(&runtime_path, "not a dir").expect("write file");

        let err = MemberRuntimeStore::list(tmp.path(), team_name).expect_err("path is file");
        match err {
            CoordinationError::Io(io) => {
                assert!(
                    io.kind() == std::io::ErrorKind::NotADirectory
                        || io.kind() == std::io::ErrorKind::Other
                );
            }
            other => panic!("expected io error, got {other:?}"),
        }
    }
}
