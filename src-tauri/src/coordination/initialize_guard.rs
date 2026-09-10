//! Taurhaus-owned initialize exclusion; Mesh never writes this directory.
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::errors::CoordinationError;

pub(crate) fn path(root: &Path, team: &str) -> PathBuf {
    root.join(team).join(".taurhaus/initialize-in-progress")
}

pub(crate) fn begin(root: &Path, team: &str) -> Result<(), CoordinationError> {
    let path = path(root, team);
    std::fs::create_dir_all(path.parent().expect("guard directory"))?;
    std::fs::write(path, b"initialize\n")?;
    Ok(())
}

pub(crate) fn active(root: &Path, team: &str) -> bool {
    let path = path(root, team);
    match std::fs::metadata(&path) {
        Ok(metadata) => {
            if metadata.modified().ok().and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > Duration::from_secs(30 * 60)) {
                // Removing only the stale guard latches the event across passes/restarts.
                if std::fs::remove_file(path).is_ok() {
                    event("coordination.initialize.stale_guard", team);
                    return false;
                }
            }
            true
        }
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

pub(crate) fn event(event: &str, team: &str) {
    taurhaus_lib::logging::emit_global("info", "coordination", event, None,
        serde_json::Map::from_iter([("team".into(), team.into())]));
}
