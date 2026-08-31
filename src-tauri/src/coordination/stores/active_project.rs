//! Persisted project -> active team mapping for Mesh restoration.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::coordination::errors::CoordinationError;

const ACTIVE_PROJECTS_FILENAME: &str = ".active-project-teams.json";
const ACTIVE_PROJECTS_TMP_FILENAME: &str = ".active-project-teams.json.tmp";
const ACTIVE_PROJECTS_LOCK_NAME: &str = "_active-project-teams";
const ACTIVE_PROJECTS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ActiveProjectTeamState {
    #[serde(default = "active_projects_schema_version")]
    schema_version: u32,
    #[serde(default)]
    project_teams: BTreeMap<String, ActiveProjectTeamEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ActiveProjectTeamEntry {
    team_name: String,
    updated_at: DateTime<Utc>,
}

fn active_projects_schema_version() -> u32 {
    ACTIVE_PROJECTS_SCHEMA_VERSION
}

#[derive(Debug, Default)]
pub struct ActiveProjectTeamStore;

impl ActiveProjectTeamStore {
    pub fn load_active_team(
        teams_dir: &Path,
        project_path: &str,
    ) -> Result<Option<String>, CoordinationError> {
        let normalized = normalize_project_path(project_path);
        if normalized.is_empty() {
            return Ok(None);
        }

        let state = load_state(teams_dir)?;
        Ok(state
            .project_teams
            .get(&normalized)
            .map(|entry| entry.team_name.clone()))
    }

    pub fn sync_team(
        teams_dir: &Path,
        team_name: &str,
        project_paths: &[String],
    ) -> Result<(), CoordinationError> {
        let _lock = super::lock::acquire_team_lock(teams_dir, ACTIVE_PROJECTS_LOCK_NAME)?;
        let mut state = load_state(teams_dir)?;
        state
            .project_teams
            .retain(|_, entry| entry.team_name != team_name);

        let updated_at = Utc::now();
        for project_path in normalize_project_paths(project_paths) {
            state.project_teams.insert(
                project_path,
                ActiveProjectTeamEntry {
                    team_name: team_name.to_string(),
                    updated_at,
                },
            );
        }

        save_state(teams_dir, &state)
    }

    pub fn clear_team(teams_dir: &Path, team_name: &str) -> Result<(), CoordinationError> {
        let _lock = super::lock::acquire_team_lock(teams_dir, ACTIVE_PROJECTS_LOCK_NAME)?;
        let mut state = load_state(teams_dir)?;
        state
            .project_teams
            .retain(|_, entry| entry.team_name != team_name);
        save_state(teams_dir, &state)
    }

    pub fn clear_project(teams_dir: &Path, project_path: &str) -> Result<(), CoordinationError> {
        let normalized = normalize_project_path(project_path);
        if normalized.is_empty() {
            return Ok(());
        }

        let _lock = super::lock::acquire_team_lock(teams_dir, ACTIVE_PROJECTS_LOCK_NAME)?;
        let mut state = load_state(teams_dir)?;
        state.project_teams.remove(&normalized);
        save_state(teams_dir, &state)
    }

    pub fn set_active_team(
        teams_dir: &Path,
        project_path: &str,
        team_name: &str,
    ) -> Result<(), CoordinationError> {
        let normalized = normalize_project_path(project_path);
        if normalized.is_empty() {
            return Ok(());
        }

        let _lock = super::lock::acquire_team_lock(teams_dir, ACTIVE_PROJECTS_LOCK_NAME)?;
        let mut state = load_state(teams_dir)?;
        state.project_teams.insert(
            normalized,
            ActiveProjectTeamEntry {
                team_name: team_name.to_string(),
                updated_at: Utc::now(),
            },
        );
        save_state(teams_dir, &state)
    }
}

fn load_state(teams_dir: &Path) -> Result<ActiveProjectTeamState, CoordinationError> {
    let path = active_projects_path(teams_dir);
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ActiveProjectTeamState::default());
        }
        Err(err) => return Err(CoordinationError::Io(err)),
    };

    serde_json::from_str(&raw).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to parse active project team state at {}: {err}",
            path.display()
        ))
    })
}

fn save_state(teams_dir: &Path, state: &ActiveProjectTeamState) -> Result<(), CoordinationError> {
    fs::create_dir_all(teams_dir)?;

    let mut normalized = state.clone();
    normalized.schema_version = ACTIVE_PROJECTS_SCHEMA_VERSION;

    let payload = serde_json::to_string_pretty(&normalized).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to serialize active project team state: {err}"
        ))
    })?;

    let path = active_projects_path(teams_dir);
    let tmp_path = teams_dir.join(ACTIVE_PROJECTS_TMP_FILENAME);
    fs::write(&tmp_path, payload.as_bytes())?;
    if let Err(err) = fs::rename(&tmp_path, &path) {
        // The last step of a successful team init runs through here; it must
        // degrade on a volume that refuses atomic replacement like every
        // other teams-dir store, not fail the init.
        if super::lock::is_windows_unsupported_rename_error(&err) {
            tracing::warn!(
                target = %path.display(),
                raw_os_error = ?err.raw_os_error(),
                "atomic active-project rename failed; falling back to direct write"
            );
            super::lock::report_atomic_write_degraded(&path, "active_project", err.raw_os_error());
            if let Err(write_err) = super::lock::write_direct_synced(&path, payload.as_bytes()) {
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

fn active_projects_path(teams_dir: &Path) -> std::path::PathBuf {
    teams_dir.join(ACTIVE_PROJECTS_FILENAME)
}

fn normalize_project_path(project_path: &str) -> String {
    let trimmed = project_path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    crate::provider::path::normalize_project_path(trimmed)
}

fn normalize_project_paths(project_paths: &[String]) -> BTreeSet<String> {
    project_paths
        .iter()
        .map(|project_path| normalize_project_path(project_path))
        .filter(|project_path| !project_path.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn sync_team_persists_active_team_per_project() {
        let tmp = TempDir::new().expect("tempdir");

        ActiveProjectTeamStore::sync_team(
            tmp.path(),
            "taurhaus-team",
            &[
                "/projects/taurhaus".to_string(),
                "/projects/api".to_string(),
                "/projects/taurhaus".to_string(),
            ],
        )
        .expect("sync team");

        assert_eq!(
            ActiveProjectTeamStore::load_active_team(tmp.path(), "/projects/taurhaus")
                .expect("load active team")
                .as_deref(),
            Some("taurhaus-team")
        );
        assert_eq!(
            ActiveProjectTeamStore::load_active_team(tmp.path(), "/projects/api")
                .expect("load active team")
                .as_deref(),
            Some("taurhaus-team")
        );
    }

    #[test]
    fn sync_team_replaces_previous_mapping_for_same_project() {
        let tmp = TempDir::new().expect("tempdir");

        ActiveProjectTeamStore::sync_team(
            tmp.path(),
            "towerhouse-product-team",
            &["/projects/taurhaus".to_string()],
        )
        .expect("seed old team");
        ActiveProjectTeamStore::sync_team(
            tmp.path(),
            "taurhaus-team",
            &["/projects/taurhaus".to_string()],
        )
        .expect("seed new team");

        assert_eq!(
            ActiveProjectTeamStore::load_active_team(tmp.path(), "/projects/taurhaus")
                .expect("load active team")
                .as_deref(),
            Some("taurhaus-team")
        );
    }

    #[test]
    fn clear_team_removes_all_team_mappings() {
        let tmp = TempDir::new().expect("tempdir");

        ActiveProjectTeamStore::sync_team(
            tmp.path(),
            "taurhaus-team",
            &[
                "/projects/taurhaus".to_string(),
                "/projects/api".to_string(),
            ],
        )
        .expect("sync team");
        ActiveProjectTeamStore::clear_team(tmp.path(), "taurhaus-team").expect("clear team");

        assert_eq!(
            ActiveProjectTeamStore::load_active_team(tmp.path(), "/projects/taurhaus")
                .expect("load active team"),
            None
        );
        assert_eq!(
            ActiveProjectTeamStore::load_active_team(tmp.path(), "/projects/api")
                .expect("load active team"),
            None
        );
    }

    #[test]
    fn set_active_team_persists_single_project_mapping() {
        let tmp = TempDir::new().expect("tempdir");

        ActiveProjectTeamStore::sync_team(
            tmp.path(),
            "towerhouse-product-team",
            &[
                "/projects/taurhaus".to_string(),
                "/projects/docs".to_string(),
            ],
        )
        .expect("seed old team");

        ActiveProjectTeamStore::set_active_team(tmp.path(), "/projects/taurhaus", "taurhaus-team")
            .expect("set active team");

        assert_eq!(
            ActiveProjectTeamStore::load_active_team(tmp.path(), "/projects/taurhaus")
                .expect("load taurhaus")
                .as_deref(),
            Some("taurhaus-team")
        );
        assert_eq!(
            ActiveProjectTeamStore::load_active_team(tmp.path(), "/projects/docs")
                .expect("load docs")
                .as_deref(),
            Some("towerhouse-product-team")
        );
    }
}
