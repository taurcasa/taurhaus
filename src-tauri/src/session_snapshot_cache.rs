use std::sync::{LazyLock, Mutex};

use crate::daemon::protocol::RuntimeSessionSnapshotResult;

static SESSION_SNAPSHOT_CACHE: LazyLock<Mutex<Option<RuntimeSessionSnapshotResult>>> =
    LazyLock::new(|| Mutex::new(None));

pub(crate) fn store(snapshot: &RuntimeSessionSnapshotResult) {
    // On Windows this is the only view of the WSL sessions the app gets, and
    // the account a session belongs to has to outlive it — Resume asks after
    // the process is gone.
    crate::session_scanner::claude_accounts::record_claude_transcripts(&snapshot.runtime_sessions);

    let mut guard = SESSION_SNAPSHOT_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    *guard = Some(snapshot.clone());
}

pub(crate) fn load() -> Option<RuntimeSessionSnapshotResult> {
    SESSION_SNAPSHOT_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn clear() {
    let mut guard = SESSION_SNAPSHOT_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    *guard = None;
}
