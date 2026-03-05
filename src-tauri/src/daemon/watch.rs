use std::collections::HashMap;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ignore::gitignore::Gitignore;
use notify::{Config, Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher};

use crate::daemon::protocol::{self, DaemonEvent, DaemonResponse};
use crate::fs::watcher::{build_gitignore, classify_notify_event};

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

    let writer_clone = writer.clone();
    // Use canonical path for event matching (FSEvents on macOS delivers
    // canonical paths, e.g. /private/var/... instead of /var/...).
    let watch_path = watch_key.clone();
    let debounce_clone = git_debounce.clone();
    let gitignores_clone = gitignores.clone();

    {
        let gi = build_gitignore(&path);
        let mut gis = gitignores.lock().unwrap_or_else(|e| e.into_inner());
        gis.insert(watch_key.clone(), gi);
    }

    let watcher_result = RecommendedWatcher::new(
        move |res: Result<NotifyEvent, notify::Error>| match res {
            Ok(event) => {
                forward_watch_event(
                    &writer_clone,
                    &watch_path,
                    &debounce_clone,
                    &gitignores_clone,
                    event,
                );
            }
            Err(e) => {
                tracing::warn!(path = %watch_path, error = %e, "file watcher error");
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
    active_watches.insert(watch_key, watcher);
    DaemonResponse::ok(id, protocol::WatchResult { ok: true })
}

/// Handle an `unwatch` request: stop watching the specified path.
pub(crate) fn handle_unwatch(
    id: &str,
    params: &serde_json::Value,
    active_watches: &mut HashMap<String, RecommendedWatcher>,
    gitignores: &Arc<Mutex<HashMap<String, Gitignore>>>,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };

    let watch_key = resolve_watch_path(&params.path)
        .to_string_lossy()
        .to_string();
    if active_watches.remove(&watch_key).is_some() {
        tracing::info!(path = %params.path, "Stopped watching directory");
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
    event: NotifyEvent,
) {
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
            gitignore_event,
        );
        let _ = next_daemon_event(&mut reader);

        forward_watch_event(
            &writer,
            &project_path,
            &debounce,
            &gitignores,
            regular_event,
        );
        assert!(
            next_daemon_event(&mut reader).is_none(),
            "file should be filtered after .gitignore matcher rebuild"
        );
    }
}
