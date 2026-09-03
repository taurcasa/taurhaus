//! Append-only pointer manifests for managed team account switches.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
        match serde_json::from_str::<AccountSwitchManifestState>(&raw) {
            Ok(state) => Ok(state.manifests),
            Err(error) => {
                let quarantine = corrupt_manifest_path(&path);
                let quarantine_error = fs::rename(&path, &quarantine).err();
                tracing::warn!(
                    team = %team_name,
                    path = %path.display(),
                    quarantine = %quarantine.display(),
                    error = %error,
                    quarantine_error = ?quarantine_error,
                    "account-switch manifest was unreadable; continuing with empty history"
                );
                let mut fields = Map::new();
                fields.insert("team".to_string(), Value::String(team_name.to_string()));
                fields.insert(
                    "path".to_string(),
                    Value::String(path.display().to_string()),
                );
                fields.insert(
                    "quarantine_path".to_string(),
                    Value::String(quarantine.display().to_string()),
                );
                taurhaus_lib::logging::emit_global(
                    "warn",
                    "coordination",
                    "coordination.account_switch.manifest_unreadable",
                    Some(
                        "Unreadable switch history was quarantined; the switch can continue"
                            .to_string(),
                    ),
                    fields,
                );
                Ok(Vec::new())
            }
        }
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

fn corrupt_manifest_path(path: &Path) -> PathBuf {
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.fZ");
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".corrupt.{timestamp}"));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::requests::AccountSwitchHandoffManifest;
    use crate::session_scanner::cli_tool::CliTool;

    // Regression: 0bc79ceb propagated a corrupt append-only pointer manifest
    // after teardown, permanently blocking every later switch for the team.
    #[test]
    fn append_quarantines_an_unreadable_manifest_and_starts_fresh() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let path = manifest_path(temp.path(), "arch");
        std::fs::create_dir_all(path.parent().expect("state dir")).expect("create state");
        std::fs::write(&path, "{truncated").expect("seed corrupt manifest");

        let count = AccountSwitchManifestStore::append(
            temp.path(),
            "arch",
            AccountSwitchHandoffManifest {
                switched_at: chrono::Utc::now(),
                cli_tool: CliTool::Codex,
                account_id: "work".to_string(),
                account_label: "Work".to_string(),
                members: Vec::new(),
                team_state_move: None,
            },
        )
        .expect("corrupt history is non-blocking");

        assert_eq!(count, 1);
        assert_eq!(
            AccountSwitchManifestStore::load(temp.path(), "arch")
                .expect("fresh manifest")
                .len(),
            1
        );
        assert!(path
            .parent()
            .expect("state dir")
            .read_dir()
            .expect("read state")
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().contains(".corrupt.")));
    }
}
