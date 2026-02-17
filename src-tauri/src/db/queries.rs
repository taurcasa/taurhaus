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
        "SELECT id, name, path, description, last_activity_at, hero_preference, created_at, updated_at,
                cached_branch, cached_is_dirty
         FROM projects WHERE id = ?1",
        [id],
        |row| {
            let dirty_int: Option<i32> = row.get(9)?;
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                description: row.get(3)?,
                last_activity_at: row.get(4)?,
                hero_preference: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                cached_branch: row.get(8)?,
                cached_is_dirty: dirty_int.map(|v| v != 0),
            })
        },
    )
    .optional()
}

/// Count total registered projects.
pub fn project_count(conn: &Connection) -> Result<i64, rusqlite::Error> {
    conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
}

/// List all projects, most recently active first.
pub fn list_projects(conn: &Connection) -> Result<Vec<Project>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, path, description, last_activity_at, hero_preference, created_at, updated_at,
                cached_branch, cached_is_dirty
         FROM projects
         ORDER BY last_activity_at DESC NULLS LAST",
    )?;

    let rows = stmt.query_map([], |row| {
        let dirty_int: Option<i32> = row.get(9)?;
        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            description: row.get(3)?,
            last_activity_at: row.get(4)?,
            hero_preference: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            cached_branch: row.get(8)?,
            cached_is_dirty: dirty_int.map(|v| v != 0),
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

/// Bump a project's `last_activity_at` to now. Returns `true` if the project exists.
pub fn touch_project_activity(conn: &Connection, id: &str) -> Result<bool, rusqlite::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let changed = conn.execute(
        "UPDATE projects SET last_activity_at = ?1, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )?;
    Ok(changed > 0)
}

/// Delete a project by UUID.  Returns `true` if a row was actually deleted.
pub fn delete_project(conn: &Connection, id: &str) -> Result<bool, rusqlite::Error> {
    let changed = conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(changed > 0)
}

/// Update the cached git status columns for a project.
pub fn update_cached_git_status(
    conn: &Connection,
    id: &str,
    branch: Option<&str>,
    is_dirty: bool,
) -> Result<bool, rusqlite::Error> {
    let changed = conn.execute(
        "UPDATE projects SET cached_branch = ?1, cached_is_dirty = ?2 WHERE id = ?3",
        params![branch, is_dirty as i32, id],
    )?;
    Ok(changed > 0)
}

/// Check if a project is registered at the given path.
pub fn project_exists_at_path(conn: &Connection, path: &str) -> Result<bool, rusqlite::Error> {
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM projects WHERE path = ?1", [path], |row| {
            row.get(0)
        })?;
    Ok(count > 0)
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
            cached_branch: None,
            cached_is_dirty: None,
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
            cached_branch: None,
            cached_is_dirty: None,
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

    // touch_project_activity bumps last_activity_at
    #[test]
    fn touch_activity_bumps_timestamp() {
        let (conn, _tmp) = test_db();
        let mut project = make_project("p1", "test", "/path");
        project.last_activity_at = Some("2020-01-01T00:00:00+00:00".to_string());
        insert_project(&conn, &project).unwrap();

        let before = get_project(&conn, "p1").unwrap().unwrap();
        assert_eq!(before.last_activity_at, Some("2020-01-01T00:00:00+00:00".into()));

        let touched = touch_project_activity(&conn, "p1").unwrap();
        assert!(touched);

        let after = get_project(&conn, "p1").unwrap().unwrap();
        assert_ne!(after.last_activity_at, before.last_activity_at);
        assert_ne!(after.updated_at, before.updated_at);
    }

    #[test]
    fn touch_activity_nonexistent_returns_false() {
        let (conn, _tmp) = test_db();
        let touched = touch_project_activity(&conn, "no-such").unwrap();
        assert!(!touched);
    }

    #[test]
    fn project_count_empty_db() {
        let (conn, _tmp) = test_db();
        assert_eq!(project_count(&conn).unwrap(), 0);
    }

    #[test]
    fn project_count_with_projects() {
        let (conn, _tmp) = test_db();
        insert_project(&conn, &make_project("p1", "a", "/a")).unwrap();
        insert_project(&conn, &make_project("p2", "b", "/b")).unwrap();
        assert_eq!(project_count(&conn).unwrap(), 2);
    }

    #[test]
    fn project_exists_at_path_true() {
        let (conn, _tmp) = test_db();
        let project = make_project("p1", "test", "/home/user/test");
        insert_project(&conn, &project).unwrap();
        assert!(project_exists_at_path(&conn, "/home/user/test").unwrap());
    }

    #[test]
    fn project_exists_at_path_false() {
        let (conn, _tmp) = test_db();
        assert!(!project_exists_at_path(&conn, "/no/such/path").unwrap());
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

    #[test]
    fn update_cached_git_status_writes_and_reads() {
        let (conn, _tmp) = test_db();
        let project = make_project("p1", "test", "/path");
        insert_project(&conn, &project).unwrap();

        // Initially NULL
        let fetched = get_project(&conn, "p1").unwrap().unwrap();
        assert!(fetched.cached_branch.is_none());
        assert!(fetched.cached_is_dirty.is_none());

        // Update cache
        let ok = update_cached_git_status(&conn, "p1", Some("main"), false).unwrap();
        assert!(ok);

        let fetched = get_project(&conn, "p1").unwrap().unwrap();
        assert_eq!(fetched.cached_branch, Some("main".to_string()));
        assert_eq!(fetched.cached_is_dirty, Some(false));

        // Update again (dirty)
        update_cached_git_status(&conn, "p1", Some("feature"), true).unwrap();
        let fetched = get_project(&conn, "p1").unwrap().unwrap();
        assert_eq!(fetched.cached_branch, Some("feature".to_string()));
        assert_eq!(fetched.cached_is_dirty, Some(true));
    }

    #[test]
    fn update_cached_git_status_nonexistent_returns_false() {
        let (conn, _tmp) = test_db();
        let ok = update_cached_git_status(&conn, "no-such", Some("main"), false).unwrap();
        assert!(!ok);
    }

    #[test]
    fn cached_git_data_appears_in_list_projects() {
        let (conn, _tmp) = test_db();
        let project = make_project("p1", "test", "/path");
        insert_project(&conn, &project).unwrap();
        update_cached_git_status(&conn, "p1", Some("develop"), true).unwrap();

        let projects = list_projects(&conn).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].cached_branch, Some("develop".to_string()));
        assert_eq!(projects[0].cached_is_dirty, Some(true));
    }
}
