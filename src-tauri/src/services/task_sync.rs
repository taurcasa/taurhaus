use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::session_scanner::RuntimeSession;
use crate::ProviderState;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ScanGenerationKey {
    project_path: String,
    source: String,
    source_key: String,
}

#[derive(Default)]
pub struct TaskScanGenerationState {
    applied_generations: Mutex<HashMap<ScanGenerationKey, u64>>,
}

const SCAN_GENERATION_RETENTION_WINDOW: u64 = 100;
static WARNED_SNAPSHOT_DAEMON_UNAVAILABLE: AtomicBool = AtomicBool::new(false);

fn should_apply_scan_generation(
    state: &TaskScanGenerationState,
    project_path: &str,
    source: &str,
    source_key: &str,
    generation: u64,
) -> bool {
    if cfg!(test) {
        return true;
    }

    let key = ScanGenerationKey {
        project_path: project_path.to_string(),
        source: source.to_string(),
        source_key: source_key.to_string(),
    };

    let mut applied = state
        .applied_generations
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match applied.get(&key) {
        Some(existing) if generation < *existing => false,
        _ => {
            applied.insert(key, generation);
            true
        }
    }
}

fn cleanup_applied_scan_generations(state: &TaskScanGenerationState, current_generation: u64) {
    let mut applied = state
        .applied_generations
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    prune_generation_map(
        &mut applied,
        current_generation,
        SCAN_GENERATION_RETENTION_WINDOW,
    );
}

fn prune_generation_map(
    applied: &mut HashMap<ScanGenerationKey, u64>,
    current_generation: u64,
    retention_window: u64,
) {
    let min_generation = current_generation.saturating_sub(retention_window);
    applied.retain(|_, generation| *generation >= min_generation);
}

pub(crate) fn persist_task_scan_with_generation(
    conn: &rusqlite::Connection,
    normalized_path: &str,
    scan_result: &crate::task_scanner::TaskResult,
    generation_state: &TaskScanGenerationState,
    scan_generation: u64,
    daemon: Option<&crate::provider::daemon_client::DaemonProvider>,
) {
    persist_task_scan_with_generation_and_publisher(
        conn,
        normalized_path,
        scan_result,
        generation_state,
        scan_generation,
        &crate::provider::platform_paths::PlatformPaths::teams_dir(),
        |params| publish_operational_snapshots_through_daemon(daemon, params),
    );
}

fn persist_task_scan_with_generation_and_publisher<P>(
    conn: &rusqlite::Connection,
    normalized_path: &str,
    scan_result: &crate::task_scanner::TaskResult,
    generation_state: &TaskScanGenerationState,
    scan_generation: u64,
    operational_teams_dir: &Path,
    publish: P,
) where
    P: FnOnce(
        crate::daemon::protocol::CoordinationPublishOperationalSnapshotsParams,
    ) -> Result<
        crate::daemon::protocol::CoordinationPublishOperationalSnapshotsResult,
        String,
    >,
{
    cleanup_applied_scan_generations(generation_state, scan_generation);
    let source_outcomes = normalized_source_outcomes(scan_result);
    let now = chrono::Utc::now().to_rfc3339();
    let mut persisted_by_key: std::collections::HashMap<
        (String, String),
        Vec<crate::db::task_queries::PersistedTask>,
    > = std::collections::HashMap::new();

    for source_outcome in &source_outcomes {
        if let crate::task_scanner::ScanOutcome::Data(tasks) = &source_outcome.outcome {
            for t in tasks {
                persisted_by_key
                    .entry((source_outcome.source.clone(), t.source_key.clone()))
                    .or_default()
                    .push(crate::db::task_queries::PersistedTask {
                        project_path: normalized_path.to_string(),
                        source: source_outcome.source.clone(),
                        source_key: t.source_key.clone(),
                        source_task_id: t.id.clone(),
                        subject: t.subject.clone(),
                        description: t.description.clone(),
                        active_form: t.active_form.clone(),
                        status: t.status.to_string(),
                        blocks: t.blocks.clone(),
                        blocked_by: t.blocked_by.clone(),
                        owner: t.owner.clone(),
                        session_id: t.session_id.clone(),
                        first_seen_at: now.clone(),
                        state_changed_at: Some(now.clone()),
                        updated_at: now.clone(),
                        archived_at: None,
                        last_status: Some(t.status.to_string()),
                        archived_reason: None,
                        effort: t.effort.clone(),
                        effort_why: t.effort_why.clone(),
                        deadline_minutes: t.deadline_minutes,
                    });
            }
        }
    }

    for ((source, source_key), persisted) in persisted_by_key {
        if !should_apply_scan_generation(
            generation_state,
            normalized_path,
            &source,
            &source_key,
            scan_generation,
        ) {
            tracing::debug!(
                source = %source,
                source_key = %source_key,
                generation = scan_generation,
                "Skipping stale task upsert generation"
            );
            continue;
        }

        if let Err(e) = crate::db::task_queries::upsert_tasks(conn, &persisted) {
            tracing::warn!(
                error = %e,
                source = %source,
                source_key = %source_key,
                "Failed to persist scanned tasks"
            );
        }
    }

    prune_stale_tasks(
        conn,
        normalized_path,
        &source_outcomes,
        generation_state,
        scan_generation,
    );

    let prepared = match crate::coordination::operational_context::prepare_project_task_snapshots(
        operational_teams_dir,
        conn,
        normalized_path,
    ) {
        Ok(prepared) => prepared,
        Err(err) => {
            tracing::warn!(
                project_path = %normalized_path,
                error = %err,
                "failed to prepare operational snapshots after task persistence"
            );
            return;
        }
    };
    if prepared.is_empty() {
        return;
    }
    let params = crate::daemon::protocol::CoordinationPublishOperationalSnapshotsParams {
        publications: prepared
            .into_iter()
            .map(|(snapshot, task_state_changed_at)| {
                crate::daemon::protocol::CoordinationOperationalSnapshotPublication {
                    snapshot,
                    task_state_changed_at,
                }
            })
            .collect(),
    };
    if let Err(err) = publish(params) {
        if !WARNED_SNAPSHOT_DAEMON_UNAVAILABLE.swap(true, Ordering::Relaxed) {
            tracing::warn!(
                project_path = %normalized_path,
                error = %err,
                "operational snapshot publication skipped because the daemon is unavailable"
            );
        }
    } else {
        WARNED_SNAPSHOT_DAEMON_UNAVAILABLE.store(false, Ordering::Relaxed);
    }
}

fn publish_operational_snapshots_through_daemon(
    daemon: Option<&crate::provider::daemon_client::DaemonProvider>,
    params: crate::daemon::protocol::CoordinationPublishOperationalSnapshotsParams,
) -> Result<crate::daemon::protocol::CoordinationPublishOperationalSnapshotsResult, String> {
    let daemon = daemon.ok_or_else(|| "daemon is unavailable".to_string())?;
    if !daemon.is_connected() && !daemon.try_reconnect() {
        return Err("daemon is not connected".to_string());
    }
    let request = crate::daemon::protocol::DaemonRequest::new(
        format!("snapshot-publish-{}", uuid::Uuid::new_v4().simple()),
        crate::daemon::protocol::method::COORDINATION_PUBLISH_OPERATIONAL_SNAPSHOTS,
        params,
    );
    let response = daemon
        .send_status_request_within(&request, std::time::Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    if let Some(error) = response.error {
        return Err(error.message);
    }
    response
        .result
        .ok_or_else(|| "snapshot publication returned no result".to_string())
        .and_then(|result| serde_json::from_value(result).map_err(|error| error.to_string()))
}

#[cfg(test)]
fn persist_task_scan_with_generation_and_operational_dir(
    conn: &rusqlite::Connection,
    normalized_path: &str,
    scan_result: &crate::task_scanner::TaskResult,
    generation_state: &TaskScanGenerationState,
    scan_generation: u64,
    operational_teams_dir: &Path,
) {
    persist_task_scan_with_generation_and_publisher(
        conn,
        normalized_path,
        scan_result,
        generation_state,
        scan_generation,
        operational_teams_dir,
        |params| {
            crate::daemon::state_writes::publish_operational_snapshots(
                operational_teams_dir,
                params,
            )
            .map_err(|error| error.to_string())
        },
    );
}

fn prune_stale_tasks(
    conn: &rusqlite::Connection,
    normalized_path: &str,
    source_outcomes: &[crate::task_scanner::SourceScanOutcome],
    generation_state: &TaskScanGenerationState,
    scan_generation: u64,
) {
    let mut active_by_source_key: std::collections::HashMap<(String, String), Vec<String>> =
        std::collections::HashMap::new();
    for source_outcome in source_outcomes {
        let source = source_outcome.source.clone();
        match &source_outcome.outcome {
            crate::task_scanner::ScanOutcome::Unavailable(reason) => {
                tracing::info!(
                    source = %source,
                    reason = %reason,
                    "Skipping stale prune for unavailable source"
                );
                continue;
            }
            crate::task_scanner::ScanOutcome::Data(tasks) => {
                for task in tasks {
                    active_by_source_key
                        .entry((source.clone(), task.source_key.clone()))
                        .or_default()
                        .push(task.id.clone());
                }
            }
            crate::task_scanner::ScanOutcome::DefinitivelyEmpty => {}
        }

        let db_keys = match crate::db::task_queries::get_active_source_keys_for_project_source(
            conn,
            normalized_path,
            &source,
        ) {
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    source = %source,
                    "Failed to load source keys for stale task pruning"
                );
                continue;
            }
            Ok(keys) => keys,
        };

        let mut source_keys: std::collections::BTreeSet<String> = db_keys.into_iter().collect();
        source_keys.extend(
            active_by_source_key
                .keys()
                .filter(|(s, _)| s == &source)
                .map(|(_, key)| key.clone()),
        );

        for source_key in source_keys {
            if !should_apply_scan_generation(
                generation_state,
                normalized_path,
                &source,
                &source_key,
                scan_generation,
            ) {
                tracing::debug!(
                    source = %source,
                    source_key = %source_key,
                    generation = scan_generation,
                    "Skipping stale prune for stale generation"
                );
                continue;
            }

            let active_ids_storage = active_by_source_key
                .get(&(source.clone(), source_key.clone()))
                .cloned()
                .unwrap_or_default();
            let active_ids: Vec<&str> = active_ids_storage.iter().map(String::as_str).collect();

            match crate::db::task_queries::archive_or_delete_stale_tasks(
                conn,
                normalized_path,
                &source,
                &source_key,
                &active_ids,
            ) {
                Ok(result) => {
                    if result.archived > 0 || result.deleted > 0 {
                        tracing::info!(
                            source = %source,
                            source_key = %source_key,
                            archived = result.archived,
                            deleted = result.deleted,
                            "Pruned stale tasks"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        source = %source,
                        source_key = %source_key,
                        "Failed to prune stale tasks"
                    );
                }
            }
        }
    }
}

fn normalized_source_outcomes(
    scan_result: &crate::task_scanner::TaskResult,
) -> Vec<crate::task_scanner::SourceScanOutcome> {
    if !scan_result.source_outcomes.is_empty() {
        return scan_result.source_outcomes.clone();
    }

    let failures: std::collections::HashMap<String, String> = scan_result
        .errors
        .iter()
        .map(|(source, reason)| (source.clone(), reason.clone()))
        .collect();

    crate::session_scanner::cli_tool::all_tools()
        .iter()
        .map(|tool| {
            let source = tool.tool.to_string();
            let outcome = if let Some(reason) = failures.get(&source) {
                crate::task_scanner::ScanOutcome::Unavailable(reason.clone())
            } else {
                let tasks: Vec<crate::task_scanner::UnifiedTask> = scan_result
                    .tasks
                    .iter()
                    .filter(|t| t.source.to_string() == source)
                    .cloned()
                    .collect();
                if tasks.is_empty() {
                    crate::task_scanner::ScanOutcome::DefinitivelyEmpty
                } else {
                    crate::task_scanner::ScanOutcome::Data(tasks)
                }
            };
            crate::task_scanner::SourceScanOutcome { source, outcome }
        })
        .collect()
}

/// Scan task files from live sources (daemon or local).
pub(crate) fn scan_tasks_from_files(
    app: &tauri::AppHandle,
    provider: &ProviderState,
    project_path: &str,
    scan_cycle_id: Option<u64>,
    cached_sessions: Option<&[RuntimeSession]>,
    cached_claude_index: Option<&crate::task_scanner::claude_index::ClaudeSourceIndex>,
) -> crate::task_scanner::TaskResult {
    if let Some((daemon, reconnected)) = daemon_for_task_scan(provider, project_path) {
        #[cfg(feature = "mesh-bridged-backend")]
        if reconnected {
            if let Err(error) = crate::commands::settings::push_launch_settings_to_daemon(app) {
                tracing::warn!(
                    error = %error,
                    "Failed to repush launch settings after task-scan reconnect"
                );
            }
        }
        #[cfg(not(feature = "mesh-bridged-backend"))]
        let _ = (app, reconnected);
        let linux_path = crate::provider::path::to_linux(project_path)
            .unwrap_or_else(|| project_path.to_string());

        let id = "scan-project-tasks";
        let request = crate::daemon::protocol::DaemonRequest::new(
            id,
            crate::daemon::protocol::method::GET_PROJECT_TASKS,
            crate::daemon::protocol::ProjectTasksParams {
                path: linux_path,
                scan_cycle_id,
            },
        );
        match daemon.send_status_request(&request) {
            Ok(response) if response.is_ok() => {
                if let Some(result_payload) = response.result {
                    match serde_json::from_value(result_payload) {
                        Ok(result) => return result,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "Failed to deserialize task scan from daemon"
                            );
                        }
                    }
                }
            }
            Ok(response) => {
                tracing::warn!(error = ?response.error, "Daemon task scan failed");
            }
            Err(e) => {
                tracing::warn!(error = %e, "Daemon request failed for task scan");
            }
        }
    }

    // Continuity read: the sessions only locate transcripts to read tasks
    // from; a degraded scan keeps the last good snapshot (degraded flag
    // dropped on purpose), nothing is bound to it.
    let all_sessions: Vec<RuntimeSession> = cached_sessions
        .map(|s| s.to_vec())
        .unwrap_or_else(|| crate::session_scanner::scan_sessions_for_runtime().0);
    let project_sessions: Vec<RuntimeSession> = all_sessions
        .into_iter()
        .filter(|s| s.project_path == project_path)
        .collect();

    crate::task_scanner::get_tasks_for_project_with_index(
        project_path,
        &project_sessions,
        cached_claude_index,
    )
}

fn daemon_for_task_scan<'a>(
    provider: &'a ProviderState,
    project_path: &str,
) -> Option<(&'a crate::provider::daemon_client::DaemonProvider, bool)> {
    if !crate::provider::path::is_wsl_path(project_path) {
        return None;
    }

    let daemon = provider.daemon.as_ref()?;
    let reconnected = !daemon.is_connected() && daemon.try_reconnect();

    daemon.is_connected().then_some((daemon, reconnected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;
    use tempfile::{NamedTempFile, TempDir};

    use crate::coordination::domain::{Member, MemberRole};
    use crate::coordination::stores::{
        OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
        OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot, TeamConfig, TeamConfigStore,
    };
    use crate::session_scanner::cli_tool::CliTool;

    #[test]
    fn generation_map_pruning_keeps_recent_window() {
        let mut map = std::collections::HashMap::new();
        for i in 1..=300_u64 {
            map.insert(
                ScanGenerationKey {
                    project_path: "/projects/foo".to_string(),
                    source: "claude".to_string(),
                    source_key: format!("session-{i}"),
                },
                i,
            );
        }

        prune_generation_map(&mut map, 300, 100);
        assert!(map.len() <= 101);
        assert!(map.values().all(|generation| *generation >= 200));
    }

    // Regression: commit 0b87699b introduced the task-scan snapshot writer in
    // the desktop process, which cannot safely replace WSL team files over 9p.
    #[test]
    fn task_scan_team_state_writes_are_daemon_routed() {
        let source = include_str!("task_sync.rs");
        let persistence = source
            .split("pub(crate) fn persist_task_scan_with_generation(")
            .nth(1)
            .expect("task persistence implementation")
            .split("fn persist_task_scan_with_generation_and_publisher")
            .next()
            .expect("task persistence body");

        assert!(source.contains("COORDINATION_PUBLISH_OPERATIONAL_SNAPSHOTS"));
        assert!(!persistence.contains("sync_project_task_snapshots"));
        assert!(!persistence.contains("OperationalContextSnapshotStore::save"));
    }

    #[test]
    fn persist_task_scan_updates_operational_snapshot_when_owner_changes() {
        let teams = TempDir::new().expect("teams dir");
        let db = NamedTempFile::new().expect("db file");
        let conn = crate::db::init_db(db.path()).expect("init db");
        let generation_state = TaskScanGenerationState::default();

        TeamConfigStore::save(
            teams.path(),
            "architecture-final",
            &TeamConfig {
                schema_version: 1,
                name: "architecture-final".to_string(),
                description: None,
                created_at: Utc::now(),
                members: vec![Member {
                    name: "frontend-dev".to_string(),
                    role: MemberRole::Agent,
                    role_id: None,
                    role_name: None,
                    focus_area: None,
                    context_summary: None,
                    behavior_summary: None,
                    communication_style: None,
                    runtime_compact_summary: None,
                    instructions: None,
                    behavioral_contract: None,
                    quality_gates: None,
                    handoff_expectations: None,
                    definition_of_done: None,
                    phase_scope: None,
                    mode: None,
                    inherits_from: None,
                    required_artifacts: None,
                    capabilities: None,
                    model: None,
                    reasoning_effort: None,
                    project_path: "proj-web".into(),
                    cli_tool: CliTool::Codex,
                    extra: Default::default(),
                }],
                extra: Default::default(),
            },
        )
        .expect("save team");

        crate::coordination::operational_context::sync_member_snapshot(
            teams.path(),
            &conn,
            "architecture-final",
            "frontend-dev",
        )
        .expect("seed snapshot");

        let scan_result = crate::task_scanner::TaskResult {
            tasks: vec![crate::task_scanner::UnifiedTask {
                id: "675".to_string(),
                source_key: "session-1".to_string(),
                subject: "Wire snapshot updates".to_string(),
                description: None,
                active_form: None,
                status: crate::task_scanner::TaskStatus::InProgress,
                source: CliTool::Claude,
                blocks: vec![],
                blocked_by: vec![],
                owner: Some("frontend-dev".to_string()),
                session_id: None,
                state_changed_at: None,
                updated_at: None,
                archived_at: None,
                last_status: None,
                archived_reason: None,
                effort: None,
                effort_why: None,
                deadline_minutes: None,
            }],
            errors: vec![],
            source_outcomes: vec![crate::task_scanner::SourceScanOutcome {
                source: "claude".to_string(),
                outcome: crate::task_scanner::ScanOutcome::Data(vec![
                    crate::task_scanner::UnifiedTask {
                        id: "675".to_string(),
                        source_key: "session-1".to_string(),
                        subject: "Wire snapshot updates".to_string(),
                        description: None,
                        active_form: None,
                        status: crate::task_scanner::TaskStatus::InProgress,
                        source: CliTool::Claude,
                        blocks: vec![],
                        blocked_by: vec![],
                        owner: Some("frontend-dev".to_string()),
                        session_id: None,
                        state_changed_at: None,
                        updated_at: None,
                        archived_at: None,
                        last_status: None,
                        archived_reason: None,
                        effort: None,
                        effort_why: None,
                        deadline_minutes: None,
                    },
                ]),
            }],
        };

        persist_task_scan_with_generation_and_operational_dir(
            &conn,
            "proj-web",
            &scan_result,
            &generation_state,
            1,
            teams.path(),
        );

        let snapshot = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");

        assert_eq!(snapshot.task.id, "675");
        assert_eq!(snapshot.task.subject, "Wire snapshot updates");
        assert_eq!(snapshot.task.status, "in_progress");
    }

    #[test]
    fn persist_task_scan_skips_operational_snapshot_rewrite_when_task_context_is_unchanged() {
        let teams = TempDir::new().expect("teams dir");
        let db = NamedTempFile::new().expect("db file");
        let conn = crate::db::init_db(db.path()).expect("init db");
        let generation_state = TaskScanGenerationState::default();

        TeamConfigStore::save(
            teams.path(),
            "architecture-final",
            &TeamConfig {
                schema_version: 1,
                name: "architecture-final".to_string(),
                description: None,
                created_at: Utc::now(),
                members: vec![Member {
                    name: "frontend-dev".to_string(),
                    role: MemberRole::Agent,
                    role_id: None,
                    role_name: None,
                    focus_area: None,
                    context_summary: None,
                    behavior_summary: None,
                    communication_style: None,
                    runtime_compact_summary: None,
                    instructions: None,
                    behavioral_contract: None,
                    quality_gates: None,
                    handoff_expectations: None,
                    definition_of_done: None,
                    phase_scope: None,
                    mode: None,
                    inherits_from: None,
                    required_artifacts: None,
                    capabilities: None,
                    model: None,
                    reasoning_effort: None,
                    project_path: "proj-web".into(),
                    cli_tool: CliTool::Codex,
                    extra: Default::default(),
                }],
                extra: Default::default(),
            },
        )
        .expect("save team");

        let seeded_at = chrono::DateTime::parse_from_rfc3339("2026-03-08T13:00:00Z")
            .expect("timestamp")
            .with_timezone(&Utc);
        OperationalContextSnapshotStore::save(
            teams.path(),
            &OperationalContextSnapshot {
                version: 1,
                team_name: "architecture-final".to_string(),
                member_name: "frontend-dev".to_string(),
                updated_at: seeded_at,
                task: OperationalTaskSnapshot {
                    id: "675".to_string(),
                    subject: "Wire snapshot updates".to_string(),
                    status: "in_progress".to_string(),
                    ..Default::default()
                },
                assignment_footer: OperationalAssignmentFooterSnapshot::default(),
                ownership: OperationalOwnershipSnapshot::default(),
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "proj-web".to_string(),
                    focal_files: Vec::new(),
                },
            },
        )
        .expect("seed snapshot");

        let scan_result = crate::task_scanner::TaskResult {
            tasks: vec![crate::task_scanner::UnifiedTask {
                id: "675".to_string(),
                source_key: "session-1".to_string(),
                subject: "Wire snapshot updates".to_string(),
                description: None,
                active_form: None,
                status: crate::task_scanner::TaskStatus::InProgress,
                source: CliTool::Claude,
                blocks: vec![],
                blocked_by: vec![],
                owner: Some("frontend-dev".to_string()),
                session_id: None,
                state_changed_at: None,
                updated_at: None,
                archived_at: None,
                last_status: None,
                archived_reason: None,
                effort: None,
                effort_why: None,
                deadline_minutes: None,
            }],
            errors: vec![],
            source_outcomes: vec![crate::task_scanner::SourceScanOutcome {
                source: "claude".to_string(),
                outcome: crate::task_scanner::ScanOutcome::Data(vec![
                    crate::task_scanner::UnifiedTask {
                        id: "675".to_string(),
                        source_key: "session-1".to_string(),
                        subject: "Wire snapshot updates".to_string(),
                        description: None,
                        active_form: None,
                        status: crate::task_scanner::TaskStatus::InProgress,
                        source: CliTool::Claude,
                        blocks: vec![],
                        blocked_by: vec![],
                        owner: Some("frontend-dev".to_string()),
                        session_id: None,
                        state_changed_at: None,
                        updated_at: None,
                        archived_at: None,
                        last_status: None,
                        archived_reason: None,
                        effort: None,
                        effort_why: None,
                        deadline_minutes: None,
                    },
                ]),
            }],
        };

        persist_task_scan_with_generation_and_operational_dir(
            &conn,
            "proj-web",
            &scan_result,
            &generation_state,
            1,
            teams.path(),
        );

        let snapshot = OperationalContextSnapshotStore::load(
            teams.path(),
            "architecture-final",
            "frontend-dev",
        )
        .expect("load snapshot")
        .expect("snapshot exists");

        assert_eq!(snapshot.updated_at, seeded_at);
        assert_eq!(snapshot.task.id, "675");
        assert_eq!(snapshot.task.status, "in_progress");
    }

    #[test]
    fn daemon_task_scan_is_skipped_for_local_paths_even_when_daemon_is_connected() {
        // Regression: task query recovery for #910/#918 started invoking task
        // scans on view load. A daemon-first task scan here stacked 5s status
        // timeouts on Windows for local-accessible paths and froze the app (#919).
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let accept_thread = thread::spawn(move || {
            let _accepted = listener.accept().expect("accept connection");
            thread::sleep(Duration::from_millis(25));
        });
        let daemon = crate::provider::daemon_client::DaemonProvider::connect(&addr.to_string())
            .expect("connect daemon");
        let provider = ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: Some(daemon),
            wsl_distro: Some("Ubuntu".to_string()),
        };

        assert!(daemon_for_task_scan(&provider, r"\\wsl.localhost\Ubuntu\home\me\repo").is_some());
        assert!(daemon_for_task_scan(&provider, "/home/me/repo").is_none());
        assert!(daemon_for_task_scan(&provider, r"C:\Users\me\repo").is_none());

        accept_thread.join().expect("accept thread joined");
    }
}
