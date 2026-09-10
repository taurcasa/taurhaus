//! Compaction delivery idempotency store and audit helpers.

use std::fs;
use std::path::{Path, PathBuf};

use crate::coordination::compaction_events::{emit_compaction_delivery, CompactionDeliveryEvent};
use crate::coordination::errors::CoordinationError;
use crate::session_scanner::cli_tool::CliTool;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const COMPACTION_SCHEMA_VERSION: u32 = 1;
const RESERVED_COMPACTION_STATE_BASENAMES: &[&str] = &["extractor-state", "signal-watcher-state"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionDeliveryResult {
    Injected,
    #[serde(alias = "stale")]
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MemberCompactionState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_boundary: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal: Option<crate::coordination::journal::JournalReceipt>,
    #[serde(default)]
    pub pending: bool,
    #[serde(default)]
    pub pending_obligation: Option<crate::coordination::recovery_card::ObligationKey>,
    #[serde(default)]
    pub satisfied_by: Option<String>,
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
        let guard = super::lock::acquire_team_lock(teams_dir, team_name)?;
        Self::save_locked(&guard, teams_dir, team_name, member_name, state)
    }

    pub(crate) fn save_locked(
        guard: &super::lock::TeamLockGuard,
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
        state: &MemberCompactionState,
    ) -> Result<(), CoordinationError> {
        if !guard.covers(teams_dir, team_name) {
            return Err(CoordinationError::Conflict(
                "compaction lock mismatch".into(),
            ));
        }
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

        super::lock::stage_synced(&tmp_path, payload.as_bytes())?;
        if let Err(err) = fs::rename(&tmp_path, &target_path) {
            if is_windows_unsupported_rename_error(&err) {
                super::lock::report_atomic_write_degraded(
                    &target_path,
                    "compaction",
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

    pub fn delete(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
    ) -> Result<(), CoordinationError> {
        let _lock = super::lock::acquire_team_lock(teams_dir, team_name)?;
        delete_state_file(teams_dir, team_name, member_name)
    }
}

/// Caller holds host exclusion through admission and delivery (also in the native hook).
pub(crate) fn record_host_boundary(
    root: &Path,
    team: &str,
    member: &str,
    boundary: &Value,
    source: &str,
) -> Result<bool, CoordinationError> {
    use CompactionDeliveryResult::Skipped;
    let thread = boundary["threadId"].as_str().unwrap_or_default();
    let mut timestamp = boundary["completedAtMs"]
        .as_i64()
        .and_then(DateTime::from_timestamp_millis)
        .unwrap_or_else(Utc::now);
    if let Some(mut previous) = MemberCompactionStore::load(root, team, member)? {
        let old = previous.host_boundary.clone().unwrap_or_default();
        let ids: Vec<_> = ["turnId", "itemId"]
            .iter()
            .filter_map(|key| {
                boundary[key]
                    .as_str()
                    .filter(|v| !v.is_empty())
                    .zip(old[key].as_str().filter(|v| !v.is_empty()))
            })
            .collect();
        let same_id = !ids.is_empty() && ids.iter().all(|(a, b)| a == b);
        // Hooks have no turn/item IDs. Correlate known opposite observers within two seconds,
        // never merge conflicting known IDs or two distinct notifications by time alone.
        let close = ids.is_empty()
            && matches!((previous.source.as_deref(), source),
                (Some("hook"), "host_notification") | (Some("host_notification"), "hook"))
            && (previous.last_compaction_timestamp - timestamp).abs()
                <= chrono::Duration::seconds(2);
        if previous.last_session_id == thread
            && (same_id
                || close
                || (ids.is_empty() && previous.source.as_deref() == Some(source) && previous.last_compaction_timestamp == timestamp))
        {
            if source == "host_notification" {
                previous.host_boundary = Some(boundary.clone());
                MemberCompactionStore::save(root, team, member, &previous)?;
            }
            return Ok(false);
        }
        // Distinct known IDs may share a millisecond; keep the runtime's timestamp key unique.
        if previous.last_compaction_timestamp == timestamp {
            timestamp += chrono::Duration::nanoseconds(1);
        }
    }
    let tool = super::MemberRuntimeStore::load(root, team, member)?
        .cli_tool
        .ok_or_else(|| CoordinationError::Conflict("host harness identity missing".into()))?;
    record_delivery_at(root, team, member, tool, thread, timestamp, Skipped)?;
    let mut state = MemberCompactionStore::load(root, team, member)?
        .ok_or_else(|| CoordinationError::Conflict("host compaction state missing".into()))?;
    state.source = Some(source.into());
    state.host_boundary = Some(boundary.clone());
    MemberCompactionStore::save(root, team, member, &state)?;
    Ok(true)
}

pub(crate) fn emit_host_compaction(
    team: &str,
    member: &str,
    boundary: &Value,
    event: &str,
    reason: Option<&str>,
) {
    let mut fields = serde_json::Map::new();
    let id = |key: &str| boundary[key].as_str().unwrap_or_default();
    for (key, value) in [
        ("team", team),
        ("member", member),
        ("thread_id", id("threadId")),
        ("turn_id", id("turnId")),
        ("item_id", id("itemId")),
    ] {
        fields.insert(key.into(), Value::String(value.chars().take(256).collect()));
    }
    if let Some(reason) = reason {
        fields.insert(
            "reason".into(),
            Value::String(reason.chars().take(256).collect()),
        );
    }
    taurhaus_lib::logging::emit_global("info", "coordination", event, None, fields);
}

fn delete_state_file(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
) -> Result<(), CoordinationError> {
    let path = compaction_state_path(teams_dir, team_name, member_name);
    // Sibling first, so a deliberate delete cannot be resurrected.
    super::lock::remove_record(&path).map_err(CoordinationError::Io)
}

impl MemberCompactionStore {
    pub(crate) fn delete_without_lock(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
    ) -> Result<(), CoordinationError> {
        delete_state_file(teams_dir, team_name, member_name)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn record_delivery_at(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    tool: CliTool,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
    result: CompactionDeliveryResult,
) -> Result<(), CoordinationError> {
    record_delivery_with_journal_at(
        teams_dir,
        team_name,
        member_name,
        tool,
        session_id,
        compaction_timestamp,
        result,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn record_delivery_with_journal_at(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    tool: CliTool,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
    result: CompactionDeliveryResult,
    journal: Option<crate::coordination::journal::JournalReceipt>,
) -> Result<(), CoordinationError> {
    record_delivery_with_transport_at(
        teams_dir,
        team_name,
        member_name,
        tool,
        session_id,
        compaction_timestamp,
        result,
        journal,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn record_delivery_with_transport_at(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    tool: CliTool,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
    result: CompactionDeliveryResult,
    journal: Option<crate::coordination::journal::JournalReceipt>,
    delivery: Option<&str>,
) -> Result<(), CoordinationError> {
    let guard = super::lock::acquire_team_lock(teams_dir, team_name)?;
    let mut pending_obligation = None;
    if result != CompactionDeliveryResult::Failed {
        if let Ok(mut runtime) = super::MemberRuntimeStore::load(teams_dir, team_name, member_name)
        {
            let boundary = format!("{session_id}:{compaction_timestamp}");
            runtime.admit_compaction(&boundary);
            if let Some((team_id, member_id)) = super::TeamConfigStore::load(teams_dir, team_name)
                .ok()
                .and_then(|c| c.team_incarnation_id)
                .zip(runtime.recovery.member_incarnation_id.clone())
            {
                if result == CompactionDeliveryResult::Skipped {
                    pending_obligation = Some(((team_id, member_id), runtime.context()));
                }
            }
            super::MemberRuntimeStore::save_recovery_locked(
                &guard,
                teams_dir,
                team_name,
                member_name,
                &runtime,
            )?;
        }
    }
    let previous = MemberCompactionStore::load(teams_dir, team_name, member_name)?;
    let same_boundary = previous.as_ref().is_some_and(|s| {
        s.last_session_id == session_id && s.last_compaction_timestamp == compaction_timestamp
    });
    // Only a new skipped boundary replaces the obligation; receipt observation satisfies it.
    let preserve_obligation = same_boundary || result != CompactionDeliveryResult::Skipped;
    let state = MemberCompactionState {
        source: previous
            .as_ref()
            .filter(|_| same_boundary)
            .and_then(|s| s.source.clone()),
        host_boundary: previous
            .as_ref()
            .filter(|_| same_boundary)
            .and_then(|s| s.host_boundary.clone()),
        journal: journal.or_else(|| {
            previous
                .as_ref()
                .filter(|_| same_boundary)
                .and_then(|s| s.journal.clone())
        }),
        pending: if preserve_obligation {
            previous.as_ref().is_some_and(|s| s.pending)
        } else {
            pending_obligation.is_some()
        },
        pending_obligation: if preserve_obligation {
            previous.as_ref().and_then(|s| s.pending_obligation.clone())
        } else {
            pending_obligation
        },
        satisfied_by: if preserve_obligation {
            previous.and_then(|s| s.satisfied_by)
        } else {
            None
        },
        version: COMPACTION_SCHEMA_VERSION,
        member_name: member_name.into(),
        last_session_id: session_id.into(),
        last_compaction_timestamp: compaction_timestamp,
        last_delivery_result: result,
    };
    MemberCompactionStore::save_locked(&guard, teams_dir, team_name, member_name, &state)?;
    drop(guard);
    emit_compaction_delivery_event(
        team_name,
        member_name,
        tool,
        session_id,
        compaction_timestamp,
        result,
        None,
        None,
        delivery,
    );
    Ok(())
}

pub fn prune_state_if_session_mismatch(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    active_session_id: Option<&str>,
) -> Result<bool, CoordinationError> {
    let Some(active_session_id) = active_session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(false);
    };

    let Some(state) = MemberCompactionStore::load(teams_dir, team_name, member_name)? else {
        return Ok(false);
    };

    if state.last_session_id == active_session_id {
        return Ok(false);
    }

    MemberCompactionStore::delete_without_lock(teams_dir, team_name, member_name)?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub fn emit_compaction_delivery_event(
    team_name: &str,
    member_name: &str,
    tool: CliTool,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
    result: CompactionDeliveryResult,
    skip_reason: Option<&str>,
    fail_reason: Option<&str>,
    delivery: Option<&str>,
) {
    let event = match result {
        CompactionDeliveryResult::Injected => "compaction.injected",
        CompactionDeliveryResult::Skipped => "compaction.skipped",
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
    emit_compaction_delivery(
        event,
        CompactionDeliveryEvent {
            delivery: delivery.map(ToOwned::to_owned),
            tool,
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
            session_id: session_id.to_string(),
            compaction_timestamp,
            delivery_result: serde_json::to_value(result)
                .ok()
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
                .unwrap_or_else(|| "unknown".to_string()),
            skip_reason: skip_reason.map(ToOwned::to_owned),
            fail_reason: fail_reason.map(ToOwned::to_owned),
        },
    );
}

use super::lock::is_windows_unsupported_rename_error;

fn compaction_state_dir(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir.join(team_name).join("state").join("compaction")
}

fn compaction_state_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    compaction_state_dir(teams_dir, team_name).join(format!("{member_name}.json"))
}

fn compaction_state_tmp_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    compaction_state_dir(teams_dir, team_name).join(format!("{member_name}.json.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use taurhaus_lib::logging::{install_global_sink, LogFileState};

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("timestamp")
            .with_timezone(&Utc)
    }

    fn sample_state() -> MemberCompactionState {
        MemberCompactionState {
            source: None,
            host_boundary: None,
            journal: None,
            pending: false,
            pending_obligation: None,
            satisfied_by: None,
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
                pending: false,
                pending_obligation: None,
                satisfied_by: None,
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
    fn prune_state_if_session_mismatch_removes_old_state() {
        let tmp = tempfile::tempdir().expect("tempdir");
        MemberCompactionStore::save(tmp.path(), "taurhaus-team", "architect", &sample_state())
            .expect("save state");

        let pruned = prune_state_if_session_mismatch(
            tmp.path(),
            "taurhaus-team",
            "architect",
            Some("session-2"),
        )
        .expect("prune mismatch");

        assert!(pruned);
        assert!(
            MemberCompactionStore::load(tmp.path(), "taurhaus-team", "architect")
                .expect("load state")
                .is_none()
        );
    }

    #[test]
    fn prune_state_if_session_mismatch_keeps_matching_state() {
        let tmp = tempfile::tempdir().expect("tempdir");
        MemberCompactionStore::save(tmp.path(), "taurhaus-team", "architect", &sample_state())
            .expect("save state");

        let pruned = prune_state_if_session_mismatch(
            tmp.path(),
            "taurhaus-team",
            "architect",
            Some("session-1"),
        )
        .expect("prune mismatch");

        assert!(!pruned);
        assert!(
            MemberCompactionStore::load(tmp.path(), "taurhaus-team", "architect")
                .expect("load state")
                .is_some()
        );
    }

    #[test]
    fn emit_compaction_delivery_event_includes_skip_and_fail_reason() {
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let log_path = tmp.path().join("compaction-delivery.log.jsonl");
        let log_state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&log_state);

        emit_compaction_delivery_event(
            "taurhaus-team",
            "developer1",
            CliTool::Codex,
            "session-1",
            timestamp("2026-03-08T14:30:16Z"),
            CompactionDeliveryResult::Failed,
            Some("intervening_user_message"),
            Some("append_inbox_failed"),
            None,
        );

        let contents = wait_for_log_contains(&log_path, "\"event\":\"compaction.failed\"");
        assert!(contents.contains("\"skip_reason\":\"intervening_user_message\""));
        assert!(contents.contains("\"fail_reason\":\"append_inbox_failed\""));
    }

    fn wait_for_log_contains(path: &std::path::Path, needle: &str) -> String {
        for _ in 0..50 {
            if let Ok(contents) = std::fs::read_to_string(path) {
                if contents.contains(needle) {
                    return contents;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        std::fs::read_to_string(path).unwrap_or_default()
    }
}
