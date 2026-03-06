use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::AppError;

/// Parsed result from a session handoff markdown file.
#[derive(Debug, Clone)]
pub struct ParsedSession {
    pub date: String,
    pub project: Option<String>,
    pub session_id: Option<String>,
    pub summary: String,
    pub next_steps: Vec<String>,
    pub open_questions: Vec<String>,
    pub metadata: serde_json::Value,
    pub body: String,
}

/// Parsed result from a companion .meta.json sidecar file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_minutes: Option<u64>,
    pub exit_reason: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub tools_used: serde_json::Value,
    #[serde(default)]
    pub files_modified: Vec<String>,
    #[serde(default)]
    pub tokens: serde_json::Value,
}

/// Raw YAML frontmatter shape — used internally for deserialization.
#[derive(Debug, Deserialize)]
struct RawFrontmatter {
    date: String,
    project: Option<String>,
    session_id: Option<String>,
    summary: String,
    #[serde(default)]
    next_steps: Vec<String>,
    #[serde(default)]
    open_questions: Vec<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

/// Parse a session handoff markdown file.
///
/// Expects YAML frontmatter delimited by `---` at the start of the file.
/// Returns the parsed core fields, metadata blob, and remaining body text.
pub fn parse_handoff(content: &str) -> Result<ParsedSession, AppError> {
    let (frontmatter_str, body) = extract_frontmatter(content)?;

    let raw: RawFrontmatter = serde_norway::from_str(&frontmatter_str)
        .map_err(|e| AppError::ParseError(format!("Invalid YAML frontmatter: {e}")))?;

    Ok(ParsedSession {
        date: raw.date,
        project: raw.project,
        session_id: raw.session_id,
        summary: raw.summary,
        next_steps: raw.next_steps,
        open_questions: raw.open_questions,
        metadata: raw.metadata,
        body,
    })
}

/// Max file size for session handoff files (5 MB).
const MAX_SESSION_FILE_SIZE: u64 = 5 * 1024 * 1024;

/// Parse a session handoff file from disk.
///
/// **Trust assumption**: `path` must come from a trusted source (project root
/// scan or OS file watcher), not from untrusted frontend input.
pub fn parse_handoff_file(path: &Path) -> Result<ParsedSession, AppError> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_SESSION_FILE_SIZE {
        return Err(AppError::ParseError(format!(
            "Session file too large ({} bytes, max {})",
            metadata.len(),
            MAX_SESSION_FILE_SIZE,
        )));
    }
    let content = std::fs::read_to_string(path)?;
    parse_handoff(&content)
}

/// Parse a companion .meta.json sidecar file.
///
/// **Trust assumption**: `path` must come from a trusted source.
pub fn parse_meta_sidecar(path: &Path) -> Result<SessionMeta, AppError> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_SESSION_FILE_SIZE {
        return Err(AppError::ParseError(format!(
            "Session meta file too large ({} bytes, max {})",
            metadata.len(),
            MAX_SESSION_FILE_SIZE,
        )));
    }
    let content = std::fs::read_to_string(path)?;
    let meta: SessionMeta = serde_json::from_str(&content)
        .map_err(|e| AppError::ParseError(format!("Invalid session meta JSON: {e}")))?;
    Ok(meta)
}

/// Given a handoff .md path, return the expected .meta.json sidecar path.
pub fn meta_sidecar_path(handoff_path: &Path) -> Option<std::path::PathBuf> {
    let stem = handoff_path.file_stem()?.to_str()?;
    let parent = handoff_path.parent()?;
    Some(parent.join(format!("{stem}.meta.json")))
}

/// Extract YAML frontmatter delimited by `---` lines.
/// Returns (frontmatter_content, body_after_frontmatter).
fn extract_frontmatter(content: &str) -> Result<(String, String), AppError> {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return Err(AppError::ParseError(
            "No YAML frontmatter found (file must start with ---)".to_string(),
        ));
    }

    // Find the closing `---` delimiter
    let after_opening = &trimmed[3..];
    let after_opening = after_opening.trim_start_matches(['\r', '\n']);

    let close_pos = after_opening.find("\n---").ok_or_else(|| {
        AppError::ParseError("No closing --- delimiter for YAML frontmatter".to_string())
    })?;

    let frontmatter = &after_opening[..close_pos];
    let rest = &after_opening[close_pos + 4..]; // skip "\n---"

    // Skip the newline after closing ---
    let body = rest.strip_prefix('\n').unwrap_or(rest);
    let body = body.strip_prefix('\r').unwrap_or(body);

    Ok((frontmatter.to_string(), body.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const VALID_HANDOFF: &str = r#"---
date: 2026-02-17T14:30:45Z
project: taurhaus
session_id: abc-123-def
summary: >
  Completed Phase 4 architecture decisions. Defined storage strategy
  (SQLite + tantivy), data model, IPC commands, and Claude Code integration.
next_steps:
  - Scaffold Tauri 2 project
  - Implement SQLite schema and migrations
  - Build project scanner module
open_questions:
  - Virtual scrolling library for large project lists
  - Tantivy vs SQLite FTS5 performance comparison at scale
metadata:
  decisions_made:
    - Hybrid storage (SQLite + tantivy)
    - UUID primary keys
    - Auto-detected relationships
  branch: main
  commit_range: d7d869b..HEAD
---

## Session Notes

This session covered the full Phase 4 architecture work.
"#;

    // AC1: Parses valid handoff file with all core fields
    #[test]
    fn parse_valid_handoff() {
        let parsed = parse_handoff(VALID_HANDOFF).unwrap();

        assert_eq!(parsed.date, "2026-02-17T14:30:45Z");
        assert_eq!(parsed.project, Some("taurhaus".to_string()));
        assert_eq!(parsed.session_id, Some("abc-123-def".to_string()));
        assert!(parsed.summary.contains("Completed Phase 4"));
        assert_eq!(parsed.next_steps.len(), 3);
        assert_eq!(parsed.next_steps[0], "Scaffold Tauri 2 project");
        assert_eq!(parsed.open_questions.len(), 2);
    }

    // AC2: Handles missing optional fields gracefully
    #[test]
    fn parse_handoff_missing_optional_fields() {
        let content = r#"---
date: 2026-02-17T14:30:45Z
summary: A short session.
---

Just some notes.
"#;
        let parsed = parse_handoff(content).unwrap();

        assert_eq!(parsed.date, "2026-02-17T14:30:45Z");
        assert!(parsed.project.is_none());
        assert!(parsed.session_id.is_none());
        assert_eq!(parsed.summary, "A short session.");
        assert!(parsed.next_steps.is_empty());
        assert!(parsed.open_questions.is_empty());
        assert_eq!(parsed.metadata, serde_json::Value::Null);
    }

    // AC3: Extracts metadata block into serde_json::Value
    #[test]
    fn parse_handoff_metadata_extraction() {
        let parsed = parse_handoff(VALID_HANDOFF).unwrap();

        assert!(parsed.metadata.is_object());
        let meta = parsed.metadata.as_object().unwrap();
        assert!(meta.contains_key("decisions_made"));
        assert!(meta.contains_key("branch"));
        assert_eq!(meta["branch"], "main");

        let decisions = meta["decisions_made"].as_array().unwrap();
        assert_eq!(decisions.len(), 3);
    }

    // AC4: Parses companion .meta.json sidecar
    #[test]
    fn parse_meta_sidecar_file() {
        let dir = TempDir::new().unwrap();
        let meta_path = dir.path().join("session-2026-02-17T14-30-45.meta.json");
        std::fs::write(
            &meta_path,
            r#"{
                "session_id": "abc-123-def",
                "started_at": "2026-02-17T12:00:00Z",
                "ended_at": "2026-02-17T14:30:45Z",
                "duration_minutes": 150,
                "exit_reason": "prompt_input_exit",
                "model": "claude-opus-4-6",
                "tools_used": {"Edit": 23, "Read": 45},
                "files_modified": ["docs/phase-4-architecture.md"],
                "tokens": {"input": 245000, "output": 38000}
            }"#,
        )
        .unwrap();

        let meta = parse_meta_sidecar(&meta_path).unwrap();
        assert_eq!(meta.session_id, Some("abc-123-def".to_string()));
        assert_eq!(meta.duration_minutes, Some(150));
        assert_eq!(meta.exit_reason, Some("prompt_input_exit".to_string()));
        assert_eq!(meta.model, Some("claude-opus-4-6".to_string()));
        assert_eq!(meta.files_modified, vec!["docs/phase-4-architecture.md"]);
    }

    // AC5: Returns error for malformed YAML frontmatter
    #[test]
    fn parse_malformed_yaml_returns_error() {
        let content = r#"---
date: 2026-02-17
summary: [invalid yaml
  this is broken
---
"#;
        let result = parse_handoff(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ParseError(msg) => assert!(msg.contains("Invalid YAML")),
            e => panic!("Expected ParseError, got: {e:?}"),
        }
    }

    // AC6: Handles files without frontmatter
    #[test]
    fn parse_no_frontmatter_returns_error() {
        let content = "# Just a regular markdown file\n\nNo frontmatter here.";
        let result = parse_handoff(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ParseError(msg) => assert!(msg.contains("No YAML frontmatter")),
            e => panic!("Expected ParseError, got: {e:?}"),
        }
    }

    // AC6b: File with only opening delimiter
    #[test]
    fn parse_unclosed_frontmatter_returns_error() {
        let content = "---\ndate: 2026-02-17\nsummary: unclosed\n";
        let result = parse_handoff(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ParseError(msg) => assert!(msg.contains("closing ---")),
            e => panic!("Expected ParseError, got: {e:?}"),
        }
    }

    // AC7: Free-text body after frontmatter is preserved
    #[test]
    fn parse_preserves_body() {
        let parsed = parse_handoff(VALID_HANDOFF).unwrap();
        assert!(parsed.body.contains("## Session Notes"));
        assert!(parsed.body.contains("Phase 4 architecture work"));
    }

    // Utility: meta_sidecar_path derives correct path
    #[test]
    fn meta_sidecar_path_derivation() {
        let handoff = Path::new("/project/docs/sessions/session-2026-02-17T14-30-45.md");
        let meta = meta_sidecar_path(handoff).unwrap();
        assert_eq!(
            meta,
            Path::new("/project/docs/sessions/session-2026-02-17T14-30-45.meta.json")
        );
    }

    // File-based parsing
    #[test]
    fn parse_handoff_from_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("session-test.md");
        std::fs::write(&file_path, VALID_HANDOFF).unwrap();

        let parsed = parse_handoff_file(&file_path).unwrap();
        assert_eq!(parsed.date, "2026-02-17T14:30:45Z");
        assert!(parsed.summary.contains("Phase 4"));
    }

    // Meta sidecar with minimal fields
    #[test]
    fn parse_meta_sidecar_minimal() {
        let dir = TempDir::new().unwrap();
        let meta_path = dir.path().join("session-test.meta.json");
        std::fs::write(&meta_path, r#"{"session_id": "test-123"}"#).unwrap();

        let meta = parse_meta_sidecar(&meta_path).unwrap();
        assert_eq!(meta.session_id, Some("test-123".to_string()));
        assert!(meta.started_at.is_none());
        assert!(meta.files_modified.is_empty());
    }

    // Verify hook script output format is parseable
    #[test]
    fn parse_hook_output_format() {
        let content = r#"---
date: 2026-02-17T14:30:45Z
project: taurhaus
session_id: test-hook-session-001
summary: >
  Completed Phase 5E implementation including relationship detection, scanner module, and frontend UI. All 202 Rust tests and 80 frontend tests pass.
next_steps:
  - Implement Phase 5F Claude Code integration
  - Create SessionEnd hook configuration
  - Build claude_code module stub
open_questions:
  - Claude Code hash algorithm for project path resolution
metadata:
  exit_reason: prompt_input_exit
---

## Session Notes

This session focused on completing Phase 5E of taurhaus implementation.
"#;
        let parsed = parse_handoff(content).unwrap();
        assert_eq!(parsed.date, "2026-02-17T14:30:45Z");
        assert_eq!(parsed.project, Some("taurhaus".to_string()));
        assert_eq!(parsed.session_id, Some("test-hook-session-001".to_string()));
        assert!(parsed.summary.contains("Phase 5E"));
        assert_eq!(parsed.next_steps.len(), 3);
        assert_eq!(parsed.open_questions.len(), 1);
        assert!(parsed.metadata.is_object());
        let meta = parsed.metadata.as_object().unwrap();
        assert_eq!(meta["exit_reason"], "prompt_input_exit");
        assert!(parsed.body.contains("Session Notes"));
    }

    // Verify hook sidecar output format is parseable
    #[test]
    fn parse_hook_sidecar_format() {
        let dir = TempDir::new().unwrap();
        let meta_path = dir.path().join("session-2026-02-17T14-30-45.meta.json");
        std::fs::write(
            &meta_path,
            r#"{
  "session_id": "test-hook-session-001",
  "ended_at": "2026-02-17T14:30:45Z",
  "exit_reason": "prompt_input_exit",
  "model": "unknown",
  "tools_used": {},
  "files_modified": [],
  "tokens": {}
}"#,
        )
        .unwrap();

        let meta = parse_meta_sidecar(&meta_path).unwrap();
        assert_eq!(meta.session_id, Some("test-hook-session-001".to_string()));
        assert_eq!(meta.ended_at, Some("2026-02-17T14:30:45Z".to_string()));
        assert_eq!(meta.exit_reason, Some("prompt_input_exit".to_string()));
        assert_eq!(meta.model, Some("unknown".to_string()));
        assert!(meta.files_modified.is_empty());
    }

    // Verify hook empty-transcript fallback format is parseable
    #[test]
    fn parse_hook_empty_transcript_format() {
        let content = r#"---
date: 2026-02-17T14:30:45Z
project: taurhaus
session_id: empty-session-001
summary: >
  Session ended without transcript data available.
next_steps: []
open_questions: []
metadata:
  exit_reason: other
---

## Session Notes

No transcript was available for this session.
"#;
        let parsed = parse_handoff(content).unwrap();
        assert_eq!(parsed.session_id, Some("empty-session-001".to_string()));
        assert!(parsed.summary.contains("without transcript"));
        assert!(parsed.next_steps.is_empty());
        assert!(parsed.open_questions.is_empty());
    }
}
