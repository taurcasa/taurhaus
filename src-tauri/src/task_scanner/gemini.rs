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
use crate::task_scanner::types::{TaskStatus, UnifiedTask};
use std::fs;
use std::path::Path;

/// Maximum file size to parse (1 MB). Skip larger files as a safety measure.
const MAX_FILE_SIZE: u64 = 1_024 * 1_024;

/// Get tasks from a project's TODO.md file.
///
/// Returns an empty vec (not an error) if the file doesn't exist.
pub fn get_tasks(project_path: &str) -> Result<Vec<UnifiedTask>, String> {
    let todo_path = Path::new(project_path).join("TODO.md");
    get_tasks_from(&todo_path)
}

/// Testable version that accepts a direct file path.
pub fn get_tasks_from(todo_path: &Path) -> Result<Vec<UnifiedTask>, String> {
    if !todo_path.exists() {
        return Ok(vec![]);
    }

    // Check file size before reading
    let meta = fs::metadata(todo_path).map_err(|e| format!("Failed to stat TODO.md: {e}"))?;
    if meta.len() > MAX_FILE_SIZE {
        return Err("TODO.md exceeds 1MB size limit".to_string());
    }

    let content =
        fs::read_to_string(todo_path).map_err(|e| format!("Failed to read TODO.md: {e}"))?;

    Ok(parse_checkboxes(&content))
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
    content
        .lines()
        .enumerate()
        .filter_map(|(line_num, line)| parse_checkbox_line(line, line_num))
        .collect()
}

/// Try to parse a single line as a markdown checkbox.
///
/// Expected format: optional whitespace, then `- [`, then ` `, `x`, or `X`,
/// then `] `, then the task text.
fn parse_checkbox_line(line: &str, line_num: usize) -> Option<UnifiedTask> {
    let trimmed = line.trim_start();

    // Must start with "- ["
    let rest = trimmed.strip_prefix("- [")?;

    // Next char determines status
    let (status, rest) = if let Some(rest) = rest.strip_prefix(' ') {
        (TaskStatus::Pending, rest)
    } else if let Some(rest) = rest.strip_prefix('x') {
        (TaskStatus::Completed, rest)
    } else if let Some(rest) = rest.strip_prefix('X') {
        (TaskStatus::Completed, rest)
    } else {
        return None;
    };

    // Must be followed by "] "
    let rest = rest.strip_prefix("] ")?;

    let subject = rest.trim();
    if subject.is_empty() {
        return None;
    }

    Some(UnifiedTask {
        id: format!("todo-{line_num}"),
        subject: subject.to_string(),
        description: None,
        active_form: None,
        status,
        source: CliTool::Gemini,
        blocks: vec![],
        blocked_by: vec![],
        owner: None,
    })
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
        let tasks = get_tasks_from(&path).unwrap();
        assert!(tasks.is_empty());
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

        let tasks = get_tasks_from(&path).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].subject, "Write unit tests");
        assert_eq!(tasks[0].status, TaskStatus::Pending);
        assert_eq!(tasks[1].subject, "Set up project");
        assert_eq!(tasks[1].status, TaskStatus::Completed);
    }

    #[test]
    fn invalid_checkbox_formats_ignored() {
        let content = "- [] Missing space\n- [y] Wrong letter\n- [x]No space after bracket\n- [ ] Valid\n";
        let tasks = parse_checkboxes(content);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Valid");
    }

    #[test]
    fn task_text_is_trimmed() {
        let tasks = parse_checkboxes("- [ ] Task with trailing spaces   \n");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Task with trailing spaces");
    }
}
