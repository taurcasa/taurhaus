use std::sync::{LazyLock, Mutex};

use crate::daemon::protocol::RuntimeSessionSnapshotResult;

static SESSION_SNAPSHOT_CACHE: LazyLock<Mutex<Option<RuntimeSessionSnapshotResult>>> =
    LazyLock::new(|| Mutex::new(None));

pub(crate) fn store(snapshot: &RuntimeSessionSnapshotResult) {
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
