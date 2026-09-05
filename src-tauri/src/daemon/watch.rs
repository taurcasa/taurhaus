use std::collections::{HashMap, HashSet};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ignore::gitignore::Gitignore;
use notify::{Config, Event as NotifyEvent, RecommendedWatcher, Watcher};

use crate::daemon::protocol::{self, DaemonEvent, DaemonResponse};
use crate::fs::watcher::{
    build_gitignore, classify_notify_event_with_state, desired_watch_dirs_for_root,
    ClassifiedNotifyEvent, DeferredReconcileLedger,
};

#[derive(Debug)]
pub(crate) struct DaemonWatchRegistration {
    registration_id: u64,
    pub(crate) path: PathBuf,
    pub(crate) watched_dirs: HashSet<PathBuf>,
    pub(crate) gitignore: Gitignore,
    pub(crate) last_git_event_at: Option<Instant>,
    pub(crate) subscribers: HashMap<u64, Arc<Mutex<TcpStream>>>,
}

#[derive(Debug, Default)]
struct SharedWatchState {
    registrations: HashMap<String, DaemonWatchRegistration>,
}

type SubscriberWriter = Arc<Mutex<TcpStream>>;
type SharedWatchDelivery = (Vec<SubscriberWriter>, Vec<DaemonEvent>);

#[derive(Debug)]
pub(crate) struct SharedDaemonWatchRegistry {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    state: Arc<Mutex<SharedWatchState>>,
    next_connection_id: AtomicU64,
    next_registration_id: AtomicU64,
}

impl SharedDaemonWatchRegistry {
    pub(crate) fn new() -> notify::Result<Arc<Self>> {
        let watcher_slot: Arc<Mutex<Option<RecommendedWatcher>>> = Arc::new(Mutex::new(None));
        let state = Arc::new(Mutex::new(SharedWatchState::default()));
        let reconciles = Arc::new(Mutex::new(DeferredReconcileLedger::default()));
        let watcher_for_callback = watcher_slot.clone();
        let state_for_callback = state.clone();
        let reconciles_for_callback = reconciles.clone();

        let watcher = RecommendedWatcher::new(
            move |res: Result<NotifyEvent, notify::Error>| match res {
                Ok(event) => {
                    forward_shared_watch_event(
                        &state_for_callback,
                        &watcher_for_callback,
                        &reconciles_for_callback,
                        event,
                    );
                }
                Err(error) => {
                    tracing::warn!(error = %error, "shared file watcher error");
                }
            },
            Config::default(),
        )?;

        {
            let mut slot = watcher_slot
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            *slot = Some(watcher);
        }

        Ok(Arc::new(Self {
            watcher: watcher_slot,
            state,
            next_connection_id: AtomicU64::new(1),
            next_registration_id: AtomicU64::new(1),
        }))
    }

    pub(crate) fn allocate_connection_id(&self) -> u64 {
        self.next_connection_id.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn physical_watch_registration_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .registrations
            .len()
    }

    pub(crate) fn logical_subscription_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .registrations
            .values()
            .map(|registration| registration.subscribers.len())
            .sum()
    }

    fn add_subscription(
        &self,
        connection_id: u64,
        watch_key: &str,
        path: &Path,
        writer: &Arc<Mutex<TcpStream>>,
    ) -> notify::Result<usize> {
        let gitignore = build_gitignore(path);
        let desired_dirs = desired_watch_dirs_for_root(path, &gitignore);
        let mut watcher = self
            .watcher
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let watcher = watcher
            .as_mut()
            .expect("shared daemon watcher missing during watch registration");

        {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(registration) = state.registrations.get_mut(watch_key) {
                registration
                    .subscribers
                    .insert(connection_id, writer.clone());
                return Ok(registration.watched_dirs.len());
            }
        }

        let mut watched_dirs = HashSet::new();
        for desired_dir in desired_dirs {
            watcher.watch(&desired_dir, notify::RecursiveMode::NonRecursive)?;
            watched_dirs.insert(desired_dir);
        }
        let watched_dir_count = watched_dirs.len();
        {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.registrations.insert(
                watch_key.to_string(),
                DaemonWatchRegistration {
                    registration_id: self.next_registration_id.fetch_add(1, Ordering::Relaxed),
                    path: path.to_path_buf(),
                    watched_dirs,
                    gitignore,
                    last_git_event_at: None,
                    subscribers: HashMap::from([(connection_id, writer.clone())]),
                },
            );
        }
        Ok(watched_dir_count)
    }

    fn remove_subscription(&self, connection_id: u64, watch_key: &str) -> Option<usize> {
        let mut watcher = self
            .watcher
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let watcher = watcher
            .as_mut()
            .expect("shared daemon watcher missing during unwatch");
        let (watched_dir_count, watched_dirs) = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let registration = state.registrations.get_mut(watch_key)?;
            registration.subscribers.remove(&connection_id);
            if !registration.subscribers.is_empty() {
                return Some(registration.watched_dirs.len());
            }

            let registration = state.registrations.remove(watch_key)?;
            (
                registration.watched_dirs.len(),
                registration.watched_dirs.into_iter().collect::<Vec<_>>(),
            )
        };
        for watched_dir in watched_dirs {
            let _ = watcher.unwatch(&watched_dir);
        }
        Some(watched_dir_count)
    }

    #[cfg(test)]
    fn registration_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .registrations
            .len()
    }

    #[cfg(test)]
    fn subscriber_count(&self, watch_key: &str) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .registrations
            .get(watch_key)
            .map(|registration| registration.subscribers.len())
            .unwrap_or(0)
    }

    #[cfg(test)]
    fn watched_dirs(&self, watch_key: &str) -> Option<HashSet<PathBuf>> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .registrations
            .get(watch_key)
            .map(|registration| registration.watched_dirs.clone())
    }
}

#[derive(Debug)]
pub(crate) struct WatchRuntime {
    pub(crate) connection_id: u64,
    pub(crate) subscriptions: HashSet<String>,
    pub(crate) registry: Arc<SharedDaemonWatchRegistry>,
}

impl WatchRuntime {
    pub(crate) fn new(registry: Arc<SharedDaemonWatchRegistry>) -> Self {
        Self {
            connection_id: registry.allocate_connection_id(),
            subscriptions: HashSet::new(),
            registry,
        }
    }

    pub(crate) fn clear(&mut self) {
        let subscriptions: Vec<String> = self.subscriptions.drain().collect();
        let mut removed_any = false;
        for watch_key in subscriptions {
            self.registry
                .remove_subscription(self.connection_id, &watch_key);
            removed_any = true;
        }
        if removed_any {
            crate::daemon::server::mark_daemon_watch_telemetry_dirty();
        }
    }
}

/// Duration to debounce git internal events pushed to clients.
pub(crate) const WATCH_GIT_DEBOUNCE_SECS: u64 = 2;

/// Handle a `watch` request: start an inotify/notify watcher for the path
/// and push classified events to the client as DaemonEvents.
pub(crate) fn handle_watch(
    id: &str,
    params: &serde_json::Value,
    writer: &Arc<Mutex<TcpStream>>,
    watch_runtime: &mut WatchRuntime,
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
    let path = resolve_watch_path(&params.path);
    let watch_key = path.to_string_lossy().to_string();

    if watch_runtime.subscriptions.contains(&watch_key) {
        return DaemonResponse::ok(id, protocol::WatchResult { ok: true });
    }

    let watched_dir_count = match watch_runtime.registry.add_subscription(
        watch_runtime.connection_id,
        &watch_key,
        &path,
        writer,
    ) {
        Ok(watched_dir_count) => watched_dir_count,
        Err(error) => return DaemonResponse::err(id, "WATCH_ERROR", error.to_string()),
    };
    watch_runtime.subscriptions.insert(watch_key.clone());
    crate::daemon::server::mark_daemon_watch_telemetry_dirty();

    tracing::info!(
        path = %params.path,
        watched_dir_count,
        connection_id = watch_runtime.connection_id,
        "Started or reused shared daemon watch registration"
    );
    DaemonResponse::ok(id, protocol::WatchResult { ok: true })
}

/// Handle an `unwatch` request: stop watching the specified path.
pub(crate) fn handle_unwatch(
    id: &str,
    params: &serde_json::Value,
    watch_runtime: &mut WatchRuntime,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };

    let watch_key = resolve_watch_path(&params.path)
        .to_string_lossy()
        .to_string();
    if watch_runtime.subscriptions.remove(&watch_key) {
        if let Some(watched_dir_count) = watch_runtime
            .registry
            .remove_subscription(watch_runtime.connection_id, &watch_key)
        {
            crate::daemon::server::mark_daemon_watch_telemetry_dirty();
            tracing::info!(
                path = %params.path,
                watched_dir_count,
                connection_id = watch_runtime.connection_id,
                "Removed shared daemon watch subscription"
            );
        }
    }
    DaemonResponse::ok(id, protocol::WatchResult { ok: true })
}

fn resolve_watch_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    path.canonicalize().unwrap_or(path)
}

fn event_matches_registration(event: &NotifyEvent, root: &Path) -> bool {
    event.paths.iter().any(|path| path.starts_with(root))
}

fn collect_daemon_events(
    project_path: &str,
    project_root: &Path,
    classified: ClassifiedNotifyEvent,
) -> Vec<DaemonEvent> {
    let mut events = Vec::new();

    if classified.emit_git_changed {
        events.push(DaemonEvent::new(
            protocol::event::GIT_CHANGED,
            protocol::GitChangedData {
                path: project_path.to_string(),
            },
        ));
    }

    for path in classified.session_files {
        events.push(DaemonEvent::new(
            protocol::event::SESSION_FILE_CREATED,
            protocol::SessionFileCreatedData {
                path: project_path.to_string(),
                file: relative_to(&path, project_root),
            },
        ));
    }

    let regular_files: Vec<String> = classified
        .regular_files
        .iter()
        .map(|path| relative_to(path, project_root))
        .collect();

    if !regular_files.is_empty() {
        events.push(DaemonEvent::new(
            protocol::event::FILE_CHANGED,
            protocol::FileChangedData {
                path: project_path.to_string(),
                files: regular_files,
            },
        ));
    }

    events
}

/// Convert an absolute file path to a project-relative string.
///
/// Falls back to the absolute path if `strip_prefix` fails.
pub(crate) fn relative_to(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|r| r.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

fn forward_shared_watch_event(
    state: &Arc<Mutex<SharedWatchState>>,
    watcher: &Arc<Mutex<Option<RecommendedWatcher>>>,
    reconciles: &Arc<Mutex<DeferredReconcileLedger>>,
    event: NotifyEvent,
) {
    let (deliveries, reconcile_requests): (Vec<SharedWatchDelivery>, Vec<(String, &'static str)>) = {
        let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
        let mut deliveries = Vec::new();
        let mut reconcile_requests = Vec::new();

        for (watch_key, registration) in state.registrations.iter_mut() {
            if !event_matches_registration(&event, &registration.path) {
                continue;
            }

            if let Some(reason) = daemon_event_reconcile_reason(&registration.watched_dirs, &event)
            {
                reconcile_requests.push((watch_key.clone(), reason));
            }

            let classified = classify_notify_event_with_state(
                &registration.path,
                WATCH_GIT_DEBOUNCE_SECS,
                &mut registration.last_git_event_at,
                &mut registration.gitignore,
                &event,
                true,
            );
            let daemon_events = collect_daemon_events(watch_key, &registration.path, classified);
            if daemon_events.is_empty() {
                continue;
            }

            let subscribers = registration.subscribers.values().cloned().collect();
            deliveries.push((subscribers, daemon_events));
        }

        (deliveries, reconcile_requests)
    };

    for (subscribers, daemon_events) in deliveries {
        for subscriber in subscribers {
            for daemon_event in &daemon_events {
                crate::daemon::server::push_event(&subscriber, daemon_event);
            }
        }
    }

    // notify control methods wait for this callback's event loop. The callback
    // therefore only classifies/delivers and schedules full reconciles elsewhere.
    for (watch_key, reason) in reconcile_requests {
        schedule_daemon_reconcile(
            state.clone(),
            watcher.clone(),
            reconciles.clone(),
            watch_key,
            reason,
        );
    }
}

fn daemon_event_reconcile_reason(
    watched_dirs: &HashSet<PathBuf>,
    event: &NotifyEvent,
) -> Option<&'static str> {
    if event.paths.iter().any(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy())
            .is_some_and(|name| name == ".gitignore" || name == ".taurhausignore")
    }) {
        return Some("gitignore_changed");
    }

    if !matches!(
        event.kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_)
    ) {
        return None;
    }

    if event.paths.iter().any(|path| path.is_dir())
        || matches!(event.kind, notify::EventKind::Remove(_))
            && event.paths.iter().any(|path| {
                watched_dirs
                    .iter()
                    .any(|watched| watched == path || watched.starts_with(path))
            })
    {
        Some("directory_topology_changed")
    } else {
        None
    }
}

fn schedule_daemon_reconcile(
    state: Arc<Mutex<SharedWatchState>>,
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    reconciles: Arc<Mutex<DeferredReconcileLedger>>,
    watch_key: String,
    reason: &'static str,
) {
    let must_start = reconciles
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .note_event(&watch_key);
    if !must_start {
        return;
    }

    let worker_key = watch_key.clone();
    let worker_reconciles = reconciles.clone();
    let spawn = std::thread::Builder::new()
        .name("taurhaus-daemon-watch-reconcile".to_string())
        .spawn(move || {
            run_daemon_reconciles(&state, &watcher, &worker_reconciles, &worker_key, reason)
        });
    if let Err(error) = spawn {
        tracing::warn!(
            %error,
            path = %watch_key,
            "daemon watch reconcile thread spawn failed; deferring to the next event"
        );
        reconciles
            .lock()
            .unwrap_or_else(|lock_error| lock_error.into_inner())
            .cancel_worker(&watch_key);
    }
}

fn run_daemon_reconciles(
    state: &Arc<Mutex<SharedWatchState>>,
    watcher: &Arc<Mutex<Option<RecommendedWatcher>>>,
    reconciles: &Arc<Mutex<DeferredReconcileLedger>>,
    watch_key: &str,
    reason: &'static str,
) {
    loop {
        let generation = reconciles
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .current_generation(watch_key);

        if let Some((count, changed)) = reconcile_daemon_watch_dirs_full(state, watcher, watch_key)
        {
            if changed {
                crate::daemon::server::mark_daemon_watch_telemetry_dirty();
                tracing::info!(
                    path = %watch_key,
                    watched_dir_count = count,
                    reason,
                    "Reconciled shared daemon watch tree"
                );
            }
        }

        if reconciles
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .finish_pass(watch_key, generation)
        {
            break;
        }
    }
}

fn reconcile_daemon_watch_dirs_full(
    state: &Arc<Mutex<SharedWatchState>>,
    watcher: &Arc<Mutex<Option<RecommendedWatcher>>>,
    watch_key: &str,
) -> Option<(usize, bool)> {
    let (registration_id, project_root, gitignore, previous_dirs) = {
        let state = state.lock().unwrap_or_else(|error| error.into_inner());
        let registration = state.registrations.get(watch_key)?;
        (
            registration.registration_id,
            registration.path.clone(),
            registration.gitignore.clone(),
            registration.watched_dirs.clone(),
        )
    };

    // Tree walking may touch a large checkout, so it must never extend a state lock.
    let desired_dirs = desired_watch_dirs_for_root(&project_root, &gitignore);
    let stale_dirs = previous_dirs
        .iter()
        .filter(|path| !desired_dirs.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    let new_dirs = desired_dirs
        .iter()
        .filter(|path| !previous_dirs.contains(*path))
        .cloned()
        .collect::<Vec<_>>();

    // Lock order: watch()/unwatch() wait for notify's callback thread. Never call
    // them while holding `state`, because the callback acquires `state`; and the
    // callback itself must never acquire `watcher` or invoke notify control methods.
    let mut watcher_guard = watcher.lock().unwrap_or_else(|error| error.into_inner());
    let watcher = watcher_guard.as_mut()?;
    let mut updated_dirs = previous_dirs;
    for path in stale_dirs {
        let _ = watcher.unwatch(&path);
        updated_dirs.remove(&path);
    }
    for path in new_dirs {
        match watcher.watch(&path, notify::RecursiveMode::NonRecursive) {
            Ok(()) => {
                updated_dirs.insert(path);
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    path = %watch_key,
                    watch_dir = %path.display(),
                    "Failed to extend shared daemon watch tree"
                );
            }
        }
    }

    let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
    let registration = state.registrations.get_mut(watch_key)?;
    if registration.registration_id != registration_id {
        return None;
    }
    let changed = registration.watched_dirs != updated_dirs;
    registration.watched_dirs = updated_dirs;
    Some((registration.watched_dirs.len(), changed))
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

    type SharedWatchTestState = (
        Arc<Mutex<Option<RecommendedWatcher>>>,
        Arc<Mutex<SharedWatchState>>,
        Arc<Mutex<DeferredReconcileLedger>>,
        String,
    );

    fn test_shared_watch_state(
        root: &Path,
        writer: &Arc<Mutex<TcpStream>>,
    ) -> SharedWatchTestState {
        let watch_key = root.to_string_lossy().to_string();
        let registration = DaemonWatchRegistration {
            registration_id: 1,
            path: root.to_path_buf(),
            watched_dirs: HashSet::new(),
            gitignore: build_gitignore(root),
            last_git_event_at: None,
            subscribers: HashMap::from([(1, writer.clone())]),
        };
        let state = Arc::new(Mutex::new(SharedWatchState {
            registrations: HashMap::from([(watch_key.clone(), registration)]),
        }));
        let watcher = Arc::new(Mutex::new(None));
        let reconciles = Arc::new(Mutex::new(DeferredReconcileLedger::default()));
        (watcher, state, reconciles, watch_key)
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
        let registry = SharedDaemonWatchRegistry::new().unwrap();
        let mut watch_runtime = WatchRuntime::new(registry.clone());

        let resp1 = handle_watch(
            "w1",
            &json!({ "path": watched.to_string_lossy() }),
            &writer,
            &mut watch_runtime,
        );
        assert!(resp1.is_ok());
        assert_eq!(registry.registration_count(), 1);

        let resp2 = handle_watch(
            "w2",
            &json!({ "path": alias.to_string_lossy() }),
            &writer,
            &mut watch_runtime,
        );
        assert!(resp2.is_ok());
        assert_eq!(registry.registration_count(), 1);

        let canonical = watched
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(registry.subscriber_count(&canonical), 1);

        let unwatch_resp = handle_unwatch(
            "u1",
            &json!({ "path": alias.to_string_lossy() }),
            &mut watch_runtime,
        );
        assert!(unwatch_resp.is_ok());
        assert_eq!(registry.registration_count(), 0);
    }

    #[test]
    fn shared_registry_keeps_watch_until_last_subscriber_drops() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("src")).unwrap();

        let (writer_stream_a, _peer_stream_a) = tcp_stream_pair();
        let writer_a = Arc::new(Mutex::new(writer_stream_a));
        let (writer_stream_b, _peer_stream_b) = tcp_stream_pair();
        let writer_b = Arc::new(Mutex::new(writer_stream_b));
        let registry = SharedDaemonWatchRegistry::new().unwrap();
        let mut runtime_a = WatchRuntime::new(registry.clone());
        let mut runtime_b = WatchRuntime::new(registry.clone());

        let watch_params = json!({ "path": root.to_string_lossy() });
        assert!(handle_watch("w1", &watch_params, &writer_a, &mut runtime_a).is_ok());
        assert!(handle_watch("w2", &watch_params, &writer_b, &mut runtime_b).is_ok());

        let watch_key = root.canonicalize().unwrap().to_string_lossy().to_string();
        assert_eq!(registry.registration_count(), 1);
        assert_eq!(registry.subscriber_count(&watch_key), 2);

        assert!(handle_unwatch("u1", &watch_params, &mut runtime_a).is_ok());
        assert_eq!(registry.registration_count(), 1);
        assert_eq!(registry.subscriber_count(&watch_key), 1);

        runtime_b.clear();
        assert_eq!(registry.registration_count(), 0);
    }

    #[test]
    fn shared_registry_stays_live_while_reconciling_directory_topology() {
        // Regression: 05fff4f6 moved daemon watches into a shared notify callback that
        // called watch() from the event-loop thread, deadlocking the daemon on topology churn.
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let dir = tempfile::TempDir::new().unwrap();
            let root = dir.path().to_path_buf();
            let original_dir = root.join("src");
            std::fs::create_dir(&original_dir).unwrap();

            let (writer_stream, peer_stream) = tcp_stream_pair();
            peer_stream
                .set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            let writer = Arc::new(Mutex::new(writer_stream));
            let mut reader = BufReader::new(peer_stream);
            let registry = SharedDaemonWatchRegistry::new().unwrap();
            let mut runtime = WatchRuntime::new(registry.clone());
            let watch_params = json!({ "path": root.to_string_lossy() });
            assert!(handle_watch("w1", &watch_params, &writer, &mut runtime).is_ok());

            let watch_key = root.canonicalize().unwrap().to_string_lossy().to_string();
            let new_dir = root.join("fresh-module");
            std::fs::create_dir(&new_dir).unwrap();

            let reconcile_deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if registry
                    .watched_dirs(&watch_key)
                    .is_some_and(|dirs| dirs.contains(&new_dir))
                {
                    break;
                }
                assert!(
                    Instant::now() < reconcile_deadline,
                    "new directory watch was not reconciled"
                );
                std::thread::sleep(Duration::from_millis(20));
            }

            let original_file = original_dir.join("after-reconcile.rs");
            std::fs::write(&original_file, "fn after_reconcile() {}\n").unwrap();
            let delivery_deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(event) = next_daemon_event(&mut reader) {
                    if event.event == protocol::event::FILE_CHANGED {
                        let payload: protocol::FileChangedData =
                            serde_json::from_value(event.data).unwrap();
                        if payload.files == vec!["src/after-reconcile.rs".to_string()] {
                            break;
                        }
                    }
                }
                assert!(
                    Instant::now() < delivery_deadline,
                    "original tree stopped delivering after topology reconcile"
                );
            }

            finished_tx.send(()).unwrap();
        });

        finished_rx
            .recv_timeout(Duration::from_secs(8))
            .expect("daemon watch sequence deadlocked");
    }

    #[test]
    fn forward_shared_watch_event_filters_gitignored_files() {
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

        let (watcher, state, reconciles, project_path) = test_shared_watch_state(&root, &writer);

        let ignored_event = NotifyEvent {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            paths: vec![root.join("output/images/test.png")],
            attrs: Default::default(),
        };
        forward_shared_watch_event(&state, &watcher, &reconciles, ignored_event);
        assert!(
            next_daemon_event(&mut reader).is_none(),
            "gitignored files should not emit daemon file_changed events"
        );

        let regular_event = NotifyEvent {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            paths: vec![root.join("src/main.rs")],
            attrs: Default::default(),
        };
        forward_shared_watch_event(&state, &watcher, &reconciles, regular_event);

        let event = next_daemon_event(&mut reader).expect("expected daemon event for src/main.rs");
        assert_eq!(event.event, protocol::event::FILE_CHANGED);
        let payload: protocol::FileChangedData = serde_json::from_value(event.data).unwrap();
        assert_eq!(payload.path, project_path);
        assert_eq!(payload.files, vec!["src/main.rs".to_string()]);
    }

    #[test]
    fn shared_watch_gitignore_change_rebuilds_matcher_for_future_events() {
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

        let (watcher, state, reconciles, _project_path) = test_shared_watch_state(&root, &writer);

        let regular_event = NotifyEvent {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            paths: vec![root.join("generated/output.txt")],
            attrs: Default::default(),
        };
        forward_shared_watch_event(&state, &watcher, &reconciles, regular_event.clone());
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
        forward_shared_watch_event(&state, &watcher, &reconciles, gitignore_event);
        let _ = next_daemon_event(&mut reader);

        forward_shared_watch_event(&state, &watcher, &reconciles, regular_event);
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
        let registry = SharedDaemonWatchRegistry::new().unwrap();
        let mut watch_runtime = WatchRuntime::new(registry.clone());

        let response = handle_watch(
            "w1",
            &json!({ "path": root.to_string_lossy() }),
            &writer,
            &mut watch_runtime,
        );
        assert!(response.is_ok());

        let watched_dirs = registry
            .watched_dirs(&root.canonicalize().unwrap().to_string_lossy())
            .expect("daemon watch registration");
        assert!(watched_dirs.contains(&root));
        assert!(watched_dirs.contains(&root.join("src")));
        assert!(watched_dirs.contains(&root.join("src/nested")));
        assert!(!watched_dirs.contains(&root.join("node_modules")));
        assert!(!watched_dirs.contains(&root.join("node_modules/react")));
        assert!(!watched_dirs.contains(&root.join("target")));
    }
}
