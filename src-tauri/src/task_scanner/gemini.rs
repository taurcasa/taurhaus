//! Gemini CLI task parser.
//!
//! Gemini tracks tasks via `TODO.md` files in the project directory.
//! These are standard markdown checkbox lists:
//! - `- [ ] Task description` → Pending
//! - `- [x] Task description` → Completed
//! - `- [X] Task description` → Completed
//!
//! No session data is needed — we just read the file from the project root.

use crate::session_scanner::cli_tool::CliTool;
use crate::task_scanner::types::{ScanOutcome, TaskStatus, UnifiedTask};
use chrono::{DateTime, TimeZone, Utc};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum file size to parse (1 MB). Skip larger files as a safety measure.
const MAX_FILE_SIZE: u64 = 1_024 * 1_024;
const GEMINI_SOURCE_KEY: &str = "gemini-todo";

#[derive(Debug, Default)]
struct CheckboxParseOutcome {
    tasks: Vec<UnifiedTask>,
    had_errors: bool,
    first_error: Option<String>,
}

impl CheckboxParseOutcome {
    fn record_error(&mut self, message: String) {
        self.had_errors = true;
        if self.first_error.is_none() {
            self.first_error = Some(message);
        }
    }
}

/// Get tasks from a project's TODO.md file.
///
/// Returns an empty vec (not an error) if the file doesn't exist.
pub fn get_tasks(project_path: &str) -> ScanOutcome {
    let todo_path = Path::new(project_path).join("TODO.md");
    let source_key = derive_source_key(project_path);
    get_tasks_from_with_source_key(&todo_path, &source_key)
}

/// Testable version that accepts a direct file path.
pub fn get_tasks_from(todo_path: &Path) -> ScanOutcome {
    get_tasks_from_with_source_key(todo_path, GEMINI_SOURCE_KEY)
}

fn get_tasks_from_with_source_key(todo_path: &Path, source_key: &str) -> ScanOutcome {
    if !todo_path.exists() {
        return ScanOutcome::DefinitivelyEmpty;
    }

    // Check file size before reading
    let meta = match fs::metadata(todo_path) {
        Ok(meta) => meta,
        Err(e) => return ScanOutcome::Unavailable(format!("Failed to stat TODO.md: {e}")),
    };
    if meta.len() > MAX_FILE_SIZE {
        return ScanOutcome::Unavailable("TODO.md exceeds 1MB size limit".to_string());
    }

    let content = match fs::read_to_string(todo_path) {
        Ok(content) => content,
        Err(e) => return ScanOutcome::Unavailable(format!("Failed to read TODO.md: {e}")),
    };

    let parsed = parse_checkboxes_with_source_key_diagnostics(&content, source_key);
    if !parsed.tasks.is_empty() {
        return ScanOutcome::Data(parsed.tasks);
    }
    if parsed.had_errors {
        return ScanOutcome::Unavailable(
            parsed
                .first_error
                .unwrap_or_else(|| "Malformed checkbox entries in TODO.md".to_string()),
        );
    }
    ScanOutcome::DefinitivelyEmpty
}

/// Parse markdown checkbox lines into UnifiedTasks.
///
/// Matches lines like:
/// - `- [ ] Task description`
/// - `- [x] Completed task`
/// - `  - [X] Indented task` (with leading whitespace)
///
/// Non-checkbox lines are ignored. Line numbers are used for synthetic IDs.
pub fn parse_checkboxes(content: &str) -> Vec<UnifiedTask> {
    parse_checkboxes_with_source_key(content, GEMINI_SOURCE_KEY)
}

fn parse_checkboxes_with_source_key(content: &str, source_key: &str) -> Vec<UnifiedTask> {
    parse_checkboxes_with_source_key_diagnostics(content, source_key).tasks
}

fn parse_checkboxes_with_source_key_diagnostics(
    content: &str,
    source_key: &str,
) -> CheckboxParseOutcome {
    let mut outcome = CheckboxParseOutcome::default();
    for (line_num, line) in content.lines().enumerate() {
        match parse_checkbox_line(line, line_num, source_key) {
            Ok(Some(task)) => outcome.tasks.push(task),
            Ok(None) => {}
            Err(e) => outcome.record_error(format!("Line {}: {e}", line_num + 1)),
        }
    }
    outcome
}

/// Try to parse a single line as a markdown checkbox.
///
/// Expected format: optional whitespace, then `- [`, then ` `, `x`, or `X`,
/// then `] `, then the task text.
fn parse_checkbox_line(
    line: &str,
    line_num: usize,
    source_key: &str,
) -> Result<Option<UnifiedTask>, String> {
    let trimmed = line.trim_start();

    // Must start with "- ["
    let Some(rest) = trimmed.strip_prefix("- [") else {
        return Ok(None);
    };

    // Next char determines status
    let (status, rest) = if let Some(rest) = rest.strip_prefix(' ') {
        (TaskStatus::Pending, rest)
    } else if let Some(rest) = rest.strip_prefix('x') {
        (TaskStatus::Completed, rest)
    } else if let Some(rest) = rest.strip_prefix('X') {
        (TaskStatus::Completed, rest)
    } else {
        return Err("Invalid checkbox marker (expected ' ', 'x', or 'X')".to_string());
    };

    // Must be followed by "] "
    let Some(rest) = rest.strip_prefix("] ") else {
        return Err("Missing closing '] ' after checkbox marker".to_string());
    };

    let subject = rest.trim();
    if subject.is_empty() {
        return Err("Checkbox task text is empty".to_string());
    }

    Ok(Some(UnifiedTask {
        id: format!("todo-{line_num}"),
        source_key: source_key.to_string(),
        subject: subject.to_string(),
        description: None,
        active_form: None,
        status,
        source: CliTool::Gemini,
        blocks: vec![],
        blocked_by: vec![],
        owner: None,
        session_id: Some(source_key.to_string()),
        state_changed_at: None,
        updated_at: None,
        archived_at: None,
        last_status: None,
        archived_reason: None,
    }))
}

/// Resolve Gemini transcript time range for a session identity.
pub fn session_time_range(
    project_path: &Path,
    session_id: &str,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let session_file = find_gemini_session_file(project_path, Some(session_id))?;
    gemini_time_range_from_file(&session_file)
}

fn derive_source_key(project_path: &str) -> String {
    let project = Path::new(project_path);
    let Some(file) = find_gemini_session_file(project, None) else {
        return GEMINI_SOURCE_KEY.to_string();
    };
    gemini_session_id_from_file(&file)
        .or_else(|| {
            file.file_stem()
                .and_then(|s| s.to_str())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| GEMINI_SOURCE_KEY.to_string())
}

fn find_gemini_session_file(project_path: &Path, session_id: Option<&str>) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let base_dir = home.join(".gemini").join("tmp");

    let project_str = project_path.to_string_lossy();
    let dir_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let hash = hex::encode(Sha256::digest(project_str.as_bytes()));

    let chats_dir_by_name = base_dir.join(dir_name).join("chats");
    let chats_dir_by_hash = base_dir.join(hash).join("chats");

    let primary = if chats_dir_by_name.is_dir() {
        chats_dir_by_name
    } else {
        chats_dir_by_hash
    };

    find_in_chats_dir(&primary, session_id)
}

fn find_in_chats_dir(chats_dir: &Path, session_id: Option<&str>) -> Option<PathBuf> {
    if !chats_dir.is_dir() {
        return None;
    }

    let mut entries: Vec<_> = fs::read_dir(chats_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();

    entries.sort_by(|a, b| {
        let mt_a = a
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let mt_b = b
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        mt_b.cmp(&mt_a)
    });

    if let Some(target) = session_id {
        for entry in entries {
            let path = entry.path();
            if gemini_session_id_from_file(&path).as_deref() == Some(target) {
                return Some(path);
            }
            if path.file_stem().and_then(|s| s.to_str()) == Some(target) {
                return Some(path);
            }
        }
        return None;
    }

    entries.into_iter().next().map(|entry| entry.path())
}

fn gemini_session_id_from_file(path: &Path) -> Option<String> {
    // Filename form: session-2026-02-23T22-17-80291013.json -> "80291013"
    let from_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.rsplit('-').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if from_stem.is_some() {
        return from_stem;
    }

    let raw = fs::read_to_string(path).ok()?;
    let parsed = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    parsed
        .get("sessionId")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn gemini_time_range_from_file(path: &Path) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let mut timestamps = Vec::new();
    if let Ok(raw) = fs::read_to_string(path) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
            collect_timestamp_fields(&parsed, &mut timestamps);
        }
    }
    timestamps.sort();
    timestamps.dedup();

    let start_from_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(parse_start_from_filename);
    let mtime = fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(DateTime::<Utc>::from);

    let start = timestamps.first().cloned().or(start_from_name).or(mtime)?;
    let mut end = timestamps.last().cloned().or(mtime).unwrap_or(start);
    if end < start {
        end = start;
    }
    Some((start, end))
}

fn collect_timestamp_fields(value: &serde_json::Value, out: &mut Vec<DateTime<Utc>>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(ts) = map.get("timestamp").and_then(|v| v.as_str()) {
                if let Ok(parsed) = ts.parse::<DateTime<Utc>>() {
                    out.push(parsed);
                }
            }
            for nested in map.values() {
                collect_timestamp_fields(nested, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for nested in arr {
                collect_timestamp_fields(nested, out);
            }
        }
        _ => {}
    }
}

fn parse_start_from_filename(stem: &str) -> Option<DateTime<Utc>> {
    let prefix = stem.rsplit_once('-')?.0;
    let ts = prefix.strip_prefix("session-")?;
    let naive = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H-%M").ok()?;
    Some(Utc.from_utc_datetime(&naive))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn standard_checkboxes() {
        let tasks = parse_checkboxes("- [ ] Pending task\n- [x] Done task\n");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "todo-0");
        assert_eq!(tasks[0].source_key, GEMINI_SOURCE_KEY);
        assert_eq!(tasks[0].session_id.as_deref(), Some(GEMINI_SOURCE_KEY));
        assert_eq!(tasks[0].subject, "Pending task");
        assert_eq!(tasks[0].status, TaskStatus::Pending);
        assert_eq!(tasks[0].source, CliTool::Gemini);
        assert_eq!(tasks[1].id, "todo-1");
        assert_eq!(tasks[1].subject, "Done task");
        assert_eq!(tasks[1].status, TaskStatus::Completed);
    }

    #[test]
    fn uppercase_x() {
        let tasks = parse_checkboxes("- [X] Done with uppercase X\n");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Completed);
    }

    #[test]
    fn mixed_with_non_checkbox_lines() {
        let content = "# TODO\n\nSome description.\n\n- [ ] First task\n- Regular list item\n- [x] Second task\n\nEnd of file.\n";
        let tasks = parse_checkboxes(content);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].subject, "First task");
        assert_eq!(tasks[1].subject, "Second task");
    }

    #[test]
    fn indented_checkboxes() {
        let content = "  - [ ] Indented task\n    - [x] Deeply indented\n";
        let tasks = parse_checkboxes(content);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].subject, "Indented task");
        assert_eq!(tasks[1].subject, "Deeply indented");
    }

    #[test]
    fn empty_content() {
        let tasks = parse_checkboxes("");
        assert!(tasks.is_empty());
    }

    #[test]
    fn no_checkboxes() {
        let content = "# Just a header\n\nSome text.\n- Normal list\n";
        let tasks = parse_checkboxes(content);
        assert!(tasks.is_empty());
    }

    #[test]
    fn empty_checkbox_text_skipped() {
        let content = "- [ ] \n- [ ]  \n- [ ] Valid task\n";
        let tasks = parse_checkboxes(content);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Valid task");
    }

    #[test]
    fn line_numbers_used_for_ids() {
        let content = "# Header\n\n- [ ] Task A\n\n- [x] Task B\n";
        let tasks = parse_checkboxes(content);
        assert_eq!(tasks.len(), 2);
        // Line 0 = "# Header", Line 1 = "", Line 2 = "- [ ] Task A", etc.
        assert_eq!(tasks[0].id, "todo-2");
        assert_eq!(tasks[1].id, "todo-4");
    }

    #[test]
    fn missing_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("TODO.md");
        let outcome = get_tasks_from(&path);
        assert_eq!(outcome, ScanOutcome::DefinitivelyEmpty);
    }

    #[test]
    fn reads_real_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("TODO.md");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "# Tasks").unwrap();
        writeln!(f, "- [ ] Write unit tests").unwrap();
        writeln!(f, "- [x] Set up project").unwrap();
        f.sync_all().unwrap();

        let tasks = match get_tasks_from(&path) {
            ScanOutcome::Data(tasks) => tasks,
            other => panic!("expected task data, got {other:?}"),
        };
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].subject, "Write unit tests");
        assert_eq!(tasks[0].status, TaskStatus::Pending);
        assert_eq!(tasks[1].subject, "Set up project");
        assert_eq!(tasks[1].status, TaskStatus::Completed);
    }

    #[test]
    fn invalid_checkbox_formats_ignored() {
        let content =
            "- [] Missing space\n- [y] Wrong letter\n- [x]No space after bracket\n- [ ] Valid\n";
        let tasks = parse_checkboxes(content);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Valid");
    }

    #[test]
    fn malformed_checkbox_without_survivors_is_unavailable() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("TODO.md");
        std::fs::write(&path, "- [y] Broken checkbox\n").unwrap();

        let outcome = get_tasks_from(&path);
        assert!(matches!(outcome, ScanOutcome::Unavailable(_)));
    }

    #[test]
    fn malformed_checkbox_with_survivors_returns_data() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("TODO.md");
        std::fs::write(&path, "- [y] Broken checkbox\n- [ ] Valid task\n").unwrap();

        let tasks = match get_tasks_from(&path) {
            ScanOutcome::Data(tasks) => tasks,
            other => panic!("expected partial data, got {other:?}"),
        };
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Valid task");
    }

    #[test]
    fn task_text_is_trimmed() {
        let tasks = parse_checkboxes("- [ ] Task with trailing spaces   \n");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Task with trailing spaces");
    }

    #[test]
    fn gemini_session_identity_from_chat_filename() {
        let tmp = TempDir::new().unwrap();
        let chats = tmp.path().join("chats");
        std::fs::create_dir_all(&chats).unwrap();
        let session_file = chats.join("session-2026-02-23T12-00-deadbeef.json");
        std::fs::write(&session_file, "{}").unwrap();

        let discovered = find_in_chats_dir(&chats, None).unwrap();
        assert_eq!(discovered, session_file);
        assert_eq!(
            gemini_session_id_from_file(&session_file).as_deref(),
            Some("deadbeef")
        );
    }
}
