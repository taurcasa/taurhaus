//! Append-only pointer manifests for managed team account switches.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::AccountSwitchHandoffManifest;

const STATE_DIRNAME: &str = "state";
const MANIFEST_FILENAME: &str = "account-switches.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountSwitchManifestState {
    #[serde(default = "schema_version")]
    schema_version: u32,
    #[serde(default)]
    manifests: Vec<AccountSwitchHandoffManifest>,
}

fn schema_version() -> u32 {
    1
}

#[derive(Debug, Default)]
pub struct AccountSwitchManifestStore;

impl AccountSwitchManifestStore {
    pub fn load(
        teams_dir: &Path,
        team_name: &str,
    ) -> Result<Vec<AccountSwitchHandoffManifest>, CoordinationError> {
        let path = manifest_path(teams_dir, team_name);
        let raw = match super::lock::read_to_string_with_retry(&path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(CoordinationError::Io(error)),
        };
        serde_json::from_str::<AccountSwitchManifestState>(&raw)
            .map(|state| state.manifests)
            .map_err(|error| {
                CoordinationError::StoreError(format!(
                    "failed to parse account-switch manifests for '{team_name}': {error}"
                ))
            })
    }

    /// Append under the team's shared state lock and publish via an atomic
    /// move-aside replacement. Returns the accumulated manifest count.
    pub fn append(
        teams_dir: &Path,
        team_name: &str,
        manifest: AccountSwitchHandoffManifest,
    ) -> Result<usize, CoordinationError> {
        let _lock = super::lock::acquire_team_lock(teams_dir, team_name)?;
        let mut state = AccountSwitchManifestState {
            schema_version: schema_version(),
            manifests: Self::load(teams_dir, team_name)?,
        };
        state.manifests.push(manifest);
        let count = state.manifests.len();
        let payload = serde_json::to_vec_pretty(&state).map_err(|error| {
            CoordinationError::StoreError(format!(
                "failed to serialize account-switch manifests: {error}"
            ))
        })?;
        let path = manifest_path(teams_dir, team_name);
        let parent = path.parent().expect("manifest path has a state parent");
        fs::create_dir_all(parent)?;
        let staged = path.with_extension(format!("json.{}.tmp", std::process::id()));
        super::lock::stage_synced(&staged, &payload)?;
        if let Err(error) = super::lock::replace_via_move_aside(&staged, &path) {
            let _ = fs::remove_file(&staged);
            return Err(CoordinationError::Io(error));
        }
        Ok(count)
    }
}

fn manifest_path(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir
        .join(team_name)
        .join(STATE_DIRNAME)
        .join(MANIFEST_FILENAME)
}
