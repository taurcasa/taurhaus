//! Resolve project paths to their Claude Code data directories.
//!
//! Claude Code stores per-project data under `~/.claude/projects/<slug>/`
//! where `<slug>` is the project's absolute path with `/` replaced by `-`.
//! Session JSONL files live at `~/.claude/projects/<slug>/<session-uuid>.jsonl`.

use chrono::{DateTime, Utc};
use std::fs::File;
use std::io::{BufRead, BufReader, Read as _, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Compute the Claude Code project slug from an absolute path.
///
/// Claude Code uses the absolute path with all path separators replaced
/// by dashes. For example: `/home/user/projects/foo` → `-home-user-projects-foo`.
pub fn project_slug(path: &Path) -> String {
    let canonical = path.to_string_lossy();
    canonical.replace(['/', '\\'], "-")
}

/// Return the Claude Code base directory (`~/.claude/`).
///
/// Uses `$HOME` on Linux/macOS, `$USERPROFILE` on Windows.
pub fn claude_base_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Resolve the Claude Code data directory for a project.
///
/// Returns `Some(path)` if the directory exists, `None` otherwise.
pub fn resolve_project_dir(project_path: &Path) -> Option<PathBuf> {
    let base = claude_base_dir()?;
    let slug = project_slug(project_path);
    let project_dir = base.join("projects").join(slug);
    if project_dir.is_dir() {
        Some(project_dir)
    } else {
        None
    }
}

/// Check whether Claude Code data exists for a given project path.
pub fn has_claude_data(project_path: &Path) -> bool {
    resolve_project_dir(project_path).is_some()
}

/// Return the memory directory for a project, if it exists.
pub fn memory_dir(project_path: &Path) -> Option<PathBuf> {
    let project_dir = resolve_project_dir(project_path)?;
    let mem_dir = project_dir.join("memory");
    if mem_dir.is_dir() {
        Some(mem_dir)
    } else {
        None
    }
}

/// Maximum bytes to read from the end of a JSONL file to find the last line.
const TAIL_READ_SIZE: u64 = 8 * 1024;

/// Extract the start and end timestamps from a session's JSONL file.
///
/// The JSONL file is at `~/.claude/projects/<slug>/<session_id>.jsonl`.
/// - **Start**: First line is `file-history-snapshot` with `snapshot.timestamp`.
/// - **End**: Last non-empty line has a top-level `timestamp` field.
///
/// Returns `None` if the file doesn't exist, is empty, or timestamps can't be parsed.
pub fn session_time_range(
    project_path: &Path,
    session_id: &str,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let project_dir = resolve_project_dir(project_path)?;
    let jsonl_path = project_dir.join(format!("{session_id}.jsonl"));
    session_time_range_from_file(&jsonl_path)
}

/// Extract start/end timestamps from a specific JSONL file path.
///
/// Separated from `session_time_range` for testability with temp files.
pub fn session_time_range_from_file(jsonl_path: &Path) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let file = File::open(jsonl_path).ok()?;
    let file_len = file.metadata().ok()?.len();
    if file_len == 0 {
        return None;
    }

    // Read first line for start timestamp
    let mut reader = BufReader::new(&file);
    let mut first_line = String::new();
    reader.read_line(&mut first_line).ok()?;
    let start = parse_start_timestamp(&first_line)?;

    // Read last non-empty line for end timestamp.
    // Seek near the end to avoid reading the entire (potentially huge) file.
    let last_line = read_last_line(&file, file_len)?;
    let end = parse_end_timestamp(&last_line)?;

    // Sanity: end should be >= start
    if end >= start {
        Some((start, end))
    } else {
        Some((start, start))
    }
}

/// Parse start timestamp from the first JSONL line (`file-history-snapshot`).
///
/// Shape: `{"type":"file-history-snapshot","snapshot":{"timestamp":"2026-02-22T03:59:01.775Z",...},...}`
fn parse_start_timestamp(line: &str) -> Option<DateTime<Utc>> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let ts_str = v.get("snapshot")?.get("timestamp")?.as_str()?;
    ts_str.parse::<DateTime<Utc>>().ok()
}

/// Parse end timestamp from a non-snapshot JSONL line.
///
/// Shape: `{"type":"user"|"assistant"|"progress",...,"timestamp":"2026-02-22T04:05:00.000Z"}`
fn parse_end_timestamp(line: &str) -> Option<DateTime<Utc>> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let ts_str = v.get("timestamp")?.as_str()?;
    ts_str.parse::<DateTime<Utc>>().ok()
}

/// Read the last non-empty line from a file by seeking near the end.
fn read_last_line(file: &File, file_len: u64) -> Option<String> {
    let mut file = file;

    // If the file is small enough, just read all lines
    if file_len <= TAIL_READ_SIZE {
        file.seek(SeekFrom::Start(0)).ok()?;
        let mut content = String::new();
        file.read_to_string(&mut content).ok()?;
        return content
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .map(|s| s.to_string());
    }

    // Seek near the end and read the tail
    let seek_pos = file_len.saturating_sub(TAIL_READ_SIZE);
    file.seek(SeekFrom::Start(seek_pos)).ok()?;
    let mut tail = String::new();
    file.read_to_string(&mut tail).ok()?;

    tail.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn slug_linux_path() {
        let path = Path::new("/home/user/projects/taurhaus");
        assert_eq!(project_slug(path), "-home-user-projects-taurhaus");
    }

    #[test]
    fn slug_windows_path() {
        let path = Path::new("C:\\Users\\dev\\projects\\foo");
        let slug = project_slug(path);
        assert!(slug.contains("-Users-dev-projects-foo"));
    }

    #[test]
    fn slug_single_component() {
        let path = Path::new("/tmp");
        assert_eq!(project_slug(path), "-tmp");
    }

    #[test]
    fn slug_deterministic() {
        let path = Path::new("/home/mstie/projects/taurhaus");
        let slug1 = project_slug(path);
        let slug2 = project_slug(path);
        assert_eq!(slug1, slug2);
    }

    #[test]
    fn resolve_nonexistent_returns_none() {
        let path = Path::new("/nonexistent/path/that/does/not/exist");
        assert!(resolve_project_dir(path).is_none());
    }

    #[test]
    fn has_claude_data_false_for_nonexistent() {
        let path = Path::new("/nonexistent/path");
        assert!(!has_claude_data(path));
    }

    #[test]
    fn resolve_with_mock_structure() {
        let dir = TempDir::new().unwrap();
        let fake_home = dir.path();

        // Create a mock .claude/projects/<slug>/ structure
        let slug = "-mock-project";
        let project_dir = fake_home.join(".claude").join("projects").join(slug);
        std::fs::create_dir_all(&project_dir).unwrap();

        // Since resolve_project_dir uses the real home dir, we test the slug
        // computation separately and verify the path construction logic
        let expected_slug = project_slug(Path::new("/mock/project"));
        assert_eq!(expected_slug, "-mock-project");
    }

    #[test]
    fn memory_dir_none_when_no_project() {
        let path = Path::new("/nonexistent/path");
        assert!(memory_dir(path).is_none());
    }

    #[test]
    fn session_time_range_from_jsonl() {
        let dir = TempDir::new().unwrap();
        let jsonl_path = dir.path().join("test-session.jsonl");
        let content = concat!(
            r#"{"type":"file-history-snapshot","snapshot":{"timestamp":"2026-02-22T03:59:01.775Z"}}"#,
            "\n",
            r#"{"type":"user","timestamp":"2026-02-22T04:00:00.000Z"}"#,
            "\n",
            r#"{"type":"assistant","timestamp":"2026-02-22T04:05:30.500Z"}"#,
            "\n",
        );
        std::fs::write(&jsonl_path, content).unwrap();

        let (start, end) = session_time_range_from_file(&jsonl_path).unwrap();
        assert_eq!(start.to_rfc3339_opts(chrono::SecondsFormat::Millis, true), "2026-02-22T03:59:01.775Z");
        assert_eq!(end.to_rfc3339_opts(chrono::SecondsFormat::Millis, true), "2026-02-22T04:05:30.500Z");
    }

    #[test]
    fn session_time_range_empty_file() {
        let dir = TempDir::new().unwrap();
        let jsonl_path = dir.path().join("empty.jsonl");
        std::fs::write(&jsonl_path, "").unwrap();

        assert!(session_time_range_from_file(&jsonl_path).is_none());
    }

    #[test]
    fn session_time_range_missing_file() {
        let dir = TempDir::new().unwrap();
        let jsonl_path = dir.path().join("nonexistent.jsonl");
        assert!(session_time_range_from_file(&jsonl_path).is_none());
    }

    #[test]
    fn session_time_range_single_line() {
        let dir = TempDir::new().unwrap();
        let jsonl_path = dir.path().join("single.jsonl");
        // Only the snapshot line — no end timestamp available
        let content = r#"{"type":"file-history-snapshot","snapshot":{"timestamp":"2026-02-22T03:59:01.775Z"}}"#;
        std::fs::write(&jsonl_path, content).unwrap();

        // The last line IS the snapshot line, which has no top-level timestamp
        assert!(session_time_range_from_file(&jsonl_path).is_none());
    }

    #[test]
    fn session_time_range_large_file_reads_tail() {
        let dir = TempDir::new().unwrap();
        let jsonl_path = dir.path().join("large.jsonl");

        let first_line = r#"{"type":"file-history-snapshot","snapshot":{"timestamp":"2026-01-01T00:00:00.000Z"}}"#;
        // Create a large file (> TAIL_READ_SIZE) with padding lines
        let mut content = String::from(first_line);
        content.push('\n');
        // Each padding line ~200 bytes, need > 8KB total
        for i in 0..100 {
            content.push_str(&format!(
                r#"{{"type":"assistant","timestamp":"2026-01-01T00:{:02}:00.000Z","data":"{}"}}"#,
                i % 60,
                "x".repeat(100)
            ));
            content.push('\n');
        }
        // Final line with the real end timestamp
        content.push_str(r#"{"type":"user","timestamp":"2026-01-01T23:59:59.000Z"}"#);
        content.push('\n');

        std::fs::write(&jsonl_path, &content).unwrap();

        let (start, end) = session_time_range_from_file(&jsonl_path).unwrap();
        assert_eq!(start.to_rfc3339_opts(chrono::SecondsFormat::Millis, true), "2026-01-01T00:00:00.000Z");
        assert_eq!(end.to_rfc3339_opts(chrono::SecondsFormat::Millis, true), "2026-01-01T23:59:59.000Z");
    }

    // Integration test: verify against real Claude Code data if available
    #[test]
    fn resolve_real_project_if_available() {
        let taurhaus_path = Path::new("/home/mstie/projects/taurhaus");
        if taurhaus_path.exists() {
            let slug = project_slug(taurhaus_path);
            assert_eq!(slug, "-home-mstie-projects-taurhaus");

            // Only check resolution if we're on the right machine
            if let Some(project_dir) = resolve_project_dir(taurhaus_path) {
                assert!(project_dir.exists());
                assert!(project_dir.ends_with("-home-mstie-projects-taurhaus"));
            }
        }
    }
}
