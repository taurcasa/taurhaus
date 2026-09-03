//! Bootstrap authority for locating team state before a team's config can be read.
//!
//! The registry always lives under the default teams root. Reads are deliberately
//! side-effect free so an existing installation with no registry entry follows
//! exactly the historical `<default>/teams/<team>` path.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::TeamConfigStore;

const REGISTRY_DIRNAME: &str = ".taurhaus";
const REGISTRY_FILENAME: &str = "team-roots.json";
const REGISTRY_LOCK_FILENAME: &str = "team-roots.lock";
const REGISTRY_TMP_FILENAME: &str = "team-roots.json.tmp";

pub(crate) fn normalize_teams_root(root: &Path) -> PathBuf {
    PathBuf::from(crate::provider::path::normalize_project_path(
        &root.to_string_lossy(),
    ))
}

pub(crate) fn same_teams_root(left: &Path, right: &Path) -> bool {
    normalize_teams_root(left) == normalize_teams_root(right)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamRootRegistryState {
    #[serde(default = "schema_version")]
    schema_version: u32,
    #[serde(default)]
    teams: BTreeMap<String, PathBuf>,
}

const fn schema_version() -> u32 {
    1
}

#[derive(Debug, Clone)]
pub struct TeamRootRegistry {
    default_teams_dir: PathBuf,
    reader_wsl_distro: Option<String>,
}

impl TeamRootRegistry {
    pub fn new(default_teams_dir: PathBuf) -> Self {
        #[cfg(target_os = "windows")]
        let reader_wsl_distro =
            crate::coordination::mesh_cli::resolve_wsl_distro_for_coordination(None);
        #[cfg(not(target_os = "windows"))]
        let reader_wsl_distro = None;
        Self {
            default_teams_dir,
            reader_wsl_distro,
        }
    }

    #[cfg(test)]
    fn with_reader_wsl_distro(
        default_teams_dir: PathBuf,
        reader_wsl_distro: Option<String>,
    ) -> Self {
        Self {
            default_teams_dir,
            reader_wsl_distro,
        }
    }

    pub fn default_teams_dir(&self) -> &Path {
        &self.default_teams_dir
    }

    pub fn path(&self) -> PathBuf {
        self.registry_dir().join(REGISTRY_FILENAME)
    }

    pub fn resolve(&self, team_name: &str) -> Result<PathBuf, CoordinationError> {
        Ok(self
            .load()?
            .teams
            .get(team_name)
            .cloned()
            .map(|root| self.path_for_reader(root))
            .unwrap_or_else(|| self.default_teams_dir.clone()))
    }

    pub fn registered(&self) -> Result<BTreeMap<String, PathBuf>, CoordinationError> {
        Ok(self
            .load()?
            .teams
            .into_iter()
            .map(|(team_name, root)| (team_name, self.path_for_reader(root)))
            .collect())
    }

    pub fn roots(&self) -> Result<Vec<PathBuf>, CoordinationError> {
        let additional = self.registered()?.into_values().collect::<BTreeSet<_>>();
        let mut roots = Vec::with_capacity(additional.len() + 1);
        roots.push(self.default_teams_dir.clone());
        for root in additional {
            if !roots
                .iter()
                .any(|existing| same_teams_root(existing, &root))
            {
                roots.push(root);
            }
        }
        Ok(roots)
    }

    /// Enumerate only team directories that agree with bootstrap authority.
    pub fn team_locations(&self) -> Result<Vec<(PathBuf, String)>, CoordinationError> {
        let registered = self.registered()?;
        let mut locations = Vec::new();
        for root in self.roots()? {
            for team_name in TeamConfigStore::list(&root)? {
                if team_name.starts_with('.') {
                    continue;
                }
                let authoritative = registered
                    .get(&team_name)
                    .unwrap_or(&self.default_teams_dir);
                if same_teams_root(authoritative, &root) {
                    locations.push((root.clone(), team_name));
                }
            }
        }
        Ok(locations)
    }

    /// Daemon-owned commit of one team authority. Pointing at the default root
    /// removes the entry, preserving the zero-migration representation.
    pub(crate) fn set(&self, team_name: &str, teams_dir: &Path) -> Result<(), CoordinationError> {
        let registry_dir = self.registry_dir();
        fs::create_dir_all(&registry_dir)?;
        let lock_path = registry_dir.join(REGISTRY_LOCK_FILENAME);
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.lock_exclusive()?;

        let mut state = self.load()?;
        state.schema_version = schema_version();
        if same_teams_root(teams_dir, &self.default_teams_dir) {
            state.teams.remove(team_name);
        } else {
            state
                .teams
                .insert(team_name.to_string(), teams_dir.to_path_buf());
        }
        let payload = serde_json::to_vec_pretty(&state).map_err(|error| {
            CoordinationError::StoreError(format!(
                "failed to serialize team-root registry: {error}"
            ))
        })?;
        let tmp_path = registry_dir.join(REGISTRY_TMP_FILENAME);
        let mut tmp = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)?;
        tmp.write_all(&payload)?;
        tmp.sync_all()?;
        fs::rename(tmp_path, self.path())?;
        Ok(())
    }

    fn registry_dir(&self) -> PathBuf {
        self.default_teams_dir
            .parent()
            .unwrap_or(&self.default_teams_dir)
            .join(REGISTRY_DIRNAME)
    }

    fn path_for_reader(&self, path: PathBuf) -> PathBuf {
        let raw = path.to_string_lossy();
        match self.reader_wsl_distro.as_deref() {
            Some(distro) if raw.starts_with('/') => {
                PathBuf::from(crate::provider::path::to_windows(&raw, distro))
            }
            _ => path,
        }
    }

    fn load(&self) -> Result<TeamRootRegistryState, CoordinationError> {
        let path = self.path();
        let raw = match super::lock::read_to_string_with_retry(&path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(TeamRootRegistryState::default());
            }
            Err(error) => return Err(CoordinationError::Io(error)),
        };
        serde_json::from_str(&raw).map_err(|error| {
            CoordinationError::StoreError(format!(
                "failed to parse team-root registry at {}: {error}",
                path.display()
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{same_teams_root, TeamRootRegistry};

    #[test]
    fn root_identity_normalizes_equivalent_wsl_unc_spellings() {
        // Regression: 18810949 compared team roots as raw PathBuf values,
        // allowing WSL UNC aliases for one directory to create two authorities.
        assert!(same_teams_root(
            std::path::Path::new(r"\\wsl$\Ubuntu\home\user\.claude\teams"),
            std::path::Path::new(r"\\wsl.localhost\Ubuntu\home\user\.claude\teams"),
        ));
    }

    #[test]
    fn missing_registry_is_a_byte_identical_default_root_lookup() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let default_teams_dir = temp.path().join("default-account").join("teams");
        let registry = TeamRootRegistry::new(default_teams_dir.clone());

        assert_eq!(
            registry.resolve("legacy-team").expect("resolve"),
            default_teams_dir
        );
        assert_eq!(
            registry.roots().expect("roots"),
            vec![default_teams_dir.clone()]
        );
        assert!(
            !registry.path().exists(),
            "a read must not migrate or write state"
        );
    }

    #[test]
    fn registry_round_trips_team_roots_and_enumerates_each_root_once() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let default_teams_dir = temp.path().join("default-account").join("teams");
        let work_teams_dir = temp.path().join("work-account").join("teams");
        let registry = TeamRootRegistry::new(default_teams_dir.clone());

        registry
            .set("work-team", &work_teams_dir)
            .expect("set work team");
        registry
            .set("other-work-team", &work_teams_dir)
            .expect("set second work team");

        assert_eq!(
            registry.resolve("work-team").expect("resolve"),
            work_teams_dir
        );
        assert_eq!(
            registry.resolve("legacy-team").expect("resolve"),
            default_teams_dir
        );
        assert_eq!(
            registry.roots().expect("roots"),
            vec![default_teams_dir, work_teams_dir]
        );
    }

    #[test]
    fn host_reader_converts_daemon_native_registered_roots_to_wsl_unc() {
        // Regression: 42840d4a persisted the daemon's Linux-native account
        // root, then handed it unchanged to Windows app filesystem readers.
        let temp = tempfile::TempDir::new().expect("tempdir");
        let default_teams_dir = temp.path().join("default-account/teams");
        let registry = TeamRootRegistry::with_reader_wsl_distro(
            default_teams_dir,
            Some("Ubuntu".to_string()),
        );
        registry
            .set(
                "work-team",
                std::path::Path::new("/home/user/.claude-work/teams"),
            )
            .expect("set daemon-native root");

        assert_eq!(
            registry.resolve("work-team").expect("host resolve"),
            std::path::PathBuf::from(
                r"\\wsl.localhost\Ubuntu\home\user\.claude-work\teams"
            )
        );
        assert_eq!(
            registry
                .registered()
                .expect("host registered roots")
                .get("work-team"),
            Some(&std::path::PathBuf::from(
                r"\\wsl.localhost\Ubuntu\home\user\.claude-work\teams"
            ))
        );
        assert_eq!(
            registry.roots().expect("host roots")[1],
            std::path::PathBuf::from(
                r"\\wsl.localhost\Ubuntu\home\user\.claude-work\teams"
            )
        );
    }

    #[test]
    fn daemon_reader_keeps_registered_roots_linux_native() {
        // Regression: 42840d4a requires conversion only at the Windows app
        // read boundary; the WSL daemon must retain its native path.
        let temp = tempfile::TempDir::new().expect("tempdir");
        let default_teams_dir = temp.path().join("default-account/teams");
        let registry = TeamRootRegistry::with_reader_wsl_distro(default_teams_dir, None);
        let work_teams_dir = std::path::PathBuf::from("/home/user/.claude-work/teams");
        registry
            .set("work-team", &work_teams_dir)
            .expect("set daemon-native root");

        assert_eq!(
            registry.resolve("work-team").expect("daemon resolve"),
            work_teams_dir
        );
    }

    #[test]
    fn hidden_move_directories_are_not_team_locations() {
        // Regression: 42840d4a staged cross-device moves inside teams roots,
        // allowing a leftover hidden backup to enumerate as a real team.
        let temp = tempfile::TempDir::new().expect("tempdir");
        let default_teams_dir = temp.path().join("default-account/teams");
        std::fs::create_dir_all(default_teams_dir.join(".arch.taurhaus-backup-deadbeef"))
            .expect("hidden backup");
        std::fs::create_dir_all(default_teams_dir.join(".arch.taurhaus-move-deadbeef"))
            .expect("hidden staging");
        let registry = TeamRootRegistry::new(default_teams_dir);

        assert!(registry.team_locations().expect("locations").is_empty());
    }
}
