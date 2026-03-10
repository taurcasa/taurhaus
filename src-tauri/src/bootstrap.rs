use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{AppHandle, Emitter, Manager};

use crate::commands::projects::DbState;
use crate::task_scanner::claude_index::{
    build_claude_source_index_with_live_sessions, ClaudeSourceIndex,
};
use crate::{db, provider, search, services, ProviderState, SearchState};

/// On startup, re-seed last_activity_at from each project's latest git commit.
/// This corrects projects whose activity timestamp was incorrectly set to
/// registration time instead of actual last-commit time.
///
/// IMPORTANT: The DB lock is released between projects so frontend IPC commands
/// are not blocked during slow git operations (especially over the daemon).
pub(crate) fn startup_reseed_activity(app: &AppHandle) {
    // Snapshot the project list, then release the DB lock immediately.
    let projects = {
        let db_state = app.state::<DbState>();
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to lock DB for activity reseed: {e}");
                return;
            }
        };
        match db::queries::list_projects(&conn) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to list projects for activity reseed: {e}");
                return;
            }
        }
        // conn lock dropped here
    };

    let provider_state = app.state::<ProviderState>();
    let db_state = app.state::<DbState>();

    let mut updated = 0;
    for project in projects {
        let provider = provider_state.resolve(&project.path);

        // Do git I/O WITHOUT holding the DB lock
        let git_status = provider.git_status(&project.path).ok();
        let commit_time = provider.latest_commit_time(&project.path).ok().flatten();

        // Brief DB lock per project to write results
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(project_id = project.id, error = %e, "reseed: db lock poisoned, skipping project");
                continue;
            }
        };

        if let Some(status) = git_status {
            if let Err(err) = db::queries::update_cached_git_status(
                &conn,
                &project.id,
                status.branch.as_deref(),
                status.is_dirty,
            ) {
                tracing::warn!(
                    project_id = project.id.as_str(),
                    error = %err,
                    "reseed: failed to update cached git status"
                );
            }
        }

        if let Some(commit_time) = commit_time {
            let commit_ts = commit_time.to_rfc3339();
            if project.last_activity_at.as_deref() != Some(&commit_ts) {
                if let Err(err) = db::queries::update_project(
                    &conn,
                    &project.id,
                    None,
                    None,
                    None,
                    Some(Some(&commit_ts)),
                    None,
                ) {
                    tracing::warn!(
                        project_id = project.id.as_str(),
                        error = %err,
                        "reseed: failed to update project activity timestamp"
                    );
                } else {
                    updated += 1;
                }
            }
        }
        // conn lock dropped here — frontend can interleave
    }

    if updated > 0 {
        tracing::info!(updated, "Re-seeded activity timestamps from git");
    }

    // Notify frontend that cached git data is now fresh — it may have loaded
    // the project list before the reseed completed (race on first launch).
    let _ = app.emit("projects-reseed-complete", ());
}

/// On startup, build the search index if it's empty.
///
/// Only holds locks briefly: checks doc count with search lock, then acquires
/// both locks for the rebuild if needed. The rebuild is a one-time operation
/// (subsequent startups skip it), so the longer hold is acceptable.
pub(crate) fn startup_search_index(app: &AppHandle) {
    // Check if index is already populated — brief lock
    {
        let search_state = app.state::<SearchState>();
        let index = match search_state.0.lock() {
            Ok(i) => i,
            Err(e) => {
                tracing::error!("Failed to lock search index for startup build: {e}");
                return;
            }
        };

        let doc_count = index.doc_count().unwrap_or(0);
        if doc_count > 0 {
            tracing::info!(
                doc_count,
                "Search index already populated, skipping rebuild"
            );
            return;
        }
        // search lock dropped here
    }

    // Index is empty — need to rebuild. This holds both locks but only happens
    // on first run (or after index wipe), so the brief block is acceptable.
    let search_state = app.state::<SearchState>();
    let mut index = match search_state.0.lock() {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("Failed to lock search index for rebuild: {e}");
            return;
        }
    };

    let db_state = app.state::<DbState>();
    let conn = match db_state.0.lock() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to lock DB for startup index build: {e}");
            return;
        }
    };

    match search::indexer::rebuild_all(&mut index, &conn) {
        Ok(total) => {
            tracing::info!(total, "Built initial search index");
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to build initial search index");
        }
    }
}

/// On startup, scan all registered projects for unimported session handoffs.
///
/// IMPORTANT: DB lock released between projects to avoid blocking frontend IPC.
pub(crate) fn startup_session_scan(app: &AppHandle) {
    // Snapshot project list, release lock immediately.
    let projects = {
        let db_state = app.state::<DbState>();
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to lock DB for startup session scan: {e}");
                return;
            }
        };
        match db::queries::list_projects(&conn) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to list projects for startup scan: {e}");
                return;
            }
        }
        // conn lock dropped here
    };

    let db_state = app.state::<DbState>();

    for project in projects {
        let project_root = std::path::Path::new(&project.path);

        // Brief lock per project for the import operation
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(project_id = project.id, error = %e, "session import: db lock poisoned, skipping project");
                continue;
            }
        };

        match services::session_import::scan_and_import_sessions(&conn, &project.id, project_root) {
            Ok(imported) if !imported.is_empty() => {
                tracing::info!(
                    project = project.name,
                    count = imported.len(),
                    "Imported sessions on startup"
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(
                    project = project.name,
                    error = %e,
                    "Failed to scan sessions on startup"
                );
            }
        }
        // conn lock dropped here — frontend can interleave
    }
}

/// On startup, scan all registered projects' tasks and seed the SQLite database.
///
/// This ensures the first frontend read has data. Subsequent updates are
/// event-driven (daemon watches `~/.claude/tasks/`).
pub(crate) fn startup_task_scan(app: &AppHandle) {
    sync_all_project_tasks_with_cycle(app, next_task_scan_cycle_id());
}

/// Trigger payload for task scan loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskScanTrigger {
    Full,
    ClaudeTaskPaths(Vec<PathBuf>),
}

#[derive(Debug, Clone)]
struct TaskScanCycleContext {
    cycle_id: u64,
    sessions: Vec<crate::session_scanner::RuntimeSession>,
    claude_index: ClaudeSourceIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TaskStatusSignature {
    task_count: usize,
    status_hash: u64,
}

static TASK_SCAN_CYCLE_COUNTER: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_task_scan_cycle_id() -> u64 {
    TASK_SCAN_CYCLE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn build_task_scan_cycle_context(cycle_id: u64) -> TaskScanCycleContext {
    let sessions = crate::session_scanner::scan_sessions_for_runtime();
    let claude_index = build_claude_source_index_with_live_sessions(&sessions);
    TaskScanCycleContext {
        cycle_id,
        sessions,
        claude_index,
    }
}

/// Background thread that handles task re-scanning with trailing-edge debounce.
///
/// Waits for a trigger signal (from file watcher events), then drains additional
/// signals for 2 seconds. After the debounce window closes, scans all projects'
/// tasks and persists to SQLite. This ensures rapid task file changes (e.g.,
/// Claude creating 4 tasks at once) result in only one scan.
pub(crate) fn task_scan_loop(rx: std::sync::mpsc::Receiver<TaskScanTrigger>, app: AppHandle) {
    use std::time::{Duration, Instant};
    const DEBOUNCE: Duration = Duration::from_secs(2);

    while let Ok(first) = rx.recv() {
        let mut full_scan = false;
        let mut claude_paths: Vec<PathBuf> = Vec::new();
        apply_task_scan_trigger(first, &mut full_scan, &mut claude_paths);

        // Trailing-edge debounce: drain for 2 seconds
        let deadline = Instant::now() + DEBOUNCE;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok(trigger) => apply_task_scan_trigger(trigger, &mut full_scan, &mut claude_paths),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }

        let cycle_id = next_task_scan_cycle_id();
        if full_scan || !sync_project_tasks_for_claude_changes(&app, &claude_paths, cycle_id) {
            sync_all_project_tasks_with_cycle(&app, cycle_id);
        }
    }
}

fn apply_task_scan_trigger(
    trigger: TaskScanTrigger,
    full_scan: &mut bool,
    claude_paths: &mut Vec<PathBuf>,
) {
    match trigger {
        TaskScanTrigger::Full => *full_scan = true,
        TaskScanTrigger::ClaudeTaskPaths(paths) => claude_paths.extend(paths),
    }
}

fn sync_project_tasks_for_claude_changes(
    app: &AppHandle,
    changed_paths: &[PathBuf],
    cycle_id: u64,
) -> bool {
    if changed_paths.is_empty() {
        tracing::warn!(
            cycle_id,
            "Task sync trigger had no changed Claude task paths"
        );
        return false;
    }

    let Some(source_keys) = collect_source_keys_from_paths(changed_paths) else {
        tracing::warn!(
            cycle_id,
            changed_paths = changed_paths.len(),
            "Task sync fallback to full scan: failed to derive source key from changed path set"
        );
        return false;
    };
    if source_keys.is_empty() {
        tracing::warn!(
            cycle_id,
            changed_paths = changed_paths.len(),
            "Task sync fallback to full scan: changed paths resolved to empty source key set"
        );
        return false;
    }

    let db_state = app.state::<DbState>();
    let projects = {
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(error) => {
                tracing::warn!(
                    cycle_id,
                    error = %error,
                    "Task sync fallback to full scan: db lock failed while loading projects"
                );
                return false;
            }
        };
        match db::queries::list_projects(&conn) {
            Ok(list) => list,
            Err(error) => {
                tracing::warn!(
                    cycle_id,
                    error = %error,
                    "Task sync fallback to full scan: project list query failed"
                );
                return false;
            }
        }
    };

    let context = build_task_scan_cycle_context(cycle_id);
    let Some(target_ids) =
        resolve_affected_project_ids(&projects, &source_keys, &context.claude_index)
    else {
        tracing::warn!(
            cycle_id,
            source_keys = source_keys.len(),
            "Task sync fallback to full scan: changed source keys did not map cleanly to projects"
        );
        return false;
    };
    if target_ids.is_empty() {
        tracing::warn!(
            cycle_id,
            source_keys = source_keys.len(),
            "Task sync fallback to full scan: no affected projects resolved from source keys"
        );
        return false;
    }

    sync_project_tasks_for_projects(app, &projects, Some(&target_ids), &context);
    true
}

fn collect_source_keys_from_paths(paths: &[PathBuf]) -> Option<BTreeSet<String>> {
    let mut source_keys = BTreeSet::new();
    for path in paths {
        let source_key = extract_source_key_from_task_path(path)?;
        source_keys.insert(source_key);
    }
    Some(source_keys)
}

fn extract_source_key_from_task_path(path: &Path) -> Option<String> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let marker = "/.claude/tasks/";
    let idx = normalized.find(marker)?;
    let rest = &normalized[idx + marker.len()..];
    let source_key = rest.split('/').next()?.trim();
    if source_key.is_empty() {
        return None;
    }
    Some(source_key.to_string())
}

fn resolve_affected_project_ids(
    projects: &[crate::models::Project],
    source_keys: &BTreeSet<String>,
    index: &ClaudeSourceIndex,
) -> Option<HashSet<String>> {
    let mut project_ids_by_path: HashMap<String, Vec<String>> = HashMap::new();
    for project in projects {
        project_ids_by_path
            .entry(provider::path::normalize_project_path(&project.path))
            .or_default()
            .push(project.id.clone());
    }

    let mut target_ids = HashSet::new();
    for source_key in source_keys {
        let mut matched_any = false;
        if let Some(project_path) = index.sessions.get(source_key) {
            let normalized =
                provider::path::normalize_project_path(&project_path.to_string_lossy());
            if let Some(ids) = project_ids_by_path.get(&normalized) {
                target_ids.extend(ids.iter().cloned());
                matched_any = true;
            }
        }

        if let Some(project_paths) = index.teams.get(source_key) {
            for project_path in project_paths {
                let normalized =
                    provider::path::normalize_project_path(&project_path.to_string_lossy());
                if let Some(ids) = project_ids_by_path.get(&normalized) {
                    target_ids.extend(ids.iter().cloned());
                    matched_any = true;
                }
            }
        }

        if !matched_any {
            return None;
        }
    }

    Some(target_ids)
}

fn sync_all_project_tasks_with_cycle(app: &AppHandle, cycle_id: u64) {
    let db_state = app.state::<DbState>();

    // Snapshot project list (brief DB lock)
    let projects = {
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(error) => {
                tracing::warn!(
                    cycle_id,
                    error = %error,
                    "Skipping full task sync cycle: db lock failed while loading projects"
                );
                return;
            }
        };
        match db::queries::list_projects(&conn) {
            Ok(list) => list,
            Err(error) => {
                tracing::warn!(
                    cycle_id,
                    error = %error,
                    "Skipping full task sync cycle: project list query failed"
                );
                return;
            }
        }
    };

    let context = build_task_scan_cycle_context(cycle_id);
    sync_project_tasks_for_projects(app, &projects, None, &context);
}

fn sync_project_tasks_for_projects(
    app: &AppHandle,
    projects: &[crate::models::Project],
    target_ids: Option<&HashSet<String>>,
    context: &TaskScanCycleContext,
) {
    let db_state = app.state::<DbState>();
    let provider_state = app.state::<ProviderState>();
    let generation_state = app.state::<services::task_sync::TaskScanGenerationState>();

    let mut total_tasks = 0;
    for project in projects {
        if let Some(ids) = target_ids {
            if !ids.contains(&project.id) {
                continue;
            }
        }

        // Scan tasks from files (daemon or local)
        let scan_result = services::task_sync::scan_tasks_from_files(
            &provider_state,
            &project.path,
            Some(context.cycle_id),
            Some(&context.sessions),
            Some(&context.claude_index),
        );

        // Normalize path for DB storage
        let normalized_path = provider::path::normalize_project_path(&project.path);

        let (before_sig, after_sig) = {
            let conn = match db_state.0.lock() {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "task scan: db lock poisoned, skipping project");
                    continue;
                }
            };
            let before = load_active_task_signature(&conn, &normalized_path);
            services::task_sync::persist_task_scan_with_generation(
                &conn,
                &normalized_path,
                &scan_result,
                generation_state.inner(),
                context.cycle_id,
            );
            let after = load_active_task_signature(&conn, &normalized_path);
            (before, after)
        };

        total_tasks += scan_result.tasks.len();

        if before_sig != after_sig {
            let task_count = after_sig
                .map(|sig| sig.task_count)
                .unwrap_or(scan_result.tasks.len());
            let _ = app.emit(
                "project-tasks-changed",
                serde_json::json!({
                    "project_id": project.id,
                    "task_count": task_count,
                }),
            );
        }
    }

    if total_tasks > 0 {
        tracing::debug!(total_tasks, projects = projects.len(), "Task sync complete");
    }
}

fn load_active_task_signature(
    conn: &rusqlite::Connection,
    normalized_path: &str,
) -> Option<TaskStatusSignature> {
    match crate::db::task_queries::get_tasks_for_project(conn, normalized_path) {
        Ok(tasks) => Some(task_status_signature(&tasks)),
        Err(error) => {
            tracing::warn!(
                normalized_path,
                error = %error,
                "Failed to load active task signature for project"
            );
            None
        }
    }
}

fn task_status_signature(tasks: &[crate::db::task_queries::PersistedTask]) -> TaskStatusSignature {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for task in tasks {
        task.source.hash(&mut hasher);
        task.source_key.hash(&mut hasher);
        task.source_task_id.hash(&mut hasher);
        task.status.hash(&mut hasher);
    }
    TaskStatusSignature {
        task_count: tasks.len(),
        status_hash: hasher.finish(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_project(id: &str, path: &str) -> crate::models::Project {
        crate::models::Project {
            id: id.to_string(),
            name: id.to_string(),
            path: path.to_string(),
            description: None,
            last_activity_at: None,
            hero_preference: None,
            created_at: "2026-03-04T00:00:00Z".to_string(),
            updated_at: "2026-03-04T00:00:00Z".to_string(),
            cached_branch: None,
            cached_is_dirty: None,
        }
    }

    fn make_task(
        source: &str,
        source_key: &str,
        source_task_id: &str,
        status: &str,
    ) -> crate::db::task_queries::PersistedTask {
        crate::db::task_queries::PersistedTask {
            project_path: "/projects/foo".to_string(),
            source: source.to_string(),
            source_key: source_key.to_string(),
            source_task_id: source_task_id.to_string(),
            subject: "x".to_string(),
            description: None,
            active_form: None,
            status: status.to_string(),
            blocks: vec![],
            blocked_by: vec![],
            owner: None,
            session_id: None,
            first_seen_at: "2026-03-04T00:00:00Z".to_string(),
            state_changed_at: None,
            updated_at: "2026-03-04T00:00:00Z".to_string(),
            archived_at: None,
            last_status: Some(status.to_string()),
            archived_reason: None,
        }
    }

    #[test]
    fn extracts_source_key_from_task_path() {
        let path = PathBuf::from("/home/user/.claude/tasks/team-ops/1.json");
        assert_eq!(
            extract_source_key_from_task_path(&path).as_deref(),
            Some("team-ops")
        );
    }

    #[test]
    fn resolves_affected_projects_from_source_keys() {
        let projects = vec![
            make_project("a", "/home/user/projects/a"),
            make_project("b", "/home/user/projects/b"),
        ];
        let mut index = ClaudeSourceIndex::default();
        index.sessions.insert(
            "session-1".to_string(),
            PathBuf::from("/home/user/projects/a"),
        );
        index.teams.insert(
            "team-x".to_string(),
            vec![PathBuf::from("/home/user/projects/b")],
        );
        let source_keys = BTreeSet::from(["session-1".to_string(), "team-x".to_string()]);

        let affected = resolve_affected_project_ids(&projects, &source_keys, &index).unwrap();
        assert_eq!(affected.len(), 2);
        assert!(affected.contains("a"));
        assert!(affected.contains("b"));
    }

    #[test]
    fn task_status_signature_stable_for_unchanged_sets() {
        let base = vec![make_task("claude", "s1", "1", "pending")];
        let same = vec![make_task("claude", "s1", "1", "pending")];
        let changed = vec![make_task("claude", "s1", "1", "completed")];

        assert_eq!(task_status_signature(&base), task_status_signature(&same));
        assert_ne!(
            task_status_signature(&base),
            task_status_signature(&changed)
        );
    }
}
