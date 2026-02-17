use rusqlite::{params, Connection, OptionalExtension};

use crate::models::Project;

/// Insert a new project.  The caller provides an already-populated `Project`
/// struct (with id, created_at, updated_at already set).
pub fn insert_project(conn: &Connection, project: &Project) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO projects (id, name, path, description, last_activity_at, hero_preference, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            project.id,
            project.name,
            project.path,
            project.description,
            project.last_activity_at,
            project.hero_preference,
            project.created_at,
            project.updated_at,
        ],
    )?;
    Ok(())
}

/// Retrieve a project by its UUID.  Returns `None` if not found.
pub fn get_project(conn: &Connection, id: &str) -> Result<Option<Project>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, name, path, description, last_activity_at, hero_preference, created_at, updated_at
         FROM projects WHERE id = ?1",
        [id],
        |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                description: row.get(3)?,
                last_activity_at: row.get(4)?,
                hero_preference: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        },
    )
    .optional()
}

/// List all projects, most recently active first.
pub fn list_projects(conn: &Connection) -> Result<Vec<Project>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, path, description, last_activity_at, hero_preference, created_at, updated_at
         FROM projects
         ORDER BY last_activity_at DESC NULLS LAST",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            description: row.get(3)?,
            last_activity_at: row.get(4)?,
            hero_preference: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;

    rows.collect()
}

/// Update a project's mutable fields.  Only non-`None` fields are changed.
/// Always bumps `updated_at` to `now`.
pub fn update_project(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    path: Option<&str>,
    description: Option<Option<&str>>,
    last_activity_at: Option<Option<&str>>,
    hero_preference: Option<Option<&str>>,
) -> Result<bool, rusqlite::Error> {
    // Build the SET clause dynamically to only touch provided fields.
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(v) = name {
        sets.push("name = ?".into());
        values.push(Box::new(v.to_string()));
    }
    if let Some(v) = path {
        sets.push("path = ?".into());
        values.push(Box::new(v.to_string()));
    }
    if let Some(v) = description {
        sets.push("description = ?".into());
        values.push(Box::new(v.map(|s| s.to_string())));
    }
    if let Some(v) = last_activity_at {
        sets.push("last_activity_at = ?".into());
        values.push(Box::new(v.map(|s| s.to_string())));
    }
    if let Some(v) = hero_preference {
        sets.push("hero_preference = ?".into());
        values.push(Box::new(v.map(|s| s.to_string())));
    }

    // Always update updated_at.
    sets.push("updated_at = datetime('now')".into());

    let sql = format!("UPDATE projects SET {} WHERE id = ?", sets.join(", "));
    values.push(Box::new(id.to_string()));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|b| b.as_ref()).collect();
    let changed = conn.execute(&sql, params_refs.as_slice())?;
    Ok(changed > 0)
}

/// Delete a project by UUID.  Returns `true` if a row was actually deleted.
pub fn delete_project(conn: &Connection, id: &str) -> Result<bool, rusqlite::Error> {
    let changed = conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use pretty_assertions::assert_eq;
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
            last_activity_at: Some("2025-01-15T10:00:00Z".to_string()),
            hero_preference: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    // AC-3: insert_project creates a project and returns it
    #[test]
    fn insert_and_get_project() {
        let (conn, _tmp) = test_db();
        let project = make_project("p1", "taurhaus", "/home/user/taurhaus");

        insert_project(&conn, &project).unwrap();
        let fetched = get_project(&conn, "p1").unwrap().expect("project should exist");

        assert_eq!(fetched.id, "p1");
        assert_eq!(fetched.name, "taurhaus");
        assert_eq!(fetched.path, "/home/user/taurhaus");
    }

    // AC-4: get_project retrieves all fields
    #[test]
    fn get_project_returns_all_fields() {
        let (conn, _tmp) = test_db();
        let project = Project {
            id: "p2".into(),
            name: "my-project".into(),
            path: "/projects/my-project".into(),
            description: Some("A cool project".into()),
            last_activity_at: Some("2025-06-01T12:00:00Z".into()),
            hero_preference: Some("session".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-03-15T08:00:00Z".into(),
        };

        insert_project(&conn, &project).unwrap();
        let fetched = get_project(&conn, "p2").unwrap().unwrap();

        assert_eq!(fetched.description, Some("A cool project".into()));
        assert_eq!(fetched.last_activity_at, Some("2025-06-01T12:00:00Z".into()));
        assert_eq!(fetched.hero_preference, Some("session".into()));
        assert_eq!(fetched.created_at, "2025-01-01T00:00:00Z");
        assert_eq!(fetched.updated_at, "2025-03-15T08:00:00Z");
    }

    // AC-4 (missing case)
    #[test]
    fn get_nonexistent_project_returns_none() {
        let (conn, _tmp) = test_db();
        let result = get_project(&conn, "no-such-id").unwrap();
        assert!(result.is_none());
    }

    // AC-5: list_projects sorted by last_activity_at descending
    #[test]
    fn list_projects_sorted_by_activity() {
        let (conn, _tmp) = test_db();

        let mut p1 = make_project("p1", "old", "/old");
        p1.last_activity_at = Some("2025-01-01T00:00:00Z".into());

        let mut p2 = make_project("p2", "new", "/new");
        p2.last_activity_at = Some("2025-06-01T00:00:00Z".into());

        let mut p3 = make_project("p3", "mid", "/mid");
        p3.last_activity_at = Some("2025-03-01T00:00:00Z".into());

        insert_project(&conn, &p1).unwrap();
        insert_project(&conn, &p2).unwrap();
        insert_project(&conn, &p3).unwrap();

        let projects = list_projects(&conn).unwrap();
        let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["new", "mid", "old"]);
    }

    // AC-6: update_project modifies fields and bumps updated_at
    #[test]
    fn update_project_changes_name() {
        let (conn, _tmp) = test_db();
        let project = make_project("p1", "original", "/path");
        insert_project(&conn, &project).unwrap();

        let changed = update_project(&conn, "p1", Some("renamed"), None, None, None, None).unwrap();
        assert!(changed);

        let fetched = get_project(&conn, "p1").unwrap().unwrap();
        assert_eq!(fetched.name, "renamed");
        // updated_at should have been bumped
        assert_ne!(fetched.updated_at, project.updated_at);
    }

    // AC-7: delete_project removes the project
    #[test]
    fn delete_project_removes_row() {
        let (conn, _tmp) = test_db();
        let project = make_project("p1", "doomed", "/doomed");
        insert_project(&conn, &project).unwrap();

        let deleted = delete_project(&conn, "p1").unwrap();
        assert!(deleted);

        let fetched = get_project(&conn, "p1").unwrap();
        assert!(fetched.is_none());
    }

    #[test]
    fn delete_nonexistent_returns_false() {
        let (conn, _tmp) = test_db();
        let deleted = delete_project(&conn, "no-such").unwrap();
        assert!(!deleted);
    }

    // AC-9: path uniqueness enforced
    #[test]
    fn duplicate_path_is_rejected() {
        let (conn, _tmp) = test_db();
        let p1 = make_project("p1", "first", "/same/path");
        let p2 = make_project("p2", "second", "/same/path");

        insert_project(&conn, &p1).unwrap();
        let result = insert_project(&conn, &p2);
        assert!(result.is_err(), "Should fail with unique constraint on path");
    }
}
