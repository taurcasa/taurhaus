use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ignore::gitignore::Gitignore;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{Map, Value};

use crate::sentinels::PYTHON_CACHE_DIR;

/// Classification of a filesystem event for taurhaus purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// Git state changed (.git/HEAD, .git/index, .git/refs/).
    /// Should be debounced before acting.
    GitChanged { project_id: String },

    /// A new session handoff file appeared in docs/sessions/.
    SessionFileCreated { project_id: String, path: PathBuf },

    /// A text file was created/modified/removed.
    FileChanged {
        project_id: String,
        paths: Vec<PathBuf>,
    },

    /// .gitignore changed — watch filters may need rebuilding.
    GitignoreChanged { project_id: String },
}

/// Classifies a filesystem path relative to a project root.
/// Returns None if the path should be ignored (e.g., inside .git but not a tracked internal).
pub fn classify_event(project_root: &Path, event_path: &Path) -> Option<EventClass> {
    let relative = event_path.strip_prefix(project_root).ok()?;
    let rel_str = relative.to_string_lossy();

    // Git internal files we care about
    if rel_str.starts_with(".git/") || rel_str.starts_with(".git\\") {
        let git_relative = rel_str
            .strip_prefix(".git/")
            .or_else(|| rel_str.strip_prefix(".git\\"))
            .unwrap_or("");

        if git_relative == "HEAD"
            || git_relative == "index"
            || git_relative.starts_with("refs/heads/")
            || git_relative.starts_with("refs\\heads\\")
        {
            return Some(EventClass::GitInternal);
        }
        // Other .git files — ignore
        return None;
    }

    // .git directory itself
    if rel_str == ".git" {
        return None;
    }

    // Session handoff files
    if (rel_str.starts_with("docs/sessions/") || rel_str.starts_with("docs\\sessions\\"))
        && rel_str.contains("session-")
        && rel_str.ends_with(".md")
        && !rel_str.ends_with(".meta.json")
    {
        return Some(EventClass::SessionFile);
    }

    // .gitignore changes
    let filename = event_path.file_name().map(|n| n.to_string_lossy());
    if filename.as_deref() == Some(".gitignore") || filename.as_deref() == Some(".taurhausignore") {
        return Some(EventClass::GitignoreChange);
    }

    // Ignore tool/build directories that write frequently but don't represent user changes
    for component in relative.components() {
        let name = component.as_os_str().to_string_lossy();
        match name.as_ref() {
            "node_modules" | "target" | "dist" | ".cache" | PYTHON_CACHE_DIR
            | ".playwright-mcp" | ".next" | ".nuxt" | ".svelte-kit" => return None,
            _ => {}
        }
    }

    Some(EventClass::RegularFile)
}

/// Internal classification of a file event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventClass {
    GitInternal,
    SessionFile,
    GitignoreChange,
    RegularFile,
}

/// Build a `Gitignore` matcher from a project's `.gitignore` file.
///
/// Returns a matcher that can test whether a path is ignored. If the
/// `.gitignore` file doesn't exist or can't be parsed, returns a no-op
/// matcher that ignores nothing.
pub(crate) fn build_gitignore(project_root: &Path) -> Gitignore {
    let gitignore_path = project_root.join(".gitignore");
    let (gi, _err) = Gitignore::new(&gitignore_path);
    gi
}

/// Shared classification output for a single notify event.
///
/// This captures domain-level watch semantics while keeping transport emission
/// (Tauri channel vs daemon socket) separate.
#[derive(Debug, Default)]
pub(crate) struct ClassifiedNotifyEvent {
    pub emit_git_changed: bool,
    pub session_files: Vec<PathBuf>,
    pub regular_files: Vec<PathBuf>,
    pub gitignore_changed: bool,
}

/// Classify and filter a notify event using shared watch rules.
///
/// - Applies Git event debounce using `watch_key`.
/// - Rebuilds the gitignore matcher on `.gitignore` / `.taurhausignore` changes.
/// - Filters regular files through `matched_path_or_any_parents`.
/// - Optionally includes gitignore-file changes in `regular_files`.
pub(crate) fn classify_notify_event(
    watch_key: &str,
    project_root: &Path,
    debounce_window_secs: u64,
    debounce: &Arc<Mutex<HashMap<String, Instant>>>,
    gitignores: &Arc<Mutex<HashMap<String, Gitignore>>>,
    event: &Event,
    include_gitignore_in_regular_files: bool,
) -> ClassifiedNotifyEvent {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return ClassifiedNotifyEvent::default(),
    }

    let mut classified = ClassifiedNotifyEvent::default();

    for path in &event.paths {
        let Some(class) = classify_event(project_root, path) else {
            continue;
        };

        match class {
            EventClass::GitInternal => {
                let mut state = debounce.lock().unwrap_or_else(|e| e.into_inner());
                let now = Instant::now();
                let should_emit = state.get(watch_key).is_none_or(|last| {
                    now.duration_since(*last) >= Duration::from_secs(debounce_window_secs)
                });

                if should_emit {
                    state.insert(watch_key.to_string(), now);
                    classified.emit_git_changed = true;
                }
            }
            EventClass::SessionFile => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    classified.session_files.push(path.clone());
                }
            }
            EventClass::GitignoreChange => {
                let gi = build_gitignore(project_root);
                let mut gis = gitignores.lock().unwrap_or_else(|e| e.into_inner());
                gis.insert(watch_key.to_string(), gi);
                classified.gitignore_changed = true;
                if include_gitignore_in_regular_files {
                    classified.regular_files.push(path.clone());
                }
            }
            EventClass::RegularFile => {
                let gis = gitignores.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(gi) = gis.get(watch_key) {
                    let is_dir = path.is_dir();
                    if gi.matched_path_or_any_parents(path, is_dir).is_ignore() {
                        continue;
                    }
                }
                classified.regular_files.push(path.clone());
            }
        }
    }

    classified
}

/// Manages file watchers for registered projects.
pub struct ProjectWatcher {
    /// Map from project_id → (project_root, watcher_handle).
    watchers: HashMap<String, (PathBuf, RecommendedWatcher)>,
    /// Channel to receive classified events.
    event_tx: mpsc::Sender<WatchEvent>,
    /// Debounce state for git events per project.
    git_debounce: Arc<Mutex<HashMap<String, Instant>>>,
    /// Per-project gitignore matchers, rebuilt when .gitignore changes.
    gitignores: Arc<Mutex<HashMap<String, Gitignore>>>,
}

/// Duration to debounce git internal events (ADR-020).
const GIT_DEBOUNCE_SECS: u64 = 2;

fn emit_watch_local_registered(project_id: &str, project_root: &Path) {
    let mut fields = Map::new();
    fields.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    fields.insert(
        "project_path".to_string(),
        Value::String(project_root.display().to_string()),
    );
    fields.insert("watch_mode".to_string(), Value::String("local".to_string()));
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "watch.local.registered",
        Some("Local project watcher registered".to_string()),
        fields,
    );
}

fn emit_watch_local_unregistered(project_id: &str, project_root: &Path) {
    let mut fields = Map::new();
    fields.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    fields.insert(
        "project_path".to_string(),
        Value::String(project_root.display().to_string()),
    );
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "watch.local.unregistered",
        Some("Local project watcher unregistered".to_string()),
        fields,
    );
}

fn emit_watch_event_dropped(project_id: &str, watch_event: &str, error_message: &str) {
    let mut fields = Map::new();
    fields.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    fields.insert(
        "watch_event".to_string(),
        Value::String(watch_event.to_string()),
    );
    fields.insert(
        "error.message".to_string(),
        Value::String(error_message.to_string()),
    );
    crate::commands::logging::emit_global(
        "warn",
        "backend",
        "watch.event.dropped",
        Some("Watch event dropped before processing".to_string()),
        fields,
    );
}

fn send_watch_event(
    tx: &mpsc::Sender<WatchEvent>,
    event: WatchEvent,
    project_id: &str,
    watch_event: &str,
) {
    if let Err(error) = tx.send(event) {
        emit_watch_event_dropped(project_id, watch_event, &error.to_string());
    }
}

impl ProjectWatcher {
    /// Create a new ProjectWatcher. Returns the watcher and a receiver for events.
    pub fn new() -> (Self, mpsc::Receiver<WatchEvent>) {
        let (tx, rx) = mpsc::channel();
        (
            Self {
                watchers: HashMap::new(),
                event_tx: tx,
                git_debounce: Arc::new(Mutex::new(HashMap::new())),
                gitignores: Arc::new(Mutex::new(HashMap::new())),
            },
            rx,
        )
    }

    /// Start watching a project directory.
    pub fn watch_project(
        &mut self,
        project_id: String,
        project_root: PathBuf,
    ) -> Result<(), notify::Error> {
        // Build gitignore matcher for this project
        {
            let gi = build_gitignore(&project_root);
            let mut gis = self.gitignores.lock().unwrap_or_else(|e| e.into_inner());
            gis.insert(project_id.clone(), gi);
        }

        let tx = self.event_tx.clone();
        let pid = project_id.clone();
        let root = project_root.clone();
        let debounce = self.git_debounce.clone();
        let gitignores = self.gitignores.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    handle_notify_event(&tx, &pid, &root, &debounce, &gitignores, event);
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        watcher.watch(&project_root, RecursiveMode::Recursive)?;
        emit_watch_local_registered(&project_id, &project_root);
        self.watchers.insert(project_id, (project_root, watcher));
        Ok(())
    }

    /// Start watching a single file path.
    pub fn watch_file(
        &mut self,
        project_id: String,
        file_path: PathBuf,
    ) -> Result<(), notify::Error> {
        let tx = self.event_tx.clone();
        let pid = project_id.clone();
        let target = file_path.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                let Ok(event) = res else {
                    return;
                };
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
                    _ => return,
                }

                let matched_paths: Vec<PathBuf> = event
                    .paths
                    .into_iter()
                    .filter(|path| path == &target)
                    .collect();
                if matched_paths.is_empty() {
                    return;
                }

                send_watch_event(
                    &tx,
                    WatchEvent::FileChanged {
                        project_id: pid.clone(),
                        paths: matched_paths,
                    },
                    &pid,
                    "file_changed",
                );
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        watcher.watch(&file_path, RecursiveMode::NonRecursive)?;
        emit_watch_local_registered(&project_id, &file_path);
        self.watchers.insert(project_id, (file_path, watcher));
        Ok(())
    }

    /// Stop watching a project.
    pub fn unwatch_project(&mut self, project_id: &str) {
        if let Some((root, mut watcher)) = self.watchers.remove(project_id) {
            let _ = watcher.unwatch(&root);
            emit_watch_local_unregistered(project_id, &root);
        }
        let mut gis = self.gitignores.lock().unwrap_or_else(|e| e.into_inner());
        gis.remove(project_id);
    }

    /// Stop all watchers.
    pub fn unwatch_all(&mut self) {
        let ids: Vec<String> = self.watchers.keys().cloned().collect();
        for id in ids {
            self.unwatch_project(&id);
        }
    }

    /// Get the list of currently watched project IDs.
    pub fn watched_projects(&self) -> Vec<String> {
        self.watchers.keys().cloned().collect()
    }

    /// Get a clone of the event sender.
    ///
    /// Used to share the channel with daemon event listeners so both local
    /// and daemon-forwarded events go through the same pipeline.
    pub fn event_sender(&self) -> mpsc::Sender<WatchEvent> {
        self.event_tx.clone()
    }
}

/// Process a single notify event and emit classified WatchEvents.
fn handle_notify_event(
    tx: &mpsc::Sender<WatchEvent>,
    project_id: &str,
    project_root: &Path,
    debounce: &Arc<Mutex<HashMap<String, Instant>>>,
    gitignores: &Arc<Mutex<HashMap<String, Gitignore>>>,
    event: Event,
) {
    let classified = classify_notify_event(
        project_id,
        project_root,
        GIT_DEBOUNCE_SECS,
        debounce,
        gitignores,
        &event,
        false,
    );

    if classified.emit_git_changed {
        send_watch_event(
            tx,
            WatchEvent::GitChanged {
                project_id: project_id.to_string(),
            },
            project_id,
            "git_changed",
        );
    }

    for path in classified.session_files {
        send_watch_event(
            tx,
            WatchEvent::SessionFileCreated {
                project_id: project_id.to_string(),
                path,
            },
            project_id,
            "session_file_created",
        );
    }

    if classified.gitignore_changed {
        send_watch_event(
            tx,
            WatchEvent::GitignoreChanged {
                project_id: project_id.to_string(),
            },
            project_id,
            "gitignore_changed",
        );
    }

    if !classified.regular_files.is_empty() {
        send_watch_event(
            tx,
            WatchEvent::FileChanged {
                project_id: project_id.to_string(),
                paths: classified.regular_files,
            },
            project_id,
            "file_changed",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::logging::{install_global_sink, LogFileState};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn root() -> PathBuf {
        PathBuf::from("/home/user/projects/taurhaus")
    }

    fn empty_gitignores() -> Arc<Mutex<HashMap<String, Gitignore>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn wait_for_lines(path: &std::path::Path, expected: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(content) = std::fs::read_to_string(path) {
                let lines: Vec<String> = content
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.to_string())
                    .collect();
                if lines.len() >= expected {
                    return lines;
                }
            }
            if Instant::now() >= deadline {
                panic!(
                    "timed out waiting for watcher log lines at {}",
                    path.display()
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    // --- classify_event tests ---

    #[test]
    fn classify_git_head() {
        let path = root().join(".git/HEAD");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::GitInternal)
        );
    }

    #[test]
    fn classify_git_index() {
        let path = root().join(".git/index");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::GitInternal)
        );
    }

    #[test]
    fn classify_git_refs_heads() {
        let path = root().join(".git/refs/heads/main");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::GitInternal)
        );
    }

    #[test]
    fn classify_git_objects_ignored() {
        let path = root().join(".git/objects/ab/1234");
        assert_eq!(classify_event(&root(), &path), None);
    }

    #[test]
    fn classify_git_directory_itself() {
        let path = root().join(".git");
        assert_eq!(classify_event(&root(), &path), None);
    }

    #[test]
    fn classify_session_file() {
        let path = root().join("docs/sessions/session-2026-02-17T14-30-45.md");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::SessionFile)
        );
    }

    #[test]
    fn classify_session_meta_json_is_regular() {
        // .meta.json is not a session handoff file — it's a regular file
        let path = root().join("docs/sessions/session-2026-02-17T14-30-45.meta.json");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::RegularFile)
        );
    }

    #[test]
    fn classify_non_session_md_in_sessions_dir() {
        let path = root().join("docs/sessions/notes.md");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::RegularFile)
        );
    }

    #[test]
    fn classify_gitignore() {
        let path = root().join(".gitignore");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::GitignoreChange)
        );
    }

    #[test]
    fn classify_nested_gitignore() {
        let path = root().join("src/.gitignore");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::GitignoreChange)
        );
    }

    #[test]
    fn classify_taurhausignore() {
        let path = root().join(".taurhausignore");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::GitignoreChange)
        );
    }

    #[test]
    fn classify_regular_rust_file() {
        let path = root().join("src/main.rs");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::RegularFile)
        );
    }

    #[test]
    fn classify_regular_nested_file() {
        let path = root().join("src/db/queries.rs");
        assert_eq!(
            classify_event(&root(), &path),
            Some(EventClass::RegularFile)
        );
    }

    #[test]
    fn classify_outside_project_returns_none() {
        let path = PathBuf::from("/home/user/other/file.txt");
        assert_eq!(classify_event(&root(), &path), None);
    }

    // --- Watcher lifecycle tests ---

    #[test]
    fn watcher_starts_and_stops() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let dir = tempfile::TempDir::new().unwrap();
        let (mut watcher, _rx) = ProjectWatcher::new();

        watcher
            .watch_project("p1".to_string(), dir.path().to_path_buf())
            .unwrap();

        assert_eq!(watcher.watched_projects().len(), 1);

        watcher.unwatch_project("p1");
        assert!(watcher.watched_projects().is_empty());
    }

    #[test]
    fn unwatch_all_clears_everything() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let d1 = tempfile::TempDir::new().unwrap();
        let d2 = tempfile::TempDir::new().unwrap();
        let (mut watcher, _rx) = ProjectWatcher::new();

        watcher
            .watch_project("p1".to_string(), d1.path().to_path_buf())
            .unwrap();
        watcher
            .watch_project("p2".to_string(), d2.path().to_path_buf())
            .unwrap();

        assert_eq!(watcher.watched_projects().len(), 2);

        watcher.unwatch_all();
        assert!(watcher.watched_projects().is_empty());
    }

    // --- Debounce logic test ---

    #[test]
    fn git_debounce_suppresses_rapid_events() {
        let debounce: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = mpsc::channel();
        let root = root();

        // Simulate two rapid git events
        let event1 = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![root.join(".git/HEAD")],
            attrs: Default::default(),
        };

        let gis = empty_gitignores();
        handle_notify_event(&tx, "p1", &root, &debounce, &gis, event1.clone());
        handle_notify_event(&tx, "p1", &root, &debounce, &gis, event1);

        // Should only receive one event (second is debounced)
        let first = rx.try_recv();
        assert!(first.is_ok());
        assert!(matches!(first.unwrap(), WatchEvent::GitChanged { .. }));

        let second = rx.try_recv();
        assert!(second.is_err(), "Second event should be debounced");
    }

    #[test]
    fn session_file_event_emitted() {
        let (tx, rx) = mpsc::channel();
        let root = root();
        let debounce = Arc::new(Mutex::new(HashMap::new()));
        let gis = empty_gitignores();

        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![root.join("docs/sessions/session-2026-02-17T14-30-45.md")],
            attrs: Default::default(),
        };

        handle_notify_event(&tx, "p1", &root, &debounce, &gis, event);

        let received = rx.try_recv().unwrap();
        match received {
            WatchEvent::SessionFileCreated { project_id, path } => {
                assert_eq!(project_id, "p1");
                assert!(path.to_string_lossy().contains("session-2026"));
            }
            other => panic!("Expected SessionFileCreated, got: {other:?}"),
        }
    }

    #[test]
    fn regular_file_change_emitted() {
        let (tx, rx) = mpsc::channel();
        let root = root();
        let debounce = Arc::new(Mutex::new(HashMap::new()));
        let gis = empty_gitignores();

        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![root.join("src/main.rs")],
            attrs: Default::default(),
        };

        handle_notify_event(&tx, "p1", &root, &debounce, &gis, event);

        let received = rx.try_recv().unwrap();
        match received {
            WatchEvent::FileChanged { project_id, paths } => {
                assert_eq!(project_id, "p1");
                assert_eq!(paths.len(), 1);
            }
            other => panic!("Expected FileChanged, got: {other:?}"),
        }
    }

    #[test]
    fn access_events_are_ignored() {
        let (tx, rx) = mpsc::channel();
        let root = root();
        let debounce = Arc::new(Mutex::new(HashMap::new()));
        let gis = empty_gitignores();

        let event = Event {
            kind: EventKind::Access(notify::event::AccessKind::Read),
            paths: vec![root.join("src/main.rs")],
            attrs: Default::default(),
        };

        handle_notify_event(&tx, "p1", &root, &debounce, &gis, event);

        assert!(rx.try_recv().is_err(), "Access events should be ignored");
    }

    #[test]
    fn gitignored_files_are_filtered_from_events() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();

        // Create a .gitignore that ignores output/ and *.log
        std::fs::write(root.join(".gitignore"), "output/\n*.log\nqueue/*.db-wal\n").unwrap();
        // Create the files so is_dir() works
        std::fs::create_dir_all(root.join("output/images")).unwrap();
        std::fs::write(root.join("output/images/test.png"), "").unwrap();
        std::fs::write(root.join("server.log"), "").unwrap();
        std::fs::create_dir_all(root.join("queue")).unwrap();
        std::fs::write(root.join("queue/data.db-wal"), "").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        let (tx, rx) = mpsc::channel();
        let debounce = Arc::new(Mutex::new(HashMap::new()));

        // Build gitignore matcher for project "p1"
        let gis = Arc::new(Mutex::new(HashMap::new()));
        {
            let gi = build_gitignore(&root);
            gis.lock().unwrap().insert("p1".to_string(), gi);
        }

        // Gitignored file should NOT produce an event
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![root.join("output/images/test.png")],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, "p1", &root, &debounce, &gis, event);
        assert!(
            rx.try_recv().is_err(),
            "gitignored output/ file should not emit event"
        );

        // .log file should NOT produce an event
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![root.join("server.log")],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, "p1", &root, &debounce, &gis, event);
        assert!(
            rx.try_recv().is_err(),
            "gitignored *.log file should not emit event"
        );

        // db-wal file should NOT produce an event
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![root.join("queue/data.db-wal")],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, "p1", &root, &debounce, &gis, event);
        assert!(
            rx.try_recv().is_err(),
            "gitignored db-wal file should not emit event"
        );

        // Non-ignored file SHOULD produce an event
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![root.join("src/main.rs")],
            attrs: Default::default(),
        };
        handle_notify_event(&tx, "p1", &root, &debounce, &gis, event);
        let received = rx.try_recv();
        assert!(
            received.is_ok(),
            "non-ignored src/main.rs should emit FileChanged"
        );
        assert!(matches!(received.unwrap(), WatchEvent::FileChanged { .. }));
    }

    #[test]
    fn tool_directories_are_ignored() {
        let root = root();
        let tool_dirs = [
            "node_modules/package/index.js",
            "target/debug/build.rs",
            "dist/bundle.js",
            ".playwright-mcp/console.log",
            ".cache/data.json",
            "__pycache__/module.pyc",
            ".next/static/chunk.js",
            ".svelte-kit/output/index.html",
        ];
        for path in tool_dirs {
            assert!(
                classify_event(&root, &root.join(path)).is_none(),
                "Should ignore tool directory path: {path}"
            );
        }
    }

    #[test]
    fn watcher_emits_structured_register_and_unregister_events() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let log_dir = tempfile::TempDir::new().expect("temp log dir");
        let log_path = log_dir.path().join("watcher-events.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let project_dir = tempfile::TempDir::new().expect("temp project");
        let (mut watcher, _rx) = ProjectWatcher::new();

        watcher
            .watch_project("p-watch".to_string(), project_dir.path().to_path_buf())
            .expect("watch project");
        watcher.unwatch_project("p-watch");

        let lines = wait_for_lines(&log_path, 2);
        let events: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();

        let registered = events
            .iter()
            .find(|value| value["event"] == "watch.local.registered")
            .expect("watch.local.registered");
        assert_eq!(registered["project_id"], "p-watch");
        assert_eq!(registered["watch_mode"], "local");

        let unregistered = events
            .iter()
            .find(|value| value["event"] == "watch.local.unregistered")
            .expect("watch.local.unregistered");
        assert_eq!(unregistered["project_id"], "p-watch");
    }

    #[test]
    fn watcher_emits_structured_drop_event_when_channel_is_closed() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let log_dir = tempfile::TempDir::new().expect("temp log dir");
        let log_path = log_dir.path().join("watcher-drop.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let (tx, rx) = mpsc::channel();
        drop(rx);

        let project_root = root();
        let debounce = Arc::new(Mutex::new(HashMap::new()));
        let gis = empty_gitignores();

        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![project_root.join("src/main.rs")],
            attrs: Default::default(),
        };

        handle_notify_event(&tx, "p-drop", &project_root, &debounce, &gis, event);

        let lines = wait_for_lines(&log_path, 1);
        let dropped: serde_json::Value = serde_json::from_str(&lines[0]).expect("json");
        assert_eq!(dropped["event"], "watch.event.dropped");
        assert_eq!(dropped["project_id"], "p-drop");
        assert_eq!(dropped["watch_event"], "file_changed");
        assert_eq!(dropped["level"], "WARN");
    }
}
