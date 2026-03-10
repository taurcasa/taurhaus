use std::collections::{HashMap, HashSet};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use ignore::gitignore::Gitignore;
use notify::{Config, Event as NotifyEvent, RecommendedWatcher, Watcher};

use crate::daemon::protocol::{self, DaemonEvent, DaemonResponse};
use crate::fs::watcher::{
    build_gitignore, classify_notify_event, reconcile_pruned_tree_watches,
    reconcile_pruned_tree_watches_for_event,
};

#[derive(Debug)]
pub(crate) struct DaemonWatchRegistration {
    pub(crate) watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    pub(crate) watched_dirs: Arc<Mutex<HashSet<PathBuf>>>,
}

/// Duration to debounce git internal events pushed to clients.
pub(crate) const WATCH_GIT_DEBOUNCE_SECS: u64 = 2;

/// Handle a `watch` request: start an inotify/notify watcher for the path
/// and push classified events to the client as DaemonEvents.
pub(crate) fn handle_watch(
    id: &str,
    params: &serde_json::Value,
    writer: &Arc<Mutex<TcpStream>>,
    active_watches: &mut HashMap<String, DaemonWatchRegistration>,
    git_debounce: &Arc<Mutex<HashMap<String, Instant>>>,
    gitignores: &Arc<Mutex<HashMap<String, Gitignore>>>,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };

    let raw_path = PathBuf::from(&params.path);
    if !raw_path.is_dir() {
        return DaemonResponse::err(
            id,
            "NOT_FOUND",
            format!("Path does not exist or is not a directory: {}", params.path),
        );
    }
    // Canonicalize the path to resolve symlinks (critical on macOS where
    // /var → /private/var; FSEvents watches the canonical path).
    let path = resolve_watch_path(&params.path);
    let watch_key = path.to_string_lossy().to_string();

    // Already watching this path?
    if active_watches.contains_key(&watch_key) {
        return DaemonResponse::ok(id, protocol::WatchResult { ok: true });
    }

    {
        let gi = build_gitignore(&path);
        let mut gis = gitignores.lock().unwrap_or_else(|e| e.into_inner());
        gis.insert(watch_key.clone(), gi);
    }

    let watcher_slot: Arc<Mutex<Option<RecommendedWatcher>>> = Arc::new(Mutex::new(None));
    let watcher_for_callback: Weak<Mutex<Option<RecommendedWatcher>>> =
        Arc::downgrade(&watcher_slot);
    let watched_dirs: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
    let watched_dirs_for_callback = watched_dirs.clone();
    let writer_clone = writer.clone();
    let watch_path = watch_key.clone();
    let debounce_clone = git_debounce.clone();
    let gitignores_clone = gitignores.clone();

    let mut watcher = match RecommendedWatcher::new(
        move |res: Result<NotifyEvent, notify::Error>| match res {
            Ok(event) => {
                let Some(watcher) = watcher_for_callback.upgrade() else {
                    return;
                };
                forward_watch_event(
                    &writer_clone,
                    &watch_path,
                    &debounce_clone,
                    &gitignores_clone,
                    &watcher,
                    &watched_dirs_for_callback,
                    event,
                );
            }
            Err(error) => {
                tracing::warn!(path = %watch_path, error = %error, "file watcher error");
            }
        },
        Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => return DaemonResponse::err(id, "WATCH_ERROR", e.to_string()),
    };

    let watched_dir_count = {
        let gis = gitignores.lock().unwrap_or_else(|e| e.into_inner());
        let gitignore = gis
            .get(&watch_key)
            .expect("handle_watch inserted gitignore before reconcile");
        let mut watched_dirs_guard = watched_dirs.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) =
            reconcile_pruned_tree_watches(&mut watcher, &mut watched_dirs_guard, &path, gitignore)
        {
            return DaemonResponse::err(id, "WATCH_ERROR", e.to_string());
        }
        watched_dirs_guard.len()
    };
    {
        let mut slot = watcher_slot.lock().unwrap_or_else(|e| e.into_inner());
        *slot = Some(watcher);
    }
    active_watches.insert(
        watch_key.clone(),
        DaemonWatchRegistration {
            watcher: watcher_slot,
            watched_dirs,
        },
    );

    tracing::info!(
        path = %params.path,
        watched_dir_count,
        "Started watching directory tree with pre-pruning"
    );
    DaemonResponse::ok(id, protocol::WatchResult { ok: true })
}

/// Handle an `unwatch` request: stop watching the specified path.
pub(crate) fn handle_unwatch(
    id: &str,
    params: &serde_json::Value,
    active_watches: &mut HashMap<String, DaemonWatchRegistration>,
    gitignores: &Arc<Mutex<HashMap<String, Gitignore>>>,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };

    let watch_key = resolve_watch_path(&params.path)
        .to_string_lossy()
        .to_string();
    if let Some(registration) = active_watches.remove(&watch_key) {
        let watched_dir_count = registration
            .watched_dirs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len();
        let mut watcher = registration
            .watcher
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let watcher = watcher
            .as_mut()
            .expect("daemon watcher missing during unwatch");
        let mut watched_dirs = registration
            .watched_dirs
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for watched_dir in watched_dirs.drain() {
            let _ = watcher.unwatch(&watched_dir);
        }
        tracing::info!(
            path = %params.path,
            watched_dir_count,
            "Stopped watching directory"
        );
    }
    let mut gis = gitignores.lock().unwrap_or_else(|e| e.into_inner());
    gis.remove(&watch_key);
    DaemonResponse::ok(id, protocol::WatchResult { ok: true })
}

fn resolve_watch_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    path.canonicalize().unwrap_or(path)
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
pub(crate) fn forward_watch_event(
    writer: &Arc<Mutex<TcpStream>>,
    project_path: &str,
    debounce: &Arc<Mutex<HashMap<String, Instant>>>,
    gitignores: &Arc<Mutex<HashMap<String, Gitignore>>>,
    watcher: &Arc<Mutex<Option<RecommendedWatcher>>>,
    watched_dirs: &Arc<Mutex<HashSet<PathBuf>>>,
    event: NotifyEvent,
) {
    {
        let gis = gitignores.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(gitignore) = gis.get(project_path) {
            let mut watcher = watcher.lock().unwrap_or_else(|error| error.into_inner());
            let mut watched_dirs = watched_dirs
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let before = watched_dirs.len();
            if let Some(watcher) = watcher.as_mut() {
                if let Ok(Some(count)) = reconcile_pruned_tree_watches_for_event(
                    watcher,
                    &mut watched_dirs,
                    Path::new(project_path),
                    gitignore,
                    &event,
                ) {
                    if count != before {
                        let reason = if event.paths.iter().any(|path| {
                            path.file_name()
                                .map(|name| name.to_string_lossy())
                                .is_some_and(|name| {
                                    name == ".gitignore" || name == ".taurhausignore"
                                })
                        }) {
                            "gitignore_changed"
                        } else {
                            "directory_topology_changed"
                        };
                        tracing::info!(
                            path = %project_path,
                            watched_dir_count = count,
                            reason,
                            "Reconciled daemon watch tree"
                        );
                    }
                }
            }
        }
    }

    let project_root = Path::new(project_path);
    let classified = classify_notify_event(
        project_path,
        project_root,
        WATCH_GIT_DEBOUNCE_SECS,
        debounce,
        gitignores,
        &event,
        true,
    );

    if classified.emit_git_changed {
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

    for path in classified.session_files {
        crate::daemon::server::push_event(
            writer,
            &DaemonEvent::new(
                protocol::event::SESSION_FILE_CREATED,
                protocol::SessionFileCreatedData {
                    path: project_path.to_string(),
                    file: relative_to(&path, project_root),
                },
            ),
        );
    }

    let regular_files: Vec<String> = classified
        .regular_files
        .iter()
        .map(|path| relative_to(path, project_root))
        .collect();

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

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{DataChange, ModifyKind};
    use notify::EventKind;
    use serde_json::json;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::time::Duration;

    type TestWatchState = (
        Arc<Mutex<Option<RecommendedWatcher>>>,
        Arc<Mutex<HashSet<PathBuf>>>,
    );

    fn empty_watch_state() -> TestWatchState {
        (
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(HashSet::new())),
        )
    }

    fn tcp_stream_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();
        (server, client)
    }

    fn next_daemon_event(reader: &mut BufReader<TcpStream>) -> Option<protocol::DaemonEvent> {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => Some(serde_json::from_str(&line).unwrap()),
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut =>
            {
                None
            }
            Err(err) => panic!("failed to read daemon event: {err}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn watch_dedupes_alias_paths_and_unwatch_resolves_alias() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let dir = tempfile::TempDir::new().unwrap();
        let watched = dir.path().join("watched");
        std::fs::create_dir(&watched).unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(&watched, &alias).unwrap();

        let (writer_stream, _peer_stream) = tcp_stream_pair();
        let writer = Arc::new(Mutex::new(writer_stream));
        let mut active_watches = HashMap::new();
        let git_debounce = Arc::new(Mutex::new(HashMap::new()));
        let gitignores = Arc::new(Mutex::new(HashMap::new()));

        let resp1 = handle_watch(
            "w1",
            &json!({ "path": watched.to_string_lossy() }),
            &writer,
            &mut active_watches,
            &git_debounce,
            &gitignores,
        );
        assert!(resp1.is_ok());
        assert_eq!(active_watches.len(), 1);

        let resp2 = handle_watch(
            "w2",
            &json!({ "path": alias.to_string_lossy() }),
            &writer,
            &mut active_watches,
            &git_debounce,
            &gitignores,
        );
        assert!(resp2.is_ok());
        assert_eq!(active_watches.len(), 1);

        let canonical = watched
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(active_watches.contains_key(&canonical));

        let unwatch_resp = handle_unwatch(
            "u1",
            &json!({ "path": alias.to_string_lossy() }),
            &mut active_watches,
            &gitignores,
        );
        assert!(unwatch_resp.is_ok());
        assert!(active_watches.is_empty());
        assert!(gitignores.lock().unwrap().is_empty());
    }

    #[test]
    fn forward_watch_event_filters_gitignored_files() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::write(root.join(".gitignore"), "output/\n*.log\n").unwrap();
        std::fs::create_dir_all(root.join("output/images")).unwrap();
        std::fs::write(root.join("output/images/test.png"), "").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        let (writer_stream, peer_stream) = tcp_stream_pair();
        peer_stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let writer = Arc::new(Mutex::new(writer_stream));
        let mut reader = BufReader::new(peer_stream);

        let debounce = Arc::new(Mutex::new(HashMap::new()));
        let gitignores = Arc::new(Mutex::new(HashMap::new()));
        let (watcher, watched_dirs) = empty_watch_state();
        let project_path = root.to_string_lossy().to_string();
        {
            let gi = build_gitignore(&root);
            gitignores.lock().unwrap().insert(project_path.clone(), gi);
        }

        let ignored_event = NotifyEvent {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            paths: vec![root.join("output/images/test.png")],
            attrs: Default::default(),
        };
        forward_watch_event(
            &writer,
            &project_path,
            &debounce,
            &gitignores,
            &watcher,
            &watched_dirs,
            ignored_event,
        );
        assert!(
            next_daemon_event(&mut reader).is_none(),
            "gitignored files should not emit daemon file_changed events"
        );

        let regular_event = NotifyEvent {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            paths: vec![root.join("src/main.rs")],
            attrs: Default::default(),
        };
        forward_watch_event(
            &writer,
            &project_path,
            &debounce,
            &gitignores,
            &watcher,
            &watched_dirs,
            regular_event,
        );

        let event = next_daemon_event(&mut reader).expect("expected daemon event for src/main.rs");
        assert_eq!(event.event, protocol::event::FILE_CHANGED);
        let payload: protocol::FileChangedData = serde_json::from_value(event.data).unwrap();
        assert_eq!(payload.path, project_path);
        assert_eq!(payload.files, vec!["src/main.rs".to_string()]);
    }

    #[test]
    fn gitignore_change_rebuilds_matcher_for_future_events() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::write(root.join(".gitignore"), "").unwrap();
        std::fs::create_dir_all(root.join("generated")).unwrap();
        std::fs::write(root.join("generated/output.txt"), "data").unwrap();

        let (writer_stream, peer_stream) = tcp_stream_pair();
        peer_stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let writer = Arc::new(Mutex::new(writer_stream));
        let mut reader = BufReader::new(peer_stream);

        let debounce = Arc::new(Mutex::new(HashMap::new()));
        let gitignores = Arc::new(Mutex::new(HashMap::new()));
        let (watcher, watched_dirs) = empty_watch_state();
        let project_path = root.to_string_lossy().to_string();
        {
            let gi = build_gitignore(&root);
            gitignores.lock().unwrap().insert(project_path.clone(), gi);
        }

        let regular_event = NotifyEvent {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            paths: vec![root.join("generated/output.txt")],
            attrs: Default::default(),
        };
        forward_watch_event(
            &writer,
            &project_path,
            &debounce,
            &gitignores,
            &watcher,
            &watched_dirs,
            regular_event.clone(),
        );
        assert!(
            next_daemon_event(&mut reader).is_some(),
            "file should emit before .gitignore is updated"
        );

        std::fs::write(root.join(".gitignore"), "generated/\n").unwrap();
        let gitignore_event = NotifyEvent {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            paths: vec![root.join(".gitignore")],
            attrs: Default::default(),
        };
        forward_watch_event(
            &writer,
            &project_path,
            &debounce,
            &gitignores,
            &watcher,
            &watched_dirs,
            gitignore_event,
        );
        let _ = next_daemon_event(&mut reader);

        forward_watch_event(
            &writer,
            &project_path,
            &debounce,
            &gitignores,
            &watcher,
            &watched_dirs,
            regular_event,
        );
        assert!(
            next_daemon_event(&mut reader).is_none(),
            "file should be filtered after .gitignore matcher rebuild"
        );
    }

    #[test]
    fn handle_watch_preprunes_tool_directories_from_registration() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("src/nested")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/react")).unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();

        let (writer_stream, _peer_stream) = tcp_stream_pair();
        let writer = Arc::new(Mutex::new(writer_stream));
        let mut active_watches = HashMap::new();
        let git_debounce = Arc::new(Mutex::new(HashMap::new()));
        let gitignores = Arc::new(Mutex::new(HashMap::new()));

        let response = handle_watch(
            "w1",
            &json!({ "path": root.to_string_lossy() }),
            &writer,
            &mut active_watches,
            &git_debounce,
            &gitignores,
        );
        assert!(response.is_ok());

        let registration = active_watches
            .values()
            .next()
            .expect("daemon watch registration");
        let watched_dirs = registration
            .watched_dirs
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        assert!(watched_dirs.contains(&root));
        assert!(watched_dirs.contains(&root.join("src")));
        assert!(watched_dirs.contains(&root.join("src/nested")));
        assert!(!watched_dirs.contains(&root.join("node_modules")));
        assert!(!watched_dirs.contains(&root.join("node_modules/react")));
        assert!(!watched_dirs.contains(&root.join("target")));
    }
}
