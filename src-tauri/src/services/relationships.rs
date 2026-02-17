use std::path::Path;

use rusqlite::Connection;

use crate::db::relationship_queries;
use crate::models::Project;

/// A detected relationship before syncing to the database.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedRelationship {
    pub target_project_id: String,
    pub relationship_type: String,
    pub detection_source: String,
}

/// Detect Cargo.toml path dependencies that reference registered projects.
///
/// Parses `[dependencies]` for entries with `path = "../some_dir"` and matches
/// the resolved path against known project paths.
pub fn detect_cargo_dependencies(
    project_root: &Path,
    all_projects: &[Project],
) -> Vec<DetectedRelationship> {
    let cargo_path = project_root.join("Cargo.toml");
    let content = match std::fs::read_to_string(&cargo_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let table: toml::Table = match content.parse() {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    let mut results = Vec::new();

    // Check both [dependencies] and [dev-dependencies]
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(deps) = table.get(section).and_then(|v| v.as_table()) {
            for (_name, value) in deps {
                let path_str = match value {
                    toml::Value::Table(t) => t.get("path").and_then(|p| p.as_str()),
                    _ => None,
                };

                if let Some(path_str) = path_str {
                    let dep_path = project_root.join(path_str);
                    let canonical = match dep_path.canonicalize() {
                        Ok(c) => c,
                        Err(_) => continue,
                    };

                    for project in all_projects {
                        let proj_path = Path::new(&project.path);
                        let proj_canonical = match proj_path.canonicalize() {
                            Ok(c) => c,
                            Err(_) => continue,
                        };
                        if canonical == proj_canonical {
                            results.push(DetectedRelationship {
                                target_project_id: project.id.clone(),
                                relationship_type: "depends_on".to_string(),
                                detection_source: "cargo_toml".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    results
}

/// Detect CLAUDE.md references to registered project names.
///
/// Performs case-insensitive search for project names in the CLAUDE.md file.
pub fn detect_claude_md_references(
    project_root: &Path,
    self_project_id: &str,
    all_projects: &[Project],
) -> Vec<DetectedRelationship> {
    let claude_path = project_root.join("CLAUDE.md");
    let content = match std::fs::read_to_string(&claude_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let content_lower = content.to_lowercase();
    let mut results = Vec::new();

    for project in all_projects {
        // Don't detect self-references
        if project.id == self_project_id {
            continue;
        }

        let name_lower = project.name.to_lowercase();
        // Require the name to be at least 3 chars to avoid false positives
        if name_lower.len() < 3 {
            continue;
        }

        if content_lower.contains(&name_lower) {
            results.push(DetectedRelationship {
                target_project_id: project.id.clone(),
                relationship_type: "references".to_string(),
                detection_source: "claude_md".to_string(),
            });
        }
    }

    results
}

/// Detect session mentions of registered project names.
///
/// Searches session summaries for the given project for references to other project names.
pub fn detect_session_mentions(
    conn: &Connection,
    project_id: &str,
    all_projects: &[Project],
) -> Vec<DetectedRelationship> {
    use crate::db::session_queries;

    // Get all sessions for this project (generous limit)
    let sessions = match session_queries::list_sessions(conn, project_id, 100, 0) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let mut results = Vec::new();
    let mut seen_targets = std::collections::HashSet::new();

    for session in &sessions {
        let summary_lower = session.summary.to_lowercase();

        for project in all_projects {
            if project.id == project_id {
                continue;
            }

            let name_lower = project.name.to_lowercase();
            if name_lower.len() < 3 {
                continue;
            }

            if summary_lower.contains(&name_lower) && seen_targets.insert(project.id.clone()) {
                results.push(DetectedRelationship {
                    target_project_id: project.id.clone(),
                    relationship_type: "mentioned_in_session".to_string(),
                    detection_source: "session_mention".to_string(),
                });
            }
        }
    }

    results
}

/// Run all detection methods for a project and return combined results.
pub fn detect_all_relationships(
    conn: &Connection,
    project_id: &str,
    project_root: &Path,
    all_projects: &[Project],
) -> Vec<DetectedRelationship> {
    let mut detected = Vec::new();

    detected.extend(detect_cargo_dependencies(project_root, all_projects));
    detected.extend(detect_claude_md_references(project_root, project_id, all_projects));
    detected.extend(detect_session_mentions(conn, project_id, all_projects));

    detected
}

/// Sync detected relationships to the database: upsert new/refreshed, remove stale auto-detected.
pub fn sync_relationships(
    conn: &Connection,
    project_id: &str,
    detected: &[DetectedRelationship],
) -> Result<(usize, usize), rusqlite::Error> {
    let mut upserted = 0;

    for det in detected {
        relationship_queries::upsert_relationship(
            conn,
            project_id,
            &det.target_project_id,
            &det.relationship_type,
            &det.detection_source,
        )?;
        upserted += 1;
    }

    // Build current set for stale removal
    let current: Vec<(String, String, String)> = detected
        .iter()
        .map(|d| {
            (
                project_id.to_string(),
                d.target_project_id.clone(),
                d.relationship_type.clone(),
            )
        })
        .collect();

    let removed = relationship_queries::remove_stale_auto_relationships(conn, project_id, &current)?;

    Ok((upserted, removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::db::queries::insert_project;
    use crate::db::relationship_queries;
    use crate::db::session_queries;
    use crate::models::{Project, SessionDetail};
    use tempfile::NamedTempFile;

    fn test_db() -> (Connection, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();
        (conn, tmp)
    }

    fn make_project(id: &str, name: &str, path: &str) -> Project {
        Project {
            id: id.to_string(),
            name: name.to_string(),
            path: path.to_string(),
            description: None,
            last_activity_at: Some("2026-01-01T00:00:00Z".to_string()),
            hero_preference: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    // AC1: Detect Cargo.toml path dependencies
    #[test]
    fn detect_cargo_path_deps() {
        let dir = tempfile::TempDir::new().unwrap();
        let dep_dir = tempfile::TempDir::new().unwrap();

        // Create a Cargo.toml with a path dependency
        let cargo_content = format!(
            r#"[package]
name = "test"
version = "0.1.0"

[dependencies]
dep-project = {{ path = "{}" }}
"#,
            dep_dir.path().display()
        );
        std::fs::write(dir.path().join("Cargo.toml"), &cargo_content).unwrap();

        let projects = vec![
            make_project("p1", "test", dir.path().to_str().unwrap()),
            make_project("p2", "dep-project", dep_dir.path().to_str().unwrap()),
        ];

        let detected = detect_cargo_dependencies(dir.path(), &projects);
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].target_project_id, "p2");
        assert_eq!(detected[0].relationship_type, "depends_on");
        assert_eq!(detected[0].detection_source, "cargo_toml");
    }

    // AC1b: No Cargo.toml returns empty
    #[test]
    fn detect_cargo_no_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let projects = vec![make_project("p1", "test", dir.path().to_str().unwrap())];

        let detected = detect_cargo_dependencies(dir.path(), &projects);
        assert!(detected.is_empty());
    }

    // AC1c: Path deps pointing to unregistered dirs are ignored
    #[test]
    fn detect_cargo_unregistered_dep() {
        let dir = tempfile::TempDir::new().unwrap();
        let dep_dir = tempfile::TempDir::new().unwrap();

        let cargo_content = format!(
            r#"[package]
name = "test"
version = "0.1.0"

[dependencies]
unknown = {{ path = "{}" }}
"#,
            dep_dir.path().display()
        );
        std::fs::write(dir.path().join("Cargo.toml"), &cargo_content).unwrap();

        // dep_dir not registered as a project
        let projects = vec![make_project("p1", "test", dir.path().to_str().unwrap())];

        let detected = detect_cargo_dependencies(dir.path(), &projects);
        assert!(detected.is_empty());
    }

    // AC2: Detect CLAUDE.md references
    #[test]
    fn detect_claude_md_refs() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("CLAUDE.md"),
            "This project uses taurui for design patterns and taursec for audits.",
        )
        .unwrap();

        let projects = vec![
            make_project("p1", "taurhaus", dir.path().to_str().unwrap()),
            make_project("p2", "taurui", "/projects/taurui"),
            make_project("p3", "taursec", "/projects/taursec"),
            make_project("p4", "aitx", "/projects/aitx"), // Only 4 chars, but >= 3
        ];

        let detected = detect_claude_md_references(dir.path(), "p1", &projects);
        assert_eq!(detected.len(), 2);

        let targets: Vec<&str> = detected.iter().map(|d| d.target_project_id.as_str()).collect();
        assert!(targets.contains(&"p2"));
        assert!(targets.contains(&"p3"));
    }

    // AC2b: No CLAUDE.md returns empty
    #[test]
    fn detect_claude_md_no_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let projects = vec![make_project("p1", "test", dir.path().to_str().unwrap())];

        let detected = detect_claude_md_references(dir.path(), "p1", &projects);
        assert!(detected.is_empty());
    }

    // AC2c: Self-references are excluded
    #[test]
    fn detect_claude_md_skips_self() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("CLAUDE.md"),
            "This is the taurhaus project.",
        )
        .unwrap();

        let projects = vec![make_project("p1", "taurhaus", dir.path().to_str().unwrap())];

        let detected = detect_claude_md_references(dir.path(), "p1", &projects);
        assert!(detected.is_empty());
    }

    // AC2d: Short names (< 3 chars) are skipped to avoid false positives
    #[test]
    fn detect_claude_md_skips_short_names() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "We use AI for everything.").unwrap();

        let projects = vec![
            make_project("p1", "main", dir.path().to_str().unwrap()),
            make_project("p2", "ai", "/projects/ai"), // 2 chars — should be skipped
        ];

        let detected = detect_claude_md_references(dir.path(), "p1", &projects);
        assert!(detected.is_empty());
    }

    // AC3: Detect session mentions
    #[test]
    fn detect_session_mentions_finds_refs() {
        let (conn, _tmp) = test_db();
        let p1 = make_project("p1", "taurhaus", "/projects/taurhaus");
        let p2 = make_project("p2", "taurui", "/projects/taurui");
        let p3 = make_project("p3", "taursec", "/projects/taursec");

        insert_project(&conn, &p1).unwrap();
        insert_project(&conn, &p2).unwrap();
        insert_project(&conn, &p3).unwrap();

        let session = SessionDetail {
            id: "s1".to_string(),
            project_id: "p1".to_string(),
            date: "2026-02-17".to_string(),
            summary: "Working on taurhaus. Used taurui design system for components.".to_string(),
            next_steps: vec![],
            open_questions: vec![],
            metadata: serde_json::Value::Null,
            file_path: "/sessions/s1.md".to_string(),
            created_at: "2026-02-17T00:00:00Z".to_string(),
        };
        session_queries::insert_session(&conn, &session).unwrap();

        let projects = vec![p1.clone(), p2.clone(), p3.clone()];
        let detected = detect_session_mentions(&conn, "p1", &projects);

        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].target_project_id, "p2");
        assert_eq!(detected[0].relationship_type, "mentioned_in_session");
    }

    // AC3b: No sessions returns empty
    #[test]
    fn detect_session_mentions_empty() {
        let (conn, _tmp) = test_db();
        let p1 = make_project("p1", "test", "/projects/test");
        insert_project(&conn, &p1).unwrap();

        let detected = detect_session_mentions(&conn, "p1", &[p1]);
        assert!(detected.is_empty());
    }

    // AC3c: Deduplication across multiple sessions
    #[test]
    fn detect_session_mentions_dedup() {
        let (conn, _tmp) = test_db();
        let p1 = make_project("p1", "taurhaus", "/projects/taurhaus");
        let p2 = make_project("p2", "taurui", "/projects/taurui");

        insert_project(&conn, &p1).unwrap();
        insert_project(&conn, &p2).unwrap();

        // Two sessions both mentioning taurui
        for (id, fp) in [("s1", "/sessions/s1.md"), ("s2", "/sessions/s2.md")] {
            let session = SessionDetail {
                id: id.to_string(),
                project_id: "p1".to_string(),
                date: "2026-02-17".to_string(),
                summary: "Used taurui components".to_string(),
                next_steps: vec![],
                open_questions: vec![],
                metadata: serde_json::Value::Null,
                file_path: fp.to_string(),
                created_at: "2026-02-17T00:00:00Z".to_string(),
            };
            session_queries::insert_session(&conn, &session).unwrap();
        }

        let projects = vec![p1, p2];
        let detected = detect_session_mentions(&conn, "p1", &projects);
        assert_eq!(detected.len(), 1); // Deduped
    }

    // AC4: sync_relationships upserts new and removes stale
    #[test]
    fn sync_upserts_and_removes_stale() {
        let (conn, _tmp) = test_db();
        let p1 = make_project("p1", "taurhaus", "/projects/taurhaus");
        let p2 = make_project("p2", "taurui", "/projects/taurui");
        let p3 = make_project("p3", "taursec", "/projects/taursec");

        insert_project(&conn, &p1).unwrap();
        insert_project(&conn, &p2).unwrap();
        insert_project(&conn, &p3).unwrap();

        // First scan: detect p2 and p3
        let detected = vec![
            DetectedRelationship {
                target_project_id: "p2".to_string(),
                relationship_type: "depends_on".to_string(),
                detection_source: "cargo_toml".to_string(),
            },
            DetectedRelationship {
                target_project_id: "p3".to_string(),
                relationship_type: "references".to_string(),
                detection_source: "claude_md".to_string(),
            },
        ];

        let (upserted, removed) = sync_relationships(&conn, "p1", &detected).unwrap();
        assert_eq!(upserted, 2);
        assert_eq!(removed, 0);
        assert_eq!(relationship_queries::list_relationships(&conn, "p1").unwrap().len(), 2);

        // Second scan: only p2 detected (p3 is stale)
        let detected = vec![DetectedRelationship {
            target_project_id: "p2".to_string(),
            relationship_type: "depends_on".to_string(),
            detection_source: "cargo_toml".to_string(),
        }];

        let (upserted, removed) = sync_relationships(&conn, "p1", &detected).unwrap();
        assert_eq!(upserted, 1);
        assert_eq!(removed, 1);
        assert_eq!(relationship_queries::list_relationships(&conn, "p1").unwrap().len(), 1);
    }

    // AC4b: detect_all combines all sources
    #[test]
    fn detect_all_combines_sources() {
        let dir = tempfile::TempDir::new().unwrap();
        let (conn, _tmp) = test_db();

        // Set up a CLAUDE.md mentioning taurui
        std::fs::write(
            dir.path().join("CLAUDE.md"),
            "Uses taurui design patterns.",
        )
        .unwrap();

        let p1 = make_project("p1", "taurhaus", dir.path().to_str().unwrap());
        let p2 = make_project("p2", "taurui", "/projects/taurui");

        insert_project(&conn, &p1).unwrap();
        insert_project(&conn, &p2).unwrap();

        let projects = vec![p1, p2];
        let detected = detect_all_relationships(&conn, "p1", dir.path(), &projects);

        // Should find CLAUDE.md reference (no Cargo.toml, no sessions)
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].detection_source, "claude_md");
    }
}
