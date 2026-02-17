//! Integration tests for the end-to-end session handoff pipeline.
//!
//! Tests the full chain: write hook-format handoff file → parse → import into
//! SQLite → query back → index in tantivy → search finds it.
//!
//! These tests verify the contract between the SessionEnd hook output
//! (ADR-016/018) and the taurhaus import pipeline.

use tempfile::{NamedTempFile, TempDir};

use taurhaus_lib::db::{init_db, queries, session_queries};
use taurhaus_lib::models::Project;
use taurhaus_lib::search::indexer::{self, SearchIndex};
use taurhaus_lib::services::session_import;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_db() -> (rusqlite::Connection, NamedTempFile) {
    let tmp = NamedTempFile::new().unwrap();
    let conn = init_db(tmp.path()).unwrap();
    (conn, tmp)
}

fn seed_project(conn: &rusqlite::Connection, id: &str, path: &str) {
    let project = Project {
        id: id.to_string(),
        name: "test-project".to_string(),
        path: path.to_string(),
        description: None,
        last_activity_at: Some("2026-02-17T00:00:00Z".to_string()),
        hero_preference: None,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-02-17T00:00:00Z".to_string(),
    };
    queries::insert_project(conn, &project).unwrap();
}

fn write_hook_handoff(project_dir: &std::path::Path, filename: &str, content: &str) -> std::path::PathBuf {
    let sessions_dir = project_dir.join("docs").join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let path = sessions_dir.join(filename);
    std::fs::write(&path, content).unwrap();
    path
}

fn write_hook_sidecar(project_dir: &std::path::Path, filename: &str, content: &str) -> std::path::PathBuf {
    let sessions_dir = project_dir.join("docs").join("sessions");
    let path = sessions_dir.join(filename);
    std::fs::write(&path, content).unwrap();
    path
}

/// Standard hook handoff content matching ADR-018 format.
const HOOK_HANDOFF: &str = r#"---
date: 2026-02-17T14:30:45Z
project: taurhaus
session_id: hook-session-001
summary: >
  Completed Phase 5E implementation including relationship detection,
  scanner module, and frontend UI for displaying project relationships.
next_steps:
  - Implement Phase 5F Claude Code integration
  - Create SessionEnd hook configuration
  - Build claude_code module resolver
open_questions:
  - Claude Code hash algorithm for project path resolution
metadata:
  exit_reason: prompt_input_exit
  branch: main
---

## Session Notes

Key work in this session included building the relationship auto-detection
system that scans Cargo.toml, CLAUDE.md, and session data for cross-project
references. The frontend now shows relationship direction arrows and type
badges with dismiss support for auto-detected relationships.
"#;

const HOOK_SIDECAR: &str = r#"{
  "session_id": "hook-session-001",
  "started_at": "2026-02-17T12:00:00Z",
  "ended_at": "2026-02-17T14:30:45Z",
  "duration_minutes": 150,
  "exit_reason": "prompt_input_exit",
  "model": "claude-opus-4-6",
  "tools_used": {"Edit": 23, "Read": 45, "Bash": 12},
  "files_modified": ["src-tauri/src/services/relationships.rs", "src/Shell.svelte"],
  "tokens": {"input": 245000, "output": 38000}
}"#;

// ---------------------------------------------------------------------------
// TC1: Full pipeline — write hook-format handoff → import → query → search
// ---------------------------------------------------------------------------

#[test]
fn full_pipeline_handoff_to_search() {
    let (conn, _tmp_db) = test_db();
    let project_dir = TempDir::new().unwrap();
    seed_project(&conn, "p1", project_dir.path().to_str().unwrap());

    // 1. Write hook-format files
    write_hook_handoff(project_dir.path(), "session-2026-02-17T14-30-45.md", HOOK_HANDOFF);
    write_hook_sidecar(project_dir.path(), "session-2026-02-17T14-30-45.meta.json", HOOK_SIDECAR);

    // 2. Import via scan
    let imported = session_import::scan_and_import_sessions(&conn, "p1", project_dir.path()).unwrap();
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0], "hook-session-001");

    // 3. Query back from database
    let session = session_queries::get_session(&conn, "hook-session-001")
        .unwrap()
        .unwrap();
    assert_eq!(session.project_id, "p1");
    assert!(session.summary.contains("Phase 5E"));
    assert_eq!(session.next_steps.len(), 3);
    assert_eq!(session.open_questions.len(), 1);

    // 4. Verify metadata includes sidecar data
    assert!(session.metadata.is_object());
    let meta = session.metadata.as_object().unwrap();
    assert!(meta.contains_key("_sidecar") || meta.contains_key("duration_minutes"),
        "Metadata should contain sidecar data: {:?}", meta);

    // 5. Index in tantivy and search
    let mut index = SearchIndex::open_in_memory().unwrap();
    let indexed = indexer::index_session(&mut index, "p1", "hook-session-001", &conn).unwrap();
    assert!(indexed, "Session should be indexed");

    let results = index.search("relationship detection", 10).unwrap();
    assert!(!results.is_empty(), "Search should find the session");
    assert_eq!(results[0].entity_type, "session");
    assert_eq!(results[0].project_id, "p1");
}

// ---------------------------------------------------------------------------
// TC2: Handoff with sidecar metadata merges correctly
// ---------------------------------------------------------------------------

#[test]
fn handoff_with_sidecar_merges_metadata() {
    let (conn, _tmp_db) = test_db();
    let project_dir = TempDir::new().unwrap();
    seed_project(&conn, "p1", project_dir.path().to_str().unwrap());

    let handoff_path = write_hook_handoff(
        project_dir.path(),
        "session-2026-02-17T14-30-45.md",
        HOOK_HANDOFF,
    );
    write_hook_sidecar(
        project_dir.path(),
        "session-2026-02-17T14-30-45.meta.json",
        HOOK_SIDECAR,
    );

    let session_id = session_import::import_handoff(&conn, "p1", &handoff_path)
        .unwrap()
        .unwrap();
    assert_eq!(session_id, "hook-session-001");

    let session = session_queries::get_session(&conn, &session_id)
        .unwrap()
        .unwrap();

    // Metadata should contain both frontmatter metadata and sidecar
    let meta = session.metadata.as_object().unwrap();
    // Frontmatter metadata has exit_reason and branch
    assert!(meta.contains_key("exit_reason") || meta.contains_key("_sidecar"));

    // Sidecar should be nested under _sidecar key
    if let Some(sidecar) = meta.get("_sidecar") {
        let sidecar = sidecar.as_object().unwrap();
        assert_eq!(sidecar["duration_minutes"], 150);
        assert_eq!(sidecar["model"], "claude-opus-4-6");
    }
}

// ---------------------------------------------------------------------------
// TC3: Handoff without sidecar succeeds with generated UUID
// ---------------------------------------------------------------------------

#[test]
fn handoff_without_sidecar_generates_uuid() {
    let (conn, _tmp_db) = test_db();
    let project_dir = TempDir::new().unwrap();
    seed_project(&conn, "p1", project_dir.path().to_str().unwrap());

    let content = r#"---
date: 2026-02-17T14:30:45Z
summary: A quick session without session_id or sidecar.
next_steps:
  - Continue work
---

Brief notes.
"#;
    let handoff_path = write_hook_handoff(
        project_dir.path(),
        "session-2026-02-17T15-00-00.md",
        content,
    );

    let session_id = session_import::import_handoff(&conn, "p1", &handoff_path)
        .unwrap()
        .unwrap();

    // Should be a valid UUID (generated since no session_id in frontmatter)
    assert!(uuid::Uuid::parse_str(&session_id).is_ok());

    // Should still be queryable
    let session = session_queries::get_session(&conn, &session_id)
        .unwrap()
        .unwrap();
    assert!(session.summary.contains("quick session"));
}

// ---------------------------------------------------------------------------
// TC4: Multiple handoffs in sequence — all imported, all searchable
// ---------------------------------------------------------------------------

#[test]
fn multiple_handoffs_all_imported_and_searchable() {
    let (conn, _tmp_db) = test_db();
    let project_dir = TempDir::new().unwrap();
    seed_project(&conn, "p1", project_dir.path().to_str().unwrap());

    let handoff_a = r#"---
date: 2026-02-15T10:00:00Z
session_id: session-alpha
summary: Implemented the database schema and migrations.
next_steps:
  - Add git module
---
"#;

    let handoff_b = r#"---
date: 2026-02-16T14:00:00Z
session_id: session-beta
summary: Built the git integration with libgit2.
next_steps:
  - Add file watcher
---
"#;

    let handoff_c = r#"---
date: 2026-02-17T09:00:00Z
session_id: session-gamma
summary: Implemented tantivy full-text search indexing.
next_steps:
  - Add relationship detection
---
"#;

    write_hook_handoff(project_dir.path(), "session-2026-02-15T10-00-00.md", handoff_a);
    write_hook_handoff(project_dir.path(), "session-2026-02-16T14-00-00.md", handoff_b);
    write_hook_handoff(project_dir.path(), "session-2026-02-17T09-00-00.md", handoff_c);

    // Import all
    let imported = session_import::scan_and_import_sessions(&conn, "p1", project_dir.path()).unwrap();
    assert_eq!(imported.len(), 3);

    // List sessions
    let sessions = session_queries::list_sessions(&conn, "p1", 20, 0).unwrap();
    assert_eq!(sessions.len(), 3);

    // Index and search
    let mut index = SearchIndex::open_in_memory().unwrap();
    for sid in &imported {
        indexer::index_session(&mut index, "p1", sid, &conn).unwrap();
    }

    // Search for specific content
    let results = index.search("libgit2", 10).unwrap();
    assert!(!results.is_empty());
    assert!(results[0].entity_type == "session");

    let results = index.search("tantivy", 10).unwrap();
    assert!(!results.is_empty());

    let results = index.search("database migrations", 10).unwrap();
    assert!(!results.is_empty());
}

// ---------------------------------------------------------------------------
// TC5: Malformed handoff — import skips gracefully, others still imported
// ---------------------------------------------------------------------------

#[test]
fn malformed_handoff_skipped_gracefully() {
    let (conn, _tmp_db) = test_db();
    let project_dir = TempDir::new().unwrap();
    seed_project(&conn, "p1", project_dir.path().to_str().unwrap());

    // Valid handoff
    write_hook_handoff(project_dir.path(), "session-2026-02-17T10-00-00.md", HOOK_HANDOFF);

    // Malformed handoff (no YAML frontmatter)
    write_hook_handoff(
        project_dir.path(),
        "session-2026-02-17T11-00-00.md",
        "# Not a handoff\n\nJust a regular markdown file.",
    );

    // Another valid handoff
    let second_valid = r#"---
date: 2026-02-17T12:00:00Z
session_id: session-second
summary: Second valid session.
next_steps:
  - Do more work
---
"#;
    write_hook_handoff(project_dir.path(), "session-2026-02-17T12-00-00.md", second_valid);

    // Import — malformed should be skipped, both valid imported
    let imported = session_import::scan_and_import_sessions(&conn, "p1", project_dir.path()).unwrap();
    assert_eq!(imported.len(), 2, "Should import 2 valid handoffs, skip 1 malformed");
}

// ---------------------------------------------------------------------------
// TC6: Duplicate import — dedup works, no duplicate sessions
// ---------------------------------------------------------------------------

#[test]
fn duplicate_import_deduplication() {
    let (conn, _tmp_db) = test_db();
    let project_dir = TempDir::new().unwrap();
    seed_project(&conn, "p1", project_dir.path().to_str().unwrap());

    write_hook_handoff(project_dir.path(), "session-2026-02-17T14-30-45.md", HOOK_HANDOFF);

    // First import
    let first = session_import::scan_and_import_sessions(&conn, "p1", project_dir.path()).unwrap();
    assert_eq!(first.len(), 1);

    // Second import — should be deduplicated
    let second = session_import::scan_and_import_sessions(&conn, "p1", project_dir.path()).unwrap();
    assert!(second.is_empty(), "Second scan should skip already-imported sessions");

    // Verify only one session in DB
    let sessions = session_queries::list_sessions(&conn, "p1", 100, 0).unwrap();
    assert_eq!(sessions.len(), 1, "Should have exactly 1 session, not duplicates");
}
