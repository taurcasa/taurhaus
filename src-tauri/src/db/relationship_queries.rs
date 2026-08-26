use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::models::Relationship;

/// Insert a new relationship. The caller provides a fully populated struct.
pub fn insert_relationship(conn: &Connection, rel: &Relationship) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO relationships (id, source_project_id, target_project_id, relationship_type, detection_source, dismissed, first_detected_at, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            rel.id,
            rel.source_project_id,
            rel.target_project_id,
            rel.relationship_type,
            rel.detection_source,
            rel.dismissed as i32,
            rel.first_detected_at,
            rel.last_seen_at,
        ],
    )?;
    Ok(())
}

/// List non-dismissed relationships where the given project is either source or target.
pub fn list_relationships(
    conn: &Connection,
    project_id: &str,
) -> Result<Vec<Relationship>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, source_project_id, target_project_id, relationship_type, detection_source, dismissed, first_detected_at, last_seen_at
         FROM relationships
         WHERE dismissed = 0 AND (source_project_id = ?1 OR target_project_id = ?1)
         ORDER BY last_seen_at DESC",
    )?;

    let rows = stmt.query_map([project_id], row_to_relationship)?;
    rows.collect()
}

/// List all relationships (including dismissed) where the project is source or target.
pub fn list_all_relationships(
    conn: &Connection,
    project_id: &str,
) -> Result<Vec<Relationship>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, source_project_id, target_project_id, relationship_type, detection_source, dismissed, first_detected_at, last_seen_at
         FROM relationships
         WHERE source_project_id = ?1 OR target_project_id = ?1
         ORDER BY last_seen_at DESC",
    )?;

    let rows = stmt.query_map([project_id], row_to_relationship)?;
    rows.collect()
}

/// Dismiss a relationship (soft delete). Returns true if a row was updated.
pub fn dismiss_relationship(conn: &Connection, id: &str) -> Result<bool, rusqlite::Error> {
    let changed = conn.execute("UPDATE relationships SET dismissed = 1 WHERE id = ?1", [id])?;
    Ok(changed > 0)
}

/// Permanently remove a relationship. Returns true if a row was deleted.
pub fn remove_relationship(conn: &Connection, id: &str) -> Result<bool, rusqlite::Error> {
    let changed = conn.execute("DELETE FROM relationships WHERE id = ?1", [id])?;
    Ok(changed > 0)
}

/// Upsert a relationship: if (source, target, type) already exists, update
/// `last_seen_at` and `detection_source`. Otherwise, insert a new row.
/// Returns the relationship ID (existing or new).
pub fn upsert_relationship(
    conn: &Connection,
    source_project_id: &str,
    target_project_id: &str,
    relationship_type: &str,
    detection_source: &str,
) -> Result<String, rusqlite::Error> {
    // Check for existing relationship
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT id, last_seen_at FROM relationships
             WHERE source_project_id = ?1 AND target_project_id = ?2 AND relationship_type = ?3",
            params![source_project_id, target_project_id, relationship_type],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let now = chrono::Utc::now().to_rfc3339();

    if let Some((id, _)) = existing {
        conn.execute(
            "UPDATE relationships SET last_seen_at = ?1, detection_source = ?2, dismissed = 0 WHERE id = ?3",
            params![now, detection_source, id],
        )?;
        Ok(id)
    } else {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO relationships (id, source_project_id, target_project_id, relationship_type, detection_source, dismissed, first_detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)",
            params![id, source_project_id, target_project_id, relationship_type, detection_source, now],
        )?;
        Ok(id)
    }
}

/// Remove auto-detected relationships for a project that are NOT in the current
/// set of detected (source, target, type) triples. Manual relationships are untouched.
/// Returns the number of removed rows.
pub fn remove_stale_auto_relationships(
    conn: &Connection,
    project_id: &str,
    current: &[(String, String, String)], // (source_id, target_id, type)
) -> Result<usize, rusqlite::Error> {
    // Get all non-dismissed auto-detected relationships for this project
    let mut stmt = conn.prepare(
        "SELECT id, source_project_id, target_project_id, relationship_type
         FROM relationships
         WHERE dismissed = 0 AND detection_source != 'manual'
           AND (source_project_id = ?1 OR target_project_id = ?1)",
    )?;

    let stale_ids: Vec<String> = stmt
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .filter(|(_, src, tgt, rel_type)| {
            !current
                .iter()
                .any(|(cs, ct, cr)| cs == src && ct == tgt && cr == rel_type)
        })
        .map(|(id, _, _, _)| id)
        .collect();

    let mut removed = 0;
    for id in &stale_ids {
        removed += conn.execute("DELETE FROM relationships WHERE id = ?1", [id])?;
    }
    Ok(removed)
}

/// Map a database row to a Relationship struct.
fn row_to_relationship(row: &rusqlite::Row<'_>) -> Result<Relationship, rusqlite::Error> {
    let dismissed_int: i32 = row.get(5)?;
    Ok(Relationship {
        id: row.get(0)?,
        source_project_id: row.get(1)?,
        target_project_id: row.get(2)?,
        relationship_type: row.get(3)?,
        detection_source: row.get(4)?,
        dismissed: dismissed_int != 0,
        first_detected_at: row.get(6)?,
        last_seen_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::db::queries::insert_project;
    use crate::models::Project;
    use pretty_assertions::assert_eq;
    use tempfile::NamedTempFile;

    fn test_db() -> (Connection, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();
        (conn, tmp)
    }

    fn seed_project(conn: &Connection, id: &str) {
        let project = Project {
            id: id.to_string(),
            name: format!("project-{id}"),
            path: format!("/projects/{id}"),
            description: None,
            last_activity_at: Some("2026-01-01T00:00:00Z".to_string()),
            hero_preference: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            cached_branch: None,
            cached_is_dirty: None,
            claude_account_id: None,
        };
        insert_project(conn, &project).unwrap();
    }

    fn make_relationship(id: &str, source: &str, target: &str, rel_type: &str) -> Relationship {
        Relationship {
            id: id.to_string(),
            source_project_id: source.to_string(),
            target_project_id: target.to_string(),
            relationship_type: rel_type.to_string(),
            detection_source: "cargo_toml".to_string(),
            dismissed: false,
            first_detected_at: "2026-01-15T00:00:00Z".to_string(),
            last_seen_at: "2026-02-01T00:00:00Z".to_string(),
        }
    }

    // AC1: insert and retrieve a relationship
    #[test]
    fn insert_and_list_relationship() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        let rel = make_relationship("r1", "p1", "p2", "depends_on");
        insert_relationship(&conn, &rel).unwrap();

        let rels = list_relationships(&conn, "p1").unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].id, "r1");
        assert_eq!(rels[0].source_project_id, "p1");
        assert_eq!(rels[0].target_project_id, "p2");
        assert_eq!(rels[0].relationship_type, "depends_on");
        assert_eq!(rels[0].detection_source, "cargo_toml");
        assert!(!rels[0].dismissed);
    }

    // AC1b: list_relationships includes both source and target matches
    #[test]
    fn list_relationships_includes_both_directions() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");
        seed_project(&conn, "p3");

        insert_relationship(&conn, &make_relationship("r1", "p1", "p2", "depends_on")).unwrap();
        insert_relationship(&conn, &make_relationship("r2", "p3", "p1", "references")).unwrap();

        let rels = list_relationships(&conn, "p1").unwrap();
        assert_eq!(rels.len(), 2);
    }

    // AC2: dismiss_relationship hides from list
    #[test]
    fn dismiss_hides_from_list() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        insert_relationship(&conn, &make_relationship("r1", "p1", "p2", "depends_on")).unwrap();
        assert_eq!(list_relationships(&conn, "p1").unwrap().len(), 1);

        let changed = dismiss_relationship(&conn, "r1").unwrap();
        assert!(changed);

        assert_eq!(list_relationships(&conn, "p1").unwrap().len(), 0);
    }

    // AC2b: list_all_relationships includes dismissed
    #[test]
    fn list_all_includes_dismissed() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        insert_relationship(&conn, &make_relationship("r1", "p1", "p2", "depends_on")).unwrap();
        dismiss_relationship(&conn, "r1").unwrap();

        let all = list_all_relationships(&conn, "p1").unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].dismissed);
    }

    // AC3: remove_relationship deletes permanently
    #[test]
    fn remove_relationship_deletes() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        insert_relationship(&conn, &make_relationship("r1", "p1", "p2", "depends_on")).unwrap();
        let removed = remove_relationship(&conn, "r1").unwrap();
        assert!(removed);

        assert_eq!(list_all_relationships(&conn, "p1").unwrap().len(), 0);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let (conn, _tmp) = test_db();
        assert!(!remove_relationship(&conn, "no-such").unwrap());
    }

    // AC4: upsert creates new relationship
    #[test]
    fn upsert_creates_new() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        let id = upsert_relationship(&conn, "p1", "p2", "depends_on", "cargo_toml").unwrap();
        assert!(!id.is_empty());

        let rels = list_relationships(&conn, "p1").unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].id, id);
        assert_eq!(rels[0].relationship_type, "depends_on");
    }

    // AC4b: upsert updates existing relationship's last_seen_at
    #[test]
    fn upsert_updates_existing() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        let mut rel = make_relationship("r1", "p1", "p2", "depends_on");
        rel.last_seen_at = "2026-01-01T00:00:00Z".to_string();
        insert_relationship(&conn, &rel).unwrap();

        let id = upsert_relationship(&conn, "p1", "p2", "depends_on", "cargo_toml").unwrap();
        assert_eq!(id, "r1");

        let rels = list_relationships(&conn, "p1").unwrap();
        assert_eq!(rels.len(), 1);
        // last_seen_at should be updated (newer than original)
        assert_ne!(rels[0].last_seen_at, "2026-01-01T00:00:00Z");
    }

    // AC4c: upsert re-activates dismissed relationship
    #[test]
    fn upsert_reactivates_dismissed() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        insert_relationship(&conn, &make_relationship("r1", "p1", "p2", "depends_on")).unwrap();
        dismiss_relationship(&conn, "r1").unwrap();

        assert_eq!(list_relationships(&conn, "p1").unwrap().len(), 0);

        upsert_relationship(&conn, "p1", "p2", "depends_on", "cargo_toml").unwrap();

        let rels = list_relationships(&conn, "p1").unwrap();
        assert_eq!(rels.len(), 1);
        assert!(!rels[0].dismissed);
    }

    // AC5: unique constraint on (source, target, type) prevents duplicates
    #[test]
    fn unique_constraint_prevents_duplicate() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        insert_relationship(&conn, &make_relationship("r1", "p1", "p2", "depends_on")).unwrap();
        let result = insert_relationship(&conn, &make_relationship("r2", "p1", "p2", "depends_on"));
        assert!(result.is_err(), "Should fail with unique constraint");
    }

    // AC5b: different types for same projects are allowed
    #[test]
    fn different_types_allowed() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        insert_relationship(&conn, &make_relationship("r1", "p1", "p2", "depends_on")).unwrap();
        insert_relationship(&conn, &make_relationship("r2", "p1", "p2", "references")).unwrap();

        let rels = list_relationships(&conn, "p1").unwrap();
        assert_eq!(rels.len(), 2);
    }

    // AC6: remove_stale_auto_relationships cleans up old auto-detected ones
    #[test]
    fn remove_stale_auto_relationships_works() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");
        seed_project(&conn, "p3");

        // Two auto-detected relationships
        insert_relationship(&conn, &make_relationship("r1", "p1", "p2", "depends_on")).unwrap();
        insert_relationship(&conn, &make_relationship("r2", "p1", "p3", "references")).unwrap();

        // Current scan only found (p1, p2, depends_on) — so r2 is stale
        let current = vec![("p1".to_string(), "p2".to_string(), "depends_on".to_string())];

        let removed = remove_stale_auto_relationships(&conn, "p1", &current).unwrap();
        assert_eq!(removed, 1);

        let rels = list_relationships(&conn, "p1").unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].id, "r1");
    }

    // AC6b: remove_stale skips manual relationships
    #[test]
    fn remove_stale_preserves_manual() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        let mut manual_rel = make_relationship("r1", "p1", "p2", "depends_on");
        manual_rel.detection_source = "manual".to_string();
        insert_relationship(&conn, &manual_rel).unwrap();

        // Empty current set — but manual should survive
        let removed = remove_stale_auto_relationships(&conn, "p1", &[]).unwrap();
        assert_eq!(removed, 0);
        assert_eq!(list_relationships(&conn, "p1").unwrap().len(), 1);
    }

    // AC7: relationships cascade-delete when project is removed
    #[test]
    fn relationships_cascade_on_project_delete() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        insert_relationship(&conn, &make_relationship("r1", "p1", "p2", "depends_on")).unwrap();
        assert_eq!(list_relationships(&conn, "p2").unwrap().len(), 1);

        crate::db::queries::delete_project(&conn, "p1").unwrap();

        // Relationship should be gone
        assert_eq!(list_relationships(&conn, "p2").unwrap().len(), 0);
    }

    // AC8: dismiss_nonexistent returns false
    #[test]
    fn dismiss_nonexistent_returns_false() {
        let (conn, _tmp) = test_db();
        assert!(!dismiss_relationship(&conn, "no-such").unwrap());
    }
}
