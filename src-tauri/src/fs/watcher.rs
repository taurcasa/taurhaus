use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use ignore::gitignore::Gitignore;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{Map, Value};

use crate::sentinels::PYTHON_CACHE_DIR;

const PRE_PRUNED_DIR_NAMES: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    ".cache",
    ".playwright-mcp",
    ".next",
    ".nuxt",
    ".svelte-kit",
    PYTHON_CACHE_DIR,
];

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

#[derive(Debug)]
pub(crate) struct TreeWatchRegistration {
    pub root: PathBuf,
    pub watched_dirs: Arc<Mutex<HashSet<PathBuf>>>,
}

#[derive(Debug)]
pub(crate) enum WatchRegistration {
    Tree(TreeWatchRegistration),
    File { path: PathBuf },
}

#[derive(Clone)]
struct NotifyEventContext {
    tx: mpsc::Sender<WatchEvent>,
    project_id: String,
    project_root: PathBuf,
    debounce: Arc<Mutex<HashMap<String, Instant>>>,
    gitignores: Arc<Mutex<HashMap<String, Gitignore>>>,
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    watched_dirs: Arc<Mutex<HashSet<PathBuf>>>,
    watch_refcounts: Option<Arc<Mutex<HashMap<PathBuf, usize>>>>,
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

fn is_pre_pruned_dir_name(name: &str) -> bool {
    PRE_PRUNED_DIR_NAMES.contains(&name)
}

fn is_git_refs_heads_dir(relative: &Path) -> bool {
    let components = relative
        .iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>();
    match components.as_slice() {
        [first] => first == ".git",
        [first, second] => first == ".git" && second == "refs",
        [first, second, third, ..] => first == ".git" && second == "refs" && third == "heads",
        [] => false,
    }
}

pub(crate) fn should_watch_directory_path(
    project_root: &Path,
    dir_path: &Path,
    gitignore: &Gitignore,
) -> bool {
    if dir_path == project_root {
        return true;
    }

    let Ok(relative) = dir_path.strip_prefix(project_root) else {
        return false;
    };

    if relative.as_os_str().is_empty() {
        return true;
    }

    if relative.iter().next().is_some_and(|part| part == ".git") {
        return is_git_refs_heads_dir(relative);
    }

    for component in relative.components() {
        let name = component.as_os_str().to_string_lossy();
        if is_pre_pruned_dir_name(name.as_ref()) {
            return false;
        }
    }

    !gitignore
        .matched_path_or_any_parents(dir_path, true)
        .is_ignore()
}

fn collect_recursive_watch_dirs(
    project_root: &Path,
    current_dir: &Path,
    gitignore: &Gitignore,
    dirs: &mut BTreeSet<PathBuf>,
) {
    let read_dir = match std::fs::read_dir(current_dir) {
        Ok(read_dir) => read_dir,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let child_path = entry.path();
        if !should_watch_directory_path(project_root, &child_path, gitignore) {
            continue;
        }

        if dirs.insert(child_path.clone()) {
            collect_recursive_watch_dirs(project_root, &child_path, gitignore, dirs);
        }
    }
}

pub(crate) fn desired_watch_dirs_for_root(
    project_root: &Path,
    gitignore: &Gitignore,
) -> BTreeSet<PathBuf> {
    let mut dirs = BTreeSet::new();
    dirs.insert(project_root.to_path_buf());
    collect_recursive_watch_dirs(project_root, project_root, gitignore, &mut dirs);
    dirs
}

pub(crate) fn reconcile_pruned_tree_watches(
    watcher: &mut RecommendedWatcher,
    watched_dirs: &mut HashSet<PathBuf>,
    project_root: &Path,
    gitignore: &Gitignore,
) -> Result<usize, notify::Error> {
    let desired_dirs = desired_watch_dirs_for_root(project_root, gitignore);

    let stale_dirs = watched_dirs
        .iter()
        .filter(|path| !desired_dirs.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    for path in stale_dirs {
        let _ = watcher.unwatch(&path);
        watched_dirs.remove(&path);
    }

    for path in desired_dirs {
        if watched_dirs.contains(&path) {
            continue;
        }
        watcher.watch(&path, RecursiveMode::NonRecursive)?;
        watched_dirs.insert(path);
    }

    Ok(watched_dirs.len())
}

fn reconcile_shared_pruned_tree_watches(
    watcher: &mut RecommendedWatcher,
    shared_refcounts: &mut HashMap<PathBuf, usize>,
    watched_dirs: &mut HashSet<PathBuf>,
    project_root: &Path,
    gitignore: &Gitignore,
) -> Result<usize, notify::Error> {
    let desired_dirs = desired_watch_dirs_for_root(project_root, gitignore);

    let stale_dirs = watched_dirs
        .iter()
        .filter(|path| !desired_dirs.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    for path in stale_dirs {
        if let Some(refcount) = shared_refcounts.get_mut(&path) {
            if *refcount <= 1 {
                shared_refcounts.remove(&path);
                let _ = watcher.unwatch(&path);
            } else {
                *refcount -= 1;
            }
        }
        watched_dirs.remove(&path);
    }

    for path in desired_dirs {
        if watched_dirs.contains(&path) {
            continue;
        }
        match shared_refcounts.get_mut(&path) {
            Some(refcount) => *refcount += 1,
            None => {
                watcher.watch(&path, RecursiveMode::NonRecursive)?;
                shared_refcounts.insert(path.clone(), 1);
            }
        }
        watched_dirs.insert(path);
    }

    Ok(watched_dirs.len())
}

fn release_shared_tree_watches(
    mut watcher: Option<&mut RecommendedWatcher>,
    shared_refcounts: &mut HashMap<PathBuf, usize>,
    watched_dirs: &mut HashSet<PathBuf>,
) {
    for path in watched_dirs.drain() {
        if let Some(refcount) = shared_refcounts.get_mut(&path) {
            if *refcount <= 1 {
                shared_refcounts.remove(&path);
                if let Some(watcher) = watcher.as_mut() {
                    let _ = watcher.unwatch(&path);
                }
            } else {
                *refcount -= 1;
            }
        }
    }
}

pub(crate) fn reconcile_pruned_tree_watches_for_event(
    watcher: &mut RecommendedWatcher,
    watched_dirs: &mut HashSet<PathBuf>,
    project_root: &Path,
    gitignore: &Gitignore,
    event: &Event,
) -> Result<Option<usize>, notify::Error> {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return Ok(None),
    }

    if event.paths.iter().any(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy())
            .is_some_and(|name| name == ".gitignore" || name == ".taurhausignore")
    }) {
        let count = reconcile_pruned_tree_watches(watcher, watched_dirs, project_root, gitignore)?;
        return Ok(Some(count));
    }

    let mut changed = false;

    if matches!(event.kind, EventKind::Remove(_)) {
        for path in &event.paths {
            let removed_dirs = watched_dirs
                .iter()
                .filter(|watched| *watched == path || watched.starts_with(path))
                .cloned()
                .collect::<Vec<_>>();
            if removed_dirs.is_empty() {
                continue;
            }
            changed = true;
            for removed in removed_dirs {
                let _ = watcher.unwatch(&removed);
                watched_dirs.remove(&removed);
            }
        }
    } else {
        for path in &event.paths {
            if !path.is_dir() {
                continue;
            }
            if !should_watch_directory_path(project_root, path, gitignore) {
                continue;
            }
            let before = watched_dirs.len();
            let _ = reconcile_pruned_tree_watches(watcher, watched_dirs, project_root, gitignore)?;
            if watched_dirs.len() != before {
                changed = true;
            }
        }
    }

    Ok(changed.then_some(watched_dirs.len()))
}

fn reconcile_shared_pruned_tree_watches_for_event(
    watcher: &mut RecommendedWatcher,
    shared_refcounts: &mut HashMap<PathBuf, usize>,
    watched_dirs: &mut HashSet<PathBuf>,
    project_root: &Path,
    gitignore: &Gitignore,
    event: &Event,
) -> Result<Option<usize>, notify::Error> {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return Ok(None),
    }

    if event.paths.iter().any(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy())
            .is_some_and(|name| name == ".gitignore" || name == ".taurhausignore")
    }) {
        let count = reconcile_shared_pruned_tree_watches(
            watcher,
            shared_refcounts,
            watched_dirs,
            project_root,
            gitignore,
        )?;
        return Ok(Some(count));
    }

    let mut changed = false;

    if matches!(event.kind, EventKind::Remove(_)) {
        for path in &event.paths {
            let removed_dirs = watched_dirs
                .iter()
                .filter(|watched| *watched == path || watched.starts_with(path))
                .cloned()
                .collect::<Vec<_>>();
            if removed_dirs.is_empty() {
                continue;
            }
            changed = true;
            for removed in removed_dirs {
                if let Some(refcount) = shared_refcounts.get_mut(&removed) {
                    if *refcount <= 1 {
                        shared_refcounts.remove(&removed);
                        let _ = watcher.unwatch(&removed);
                    } else {
                        *refcount -= 1;
                    }
                }
                watched_dirs.remove(&removed);
            }
        }
    } else {
        for path in &event.paths {
            if !path.is_dir() {
                continue;
            }
            if !should_watch_directory_path(project_root, path, gitignore) {
                continue;
            }
            let before = watched_dirs.len();
            let _ = reconcile_shared_pruned_tree_watches(
                watcher,
                shared_refcounts,
                watched_dirs,
                project_root,
                gitignore,
            )?;
            if watched_dirs.len() != before {
                changed = true;
            }
        }
    }

    Ok(changed.then_some(watched_dirs.len()))
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
    let mut debounce_state = debounce.lock().unwrap_or_else(|e| e.into_inner());
    let mut gitignore_state = gitignores.lock().unwrap_or_else(|e| e.into_inner());
    let mut last_git_event_at = debounce_state.get(watch_key).copied();
    let gitignore = gitignore_state
        .entry(watch_key.to_string())
        .or_insert_with(|| build_gitignore(project_root));

    let classified = classify_notify_event_with_state(
        project_root,
        debounce_window_secs,
        &mut last_git_event_at,
        gitignore,
        event,
        include_gitignore_in_regular_files,
    );

    match last_git_event_at {
        Some(last_git_event_at) => {
            debounce_state.insert(watch_key.to_string(), last_git_event_at);
        }
        None => {
            debounce_state.remove(watch_key);
        }
    }

    classified
}

pub(crate) fn classify_notify_event_with_state(
    project_root: &Path,
    debounce_window_secs: u64,
    last_git_event_at: &mut Option<Instant>,
    gitignore: &mut Gitignore,
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
                let now = Instant::now();
                let should_emit = last_git_event_at.is_none_or(|last| {
                    now.duration_since(last) >= Duration::from_secs(debounce_window_secs)
                });

                if should_emit {
                    *last_git_event_at = Some(now);
                    classified.emit_git_changed = true;
                }
            }
            EventClass::SessionFile => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    classified.session_files.push(path.clone());
                }
            }
            EventClass::GitignoreChange => {
                *gitignore = build_gitignore(project_root);
                classified.gitignore_changed = true;
                if include_gitignore_in_regular_files {
                    classified.regular_files.push(path.clone());
                }
            }
            EventClass::RegularFile => {
                let is_dir = path.is_dir();
                if gitignore
                    .matched_path_or_any_parents(path, is_dir)
                    .is_ignore()
                {
                    continue;
                }
                classified.regular_files.push(path.clone());
            }
        }
    }

    classified
}

/// Manages file watchers for registered projects.
pub struct ProjectWatcher {
    /// Map from project_id → watch registration metadata.
    watchers: Arc<Mutex<HashMap<String, WatchRegistration>>>,
    /// Channel to receive classified events.
    event_tx: mpsc::Sender<WatchEvent>,
    /// Per-project gitignore matchers, rebuilt when .gitignore changes.
    gitignores: Arc<Mutex<HashMap<String, Gitignore>>>,
    /// Shared watcher for all local project tree watches.
    tree_watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    /// Shared watcher for singleton local file watches.
    file_watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    /// Refcounts for tree directories watched through the shared tree watcher.
    tree_watch_refcounts: Arc<Mutex<HashMap<PathBuf, usize>>>,
    /// Refcounts for exact singleton file paths watched through the shared file watcher.
    file_watch_refcounts: Arc<Mutex<HashMap<PathBuf, usize>>>,
}

/// Duration to debounce git internal events (ADR-020).
const GIT_DEBOUNCE_SECS: u64 = 2;

fn emit_watch_local_registered(project_id: &str, project_root: &Path, watched_dir_count: usize) {
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
    fields.insert(
        "watched_dir_count".to_string(),
        Value::Number(serde_json::Number::from(watched_dir_count as u64)),
    );
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "watch.local.registered",
        Some("Local project watcher registered".to_string()),
        fields,
    );
}

fn emit_watch_local_unregistered(project_id: &str, project_root: &Path, watched_dir_count: usize) {
    let mut fields = Map::new();
    fields.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    fields.insert(
        "project_path".to_string(),
        Value::String(project_root.display().to_string()),
    );
    fields.insert(
        "watched_dir_count".to_string(),
        Value::Number(serde_json::Number::from(watched_dir_count as u64)),
    );
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "watch.local.unregistered",
        Some("Local project watcher unregistered".to_string()),
        fields,
    );
}

fn emit_watch_local_reconciled(
    project_id: &str,
    project_root: &Path,
    watched_dir_count: usize,
    reason: &str,
) {
    let mut fields = Map::new();
    fields.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    fields.insert(
        "project_path".to_string(),
        Value::String(project_root.display().to_string()),
    );
    fields.insert(
        "watched_dir_count".to_string(),
        Value::Number(serde_json::Number::from(watched_dir_count as u64)),
    );
    fields.insert("reason".to_string(), Value::String(reason.to_string()));
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "watch.local.reconciled",
        Some("Local project watcher reconciled".to_string()),
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

fn matching_tree_notify_contexts(
    registrations: &Arc<Mutex<HashMap<String, WatchRegistration>>>,
    tx: &mpsc::Sender<WatchEvent>,
    debounce: &Arc<Mutex<HashMap<String, Instant>>>,
    gitignores: &Arc<Mutex<HashMap<String, Gitignore>>>,
    watcher: &Arc<Mutex<Option<RecommendedWatcher>>>,
    watch_refcounts: &Arc<Mutex<HashMap<PathBuf, usize>>>,
    event: &Event,
) -> Vec<NotifyEventContext> {
    let registrations = registrations
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    registrations
        .iter()
        .filter_map(|(project_id, registration)| {
            let WatchRegistration::Tree(tree) = registration else {
                return None;
            };
            if !event.paths.iter().any(|path| path.starts_with(&tree.root)) {
                return None;
            }
            Some(NotifyEventContext {
                tx: tx.clone(),
                project_id: project_id.clone(),
                project_root: tree.root.clone(),
                debounce: debounce.clone(),
                gitignores: gitignores.clone(),
                watcher: watcher.clone(),
                watched_dirs: tree.watched_dirs.clone(),
                watch_refcounts: Some(watch_refcounts.clone()),
            })
        })
        .collect()
}

fn handle_shared_tree_notify_event(
    registrations: &Arc<Mutex<HashMap<String, WatchRegistration>>>,
    tx: &mpsc::Sender<WatchEvent>,
    debounce: &Arc<Mutex<HashMap<String, Instant>>>,
    gitignores: &Arc<Mutex<HashMap<String, Gitignore>>>,
    watcher: &Arc<Mutex<Option<RecommendedWatcher>>>,
    watch_refcounts: &Arc<Mutex<HashMap<PathBuf, usize>>>,
    event: Event,
) {
    let contexts = matching_tree_notify_contexts(
        registrations,
        tx,
        debounce,
        gitignores,
        watcher,
        watch_refcounts,
        &event,
    );
    for context in contexts {
        handle_notify_event(&context, event.clone());
    }
}

fn handle_shared_file_notify_event(
    registrations: &Arc<Mutex<HashMap<String, WatchRegistration>>>,
    tx: &mpsc::Sender<WatchEvent>,
    event: Event,
) {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return,
    }

    let registrations = registrations
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    for (project_id, registration) in registrations.iter() {
        let WatchRegistration::File { path } = registration else {
            continue;
        };
        let matched_paths: Vec<PathBuf> = event
            .paths
            .iter()
            .filter(|event_path| *event_path == path)
            .cloned()
            .collect();
        if matched_paths.is_empty() {
            continue;
        }
        send_watch_event(
            tx,
            WatchEvent::FileChanged {
                project_id: project_id.clone(),
                paths: matched_paths,
            },
            project_id,
            "file_changed",
        );
    }
}

impl ProjectWatcher {
    /// Create a new ProjectWatcher. Returns the watcher and a receiver for events.
    pub fn new() -> (Self, mpsc::Receiver<WatchEvent>) {
        let (tx, rx) = mpsc::channel();
        let registrations = Arc::new(Mutex::new(HashMap::new()));
        let git_debounce = Arc::new(Mutex::new(HashMap::new()));
        let gitignores = Arc::new(Mutex::new(HashMap::new()));
        let tree_watch_refcounts = Arc::new(Mutex::new(HashMap::new()));
        let file_watch_refcounts = Arc::new(Mutex::new(HashMap::new()));

        let tree_watcher_slot: Arc<Mutex<Option<RecommendedWatcher>>> = Arc::new(Mutex::new(None));
        let tree_watcher_for_callback: Weak<Mutex<Option<RecommendedWatcher>>> =
            Arc::downgrade(&tree_watcher_slot);
        let registrations_for_tree = registrations.clone();
        let tx_for_tree = tx.clone();
        let debounce_for_tree = git_debounce.clone();
        let gitignores_for_tree = gitignores.clone();
        let tree_refcounts_for_callback = tree_watch_refcounts.clone();
        let tree_watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                let Ok(event) = res else {
                    return;
                };
                let Some(tree_watcher) = tree_watcher_for_callback.upgrade() else {
                    return;
                };
                handle_shared_tree_notify_event(
                    &registrations_for_tree,
                    &tx_for_tree,
                    &debounce_for_tree,
                    &gitignores_for_tree,
                    &tree_watcher,
                    &tree_refcounts_for_callback,
                    event,
                );
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .expect("shared tree watcher should initialize");
        {
            let mut slot = tree_watcher_slot
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            *slot = Some(tree_watcher);
        }

        let file_watcher_slot: Arc<Mutex<Option<RecommendedWatcher>>> = Arc::new(Mutex::new(None));
        let registrations_for_file = registrations.clone();
        let tx_for_file = tx.clone();
        let file_watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                let Ok(event) = res else {
                    return;
                };
                handle_shared_file_notify_event(&registrations_for_file, &tx_for_file, event);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .expect("shared file watcher should initialize");
        {
            let mut slot = file_watcher_slot
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            *slot = Some(file_watcher);
        }

        (
            Self {
                watchers: registrations,
                event_tx: tx,
                gitignores,
                tree_watcher: tree_watcher_slot,
                file_watcher: file_watcher_slot,
                tree_watch_refcounts,
                file_watch_refcounts,
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
        self.unwatch_project(&project_id);

        let gitignore = build_gitignore(&project_root);
        {
            let mut gis = self.gitignores.lock().unwrap_or_else(|e| e.into_inner());
            gis.insert(project_id.clone(), gitignore);
        }

        let watched_dirs = Arc::new(Mutex::new(HashSet::new()));

        let watched_dir_count = {
            let mut watched_dirs_guard = watched_dirs.lock().unwrap_or_else(|e| e.into_inner());
            let mut refcounts = self
                .tree_watch_refcounts
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut watcher = self
                .tree_watcher
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let watcher = watcher
                .as_mut()
                .expect("shared tree watcher missing during watch registration");
            let gis = self.gitignores.lock().unwrap_or_else(|e| e.into_inner());
            let gitignore = gis
                .get(&project_id)
                .expect("watch_project inserted gitignore before reconcile");
            reconcile_shared_pruned_tree_watches(
                watcher,
                &mut refcounts,
                &mut watched_dirs_guard,
                &project_root,
                gitignore,
            )?
        };

        emit_watch_local_registered(&project_id, &project_root, watched_dir_count);
        self.watchers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                project_id,
                WatchRegistration::Tree(TreeWatchRegistration {
                    root: project_root,
                    watched_dirs,
                }),
            );
        Ok(())
    }

    /// Start watching a single file path.
    pub fn watch_file(
        &mut self,
        project_id: String,
        file_path: PathBuf,
    ) -> Result<(), notify::Error> {
        self.unwatch_project(&project_id);

        {
            let mut refcounts = self
                .file_watch_refcounts
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut watcher = self
                .file_watcher
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let watcher = watcher
                .as_mut()
                .expect("shared file watcher missing during watch registration");
            match refcounts.get_mut(&file_path) {
                Some(refcount) => *refcount += 1,
                None => {
                    watcher.watch(&file_path, RecursiveMode::NonRecursive)?;
                    refcounts.insert(file_path.clone(), 1);
                }
            }
        }

        emit_watch_local_registered(&project_id, &file_path, 1);
        self.watchers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(project_id, WatchRegistration::File { path: file_path });
        Ok(())
    }

    /// Stop watching a project.
    pub fn unwatch_project(&mut self, project_id: &str) {
        let registration = self
            .watchers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(project_id);
        if let Some(registration) = registration {
            match registration {
                WatchRegistration::Tree(tree) => {
                    let watched_dir_count = tree
                        .watched_dirs
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .len();
                    let mut watcher = self
                        .tree_watcher
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    let watcher = watcher
                        .as_mut()
                        .expect("shared tree watcher missing during unwatch");
                    let mut refcounts = self
                        .tree_watch_refcounts
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    let mut watched_dirs = tree
                        .watched_dirs
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    release_shared_tree_watches(Some(watcher), &mut refcounts, &mut watched_dirs);
                    emit_watch_local_unregistered(project_id, &tree.root, watched_dir_count);
                }
                WatchRegistration::File { path } => {
                    let mut watcher = self
                        .file_watcher
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    let watcher = watcher
                        .as_mut()
                        .expect("shared file watcher missing during unwatch");
                    let mut refcounts = self
                        .file_watch_refcounts
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    if let Some(refcount) = refcounts.get_mut(&path) {
                        if *refcount <= 1 {
                            refcounts.remove(&path);
                            let _ = watcher.unwatch(&path);
                        } else {
                            *refcount -= 1;
                        }
                    }
                    emit_watch_local_unregistered(project_id, &path, 1);
                }
            }
        }
        let mut gis = self.gitignores.lock().unwrap_or_else(|e| e.into_inner());
        gis.remove(project_id);
    }

    /// Stop all watchers.
    pub fn unwatch_all(&mut self) {
        let ids: Vec<String> = self
            .watchers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .keys()
            .cloned()
            .collect();
        for id in ids {
            self.unwatch_project(&id);
        }
    }

    /// Get the list of currently watched project IDs.
    pub fn watched_projects(&self) -> Vec<String> {
        self.watchers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .keys()
            .cloned()
            .collect()
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
fn handle_notify_event(context: &NotifyEventContext, event: Event) {
    if let Some(reason) = reconcile_watch_dirs_for_event(
        &context.project_id,
        &context.project_root,
        &context.gitignores,
        &context.watcher,
        &context.watched_dirs,
        context.watch_refcounts.as_ref(),
        &event,
    ) {
        emit_watch_local_reconciled(
            &context.project_id,
            &context.project_root,
            reason.1,
            reason.0,
        );
    }

    let classified = classify_notify_event(
        &context.project_id,
        &context.project_root,
        GIT_DEBOUNCE_SECS,
        &context.debounce,
        &context.gitignores,
        &event,
        false,
    );

    if classified.emit_git_changed {
        send_watch_event(
            &context.tx,
            WatchEvent::GitChanged {
                project_id: context.project_id.clone(),
            },
            &context.project_id,
            "git_changed",
        );
    }

    for path in classified.session_files {
        send_watch_event(
            &context.tx,
            WatchEvent::SessionFileCreated {
                project_id: context.project_id.clone(),
                path,
            },
            &context.project_id,
            "session_file_created",
        );
    }

    if classified.gitignore_changed {
        send_watch_event(
            &context.tx,
            WatchEvent::GitignoreChanged {
                project_id: context.project_id.clone(),
            },
            &context.project_id,
            "gitignore_changed",
        );
    }

    if !classified.regular_files.is_empty() {
        send_watch_event(
            &context.tx,
            WatchEvent::FileChanged {
                project_id: context.project_id.clone(),
                paths: classified.regular_files,
            },
            &context.project_id,
            "file_changed",
        );
    }
}

fn reconcile_watch_dirs_for_event(
    project_id: &str,
    project_root: &Path,
    gitignores: &Arc<Mutex<HashMap<String, Gitignore>>>,
    watcher: &Arc<Mutex<Option<RecommendedWatcher>>>,
    watched_dirs: &Arc<Mutex<HashSet<PathBuf>>>,
    watch_refcounts: Option<&Arc<Mutex<HashMap<PathBuf, usize>>>>,
    event: &Event,
) -> Option<(&'static str, usize)> {
    let gis = gitignores.lock().unwrap_or_else(|error| error.into_inner());
    let gitignore = gis.get(project_id)?;
    let mut watcher = watcher.lock().unwrap_or_else(|error| error.into_inner());
    let watcher = watcher.as_mut()?;
    let mut watched_dirs = watched_dirs
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let before = watched_dirs.len();
    let count = if let Some(refcounts) = watch_refcounts {
        let mut refcounts = refcounts.lock().unwrap_or_else(|error| error.into_inner());
        reconcile_shared_pruned_tree_watches_for_event(
            watcher,
            &mut refcounts,
            &mut watched_dirs,
            project_root,
            gitignore,
            event,
        )
        .ok()??
    } else {
        reconcile_pruned_tree_watches_for_event(
            watcher,
            &mut watched_dirs,
            project_root,
            gitignore,
            event,
        )
        .ok()??
    };
    if count == before {
        return None;
    }
    let reason = if event.paths.iter().any(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy())
            .is_some_and(|name| name == ".gitignore" || name == ".taurhausignore")
    }) {
        "gitignore_changed"
    } else {
        "directory_topology_changed"
    };
    Some((reason, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::logging::{install_global_sink, LogFileState};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    type TestWatchState = (
        Arc<Mutex<Option<RecommendedWatcher>>>,
        Arc<Mutex<HashSet<PathBuf>>>,
    );

    fn root() -> PathBuf {
        PathBuf::from("/home/user/projects/taurhaus")
    }

    fn empty_gitignores() -> Arc<Mutex<HashMap<String, Gitignore>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn empty_tree_watch_state() -> TestWatchState {
        (
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(HashSet::new())),
        )
    }

    fn test_notify_context(
        tx: &mpsc::Sender<WatchEvent>,
        project_id: &str,
        project_root: &Path,
        debounce: &Arc<Mutex<HashMap<String, Instant>>>,
        gitignores: &Arc<Mutex<HashMap<String, Gitignore>>>,
        watcher: &Arc<Mutex<Option<RecommendedWatcher>>>,
        watched_dirs: &Arc<Mutex<HashSet<PathBuf>>>,
    ) -> NotifyEventContext {
        NotifyEventContext {
            tx: tx.clone(),
            project_id: project_id.to_string(),
            project_root: project_root.to_path_buf(),
            debounce: debounce.clone(),
            gitignores: gitignores.clone(),
            watcher: watcher.clone(),
            watched_dirs: watched_dirs.clone(),
            watch_refcounts: None,
        }
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

    #[test]
    fn shared_tree_watcher_keeps_refcount_until_last_project_unwatches() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("src")).unwrap();
        let mut shared_refcounts =
            HashMap::from([(root.clone(), 2usize), (root.join("src"), 2usize)]);
        let mut p1_dirs = HashSet::from([root.clone(), root.join("src")]);
        let mut p2_dirs = HashSet::from([root.clone(), root.join("src")]);

        release_shared_tree_watches(None, &mut shared_refcounts, &mut p1_dirs);
        assert!(p1_dirs.is_empty());
        assert_eq!(shared_refcounts.get(&root), Some(&1));
        assert_eq!(shared_refcounts.get(&root.join("src")), Some(&1));

        release_shared_tree_watches(None, &mut shared_refcounts, &mut p2_dirs);
        assert!(p2_dirs.is_empty());
        assert!(shared_refcounts.is_empty());
    }

    // --- Debounce logic test ---

    #[test]
    fn git_debounce_suppresses_rapid_events() {
        let debounce: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = mpsc::channel();
        let root = root();
        let (watcher, watched_dirs) = empty_tree_watch_state();

        // Simulate two rapid git events
        let event1 = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![root.join(".git/HEAD")],
            attrs: Default::default(),
        };

        let gis = empty_gitignores();
        let context =
            test_notify_context(&tx, "p1", &root, &debounce, &gis, &watcher, &watched_dirs);
        handle_notify_event(&context, event1.clone());
        handle_notify_event(&context, event1);

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
        let (watcher, watched_dirs) = empty_tree_watch_state();

        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![root.join("docs/sessions/session-2026-02-17T14-30-45.md")],
            attrs: Default::default(),
        };

        let context =
            test_notify_context(&tx, "p1", &root, &debounce, &gis, &watcher, &watched_dirs);
        handle_notify_event(&context, event);

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
        let (watcher, watched_dirs) = empty_tree_watch_state();

        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![root.join("src/main.rs")],
            attrs: Default::default(),
        };

        let context =
            test_notify_context(&tx, "p1", &root, &debounce, &gis, &watcher, &watched_dirs);
        handle_notify_event(&context, event);

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
        let (watcher, watched_dirs) = empty_tree_watch_state();

        let event = Event {
            kind: EventKind::Access(notify::event::AccessKind::Read),
            paths: vec![root.join("src/main.rs")],
            attrs: Default::default(),
        };

        let context =
            test_notify_context(&tx, "p1", &root, &debounce, &gis, &watcher, &watched_dirs);
        handle_notify_event(&context, event);

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
        let (watcher, watched_dirs) = empty_tree_watch_state();

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
        let context =
            test_notify_context(&tx, "p1", &root, &debounce, &gis, &watcher, &watched_dirs);
        handle_notify_event(&context, event);
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
        let context =
            test_notify_context(&tx, "p1", &root, &debounce, &gis, &watcher, &watched_dirs);
        handle_notify_event(&context, event);
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
        let context =
            test_notify_context(&tx, "p1", &root, &debounce, &gis, &watcher, &watched_dirs);
        handle_notify_event(&context, event);
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
        let context =
            test_notify_context(&tx, "p1", &root, &debounce, &gis, &watcher, &watched_dirs);
        handle_notify_event(&context, event);
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
    fn desired_watch_dirs_preprunes_tool_dirs_but_keeps_git_signal_dirs() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src/nested")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/react")).unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::create_dir_all(root.join(".git/refs/heads/feature")).unwrap();
        std::fs::create_dir_all(root.join(".git/objects/pack")).unwrap();

        let gitignore = build_gitignore(root);
        let watched = desired_watch_dirs_for_root(root, &gitignore);

        assert!(watched.contains(&root.to_path_buf()));
        assert!(watched.contains(&root.join("src")));
        assert!(watched.contains(&root.join("src/nested")));
        assert!(watched.contains(&root.join(".git")));
        assert!(watched.contains(&root.join(".git/refs")));
        assert!(watched.contains(&root.join(".git/refs/heads")));
        assert!(watched.contains(&root.join(".git/refs/heads/feature")));
        assert!(!watched.contains(&root.join("node_modules")));
        assert!(!watched.contains(&root.join("node_modules/react")));
        assert!(!watched.contains(&root.join("target")));
        assert!(!watched.contains(&root.join(".git/objects")));
    }

    #[test]
    fn reconcile_pruned_tree_watches_reloads_on_gitignore_change() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("generated/cache")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join(".gitignore"), "").unwrap();

        let mut watcher = RecommendedWatcher::new(
            |_| {},
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .unwrap();
        let mut watched_dirs = HashSet::new();
        let gitignore = build_gitignore(&root);
        reconcile_pruned_tree_watches(&mut watcher, &mut watched_dirs, &root, &gitignore).unwrap();
        assert!(watched_dirs.contains(&root.join("generated")));

        std::fs::write(root.join(".gitignore"), "generated/\n").unwrap();
        let gitignore = build_gitignore(&root);
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![root.join(".gitignore")],
            attrs: Default::default(),
        };
        reconcile_pruned_tree_watches_for_event(
            &mut watcher,
            &mut watched_dirs,
            &root,
            &gitignore,
            &event,
        )
        .unwrap();

        assert!(!watched_dirs.contains(&root.join("generated")));
        assert!(!watched_dirs.contains(&root.join("generated/cache")));
        assert!(watched_dirs.contains(&root.join("src")));
    }

    #[test]
    fn reconcile_pruned_tree_watches_skips_newly_created_ignored_dir() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("src")).unwrap();

        let mut watcher = RecommendedWatcher::new(
            |_| {},
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .unwrap();
        let mut watched_dirs = HashSet::new();
        let gitignore = build_gitignore(&root);
        reconcile_pruned_tree_watches(&mut watcher, &mut watched_dirs, &root, &gitignore).unwrap();

        std::fs::create_dir_all(root.join("node_modules/react")).unwrap();
        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::Folder),
            paths: vec![root.join("node_modules")],
            attrs: Default::default(),
        };
        let result = reconcile_pruned_tree_watches_for_event(
            &mut watcher,
            &mut watched_dirs,
            &root,
            &gitignore,
            &event,
        )
        .unwrap();

        assert!(result.is_none());
        assert!(!watched_dirs.contains(&root.join("node_modules")));
        assert!(!watched_dirs.contains(&root.join("node_modules/react")));
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
        let (watcher, watched_dirs) = empty_tree_watch_state();

        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![project_root.join("src/main.rs")],
            attrs: Default::default(),
        };

        let context = test_notify_context(
            &tx,
            "p-drop",
            &project_root,
            &debounce,
            &gis,
            &watcher,
            &watched_dirs,
        );
        handle_notify_event(&context, event);

        let lines = wait_for_lines(&log_path, 1);
        let dropped: serde_json::Value = serde_json::from_str(&lines[0]).expect("json");
        assert_eq!(dropped["event"], "watch.event.dropped");
        assert_eq!(dropped["project_id"], "p-drop");
        assert_eq!(dropped["watch_event"], "file_changed");
        assert_eq!(dropped["level"], "WARN");
    }
}
