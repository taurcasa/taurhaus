use std::path::Path;

use rusqlite::Connection;

use crate::db::session_queries;
use crate::errors::AppError;
use crate::models::SessionDetail;
use crate::session::parser;

/// Import a session handoff file into the database.
///
/// Parses the handoff markdown file, optionally reads the companion .meta.json
/// sidecar, and inserts into the sessions table. Returns the session ID.
///
/// Skips import (returns Ok(None)) if the file has already been imported (dedup).
pub fn import_handoff(
    conn: &Connection,
    project_id: &str,
    handoff_path: &Path,
) -> Result<Option<String>, AppError> {
    let file_path_str = handoff_path.to_string_lossy().to_string();

    // Dedup: skip if already imported
    if session_queries::session_exists_by_file_path(conn, &file_path_str)? {
        tracing::debug!(path = %file_path_str, "Session already imported, skipping");
        return Ok(None);
    }

    // Parse the handoff file
    let parsed = parser::parse_handoff_file(handoff_path)?;

    // Read optional .meta.json sidecar
    let meta = parser::meta_sidecar_path(handoff_path)
        .filter(|p| p.exists())
        .and_then(|p| parser::parse_meta_sidecar(&p).ok());

    // Use session_id from frontmatter, then sidecar, then generate one
    let session_id = parsed
        .session_id
        .or_else(|| meta.as_ref().and_then(|m| m.session_id.clone()))
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Merge sidecar metadata into frontmatter metadata
    let metadata = if let Some(ref meta) = meta {
        let mut combined = parsed.metadata.clone();
        if let Some(obj) = combined.as_object_mut() {
            if let Ok(sidecar_val) = serde_json::to_value(meta) {
                obj.insert("_sidecar".to_string(), sidecar_val);
            }
        } else if combined.is_null() {
            // frontmatter had no metadata, use sidecar as-is
            combined =
                serde_json::to_value(meta).expect("SessionMeta should always serialize to JSON");
        }
        combined
    } else {
        parsed.metadata
    };

    let now = chrono::Utc::now().to_rfc3339();

    let session = SessionDetail {
        id: session_id.clone(),
        project_id: project_id.to_string(),
        date: parsed.date,
        summary: parsed.summary,
        next_steps: parsed.next_steps,
        open_questions: parsed.open_questions,
        metadata,
        file_path: file_path_str,
        created_at: now,
    };

    session_queries::insert_session(conn, &session)?;

    tracing::info!(session_id = %session_id, "Imported session handoff");
    Ok(Some(session_id))
}

/// Scan a project's docs/sessions/ directory for unimported handoff files.
/// Returns the list of newly imported session IDs.
pub fn scan_and_import_sessions(
    conn: &Connection,
    project_id: &str,
    project_root: &Path,
) -> Result<Vec<String>, AppError> {
    let sessions_dir = project_root.join("docs").join("sessions");
    if !sessions_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut imported = Vec::new();

    let entries = std::fs::read_dir(&sessions_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Only process session-*.md files
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !filename.starts_with("session-") || !filename.ends_with(".md") {
            continue;
        }

        match import_handoff(conn, project_id, &path) {
            Ok(Some(session_id)) => imported.push(session_id),
            Ok(None) => {} // already imported
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to import session");
            }
        }
    }

    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::db::queries::insert_project;
    use crate::models::Project;
    use tempfile::{NamedTempFile, TempDir};

    fn test_db() -> (Connection, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();
        (conn, tmp)
    }

    fn seed_project(conn: &Connection, id: &str, path: &str) {
        let project = Project {
            id: id.to_string(),
            name: "test".to_string(),
            path: path.to_string(),
            description: None,
            last_activity_at: Some("2026-01-01T00:00:00Z".to_string()),
            hero_preference: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            cached_branch: None,
            cached_is_dirty: None,
            account_memory: Default::default(),
        };
        insert_project(conn, &project).unwrap();
    }

    fn write_handoff(dir: &Path, filename: &str, content: &str) -> std::path::PathBuf {
        let sessions_dir = dir.join("docs").join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        let path = sessions_dir.join(filename);
        std::fs::write(&path, content).unwrap();
        path
    }

    const VALID_HANDOFF: &str = r#"---
date: 2026-02-17T14:30:45Z
project: taurhaus
session_id: test-session-001
summary: Completed Phase 4 architecture.
next_steps:
  - Scaffold project
  - Implement schema
open_questions:
  - Which scrolling library?
---

## Notes

Session notes here.
"#;

    #[test]
    fn import_handoff_creates_session() {
        let (conn, _tmp) = test_db();
        let dir = TempDir::new().unwrap();
        seed_project(&conn, "p1", dir.path().to_str().unwrap());

        let path = write_handoff(dir.path(), "session-2026-02-17T14-30-45.md", VALID_HANDOFF);

        let result = import_handoff(&conn, "p1", &path).unwrap();
        assert_eq!(result, Some("test-session-001".to_string()));

        // Verify it's in the database
        let session = session_queries::get_session(&conn, "test-session-001")
            .unwrap()
            .unwrap();
        assert_eq!(session.summary, "Completed Phase 4 architecture.");
        assert_eq!(session.next_steps.len(), 2);
        assert_eq!(session.open_questions.len(), 1);
    }

    #[test]
    fn import_handoff_dedup() {
        let (conn, _tmp) = test_db();
        let dir = TempDir::new().unwrap();
        seed_project(&conn, "p1", dir.path().to_str().unwrap());

        let path = write_handoff(dir.path(), "session-2026-02-17T14-30-45.md", VALID_HANDOFF);

        // First import succeeds
        let result1 = import_handoff(&conn, "p1", &path).unwrap();
        assert!(result1.is_some());

        // Second import is skipped (dedup)
        let result2 = import_handoff(&conn, "p1", &path).unwrap();
        assert!(result2.is_none());
    }

    #[test]
    fn import_handoff_with_sidecar() {
        let (conn, _tmp) = test_db();
        let dir = TempDir::new().unwrap();
        seed_project(&conn, "p1", dir.path().to_str().unwrap());

        let path = write_handoff(dir.path(), "session-test.md", VALID_HANDOFF);

        // Write companion sidecar
        let sidecar_path = dir.path().join("docs/sessions/session-test.meta.json");
        std::fs::write(
            &sidecar_path,
            r#"{"session_id":"test-session-001","duration_minutes":120,"model":"claude-opus-4-6"}"#,
        )
        .unwrap();

        let result = import_handoff(&conn, "p1", &path).unwrap();
        assert!(result.is_some());

        let session = session_queries::get_session(&conn, "test-session-001")
            .unwrap()
            .unwrap();
        // Metadata should include sidecar data
        assert!(session.metadata.is_object());
    }

    #[test]
    fn import_handoff_generates_uuid_when_no_session_id() {
        let (conn, _tmp) = test_db();
        let dir = TempDir::new().unwrap();
        seed_project(&conn, "p1", dir.path().to_str().unwrap());

        let content = r#"---
date: 2026-02-17T14:30:45Z
summary: No session ID in this one.
---

Body text.
"#;
        let path = write_handoff(dir.path(), "session-no-id.md", content);

        let result = import_handoff(&conn, "p1", &path).unwrap();
        let session_id = result.unwrap();
        // Should be a valid UUID
        assert!(uuid::Uuid::parse_str(&session_id).is_ok());
    }

    #[test]
    fn scan_and_import_finds_all_sessions() {
        let (conn, _tmp) = test_db();
        let dir = TempDir::new().unwrap();
        seed_project(&conn, "p1", dir.path().to_str().unwrap());

        // Write multiple session files
        write_handoff(
            dir.path(),
            "session-2026-02-17T14-00-00.md",
            &VALID_HANDOFF.replace("test-session-001", "sess-1"),
        );
        write_handoff(
            dir.path(),
            "session-2026-02-18T10-00-00.md",
            &VALID_HANDOFF.replace("test-session-001", "sess-2"),
        );
        // Non-session file should be skipped
        write_handoff(
            dir.path(),
            "notes.md",
            "---\ndate: today\nsummary: not a session\n---\n",
        );

        let imported = scan_and_import_sessions(&conn, "p1", dir.path()).unwrap();
        assert_eq!(imported.len(), 2);
    }

    #[test]
    fn scan_empty_dir_returns_empty() {
        let (conn, _tmp) = test_db();
        let dir = TempDir::new().unwrap();
        seed_project(&conn, "p1", dir.path().to_str().unwrap());

        let imported = scan_and_import_sessions(&conn, "p1", dir.path()).unwrap();
        assert!(imported.is_empty());
    }

    #[test]
    fn scan_skips_already_imported() {
        let (conn, _tmp) = test_db();
        let dir = TempDir::new().unwrap();
        seed_project(&conn, "p1", dir.path().to_str().unwrap());

        write_handoff(dir.path(), "session-2026-02-17T14-00-00.md", VALID_HANDOFF);

        // First scan imports
        let first = scan_and_import_sessions(&conn, "p1", dir.path()).unwrap();
        assert_eq!(first.len(), 1);

        // Second scan skips
        let second = scan_and_import_sessions(&conn, "p1", dir.path()).unwrap();
        assert!(second.is_empty());
    }
}
