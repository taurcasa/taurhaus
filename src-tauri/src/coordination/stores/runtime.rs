//! Member runtime state store.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use taurhaus_lib::logging::emit_global;

use super::compaction::prune_state_if_session_mismatch;
use super::config::extension_fields_only;
use crate::coordination::domain::{DeliveryLease, HealthState};
use crate::coordination::errors::CoordinationError;
use crate::session_scanner::cli_tool::CliTool;

const RUNTIME_DIRNAME: &str = "runtime";
const RUNTIME_SCHEMA_VERSION: u32 = 3;
static RUNTIME_RECORD_SKIP_REASONS: OnceLock<Mutex<HashMap<(String, String), String>>> =
    OnceLock::new();
const SAVE_RETRY_BACKOFFS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(500),
];

/// Runtime record persisted at `teams/<team>/runtime/<member>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRuntimeRecord {
    #[serde(default = "schema_version_one")]
    pub schema_version: u32,
    #[serde(default)]
    pub member_name: String,
    pub cli_tool: Option<CliTool>,
    pub project_path: Option<PathBuf>,
    pub pane_id: Option<String>,
    #[serde(default)]
    pub pane_pid: Option<u32>,
    #[serde(default)]
    pub pane_start_time: Option<u64>,
    pub session_id: Option<String>,
    pub jsonl_path: Option<PathBuf>,
    pub daemon_pid: Option<u32>,
    #[serde(
        default = "default_runtime_health",
        deserialize_with = "deserialize_runtime_health"
    )]
    pub health: HealthState,
    #[serde(default, alias = "deliveryLease")]
    pub delivery_lease: Option<DeliveryLease>,
    #[serde(default, alias = "attachedAt")]
    pub attached_at: Option<DateTime<Utc>>,
    #[serde(default, alias = "lastSeenAt")]
    pub last_seen_at: Option<DateTime<Utc>>,
    /// Reasoning effort currently in force for the running session.
    ///
    /// Shared with mesh, which reads and writes it under `appliedEffort`
    /// before it types `/effort` into the pane, so the key spelling is part of
    /// the contract. Seeded by the launch effort so a member is never asked to
    /// switch to the level it already runs at.
    #[serde(
        default,
        rename = "appliedEffort",
        skip_serializing_if = "Option::is_none"
    )]
    pub applied_effort: Option<String>,
    /// Relaunches that tried and failed to reach one assignment's effort.
    ///
    /// A failed relaunch leaves `applied_effort` at the level the session was
    /// actually running, so the switch stays pending and is tried again; this
    /// is what keeps that retry bounded. Cleared by any launch that commits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort_resume_failure: Option<EffortResumeFailure>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

/// How often a member has failed to reach one requested effort level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffortResumeFailure {
    /// The level that could not be reached.
    pub level: String,
    /// Attempts spent on it since the last launch that committed.
    pub attempts: u32,
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
    ///
    /// Two locks, always in this order: the team `.lock` serializes taurhaus's
    /// own stores, and the record file's own advisory lock is the one mesh
    /// takes — `appliedEffort` lives in this file and mesh writes it, so
    /// without the second lock the two processes' whole snapshots overwrite
    /// each other.
    pub fn save(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
        record: &MemberRuntimeRecord,
    ) -> Result<(), CoordinationError> {
        let lock_path = team_dir(teams_dir, team_name).join(".lock");
        let _lock = super::lock::acquire_team_lock(teams_dir, team_name).inspect_err(|err| {
            log_runtime_store_error("lock", &lock_path, err, None);
        })?;
        let target_path = runtime_record_path(teams_dir, team_name, member_name);
        let target_lock = super::lock::TargetFileLock::acquire_or_create(&target_path)
            .inspect_err(|err| log_runtime_store_error("lock", &target_path, err, None))?;

        let mut record = record.clone();
        merge_current_extension_fields(&mut record, &target_lock.read_contents()?, false);

        save_runtime_record_locked(teams_dir, team_name, member_name, &record, &target_lock)
    }

    /// Save a snapshot loaded earlier, keeping mesh-owned extension fields and
    /// whatever level mesh has put into force since.
    ///
    /// The liveness passes load every record for a team, decide what changed,
    /// and save the whole snapshot back. mesh is the only writer of
    /// `appliedEffort` and writes it into the same file, so a snapshot taken
    /// before that write would silently revert the level the member is actually
    /// running at. Re-reading the field under the target-file lock — the lock
    /// mesh takes too — is what keeps a save that owns nothing about the level
    /// from overwriting it.
    ///
    /// For a caller that *does* own the level — a launch, which puts the member
    /// at the effort its own command carried — [`Self::save`] is the right
    /// entry point.
    pub fn save_preserving_applied_effort(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
        record: &MemberRuntimeRecord,
    ) -> Result<(), CoordinationError> {
        let lock_path = team_dir(teams_dir, team_name).join(".lock");
        let _lock = super::lock::acquire_team_lock(teams_dir, team_name).inspect_err(|err| {
            log_runtime_store_error("lock", &lock_path, err, None);
        })?;
        let target_path = runtime_record_path(teams_dir, team_name, member_name);
        let target_lock = super::lock::TargetFileLock::acquire_or_create(&target_path)
            .inspect_err(|err| log_runtime_store_error("lock", &target_path, err, None))?;

        let mut record = record.clone();
        merge_current_extension_fields(&mut record, &target_lock.read_contents()?, true);

        save_runtime_record_locked(teams_dir, team_name, member_name, &record, &target_lock)
    }

    /// Update a runtime record while holding both locks across read and write.
    pub fn update<F>(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
        update: F,
    ) -> Result<MemberRuntimeRecord, CoordinationError>
    where
        F: FnOnce(&mut MemberRuntimeRecord),
    {
        let lock_path = team_dir(teams_dir, team_name).join(".lock");
        let _lock = super::lock::acquire_team_lock(teams_dir, team_name).inspect_err(|err| {
            log_runtime_store_error("lock", &lock_path, err, None);
        })?;
        let path = runtime_record_path(teams_dir, team_name, member_name);
        let not_found = || {
            CoordinationError::NotFound(format!(
                "runtime state not found for member '{member_name}' in team '{team_name}'"
            ))
        };
        // The record must already exist, so the lock is taken on the file
        // itself rather than created: locking by creating would turn a missing
        // record into an empty one every later read has to treat as corrupt.
        let Some(target_lock) = super::lock::TargetFileLock::acquire_if_exists(&path)
            .inspect_err(|err| log_runtime_store_error("lock", &path, err, None))?
        else {
            return Err(not_found());
        };
        let raw = target_lock.read_contents()?;
        if raw.trim().is_empty() {
            return Err(not_found());
        }
        let mut record = parse_runtime_record(&raw, team_name, member_name)?;
        update(&mut record);
        record.schema_version = RUNTIME_SCHEMA_VERSION;
        record.member_name = member_name.to_string();
        save_runtime_record_locked(teams_dir, team_name, member_name, &record, &target_lock)?;
        Ok(record)
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
                None => {
                    log_runtime_record_skipped(team_name, "<invalid>", "invalid member filename");
                    continue;
                }
            };
            let raw = match fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(err) => {
                    log_runtime_record_skipped(
                        team_name,
                        &member_name,
                        &format!("failed to read runtime record: {err}"),
                    );
                    continue;
                }
            };
            match parse_runtime_record(&raw, team_name, &member_name) {
                Ok(record) => {
                    clear_runtime_record_skipped(team_name, &member_name);
                    results.push((member_name, record));
                }
                Err(err) => {
                    log_runtime_record_skipped(team_name, &member_name, &err.to_string());
                    continue;
                }
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
                Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                    Err(_) => true,
                    Ok(Value::Object(object)) => {
                        match parse_runtime_record(&raw, team_name, &member_name) {
                            Ok(record) if has_staleness_fields(&object) => {
                                is_stale(&record, cutoff)
                            }
                            // Valid objects may be partial mesh records or use
                            // fields from a newer schema. Neither is corrupt.
                            Ok(_) | Err(_) => false,
                        }
                    }
                    Ok(_) => true,
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

fn save_runtime_record_locked(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    record: &MemberRuntimeRecord,
    _target_lock: &super::lock::TargetFileLock,
) -> Result<(), CoordinationError> {
    let mut normalized = record.clone();
    normalized.schema_version = RUNTIME_SCHEMA_VERSION;
    normalized.member_name = member_name.to_string();
    normalized.extra = extension_fields_only(normalized.extra, RUNTIME_AUTHORED_KEYS);

    let runtime_dir = runtime_dir_path(teams_dir, team_name);
    fs::create_dir_all(&runtime_dir).map_err(|err| {
        let coordination_err = CoordinationError::Io(err);
        log_runtime_store_error("create_dir", &runtime_dir, &coordination_err, None);
        coordination_err
    })?;

    let target_path = runtime_record_path(teams_dir, team_name, member_name);
    let tmp_path = runtime_tmp_path(teams_dir, team_name, member_name);
    let payload = serde_json::to_string_pretty(&normalized).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to serialize runtime record for '{member_name}': {err}"
        ))
    })?;

    retry_file_operation(
        "write",
        &tmp_path,
        None,
        &SAVE_RETRY_BACKOFFS,
        || write_file_synced(&tmp_path, &payload),
        |err| log_runtime_store_io_error("write", &tmp_path, err, None),
    )
    .map_err(CoordinationError::Io)?;

    if let Err(err) = retry_file_operation(
        "rename",
        &target_path,
        Some(&tmp_path),
        &SAVE_RETRY_BACKOFFS,
        || fs::rename(&tmp_path, &target_path),
        |err| log_runtime_store_io_error("rename", &target_path, err, Some(&tmp_path)),
    ) {
        if is_atomic_write_fallback_error(&err) {
            tracing::warn!(
                member_name,
                team_name,
                target = %target_path.display(),
                raw_os_error = ?err.raw_os_error(),
                "atomic runtime rename failed on team state save; falling back to direct write"
            );
            retry_file_operation(
                "write",
                &target_path,
                None,
                &SAVE_RETRY_BACKOFFS,
                || write_file_synced(&target_path, &payload),
                |write_err| log_runtime_store_io_error("write", &target_path, write_err, None),
            )
            .map_err(CoordinationError::Io)?;
            let _ = fs::remove_file(&tmp_path);
        } else {
            let _ = fs::remove_file(&tmp_path);
            return Err(CoordinationError::Io(err));
        }
    }

    prune_state_if_session_mismatch(
        teams_dir,
        team_name,
        member_name,
        normalized.session_id.as_deref(),
    )?;

    Ok(())
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
        #[serde(default, alias = "panePid")]
        pane_pid: Option<u32>,
        #[serde(default, alias = "paneStartTime")]
        pane_start_time: Option<u64>,
        #[serde(default, alias = "sessionId")]
        session_id: Option<String>,
        #[serde(default, alias = "jsonlPath")]
        jsonl_path: Option<PathBuf>,
        #[serde(default, alias = "daemonPid")]
        daemon_pid: Option<u32>,
        #[serde(
            default = "default_runtime_health",
            deserialize_with = "deserialize_runtime_health"
        )]
        health: HealthState,
        #[serde(default, alias = "deliveryLease")]
        delivery_lease: Option<DeliveryLease>,
        #[serde(default, alias = "attachedAt")]
        attached_at: Option<DateTime<Utc>>,
        #[serde(default, alias = "lastSeenAt")]
        last_seen_at: Option<DateTime<Utc>>,
        #[serde(default, alias = "appliedEffort")]
        applied_effort: Option<String>,
        #[serde(default, alias = "effortResumeFailure")]
        effort_resume_failure: Option<EffortResumeFailure>,
        #[serde(flatten, default)]
        extra: BTreeMap<String, Value>,
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
        pane_pid: wire.pane_pid,
        pane_start_time: wire.pane_start_time,
        session_id: wire.session_id,
        jsonl_path: wire.jsonl_path,
        daemon_pid: wire.daemon_pid,
        health: wire.health,
        delivery_lease: wire.delivery_lease,
        attached_at: wire.attached_at,
        last_seen_at: wire.last_seen_at,
        applied_effort: wire
            .applied_effort
            .map(|level| level.trim().to_string())
            .filter(|level| !level.is_empty()),
        effort_resume_failure: wire.effort_resume_failure,
        extra: extension_fields_only(wire.extra, RUNTIME_AUTHORED_KEYS),
    })
}

fn merge_current_extension_fields(
    record: &mut MemberRuntimeRecord,
    current_raw: &str,
    preserve_applied_effort: bool,
) {
    if current_raw.trim().is_empty() {
        return;
    }

    // Extract extensions from the JSON object directly: a future value for a
    // taurhaus-owned field must not make unrelated mesh keys unreadable.
    let Ok(Value::Object(current)) = serde_json::from_str::<Value>(current_raw) else {
        return;
    };
    let current_applied_effort = current
        .get("appliedEffort")
        .or_else(|| current.get("applied_effort"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|level| !level.is_empty())
        .map(ToString::to_string);
    // The file is the authority for foreign keys: a key mesh deleted between
    // this snapshot's load and its save must stay deleted, so the snapshot's
    // own extras are replaced, never unioned.
    record.extra = extension_fields_only(current.into_iter().collect(), RUNTIME_AUTHORED_KEYS);
    if preserve_applied_effort {
        record.applied_effort = current_applied_effort;
    }
}

const RUNTIME_AUTHORED_KEYS: &[&str] = &[
    "schema_version",
    "schemaVersion",
    "member_name",
    "memberName",
    "cli_tool",
    "cliTool",
    "project_path",
    "projectPath",
    "cwd",
    "pane_id",
    "paneId",
    "pane_pid",
    "panePid",
    "pane_start_time",
    "paneStartTime",
    "session_id",
    "sessionId",
    "jsonl_path",
    "jsonlPath",
    "daemon_pid",
    "daemonPid",
    "health",
    "delivery_lease",
    "deliveryLease",
    "attached_at",
    "attachedAt",
    "last_seen_at",
    "lastSeenAt",
    "applied_effort",
    "appliedEffort",
    "effort_resume_failure",
    "effortResumeFailure",
];

fn is_stale(record: &MemberRuntimeRecord, cutoff: DateTime<Utc>) -> bool {
    latest_activity(record).is_none_or(|ts| ts <= cutoff)
}

fn has_staleness_fields(object: &Map<String, Value>) -> bool {
    [
        "delivery_lease",
        "deliveryLease",
        "attached_at",
        "attachedAt",
        "last_seen_at",
        "lastSeenAt",
    ]
    .iter()
    .any(|key| object.contains_key(*key))
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

fn is_transient_lock_error(err: &std::io::Error) -> bool {
    super::lock::is_transient_file_lock_error(err)
}

fn is_atomic_write_fallback_error(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(1 | 5 | 32))
}

fn clone_io_error(err: &std::io::Error) -> std::io::Error {
    err.raw_os_error()
        .map(std::io::Error::from_raw_os_error)
        .unwrap_or_else(|| std::io::Error::new(err.kind(), err.to_string()))
}

fn log_runtime_store_io_error(
    operation: &str,
    path: &Path,
    err: &std::io::Error,
    from_path: Option<&Path>,
) {
    let coordination_err = CoordinationError::Io(clone_io_error(err));
    log_runtime_store_error(operation, path, &coordination_err, from_path);
}

fn retry_file_operation<F, Log>(
    operation: &str,
    path: &Path,
    from_path: Option<&Path>,
    backoffs: &[Duration],
    mut work: F,
    mut log_failure: Log,
) -> std::io::Result<()>
where
    F: FnMut() -> std::io::Result<()>,
    Log: FnMut(&std::io::Error),
{
    retry_file_operation_with_sleep(
        operation,
        path,
        from_path,
        backoffs,
        &mut work,
        &mut log_failure,
        thread::sleep,
    )
}

fn retry_file_operation_with_sleep<F, Log, Sleep>(
    operation: &str,
    path: &Path,
    from_path: Option<&Path>,
    backoffs: &[Duration],
    work: &mut F,
    log_failure: &mut Log,
    mut sleep: Sleep,
) -> std::io::Result<()>
where
    F: FnMut() -> std::io::Result<()>,
    Log: FnMut(&std::io::Error),
    Sleep: FnMut(Duration),
{
    let total_attempts = backoffs.len() + 1;
    let from_path_display = from_path.map(|value| value.display().to_string());

    for attempt in 0..total_attempts {
        match work() {
            Ok(()) => return Ok(()),
            Err(err) => {
                log_failure(&err);

                if is_transient_lock_error(&err) && attempt < backoffs.len() {
                    let delay = backoffs[attempt];
                    tracing::warn!(
                        operation,
                        path = %path.display(),
                        from_path = from_path_display.as_deref(),
                        attempt = attempt + 1,
                        max_attempts = total_attempts,
                        retry_in_ms = delay.as_millis() as u64,
                        raw_os_error = ?err.raw_os_error(),
                        "transient team state file lock detected; retrying save operation"
                    );
                    sleep(delay);
                    continue;
                }

                return Err(err);
            }
        }
    }

    Ok(())
}

fn write_file_synced(path: &Path, payload: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(payload.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

fn runtime_store_error_fields(
    operation: &str,
    path: &Path,
    err: &CoordinationError,
    from_path: Option<&Path>,
) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert(
        "operation".to_string(),
        Value::String(operation.to_string()),
    );
    fields.insert(
        "path".to_string(),
        Value::String(path.display().to_string()),
    );
    fields.insert("error".to_string(), Value::String(err.to_string()));
    fields.insert(
        "raw_os_error".to_string(),
        err.raw_os_error()
            .map(|code| Value::Number(code.into()))
            .unwrap_or(Value::Null),
    );
    if let Some(from_path) = from_path {
        fields.insert(
            "from_path".to_string(),
            Value::String(from_path.display().to_string()),
        );
    }
    fields
}

fn log_runtime_store_error(
    operation: &str,
    path: &Path,
    err: &CoordinationError,
    from_path: Option<&Path>,
) {
    let fields = runtime_store_error_fields(operation, path, err, from_path);
    let from_path_display = from_path.map(|value| value.display().to_string());
    emit_global(
        "warn",
        "coordination",
        "coordination.runtime_store.io_failed",
        Some("Member runtime store file operation failed".to_string()),
        fields,
    );
    tracing::warn!(
        operation,
        path = %path.display(),
        from_path = from_path_display.as_deref(),
        error = %err,
        raw_os_error = ?err.raw_os_error(),
        "member runtime store file operation failed"
    );
}

fn log_runtime_record_skipped(team_name: &str, member_name: &str, reason: &str) {
    let skip_reasons = RUNTIME_RECORD_SKIP_REASONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut skip_reasons = skip_reasons
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let key = (team_name.to_string(), member_name.to_string());
    if skip_reasons.get(&key).map(String::as_str) == Some(reason) {
        return;
    }
    skip_reasons.insert(key, reason.to_string());
    drop(skip_reasons);

    let mut fields = Map::new();
    fields.insert("team".to_string(), Value::String(team_name.to_string()));
    fields.insert("member".to_string(), Value::String(member_name.to_string()));
    fields.insert("reason".to_string(), Value::String(reason.to_string()));
    emit_global(
        "warn",
        "coordination",
        "coordination.runtime.record_skipped",
        Some("Member runtime record was skipped".to_string()),
        fields,
    );
    tracing::warn!(
        team = team_name,
        member = member_name,
        reason,
        "member runtime record was skipped"
    );
}

fn clear_runtime_record_skipped(team_name: &str, member_name: &str) {
    let Some(skip_reasons) = RUNTIME_RECORD_SKIP_REASONS.get() else {
        return;
    };
    skip_reasons
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&(team_name.to_string(), member_name.to_string()));
}

const fn schema_version_one() -> u32 {
    RUNTIME_SCHEMA_VERSION
}

const fn default_runtime_health() -> HealthState {
    HealthState::SessionDead
}

fn deserialize_runtime_health<'de, D>(deserializer: D) -> Result<HealthState, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_else(|_| default_runtime_health()))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::TimeZone;
    use taurhaus_lib::logging::{install_global_sink, LogFileState};
    use tempfile::TempDir;

    use super::*;
    use crate::coordination::stores::{
        CompactionDeliveryResult, MemberCompactionState, MemberCompactionStore,
    };

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
            pane_pid: Some(1200),
            pane_start_time: Some(1_755_000_000),
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
            applied_effort: None,
            effort_resume_failure: None,
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn applied_effort_round_trips_through_the_key_mesh_reads() {
        // mesh writes `appliedEffort` into this record before it types
        // `/effort` into the pane, and reads it back to decide whether the next
        // assignment needs the command at all. A save from taurhaus that drops
        // the key makes mesh restate the level on every assignment — and, for
        // Claude Code, rewrite the user's saved default each time.
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";
        let mut record = sample_record(member_name);
        record.applied_effort = Some("high".to_string());

        MemberRuntimeStore::save(teams_dir, team_name, member_name, &record)
            .expect("save should succeed");

        let raw = fs::read_to_string(
            teams_dir
                .join(team_name)
                .join("runtime")
                .join(format!("{member_name}.json")),
        )
        .expect("runtime record on disk");
        let value: Value = serde_json::from_str(&raw).expect("runtime record is json");
        assert_eq!(
            value.get("appliedEffort").and_then(Value::as_str),
            Some("high"),
            "mesh reads the level under `appliedEffort`"
        );

        let loaded = MemberRuntimeStore::load(teams_dir, team_name, member_name)
            .expect("load should succeed");
        assert_eq!(loaded.applied_effort.as_deref(), Some("high"));
    }

    #[test]
    fn a_record_written_by_mesh_keeps_its_applied_effort() {
        // mesh's own write is a read-modify-write of the raw JSON, so the key
        // arrives without any of taurhaus's own fields being restated.
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
        .expect("save should succeed");

        let path = teams_dir
            .join(team_name)
            .join("runtime")
            .join(format!("{member_name}.json"));
        let mut value: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("record")).expect("json");
        value["appliedEffort"] = Value::String("medium".to_string());
        fs::write(&path, serde_json::to_string(&value).expect("json")).expect("write");

        let loaded = MemberRuntimeStore::load(teams_dir, team_name, member_name)
            .expect("load should succeed");
        assert_eq!(loaded.applied_effort.as_deref(), Some("medium"));
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
    fn locked_update_preserves_fields_outside_the_patch() {
        // Regression: 694b130 made inbox wake load and save the whole runtime
        // record, reverting concurrent pane/session/health updates.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "runtime-update";
        let member_name = "builder";
        let record = sample_record(member_name);
        MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

        let updated = MemberRuntimeStore::update(tmp.path(), team_name, member_name, |runtime| {
            runtime.daemon_pid = Some(9001);
            runtime.last_seen_at = Some(ts("2026-03-01T21:06:00Z"));
        })
        .expect("locked update");

        assert_eq!(updated.daemon_pid, Some(9001));
        assert_eq!(updated.pane_id, record.pane_id);
        assert_eq!(updated.session_id, record.session_id);
        assert_eq!(updated.health, record.health);
        assert_eq!(
            MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load updated"),
            updated
        );
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
        assert_eq!(loaded.pane_pid, None);
        assert_eq!(loaded.pane_start_time, None);
        assert_eq!(loaded.session_id.as_deref(), Some("session-123"));
        assert_eq!(loaded.jsonl_path, None);
    }

    #[test]
    fn minimal_mesh_runtime_loads_and_is_not_cleaned_up_as_stale() {
        // Regression: 50fc736 made `health` mandatory, so mesh's minimal
        // applied-effort record was rejected and then deleted as corrupt.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "mesh-minimal";
        let member_name = "builder";
        let runtime_dir = runtime_dir_path(tmp.path(), team_name);
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let path = runtime_dir.join(format!("{member_name}.json"));
        fs::write(&path, r#"{"appliedEffort":"medium"}"#).expect("minimal runtime");

        let record = MemberRuntimeStore::load(tmp.path(), team_name, member_name)
            .expect("minimal mesh record should load");
        assert_eq!(record.health, HealthState::SessionDead);
        assert_eq!(record.applied_effort.as_deref(), Some("medium"));
        assert_eq!(
            MemberRuntimeStore::load_all(tmp.path(), team_name)
                .expect("load all")
                .len(),
            1,
            "a partial object is a runtime record, not corrupt input"
        );

        let removed = MemberRuntimeStore::cleanup_stale(
            tmp.path(),
            team_name,
            Duration::from_secs(60),
            ts("2026-03-01T21:10:00Z"),
        )
        .expect("cleanup");
        assert!(removed.is_empty(), "partial mesh record must be preserved");
        assert!(path.exists());
    }

    #[test]
    fn camel_case_activity_timestamp_keeps_a_fresh_runtime_record() {
        // Regression: 44540568 classified `lastSeenAt` as taurhaus-authored
        // without teaching the runtime decoder to read that spelling.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "camel-activity";
        let member_name = "builder";
        let runtime_dir = runtime_dir_path(tmp.path(), team_name);
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let path = runtime_dir.join(format!("{member_name}.json"));
        fs::write(
            &path,
            serde_json::json!({
                "paneId": "%7",
                "deliveryLease": {
                    "owner_pid": 42,
                    "instance_uuid": "mesh-instance",
                    "hostname": "mesh-host",
                    "heartbeat_at": "2026-03-01T21:09:40Z",
                    "started_at": "2026-03-01T21:00:00Z"
                },
                "attachedAt": "2026-03-01T21:09:20Z",
                "lastSeenAt": "2026-03-01T21:09:30Z"
            })
            .to_string(),
        )
        .expect("camel-case runtime");

        let record = MemberRuntimeStore::load(tmp.path(), team_name, member_name)
            .expect("camel-case activity should load");
        assert_eq!(
            record.delivery_lease.as_ref().map(|lease| lease.owner_pid),
            Some(42)
        );
        assert_eq!(record.attached_at, Some(ts("2026-03-01T21:09:20Z")));
        assert_eq!(record.last_seen_at, Some(ts("2026-03-01T21:09:30Z")));

        let removed = MemberRuntimeStore::cleanup_stale(
            tmp.path(),
            team_name,
            Duration::from_secs(60),
            ts("2026-03-01T21:10:00Z"),
        )
        .expect("cleanup");
        assert!(removed.is_empty(), "the fresh record must be retained");
        assert!(path.exists());
    }

    #[test]
    fn future_health_value_does_not_hide_an_otherwise_valid_runtime_record() {
        // Regression: 56712858 preserved forward-compatible JSON objects in
        // cleanup but still made them invisible to load and member resume.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "future-health";
        let member_name = "builder";
        let runtime_dir = runtime_dir_path(tmp.path(), team_name);
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        fs::write(
            runtime_dir.join(format!("{member_name}.json")),
            r#"{"paneId":"%7","health":"future_mesh_state"}"#,
        )
        .expect("future runtime");

        let record = MemberRuntimeStore::load(tmp.path(), team_name, member_name)
            .expect("future health should degrade one field, not the record");
        assert_eq!(record.pane_id.as_deref(), Some("%7"));
        assert_eq!(record.health, HealthState::SessionDead);
        assert_eq!(
            MemberRuntimeStore::load_all(tmp.path(), team_name)
                .expect("load all")
                .len(),
            1
        );
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
    fn save_prunes_compaction_state_when_session_id_changes() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let member_name = "architect";

        MemberCompactionStore::save(
            teams_dir,
            team_name,
            member_name,
            &MemberCompactionState {
                version: 1,
                member_name: member_name.to_string(),
                last_session_id: "sess-123".to_string(),
                last_compaction_timestamp: ts("2026-03-10T09:00:00Z"),
                last_delivery_result: CompactionDeliveryResult::Skipped,
            },
        )
        .expect("save compaction state");

        let mut record = sample_record(member_name);
        record.session_id = Some("session-123".to_string());
        MemberRuntimeStore::save(teams_dir, team_name, member_name, &record).expect("save runtime");

        assert!(
            MemberCompactionStore::load(teams_dir, team_name, member_name)
                .expect("load compaction state")
                .is_none(),
            "mismatched compaction state should be pruned when runtime session changes"
        );
    }

    #[test]
    fn legacy_stale_compaction_state_does_not_block_runtime_save() {
        // Regression: 7516a07 retired CompactionDeliveryResult::Stale without
        // preserving the shipped 0.8.x wire value, so member activation failed
        // while saving any runtime record for a member with that state file.
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let member_name = "architect";
        let compaction_dir = teams_dir.join(team_name).join("state").join("compaction");
        fs::create_dir_all(&compaction_dir).expect("compaction state dir");
        fs::write(
            compaction_dir.join(format!("{member_name}.json")),
            r#"{
  "version": 1,
  "member_name": "architect",
  "last_session_id": "session-123",
  "last_compaction_timestamp": "2026-03-10T09:00:00Z",
  "last_delivery_result": "stale"
}"#,
        )
        .expect("0.8.x compaction state");

        let state = MemberCompactionStore::load(teams_dir, team_name, member_name)
            .expect("legacy compaction state should remain loadable")
            .expect("legacy compaction state should exist");
        assert_eq!(
            state.last_delivery_result,
            CompactionDeliveryResult::Skipped
        );

        MemberRuntimeStore::save(
            teams_dir,
            team_name,
            member_name,
            &sample_record(member_name),
        )
        .expect("legacy compaction state must not block member activation");
    }

    #[test]
    fn cleanup_stale_removes_corrupt_runtime_file() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let runtime_dir = runtime_dir_path(teams_dir, team_name);
        fs::create_dir_all(&runtime_dir).expect("create runtime dir");
        // Invalid JSON has no extension map or timestamp information that can
        // be preserved safely, so it remains genuinely corrupt.
        fs::write(runtime_dir.join("broken.json"), "not json").expect("write broken json");
        // A valid JSON value that is not an object cannot be a runtime record.
        fs::write(runtime_dir.join("array.json"), "[]").expect("write non-object json");

        let removed = MemberRuntimeStore::cleanup_stale(
            teams_dir,
            team_name,
            Duration::from_secs(60),
            Utc.timestamp_opt(0, 0).single().expect("epoch"),
        )
        .expect("cleanup should succeed");

        assert_eq!(removed, vec!["array".to_string(), "broken".to_string()]);
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
            pane_pid: None,
            pane_start_time: None,
            session_id: None,
            jsonl_path: None,
            daemon_pid: None,
            health: HealthState::SessionDead,
            delivery_lease: None,
            attached_at: None,
            last_seen_at: None,
            applied_effort: None,
            effort_resume_failure: None,
            extra: BTreeMap::new(),
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
    fn retry_file_operation_retries_transient_lock_errors_until_success() {
        let mut attempts = 0;
        let mut slept = Vec::new();

        let result = retry_file_operation_with_sleep(
            "rename",
            Path::new("/tmp/runtime.json"),
            Some(Path::new("/tmp/runtime.json.tmp")),
            &SAVE_RETRY_BACKOFFS,
            &mut || {
                attempts += 1;
                if attempts < 3 {
                    Err(std::io::Error::from_raw_os_error(5))
                } else {
                    Ok(())
                }
            },
            &mut |_| {},
            |delay| slept.push(delay),
        );

        assert!(result.is_ok());
        assert_eq!(attempts, 3);
        assert_eq!(slept, vec![SAVE_RETRY_BACKOFFS[0], SAVE_RETRY_BACKOFFS[1]]);
    }

    #[test]
    fn retry_file_operation_does_not_retry_non_transient_errors() {
        let mut attempts = 0;
        let mut slept = Vec::new();

        let err = retry_file_operation_with_sleep(
            "rename",
            Path::new("/tmp/runtime.json"),
            Some(Path::new("/tmp/runtime.json.tmp")),
            &SAVE_RETRY_BACKOFFS,
            &mut || {
                attempts += 1;
                Err(std::io::Error::from_raw_os_error(13))
            },
            &mut |_| {},
            |delay| slept.push(delay),
        )
        .expect_err("non-transient error should surface immediately");

        assert_eq!(err.raw_os_error(), Some(13));
        assert_eq!(attempts, 1);
        assert!(slept.is_empty());
    }

    #[test]
    fn atomic_write_fallback_error_detection_includes_unc_locking_codes() {
        assert!(is_atomic_write_fallback_error(
            &std::io::Error::from_raw_os_error(1)
        ));
        assert!(is_atomic_write_fallback_error(
            &std::io::Error::from_raw_os_error(5)
        ));
        assert!(is_atomic_write_fallback_error(
            &std::io::Error::from_raw_os_error(32)
        ));
        assert!(!is_atomic_write_fallback_error(
            &std::io::Error::from_raw_os_error(13)
        ));
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
    fn load_all_skips_corrupt_files_with_one_diagnostic_across_repeated_loads() {
        // Regression: 56712858 emitted the same warning on every two-second
        // status poll while a persistent corrupt runtime file remained.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "runtime-skip-dedup";

        let valid = sample_record("valid-agent");
        MemberRuntimeStore::save(teams_dir, team_name, "valid-agent", &valid).expect("save");

        let runtime_dir = runtime_dir_path(teams_dir, team_name);
        // This is invalid JSON, not a merely partial runtime object, so it is
        // skipped while the valid record remains available.
        fs::write(runtime_dir.join("corrupt-agent.json"), "{{bad json").expect("write corrupt");
        let log_path = tmp.path().join("runtime.log.jsonl");
        let log_state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&log_state);

        let results =
            MemberRuntimeStore::load_all(teams_dir, team_name).expect("load_all should succeed");
        let repeated_results =
            MemberRuntimeStore::load_all(teams_dir, team_name).expect("repeat should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "valid-agent");
        assert_eq!(results[0].1, valid);
        assert_eq!(repeated_results, results);

        log_state
            .flush_for_test()
            .expect("flush runtime diagnostics");
        let contents = fs::read_to_string(&log_path).expect("read runtime diagnostics");
        assert_eq!(
            contents
                .matches("\"event\":\"coordination.runtime.record_skipped\"")
                .count(),
            1,
            "one skipped file should emit exactly one diagnostic"
        );
        assert!(contents.contains("\"member\":\"corrupt-agent\""));
        assert!(contents.contains("\"reason\":"));
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

    // Regression: 50fc736 made `appliedEffort` a field mesh reads and writes
    // in this same file, but the store still guarded its read-modify-write
    // with the team `.lock` alone. mesh takes the target file's own advisory
    // lock, so the two writers never excluded each other and whichever
    // renamed last replaced the other's whole snapshot.
    #[test]
    fn a_runtime_update_waits_for_a_cross_writer_holding_the_target_file_lock() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().to_path_buf();
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";
        let mut seeded = sample_record(member_name);
        seeded.applied_effort = Some("low".to_string());
        MemberRuntimeStore::save(&teams_dir, team_name, member_name, &seeded).expect("seed record");

        // Stand in for mesh: hold the target file's own lock across a
        // read-modify-write, which is what it does before typing `/effort`.
        let record_path = runtime_record_path(&teams_dir, team_name, member_name);
        let foreign = super::super::lock::TargetFileLock::acquire_or_create(&record_path)
            .expect("cross-writer lock");
        let mut snapshot: Value = serde_json::from_str(&foreign.read_contents().expect("read"))
            .expect("runtime record is json");
        snapshot["appliedEffort"] = Value::String("high".to_string());

        let (started, wait_for_start) = std::sync::mpsc::channel();
        let updater = {
            let teams_dir = teams_dir.clone();
            thread::spawn(move || {
                started.send(()).expect("signal the updater started");
                MemberRuntimeStore::update(&teams_dir, team_name, member_name, |record| {
                    record.health = HealthState::SessionDead;
                })
            })
        };
        wait_for_start.recv().expect("updater started");
        thread::sleep(Duration::from_millis(300));

        fs::write(
            &record_path,
            serde_json::to_string_pretty(&snapshot).expect("payload"),
        )
        .expect("cross-writer write");
        drop(foreign);

        updater
            .join()
            .expect("updater thread")
            .expect("update succeeds");

        let record =
            MemberRuntimeStore::load(&teams_dir, team_name, member_name).expect("runtime record");
        assert_eq!(
            record.applied_effort.as_deref(),
            Some("high"),
            "the level mesh wrote survives taurhaus's own update"
        );
        assert_eq!(
            record.health,
            HealthState::SessionDead,
            "taurhaus's update is applied on top of what mesh wrote, not under it"
        );
    }

    // Regression: ad75a7b took the target-file lock only around the write, so
    // it protected `update` but not the shape production liveness actually
    // uses — load every record, decide what changed, save the whole snapshot
    // back later. mesh writes `appliedEffort` into the same file in that
    // window, and the stale snapshot went straight back over it.
    #[test]
    fn a_save_of_a_snapshot_keeps_the_level_mesh_wrote_after_it_was_loaded() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().to_path_buf();
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";
        let mut seeded = sample_record(member_name);
        seeded.applied_effort = Some("low".to_string());
        MemberRuntimeStore::save(&teams_dir, team_name, member_name, &seeded).expect("seed record");

        // What a liveness pass holds: a snapshot read before it decided.
        let mut snapshot =
            MemberRuntimeStore::load(&teams_dir, team_name, member_name).expect("load snapshot");

        // Stand in for mesh, writing between that load and the save.
        let record_path = runtime_record_path(&teams_dir, team_name, member_name);
        let mut on_disk: Value =
            serde_json::from_str(&fs::read_to_string(&record_path).expect("read"))
                .expect("runtime record is json");
        on_disk["appliedEffort"] = Value::String("high".to_string());
        fs::write(
            &record_path,
            serde_json::to_string_pretty(&on_disk).expect("payload"),
        )
        .expect("cross-writer write");

        snapshot.health = HealthState::SessionDead;
        MemberRuntimeStore::save_preserving_applied_effort(
            &teams_dir,
            team_name,
            member_name,
            &snapshot,
        )
        .expect("save the reconciled snapshot");

        let record =
            MemberRuntimeStore::load(&teams_dir, team_name, member_name).expect("runtime record");
        assert_eq!(
            record.applied_effort.as_deref(),
            Some("high"),
            "the level mesh put into force survives a save of an older snapshot"
        );
        assert_eq!(
            record.health,
            HealthState::SessionDead,
            "the caller's own change is still written"
        );
    }

    #[test]
    fn a_save_of_a_stale_snapshot_preserves_foreign_keys_but_owns_runtime_fields() {
        // Regression: 50fc736 preserved only `appliedEffort`, so a later
        // taurhaus save erased every other key mesh added after the snapshot
        // was loaded.
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "two-writer-runtime";
        let member_name = "codex-reviewer";
        MemberRuntimeStore::save(
            teams_dir,
            team_name,
            member_name,
            &sample_record(member_name),
        )
        .expect("seed record");
        let mut stale =
            MemberRuntimeStore::load(teams_dir, team_name, member_name).expect("load snapshot");

        let path = runtime_record_path(teams_dir, team_name, member_name);
        let mut current: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read record")).expect("json");
        current["futureMeshField"] = serde_json::json!({ "preserve": [1, 2, 3] });
        current["health"] = Value::String("healthy".to_string());
        fs::write(
            &path,
            serde_json::to_string_pretty(&current).expect("serialize mesh write"),
        )
        .expect("write mesh update");

        stale.health = HealthState::SessionDead;
        MemberRuntimeStore::save_preserving_applied_effort(
            teams_dir,
            team_name,
            member_name,
            &stale,
        )
        .expect("save stale snapshot");

        let saved: Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read saved record"))
                .expect("saved json");
        assert_eq!(
            saved["futureMeshField"],
            serde_json::json!({ "preserve": [1, 2, 3] }),
            "mesh-owned extension fields must survive a stale taurhaus save"
        );
        assert_eq!(
            saved["health"], "session_dead",
            "the taurhaus-owned field from the snapshot must win"
        );
    }

    #[test]
    fn an_owning_save_preserves_foreign_keys_but_keeps_its_applied_effort() {
        // Regression: 50fc736 left the ordinary save path able to erase
        // foreign keys even though launches legitimately own `appliedEffort`.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "owning-runtime-save";
        let member_name = "codex-reviewer";
        let mut seeded = sample_record(member_name);
        seeded.applied_effort = Some("low".to_string());
        MemberRuntimeStore::save(tmp.path(), team_name, member_name, &seeded).expect("seed record");
        let stale =
            MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load snapshot");

        let path = runtime_record_path(tmp.path(), team_name, member_name);
        let mut current: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read record")).expect("json");
        current["futureMeshField"] = Value::String("keep me".to_string());
        current["appliedEffort"] = Value::String("high".to_string());
        fs::write(
            &path,
            serde_json::to_string_pretty(&current).expect("serialize mesh write"),
        )
        .expect("write mesh update");

        MemberRuntimeStore::save(tmp.path(), team_name, member_name, &stale).expect("owning save");

        let saved: Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read saved record"))
                .expect("saved json");
        assert_eq!(saved["futureMeshField"], "keep me");
        assert_eq!(
            saved["appliedEffort"], "low",
            "an owning save still writes the effort carried by its launch snapshot"
        );
    }

    // Regression: merge-on-save unioned the snapshot's extras with the file's,
    // so a foreign key mesh deleted between load and save came back on the
    // next taurhaus save (Opus review of 2a-i, remaining minor).
    #[test]
    fn a_save_does_not_resurrect_a_foreign_key_the_file_no_longer_has() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "no-resurrection";
        let member_name = "codex-reviewer";
        let seeded = sample_record(member_name);
        MemberRuntimeStore::save(tmp.path(), team_name, member_name, &seeded).expect("seed record");

        let path = runtime_record_path(tmp.path(), team_name, member_name);
        let mut current: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read record")).expect("json");
        current["meshOnly"] = Value::String("v1".to_string());
        fs::write(
            &path,
            serde_json::to_string_pretty(&current).expect("serialize"),
        )
        .expect("mesh write");
        let stale =
            MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load snapshot");
        assert!(
            stale.extra.contains_key("meshOnly"),
            "the snapshot saw the foreign key"
        );

        // mesh deletes the key after the snapshot was taken.
        let mut current: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read record")).expect("json");
        current.as_object_mut().expect("object").remove("meshOnly");
        fs::write(
            &path,
            serde_json::to_string_pretty(&current).expect("serialize"),
        )
        .expect("mesh delete");

        MemberRuntimeStore::save(tmp.path(), team_name, member_name, &stale).expect("stale save");

        let saved: Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read saved record"))
                .expect("json");
        assert!(
            saved.get("meshOnly").is_none(),
            "a key the file no longer has must not come back from a stale snapshot"
        );
    }

    #[test]
    fn runtime_authored_keys_cover_every_serialized_record_field() {
        let member_name = "authored-key-guard";
        let mut record = sample_record(member_name);
        record.applied_effort = Some("high".to_string());
        record.effort_resume_failure = Some(EffortResumeFailure {
            level: "high".to_string(),
            attempts: 2,
        });
        let Value::Object(serialized) = serde_json::to_value(record).expect("serialize record")
        else {
            panic!("runtime record must serialize as an object");
        };

        for key in serialized.keys() {
            assert!(
                RUNTIME_AUTHORED_KEYS.contains(&key.as_str()),
                "serialized runtime key '{key}' is missing from RUNTIME_AUTHORED_KEYS"
            );
        }
    }

    #[test]
    fn stale_save_preserves_extensions_from_a_forward_compatible_object() {
        // Regression: 50fc736 made extension preservation depend on decoding
        // the whole current record, so one future owned value could still
        // cause unrelated mesh keys to be erased.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "forward-runtime";
        let member_name = "codex-reviewer";
        MemberRuntimeStore::save(
            tmp.path(),
            team_name,
            member_name,
            &sample_record(member_name),
        )
        .expect("seed record");
        let mut stale =
            MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load snapshot");

        let path = runtime_record_path(tmp.path(), team_name, member_name);
        let mut current: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read record")).expect("json");
        current["health"] = Value::String("future_mesh_state".to_string());
        current["appliedEffort"] = Value::String("high".to_string());
        current["futureMeshField"] = serde_json::json!({ "keep": true });
        fs::write(
            &path,
            serde_json::to_string_pretty(&current).expect("serialize future object"),
        )
        .expect("write future object");

        stale.health = HealthState::SessionDead;
        MemberRuntimeStore::save_preserving_applied_effort(
            tmp.path(),
            team_name,
            member_name,
            &stale,
        )
        .expect("save stale snapshot");

        let saved: Value =
            serde_json::from_str(&fs::read_to_string(path).expect("read saved record"))
                .expect("saved json");
        assert_eq!(
            saved["futureMeshField"],
            serde_json::json!({ "keep": true })
        );
        assert_eq!(saved["appliedEffort"], "high");
        assert_eq!(saved["health"], "session_dead");
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
