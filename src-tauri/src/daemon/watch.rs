use std::collections::HashMap;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{Config, Event as NotifyEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::daemon::protocol::{self, DaemonEvent, DaemonResponse};
use crate::fs::watcher::{classify_event, EventClass};

/// Duration to debounce git internal events pushed to clients.
pub(crate) const WATCH_GIT_DEBOUNCE_SECS: u64 = 2;

/// Handle a `watch` request: start an inotify/notify watcher for the path
/// and push classified events to the client as DaemonEvents.
pub(crate) fn handle_watch(
    id: &str,
    params: &serde_json::Value,
    writer: &Arc<Mutex<TcpStream>>,
    active_watches: &mut HashMap<String, RecommendedWatcher>,
    git_debounce: &Arc<Mutex<HashMap<String, Instant>>>,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };

    let path = PathBuf::from(&params.path);
    if !path.is_dir() {
        return DaemonResponse::err(
            id,
            "NOT_FOUND",
            format!("Path does not exist or is not a directory: {}", params.path),
        );
    }
    // Canonicalize the path to resolve symlinks (critical on macOS where
    // /var → /private/var; FSEvents watches the canonical path).
    let path = path.canonicalize().unwrap_or(path);

    // Already watching this path?
    if active_watches.contains_key(&params.path) {
        return DaemonResponse::ok(id, protocol::WatchResult { ok: true });
    }

    let writer_clone = writer.clone();
    // Use canonical path for event matching (FSEvents on macOS delivers
    // canonical paths, e.g. /private/var/... instead of /var/...).
    let watch_path = path.to_string_lossy().to_string();
    let debounce_clone = git_debounce.clone();

    let watcher_result = RecommendedWatcher::new(
        move |res: Result<NotifyEvent, notify::Error>| {
            if let Ok(event) = res {
                forward_watch_event(&writer_clone, &watch_path, &debounce_clone, event);
            }
        },
        Config::default(),
    );

    let mut watcher = match watcher_result {
        Ok(w) => w,
        Err(e) => return DaemonResponse::err(id, "WATCH_ERROR", e.to_string()),
    };

    if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
        return DaemonResponse::err(id, "WATCH_ERROR", e.to_string());
    }

    tracing::info!(path = %params.path, "Started watching directory");
    active_watches.insert(params.path, watcher);
    DaemonResponse::ok(id, protocol::WatchResult { ok: true })
}

/// Handle an `unwatch` request: stop watching the specified path.
pub(crate) fn handle_unwatch(
    id: &str,
    params: &serde_json::Value,
    active_watches: &mut HashMap<String, RecommendedWatcher>,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };

    if active_watches.remove(&params.path).is_some() {
        tracing::info!(path = %params.path, "Stopped watching directory");
    }
    DaemonResponse::ok(id, protocol::WatchResult { ok: true })
}

/// Convert an absolute file path to a project-relative string.
///
/// Falls back to the absolute path if `strip_prefix` fails.
pub(crate) fn relative_to(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|r| r.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

/// Classify a notify event and push the appropriate DaemonEvent to the client.
///
/// Uses the same `classify_event` logic as the local `ProjectWatcher` to ensure
/// consistent event classification between local and daemon-forwarded watching.
pub(crate) fn forward_watch_event(
    writer: &Arc<Mutex<TcpStream>>,
    project_path: &str,
    debounce: &Arc<Mutex<HashMap<String, Instant>>>,
    event: NotifyEvent,
) {
    // Only care about create, modify, remove events
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return,
    }

    let project_root = Path::new(project_path);
    let mut regular_files = Vec::new();

    for path in &event.paths {
        let Some(class) = classify_event(project_root, path) else {
            continue;
        };

        match class {
            EventClass::GitInternal => {
                // Debounce: only emit if enough time has passed
                if let Ok(mut state) = debounce.lock() {
                    let now = Instant::now();
                    let should_emit = state.get(project_path).is_none_or(|last| {
                        now.duration_since(*last) >= Duration::from_secs(WATCH_GIT_DEBOUNCE_SECS)
                    });

                    if should_emit {
                        state.insert(project_path.to_string(), now);
                        crate::daemon::server::push_event(
                            writer,
                            &DaemonEvent::new(
                                protocol::event::GIT_CHANGED,
                                protocol::GitChangedData {
                                    path: project_path.to_string(),
                                },
                            ),
                        );
                    }
                }
            }
            EventClass::SessionFile => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    crate::daemon::server::push_event(
                        writer,
                        &DaemonEvent::new(
                            protocol::event::SESSION_FILE_CREATED,
                            protocol::SessionFileCreatedData {
                                path: project_path.to_string(),
                                file: relative_to(path, project_root),
                            },
                        ),
                    );
                }
            }
            EventClass::GitignoreChange | EventClass::RegularFile => {
                regular_files.push(relative_to(path, project_root));
            }
        }
    }

    if !regular_files.is_empty() {
        crate::daemon::server::push_event(
            writer,
            &DaemonEvent::new(
                protocol::event::FILE_CHANGED,
                protocol::FileChangedData {
                    path: project_path.to_string(),
                    files: regular_files,
                },
            ),
        );
    }
}
