//! Operational context snapshot store.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::coordination::errors::CoordinationError;
use crate::coordination::mesh_cli;

const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";
const OPERATIONAL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalTaskSnapshot {
    pub id: String,
    pub subject: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalAssignmentFooterSnapshot {
    #[serde(default)]
    pub execution_mode: String,
    #[serde(default)]
    pub file_ownership_boundary: Vec<String>,
    #[serde(default)]
    pub adjacent_fix_policy: String,
    #[serde(default)]
    pub validation_expectation: String,
    #[serde(default)]
    pub response_expectation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalOwnershipSnapshot {
    pub override_allowed: bool,
    pub active_override_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalWorkingSetSnapshot {
    #[serde(default)]
    pub project_path: String,
    #[serde(default)]
    pub focal_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalContextSnapshot {
    pub version: u32,
    pub team_name: String,
    pub member_name: String,
    pub updated_at: DateTime<Utc>,
    pub task: OperationalTaskSnapshot,
    pub assignment_footer: OperationalAssignmentFooterSnapshot,
    pub ownership: OperationalOwnershipSnapshot,
    pub working_set: OperationalWorkingSetSnapshot,
}

#[derive(Debug, Default)]
pub struct OperationalContextSnapshotStore;

impl OperationalContextSnapshotStore {
    pub fn load(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
    ) -> Result<Option<OperationalContextSnapshot>, CoordinationError> {
        let path = operational_snapshot_path(teams_dir, team_name, member_name);
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(CoordinationError::Io(err)),
        };

        let snapshot = serde_json::from_str::<OperationalContextSnapshot>(&raw).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to parse operational snapshot for member '{member_name}' in team '{team_name}': {err}"
            ))
        })?;

        Ok(Some(snapshot))
    }

    pub fn save(
        teams_dir: &Path,
        snapshot: &OperationalContextSnapshot,
    ) -> Result<(), CoordinationError> {
        let _lock = super::lock::acquire_team_lock(teams_dir, &snapshot.team_name)?;

        let mut normalized = snapshot.clone();
        normalized.version = OPERATIONAL_SCHEMA_VERSION;

        let operational_dir = operational_snapshot_dir(teams_dir, &normalized.team_name);
        fs::create_dir_all(&operational_dir)?;

        let target_path =
            operational_snapshot_path(teams_dir, &normalized.team_name, &normalized.member_name);
        let tmp_path = operational_snapshot_tmp_path(
            teams_dir,
            &normalized.team_name,
            &normalized.member_name,
        );
        let payload = serde_json::to_string_pretty(&normalized).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to serialize operational snapshot for member '{}': {err}",
                normalized.member_name
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
}

pub fn read_snapshot(team_name: &str, member_name: &str) -> Option<OperationalContextSnapshot> {
    match OperationalContextSnapshotStore::load(
        &default_operational_teams_dir(),
        team_name,
        member_name,
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::warn!(
                team_name = team_name,
                member_name = member_name,
                error = %error,
                "failed to read operational snapshot"
            );
            None
        }
    }
}

pub fn write_snapshot(snapshot: &OperationalContextSnapshot) -> Result<(), CoordinationError> {
    OperationalContextSnapshotStore::save(&default_operational_teams_dir(), snapshot)
}

pub(crate) fn default_operational_teams_dir() -> PathBuf {
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

fn operational_snapshot_dir(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir.join(team_name).join("state").join("operational")
}

fn operational_snapshot_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    operational_snapshot_dir(teams_dir, team_name).join(format!("{member_name}.json"))
}

fn operational_snapshot_tmp_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    operational_snapshot_dir(teams_dir, team_name).join(format!("{member_name}.json.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot() -> OperationalContextSnapshot {
        OperationalContextSnapshot {
            version: 99,
            team_name: "taurhaus-team".to_string(),
            member_name: "developer1".to_string(),
            updated_at: DateTime::parse_from_rfc3339("2026-03-08T14:10:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
            task: OperationalTaskSnapshot {
                id: "674".to_string(),
                subject: "Add canonical OperationalContextSnapshot model and store".to_string(),
                status: "in_progress".to_string(),
            },
            assignment_footer: OperationalAssignmentFooterSnapshot {
                execution_mode: "implement".to_string(),
                file_ownership_boundary: vec![
                    "src-tauri/src/coordination/stores/operational.rs".to_string()
                ],
                adjacent_fix_policy: "no".to_string(),
                validation_expectation: "cargo check --tests".to_string(),
                response_expectation: "report-on-completion".to_string(),
            },
            ownership: OperationalOwnershipSnapshot {
                override_allowed: false,
                active_override_reason: None,
            },
            working_set: OperationalWorkingSetSnapshot {
                project_path: "/home/mstie/projects/taurhaus".to_string(),
                focal_files: vec![
                    "src-tauri/src/coordination/stores/operational.rs".to_string(),
                    "src-tauri/src/coordination/stores/mod.rs".to_string(),
                ],
            },
        }
    }

    #[test]
    fn operational_snapshot_round_trips_through_store() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let snapshot = sample_snapshot();

        OperationalContextSnapshotStore::save(tmp.path(), &snapshot).expect("save snapshot");
        let stored = OperationalContextSnapshotStore::load(
            tmp.path(),
            &snapshot.team_name,
            &snapshot.member_name,
        )
        .expect("load snapshot")
        .expect("snapshot should exist");

        assert_eq!(stored.version, OPERATIONAL_SCHEMA_VERSION);
        assert_eq!(
            stored,
            OperationalContextSnapshot {
                version: OPERATIONAL_SCHEMA_VERSION,
                ..snapshot
            }
        );
    }

    #[test]
    fn operational_snapshot_load_returns_none_when_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let loaded =
            OperationalContextSnapshotStore::load(tmp.path(), "missing-team", "missing-member")
                .expect("missing snapshot should not error");

        assert_eq!(loaded, None);
    }

    #[test]
    fn operational_snapshot_save_creates_operational_directory_lazily() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let snapshot = sample_snapshot();
        let dir = operational_snapshot_dir(tmp.path(), &snapshot.team_name);

        assert!(!dir.exists());

        OperationalContextSnapshotStore::save(tmp.path(), &snapshot).expect("save snapshot");

        assert!(dir.is_dir());
        assert!(
            operational_snapshot_path(tmp.path(), &snapshot.team_name, &snapshot.member_name)
                .is_file()
        );
    }
}
