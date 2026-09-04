//! Claude Code task parser.
//!
//! Claude Code stores structured task JSON at `~/.claude/tasks/{source-key}/*.json`.
//! Each file contains a single task object with rich metadata including dependencies,
//! owners, and active forms.
//!
//! Discovery uses a unified scan-all approach:
//! 1. Build a source index (session id -> project, team name -> projects)
//! 2. Scan all task directories under `~/.claude/tasks/`
//! 3. Classify each directory via the source index and keep only those that map
//!    to the requested project path.

use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::RuntimeSession;
use crate::task_scanner::claude_index::{
    build_claude_source_index_in, build_claude_source_index_with_live_sessions, ClaudeSourceIndex,
    ClaudeTaskRoot,
};
use crate::task_scanner::types::{ScanOutcome, TaskStatus, UnifiedTask};
use std::fs;
use std::path::Path;

/// Parser slice for harness transcript-backed tasks and compaction signals.
pub trait TranscriptParser: Send + Sync {
    fn tool(&self) -> CliTool;

    fn get_tasks(
        &self,
        project_path: &str,
        sessions: &[&RuntimeSession],
        claude_index: Option<&ClaudeSourceIndex>,
    ) -> ScanOutcome;

    #[cfg(feature = "mesh-bridged-backend")]
    fn parse_compaction_boundary(
        &self,
        _line: &str,
        _jsonl_offset: u64,
    ) -> Option<crate::session_scanner::transcript_boundary::ParsedSignalBoundary> {
        None
    }
}

pub struct ClaudeTranscriptParser;

impl TranscriptParser for ClaudeTranscriptParser {
    fn tool(&self) -> CliTool {
        CliTool::Claude
    }

    fn get_tasks(
        &self,
        project_path: &str,
        sessions: &[&RuntimeSession],
        claude_index: Option<&ClaudeSourceIndex>,
    ) -> ScanOutcome {
        get_tasks_with_index(project_path, sessions, claude_index)
    }
}

/// Maximum file size to parse (1 MB). Skip larger files as a safety measure.
const MAX_FILE_SIZE: u64 = 1_024 * 1_024;

#[derive(Debug, Default)]
struct ClaudeScanOutcome {
    tasks: Vec<UnifiedTask>,
    had_errors: bool,
    first_error: Option<String>,
}

impl ClaudeScanOutcome {
    fn record_error(&mut self, message: String) {
        self.had_errors = true;
        if self.first_error.is_none() {
            self.first_error = Some(message);
        }
    }
}

#[derive(Debug, Default)]
struct DirectoryParseOutcome {
    tasks: Vec<UnifiedTask>,
    had_errors: bool,
    first_error: Option<String>,
}

impl DirectoryParseOutcome {
    fn record_error(&mut self, message: String) {
        self.had_errors = true;
        if self.first_error.is_none() {
            self.first_error = Some(message);
        }
    }
}

/// Raw Claude task JSON shape (matches disk format exactly).
#[derive(serde::Deserialize)]
struct RawClaudeTask {
    id: String,
    subject: String,
    description: Option<String>,
    #[serde(rename = "activeForm")]
    active_form: Option<String>,
    status: String,
    #[serde(default)]
    blocks: Vec<String>,
    #[serde(rename = "blockedBy", default)]
    blocked_by: Vec<String>,
    #[serde(default)]
    owner: Option<String>,
    /// Assignment metadata `mesh task assign` writes; the effort and its reason
    /// live here.
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

/// One trimmed, non-empty string metadata value, under any of `keys`.
fn metadata_string(metadata: Option<&serde_json::Value>, keys: &[&str]) -> Option<String> {
    let metadata = metadata?;
    keys.iter().find_map(|key| {
        metadata
            .get(*key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

fn metadata_u32(metadata: Option<&serde_json::Value>, key: &str) -> Option<u32> {
    let value = metadata?.get(key)?;
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| value.as_str()?.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
}

fn metadata_has_review_ruling(metadata: Option<&serde_json::Value>) -> bool {
    metadata
        .and_then(|metadata| metadata.get("rulings"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|rulings| {
            rulings.iter().any(|ruling| {
                matches!(
                    ruling.get("kind").and_then(serde_json::Value::as_str),
                    Some("verdict" | "score" | "ruling")
                )
            })
        })
}

/// Get tasks for a project from Claude Code's task storage.
pub fn get_tasks(project_path: &str, sessions: &[&RuntimeSession]) -> ScanOutcome {
    get_tasks_with_index(project_path, sessions, None)
}

/// Get tasks for a project with an optional pre-built source index.
pub fn get_tasks_with_index(
    project_path: &str,
    sessions: &[&RuntimeSession],
    prebuilt_index: Option<&ClaudeSourceIndex>,
) -> ScanOutcome {
    let tasks_base = PlatformPaths::claude_dir().join("tasks");
    let projects_base = PlatformPaths::tool_session_root(CliTool::Claude);
    let teams_base = PlatformPaths::teams_dir();

    let live_sessions = sessions
        .iter()
        .map(|session| (*session).clone())
        .collect::<Vec<_>>();
    let built_index;
    let index = match prebuilt_index {
        Some(index) => index,
        None => {
            built_index = build_claude_source_index_with_live_sessions(&live_sessions);
            &built_index
        }
    };
    get_tasks_in_with_index(
        project_path,
        sessions,
        &tasks_base,
        &projects_base,
        &teams_base,
        Some(index),
    )
}

/// Testable version with injectable directories.
pub fn get_tasks_in(
    project_path: &str,
    sessions: &[&RuntimeSession],
    tasks_base: &Path,
    projects_base: &Path,
    teams_base: &Path,
) -> ScanOutcome {
    get_tasks_in_with_index(
        project_path,
        sessions,
        tasks_base,
        projects_base,
        teams_base,
        None,
    )
}

pub fn get_tasks_in_with_index(
    project_path: &str,
    sessions: &[&RuntimeSession],
    tasks_base: &Path,
    projects_base: &Path,
    teams_base: &Path,
    prebuilt_index: Option<&ClaudeSourceIndex>,
) -> ScanOutcome {
    let live_sessions: Vec<RuntimeSession> = sessions.iter().map(|s| (*s).clone()).collect();
    let built_index;
    let index = match prebuilt_index {
        Some(i) => i,
        None => {
            built_index =
                build_claude_source_index_in(&live_sessions, tasks_base, projects_base, teams_base);
            &built_index
        }
    };
    let task_roots = if index.task_roots.is_empty() {
        vec![ClaudeTaskRoot {
            path: tasks_base.to_path_buf(),
            authoritative_teams: index.teams.keys().cloned().collect(),
        }]
    } else {
        index.task_roots.clone()
    };
    if tasks_base.exists() && !tasks_base.is_dir() {
        return ScanOutcome::Unavailable(format!(
            "Claude tasks base is not a directory: {}",
            tasks_base.display()
        ));
    }
    if !task_roots.iter().any(|root| root.path.is_dir()) {
        return ScanOutcome::Unavailable(format!(
            "Claude tasks base does not exist: {}",
            tasks_base.display()
        ));
    }

    let scan = scan_all_task_directories(project_path, &task_roots, index);
    if !scan.tasks.is_empty() {
        return ScanOutcome::Data(scan.tasks);
    }
    if scan.had_errors {
        return ScanOutcome::Unavailable(
            scan.first_error.unwrap_or_else(|| {
                "Claude task scan had degraded I/O or parse failures".to_string()
            }),
        );
    }
    ScanOutcome::DefinitivelyEmpty
}

fn scan_all_task_directories(
    project_path: &str,
    task_roots: &[ClaudeTaskRoot],
    index: &ClaudeSourceIndex,
) -> ClaudeScanOutcome {
    let mut outcome = ClaudeScanOutcome::default();
    let project_key = crate::provider::path::normalize_project_path(project_path);
    for task_root in task_roots {
        if !task_root.path.exists() {
            continue;
        }
        scan_task_root(task_root, &project_key, index, &mut outcome);
    }

    outcome.tasks.sort_by(|a, b| {
        a.session_id
            .cmp(&b.session_id)
            .then_with(|| {
                let a_num: Option<u32> = a.id.parse().ok();
                let b_num: Option<u32> = b.id.parse().ok();
                match (a_num, b_num) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    _ => a.id.cmp(&b.id),
                }
            })
            .then_with(|| a.subject.cmp(&b.subject))
    });
    outcome
}

fn scan_task_root(
    task_root: &ClaudeTaskRoot,
    project_key: &str,
    index: &ClaudeSourceIndex,
    outcome: &mut ClaudeScanOutcome,
) {
    let entries = match fs::read_dir(&task_root.path) {
        Ok(entries) => entries,
        Err(e) => {
            outcome.record_error(format!("Failed to read tasks base: {e}"));
            return;
        }
    };

    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(e) => {
                outcome.record_error(format!("Failed to read tasks base entry: {e}"));
                continue;
            }
        };
        let task_dir = entry.path();
        if !task_dir.is_dir() {
            continue;
        }

        let Some(source_key) = task_dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::trim)
            .filter(|n| !n.is_empty())
        else {
            continue;
        };

        if index.teams.contains_key(source_key)
            && !task_root.authoritative_teams.contains(source_key)
        {
            continue;
        }

        if !source_matches_project(source_key, project_key, index) {
            continue;
        }

        let parsed = parse_task_directory(&task_dir, source_key);
        if parsed.had_errors {
            outcome.record_error(parsed.first_error.unwrap_or_else(|| {
                format!(
                    "Failed to parse one or more task files in {}",
                    task_dir.display()
                )
            }));
        }
        outcome.tasks.extend(parsed.tasks);
    }
}

fn source_matches_project(source_key: &str, project_key: &str, index: &ClaudeSourceIndex) -> bool {
    if let Some(session_project) = index.sessions.get(source_key) {
        return crate::provider::path::normalize_project_path(&session_project.to_string_lossy())
            == project_key;
    }

    if let Some(team_projects) = index.teams.get(source_key) {
        return team_projects.iter().any(|p| {
            crate::provider::path::normalize_project_path(&p.to_string_lossy()) == project_key
        });
    }

    tracing::warn!(
        source_key,
        "Skipping orphan Claude task directory with no source-index mapping"
    );
    false
}

/// Parse all task JSON files in a directory for a specific source key.
fn parse_task_directory(dir: &Path, source_key: &str) -> DirectoryParseOutcome {
    let mut outcome = DirectoryParseOutcome::default();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            outcome.record_error(format!("Failed to read task dir: {e}"));
            return outcome;
        }
    };
    let source_key = Some(source_key.to_string());

    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(e) => {
                outcome.record_error(format!("Failed to read task dir entry: {e}"));
                continue;
            }
        };
        let path = entry.path();

        // Only parse .json files
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        // Skip oversized files
        match fs::metadata(&path) {
            Ok(meta) if meta.len() > MAX_FILE_SIZE => {
                tracing::warn!(path = %path.display(), "Skipping oversized task file (> 1MB)");
                continue;
            }
            Ok(_) => {}
            Err(e) => {
                outcome.record_error(format!(
                    "Failed to read task metadata for {}: {e}",
                    path.display()
                ));
            }
        }

        match parse_task_file(&path, source_key.clone()) {
            Ok(Some(task)) => outcome.tasks.push(task),
            Ok(None) => {} // Deleted task — silently skip
            Err(e) => {
                outcome.record_error(format!("Failed to parse task file {}: {e}", path.display()));
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "Skipping malformed task file"
                );
            }
        }
    }

    // Sort by ID for stable ordering
    outcome.tasks.sort_by(|a, b| {
        let a_num: Option<u32> = a.id.parse().ok();
        let b_num: Option<u32> = b.id.parse().ok();
        match (a_num, b_num) {
            (Some(a), Some(b)) => a.cmp(&b),
            _ => a.id.cmp(&b.id),
        }
    });

    outcome
}

/// Parse a single Claude task JSON file into a UnifiedTask.
///
/// Returns `Ok(None)` for deleted tasks (status: "deleted") so they are
/// silently excluded from the board without logging a warning.
pub fn parse_task_file(
    path: &Path,
    source_key: Option<String>,
) -> Result<Option<UnifiedTask>, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Read error: {e}"))?;
    let raw: RawClaudeTask =
        serde_json::from_str(&content).map_err(|e| format!("Parse error: {e}"))?;

    // Deleted tasks are excluded entirely — they should not appear on the board
    if raw.status == "deleted" {
        return Ok(None);
    }

    let status = match raw.status.as_str() {
        "in_progress" => TaskStatus::InProgress,
        "completed" => TaskStatus::Completed,
        "stale" => TaskStatus::Stale,
        _ => TaskStatus::Pending, // "pending" and anything unknown → Pending
    };

    let task_source_key = source_key.unwrap_or_else(|| "legacy-claude".to_string());
    let effort =
        metadata_string(raw.metadata.as_ref(), &["effort"]).map(|level| level.to_ascii_lowercase());
    let effort_why = metadata_string(raw.metadata.as_ref(), &["effort_why", "effortWhy"]);
    let deadline_minutes = metadata_u32(raw.metadata.as_ref(), "deadline_minutes");
    let has_review_ruling = metadata_has_review_ruling(raw.metadata.as_ref());
    Ok(Some(UnifiedTask {
        id: raw.id,
        source_key: task_source_key.clone(),
        subject: raw.subject,
        description: raw.description,
        active_form: raw.active_form,
        status,
        source: CliTool::Claude,
        blocks: raw.blocks,
        blocked_by: raw.blocked_by,
        owner: raw.owner,
        session_id: Some(task_source_key),
        state_changed_at: None,
        updated_at: None,
        archived_at: None,
        last_status: None,
        archived_reason: None,
        effort,
        effort_why,
        deadline_minutes,
        has_review_ruling,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn write_task(dir: &Path, filename: &str, content: &str) {
        let path = dir.join(filename);
        let mut f = File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.sync_all().unwrap();
    }

    fn parse_task_directory_for_test(dir: &Path) -> Vec<UnifiedTask> {
        let source_key = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("test-source");
        parse_task_directory(dir, source_key).tasks
    }

    #[test]
    fn prebuilt_index_scans_the_task_root_that_owns_a_registered_team() {
        let tmp = TempDir::new().unwrap();
        let default_tasks = tmp.path().join("default/tasks");
        let account_tasks = tmp.path().join("work/tasks");
        let team_tasks = account_tasks.join("work-team");
        fs::create_dir_all(&team_tasks).unwrap();
        write_task(
            &team_tasks,
            "1.json",
            r#"{"id":"1","subject":"Account-root task","status":"pending"}"#,
        );
        let mut index = ClaudeSourceIndex {
            task_roots: vec![
                ClaudeTaskRoot {
                    path: default_tasks.clone(),
                    authoritative_teams: BTreeSet::new(),
                },
                ClaudeTaskRoot {
                    path: account_tasks,
                    authoritative_teams: BTreeSet::from(["work-team".to_string()]),
                },
            ],
            ..Default::default()
        };
        index.teams.insert(
            "work-team".to_string(),
            vec![PathBuf::from("/projects/work")],
        );

        let outcome = get_tasks_in_with_index(
            "/projects/work",
            &[],
            &default_tasks,
            &tmp.path().join("default/projects"),
            &tmp.path().join("default/teams"),
            Some(&index),
        );

        let ScanOutcome::Data(tasks) = outcome else {
            panic!("registered task root should remain available");
        };
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Account-root task");
    }

    #[test]
    fn registered_team_tasks_are_scanned_only_from_the_authoritative_root() {
        // Regression: 21b63ff scanned every account's same-named team task
        // directory after resolving the team itself to just one authoritative root.
        let tmp = TempDir::new().unwrap();
        let default_tasks = tmp.path().join("default/tasks");
        let account_tasks = tmp.path().join("work/tasks");
        for task_root in [&default_tasks, &account_tasks] {
            fs::create_dir_all(task_root.join("work-team")).unwrap();
        }
        write_task(
            &default_tasks.join("work-team"),
            "1.json",
            r#"{"id":"1","subject":"Stale default-root task","status":"pending"}"#,
        );
        write_task(
            &account_tasks.join("work-team"),
            "1.json",
            r#"{"id":"1","subject":"Authoritative work-root task","status":"pending"}"#,
        );

        let mut index = ClaudeSourceIndex {
            task_roots: vec![
                ClaudeTaskRoot {
                    path: default_tasks.clone(),
                    authoritative_teams: BTreeSet::new(),
                },
                ClaudeTaskRoot {
                    path: account_tasks,
                    authoritative_teams: BTreeSet::from(["work-team".to_string()]),
                },
            ],
            ..Default::default()
        };
        index.teams.insert(
            "work-team".to_string(),
            vec![PathBuf::from("/projects/work")],
        );

        let outcome = get_tasks_in_with_index(
            "/projects/work",
            &[],
            &default_tasks,
            &tmp.path().join("default/projects"),
            &tmp.path().join("default/teams"),
            Some(&index),
        );

        let ScanOutcome::Data(tasks) = outcome else {
            panic!("authoritative task root should remain available");
        };
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Authoritative work-root task");
    }

    #[test]
    fn a_task_carries_the_effort_the_lead_assigned_it() {
        // Regression: 7fb03376 modeled deadline_minutes as a JSON number even
        // though mesh 0.2.24 writes assignment metadata values as strings.
        // `mesh task assign` requires an effort and a reason and writes both
        // into the task record's metadata, so the board can show what the lead
        // asked for without reading the assignment notice.
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("architecture-final");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "7.json",
            r#"{
                "id": "7",
                "subject": "Migrate the account store",
                "status": "in_progress",
                "owner": "frontend-dev",
                "metadata": {
                    "effort": "high",
                    "effort_why": "the migration is irreversible",
                    "deadline_minutes": "20",
                    "first_step": "read the migration"
                }
            }"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks[0].effort.as_deref(), Some("high"));
        assert_eq!(
            tasks[0].effort_why.as_deref(),
            Some("the migration is irreversible")
        );
        assert_eq!(tasks[0].deadline_minutes, Some(20));
    }

    #[test]
    fn terminal_task_reports_whether_the_ledger_has_a_review_ruling() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("routing-team");
        fs::create_dir_all(&task_dir).unwrap();
        write_task(
            &task_dir,
            "42.json",
            r#"{
                "id": "42",
                "subject": "Ship telemetry",
                "status": "completed",
                "owner": "builder",
                "metadata": {
                    "rulings": [
                        {"seq": 1, "kind": "verdict", "value": "accepted", "by": "reviewer"}
                    ]
                }
            }"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert!(tasks[0].has_review_ruling);
    }

    #[test]
    fn a_numeric_deadline_remains_tolerated() {
        assert_eq!(
            metadata_u32(
                Some(&serde_json::json!({ "deadline_minutes": 20 })),
                "deadline_minutes"
            ),
            Some(20)
        );
    }

    #[test]
    fn a_task_with_no_assignment_metadata_carries_no_effort() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("architecture-final");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "8.json",
            r#"{"id": "8", "subject": "Unassigned idea", "status": "pending"}"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks[0].effort, None);
        assert_eq!(tasks[0].effort_why, None);
        assert_eq!(tasks[0].deadline_minutes, None);
    }

    #[test]
    fn a_camel_case_reason_reads_the_same_as_the_snake_case_one() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("architecture-final");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "9.json",
            r#"{
                "id": "9",
                "subject": "Tidy the lane",
                "status": "pending",
                "metadata": { "effort": " Medium ", "effortWhy": "routine lane work" }
            }"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks[0].effort.as_deref(), Some("medium"));
        assert_eq!(tasks[0].effort_why.as_deref(), Some("routine lane work"));
    }

    #[test]
    fn parse_well_formed_task() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-123");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{
                "id": "1",
                "subject": "Implement feature X",
                "description": "A longer description",
                "activeForm": "Implementing feature X",
                "status": "in_progress",
                "blocks": ["2"],
                "blockedBy": [],
                "owner": "agent-1"
            }"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "1");
        assert_eq!(tasks[0].subject, "Implement feature X");
        assert_eq!(
            tasks[0].description.as_deref(),
            Some("A longer description")
        );
        assert_eq!(
            tasks[0].active_form.as_deref(),
            Some("Implementing feature X")
        );
        assert_eq!(tasks[0].status, TaskStatus::InProgress);
        assert_eq!(tasks[0].source, CliTool::Claude);
        assert_eq!(tasks[0].blocks, vec!["2"]);
        assert!(tasks[0].blocked_by.is_empty());
        assert_eq!(tasks[0].owner.as_deref(), Some("agent-1"));
        assert_eq!(tasks[0].session_id.as_deref(), Some("session-123"));
    }

    #[test]
    fn session_id_extracted_from_directory_name() {
        let tmp = TempDir::new().unwrap();
        let uuid = "a7a1946e-6c27-468b-a46b-0eb005992454";
        let task_dir = tmp.path().join(uuid);
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Test","status":"pending"}"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks[0].session_id.as_deref(), Some(uuid));
    }

    #[test]
    fn parse_multiple_tasks_sorted_numerically() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-456");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "3.json",
            r#"{"id":"3","subject":"Third","status":"pending"}"#,
        );
        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"First","status":"completed"}"#,
        );
        write_task(
            &task_dir,
            "2.json",
            r#"{"id":"2","subject":"Second","status":"in_progress"}"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id, "1");
        assert_eq!(tasks[1].id, "2");
        assert_eq!(tasks[2].id, "3");
    }

    #[test]
    fn empty_directory_returns_empty_vec() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("empty-session");
        fs::create_dir_all(&task_dir).unwrap();

        let tasks = parse_task_directory_for_test(&task_dir);
        assert!(tasks.is_empty());
    }

    #[test]
    fn malformed_json_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-bad");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(&task_dir, "1.json", "not valid json");
        write_task(
            &task_dir,
            "2.json",
            r#"{"id":"2","subject":"Valid","status":"pending"}"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "2");
    }

    #[test]
    fn status_mapping() {
        // Regression: 1bb8668e made the deadline pass write `stale`, but the
        // Claude task importer decoded that token as `pending` and reopened
        // the assignment on the next scan.
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-status");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Pending","status":"pending"}"#,
        );
        write_task(
            &task_dir,
            "2.json",
            r#"{"id":"2","subject":"In progress","status":"in_progress"}"#,
        );
        write_task(
            &task_dir,
            "3.json",
            r#"{"id":"3","subject":"Completed","status":"completed"}"#,
        );
        write_task(
            &task_dir,
            "4.json",
            r#"{"id":"4","subject":"Stale","status":"stale"}"#,
        );
        write_task(
            &task_dir,
            "5.json",
            r#"{"id":"5","subject":"Unknown","status":"unknown_value"}"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks[0].status, TaskStatus::Pending);
        assert_eq!(tasks[1].status, TaskStatus::InProgress);
        assert_eq!(tasks[2].status, TaskStatus::Completed);
        assert_eq!(tasks[3].status, TaskStatus::Stale);
        assert_eq!(tasks[4].status, TaskStatus::Pending); // unknown → Pending
    }

    #[test]
    fn deleted_tasks_are_excluded() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-deleted");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Active task","status":"in_progress"}"#,
        );
        write_task(
            &task_dir,
            "2.json",
            r#"{"id":"2","subject":"Deleted task","status":"deleted"}"#,
        );
        write_task(
            &task_dir,
            "3.json",
            r#"{"id":"3","subject":"Another active","status":"pending"}"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "1");
        assert_eq!(tasks[1].id, "3");
        // Deleted task #2 should not appear
    }

    #[test]
    fn preserves_dependency_arrays() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-deps");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Task","status":"pending","blocks":["2","3"],"blockedBy":["0"]}"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks[0].blocks, vec!["2", "3"]);
        assert_eq!(tasks[0].blocked_by, vec!["0"]);
    }

    #[test]
    fn skips_non_json_files() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-mixed");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Valid","status":"pending"}"#,
        );
        write_task(&task_dir, "notes.txt", "some notes");
        write_task(&task_dir, "data.jsonl", r#"{"line":1}"#);

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn skips_oversized_files() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-big");
        fs::create_dir_all(&task_dir).unwrap();

        // Create a file > 1MB
        let big_path = task_dir.join("1.json");
        let mut f = File::create(&big_path).unwrap();
        let padding = " ".repeat(1_100_000);
        write!(
            f,
            r#"{{"id":"1","subject":"Big","status":"pending","description":"{padding}"}}"#
        )
        .unwrap();
        f.sync_all().unwrap();

        // Also a normal-sized file
        write_task(
            &task_dir,
            "2.json",
            r#"{"id":"2","subject":"Small","status":"pending"}"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "2");
    }

    #[test]
    fn missing_optional_fields_default() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-minimal");
        fs::create_dir_all(&task_dir).unwrap();

        // Minimal valid task — only required fields
        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Minimal task","status":"pending"}"#,
        );

        let tasks = parse_task_directory_for_test(&task_dir);
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].description.is_none());
        assert!(tasks[0].active_form.is_none());
        assert!(tasks[0].blocks.is_empty());
        assert!(tasks[0].blocked_by.is_empty());
        assert!(tasks[0].owner.is_none());
    }

    fn write_session_jsonl(projects_base: &Path, slug: &str, session_id: &str, cwd: &str) {
        let project_dir = projects_base.join(slug);
        fs::create_dir_all(&project_dir).unwrap();
        let mut f = File::create(project_dir.join(format!("{session_id}.jsonl"))).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","sessionId":"{session_id}","cwd":"{cwd}"}}"#
        )
        .unwrap();
        f.sync_all().unwrap();
    }

    fn write_team_config(teams_base: &Path, team_name: &str, project_path: &str) {
        let team_dir = teams_base.join(team_name);
        fs::create_dir_all(&team_dir).unwrap();
        write_task(
            &team_dir,
            "config.json",
            &format!(
                r#"{{
  "name": "{team_name}",
  "members": [{{"projectPath": "{project_path}"}}]
}}"#
            ),
        );
    }

    fn make_live_session(session_id: &str, project_path: &str) -> RuntimeSession {
        RuntimeSession {
            pid: 1234,
            project_path: project_path.to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "claude".to_string(),
            cli_tool: CliTool::Claude,
            tmux_session: None,
            tmux_window: None,
            tmux_pane: None,
            tmux_window_name: None,
            state: crate::session_scanner::SessionState::Active,
            session_id: Some(session_id.to_string()),
            jsonl_path: None,
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: crate::session_scanner::ActivityConfidence::High,
            activity_attribution: crate::session_scanner::ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: crate::session_scanner::SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
            workflow_activity: None,
        }
    }

    #[test]
    fn unified_scan_finds_tasks_in_session_id_dirs() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        let session_dir = tasks_base.join("live-session");
        fs::create_dir_all(&session_dir).unwrap();
        write_task(
            &session_dir,
            "1.json",
            r#"{"id":"1","subject":"Live session task","status":"in_progress"}"#,
        );

        let live_session = make_live_session("live-session", "/home/user/projects/myapp");

        let tasks = match get_tasks_in(
            "/home/user/projects/myapp",
            &[&live_session],
            &tasks_base,
            &projects_base,
            &teams_base,
        ) {
            ScanOutcome::Data(tasks) => tasks,
            other => panic!("expected task data, got {other:?}"),
        };

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Live session task");
        assert_eq!(tasks[0].session_id.as_deref(), Some("live-session"));
    }

    #[test]
    fn unified_scan_finds_tasks_in_team_name_dirs() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        let team_name = "taurhaus-team";
        let team_dir = tasks_base.join(team_name);
        fs::create_dir_all(&team_dir).unwrap();
        write_task(
            &team_dir,
            "27.json",
            r#"{"id":"27","subject":"Team task","status":"completed"}"#,
        );
        write_team_config(&teams_base, team_name, "/home/user/projects/myapp");

        let tasks = match get_tasks_in(
            "/home/user/projects/myapp",
            &[],
            &tasks_base,
            &projects_base,
            &teams_base,
        ) {
            ScanOutcome::Data(tasks) => tasks,
            other => panic!("expected task data, got {other:?}"),
        };

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "27");
        assert_eq!(tasks[0].subject, "Team task");
        assert_eq!(tasks[0].session_id.as_deref(), Some(team_name));
    }

    #[test]
    fn orphan_and_empty_dirs_are_skipped() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        // Orphan: has task json but no session/team mapping.
        let orphan_dir = tasks_base.join("orphan-source");
        fs::create_dir_all(&orphan_dir).unwrap();
        write_task(
            &orphan_dir,
            "1.json",
            r#"{"id":"1","subject":"Orphan","status":"pending"}"#,
        );

        // Empty mapped team dir: no .json task files.
        let empty_team_name = "empty-team";
        let empty_team_dir = tasks_base.join(empty_team_name);
        fs::create_dir_all(&empty_team_dir).unwrap();
        write_team_config(&teams_base, empty_team_name, "/home/user/projects/myapp");

        let outcome = get_tasks_in(
            "/home/user/projects/myapp",
            &[],
            &tasks_base,
            &projects_base,
            &teams_base,
        );

        assert_eq!(outcome, ScanOutcome::DefinitivelyEmpty);
    }

    #[test]
    fn malformed_task_files_without_survivors_are_unavailable() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        let session_dir = tasks_base.join("broken-session");
        fs::create_dir_all(&session_dir).unwrap();
        write_task(&session_dir, "1.json", "not valid json");

        let live_session = make_live_session("broken-session", "/home/user/projects/myapp");
        let outcome = get_tasks_in(
            "/home/user/projects/myapp",
            &[&live_session],
            &tasks_base,
            &projects_base,
            &teams_base,
        );
        assert!(matches!(outcome, ScanOutcome::Unavailable(_)));
    }

    #[test]
    fn default_tasks_base_file_reports_not_a_directory() {
        // Regression: 4ca848a checked whether any task root was a directory
        // first, making the historical default-root file diagnostic unreachable.
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::write(&tasks_base, "not a directory").unwrap();

        let outcome = get_tasks_in(
            "/home/user/projects/myapp",
            &[],
            &tasks_base,
            &projects_base,
            &teams_base,
        );

        assert_eq!(
            outcome,
            ScanOutcome::Unavailable(format!(
                "Claude tasks base is not a directory: {}",
                tasks_base.display()
            ))
        );
    }

    #[test]
    fn malformed_task_files_with_survivors_return_data() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        let session_dir = tasks_base.join("partial-session");
        fs::create_dir_all(&session_dir).unwrap();
        write_task(&session_dir, "1.json", "not valid json");
        write_task(
            &session_dir,
            "2.json",
            r#"{"id":"2","subject":"Still parseable","status":"pending"}"#,
        );

        let live_session = make_live_session("partial-session", "/home/user/projects/myapp");
        let tasks = match get_tasks_in(
            "/home/user/projects/myapp",
            &[&live_session],
            &tasks_base,
            &projects_base,
            &teams_base,
        ) {
            ScanOutcome::Data(tasks) => tasks,
            other => panic!("expected partial data, got {other:?}"),
        };
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "2");
    }

    #[test]
    fn unreadable_json_entry_without_survivors_is_unavailable() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        let session_dir = tasks_base.join("io-session");
        fs::create_dir_all(&session_dir).unwrap();
        fs::create_dir_all(session_dir.join("1.json")).unwrap();

        let live_session = make_live_session("io-session", "/home/user/projects/myapp");
        let outcome = get_tasks_in(
            "/home/user/projects/myapp",
            &[&live_session],
            &tasks_base,
            &projects_base,
            &teams_base,
        );
        assert!(matches!(outcome, ScanOutcome::Unavailable(_)));
    }

    #[test]
    fn unified_scan_associates_tasks_with_projects_via_index() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        // Session-mapped task for project A.
        let session_id = "sess-a";
        let sess_dir = tasks_base.join(session_id);
        fs::create_dir_all(&sess_dir).unwrap();
        write_task(
            &sess_dir,
            "1.json",
            r#"{"id":"1","subject":"Session A task","status":"pending"}"#,
        );
        write_session_jsonl(
            &projects_base,
            "-home-user-projects-a",
            session_id,
            "/home/user/projects/a",
        );

        // Team-mapped task for project B.
        let team_name = "team-b";
        let team_dir = tasks_base.join(team_name);
        fs::create_dir_all(&team_dir).unwrap();
        write_task(
            &team_dir,
            "2.json",
            r#"{"id":"2","subject":"Team B task","status":"completed"}"#,
        );
        write_team_config(&teams_base, team_name, "/home/user/projects/b");

        let a_tasks = match get_tasks_in(
            "/home/user/projects/a",
            &[],
            &tasks_base,
            &projects_base,
            &teams_base,
        ) {
            ScanOutcome::Data(tasks) => tasks,
            other => panic!("expected task data, got {other:?}"),
        };
        assert_eq!(a_tasks.len(), 1);
        assert_eq!(a_tasks[0].subject, "Session A task");
        assert_eq!(a_tasks[0].session_id.as_deref(), Some(session_id));

        let b_tasks = match get_tasks_in(
            "/home/user/projects/b",
            &[],
            &tasks_base,
            &projects_base,
            &teams_base,
        ) {
            ScanOutcome::Data(tasks) => tasks,
            other => panic!("expected task data, got {other:?}"),
        };
        assert_eq!(b_tasks.len(), 1);
        assert_eq!(b_tasks[0].subject, "Team B task");
        assert_eq!(b_tasks[0].session_id.as_deref(), Some(team_name));
    }
}
