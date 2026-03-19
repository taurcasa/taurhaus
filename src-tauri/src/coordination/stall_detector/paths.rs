use std::path::PathBuf;

use crate::coordination::mesh_cli;

const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";

pub(super) fn default_coordination_teams_dir() -> PathBuf {
    if let Some(path) = std::env::var_os(CLAUDE_DIR_OVERRIDE_ENV) {
        if !path.is_empty() {
            return PathBuf::from(path).join("teams");
        }
    }
    if let Some(path) = mesh_cli::resolve_windows_mesh_teams_dir() {
        return path;
    }
    let base = if let Some(home_dir) = dirs::home_dir() {
        home_dir
    } else {
        let fallback = std::env::temp_dir().join("taurhaus-home");
        tracing::warn!(
            fallback = %fallback.display(),
            "home directory unavailable; falling back to temp directory for stall snapshot path"
        );
        fallback
    };
    base.join(".claude").join("teams")
}
