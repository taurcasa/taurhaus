//! Taurhaus-owned initialize exclusion; Mesh never writes this directory.
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::errors::CoordinationError;

pub(crate) fn path(root: &Path, team: &str) -> PathBuf {
    root.join(".taurhaus-initialize-pending")
        .join(format!("{team}.guard"))
}

pub(crate) fn begin(root: &Path, team: &str) -> Result<(), CoordinationError> {
    let path = path(root, team);
    std::fs::create_dir_all(path.parent().expect("guard directory"))?;
    std::fs::write(path, b"initialize\n")?;
    Ok(())
}

pub(crate) fn active(root: &Path, team: &str) -> bool {
    let path = path(root, team);
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                diagnostic_once(&path, team, "unreadable_guard");
            }
            return false;
        }
    };
    let stale = metadata
        .modified()
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age > Duration::from_secs(30 * 60));
    // Removal latches the event across passes and restarts.
    if stale {
        if std::fs::remove_file(&path).is_ok() {
            event("coordination.initialize.stale_guard", team, "expired");
        } else {
            diagnostic_once(&path, team, "expired_cleanup_failed");
        }
        return false;
    }
    true
}

pub(crate) fn clear(root: &Path, team: &str) {
    if let Err(error) = std::fs::remove_file(path(root, team)) {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(%error, team, "failed to remove initialize guard");
        }
    }
}

fn diagnostic_once(path: &Path, team: &str, reason: &str) {
    use std::sync::{LazyLock, Mutex};
    static REPORTED: LazyLock<Mutex<std::collections::HashSet<PathBuf>>> =
        LazyLock::new(Default::default);
    let mut reported = REPORTED.lock().unwrap_or_else(|e| e.into_inner());
    if reported.len() < 4096 && reported.insert(path.into()) {
        event("coordination.initialize.stale_guard", team, reason);
    }
}

pub(crate) fn event(event: &str, team: &str, reason: &str) {
    taurhaus_lib::logging::emit_global(
        "info",
        "coordination",
        event,
        None,
        serde_json::Map::from_iter([
            ("team".into(), team.into()),
            ("reason".into(), reason.into()),
        ]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression: 77616a34, attempt 10 / L4 run 3 fix: bad guard metadata
    // permanently disabled healing; c9e18117 put the guard in Mesh's team tree.
    #[test]
    fn initialize_owner_race_guard_is_outside_team_and_bad_metadata_is_inactive() {
        let tmp = tempfile::tempdir().unwrap();
        let guard = path(tmp.path(), "race");
        assert!(!guard.starts_with(tmp.path().join("race")));
        std::fs::create_dir_all(guard.parent().unwrap().parent().unwrap()).unwrap();
        std::fs::write(guard.parent().unwrap(), b"not a directory").unwrap();
        assert!(!active(tmp.path(), "race"));
    }
}
