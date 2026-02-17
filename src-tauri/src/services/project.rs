use std::path::Path;

use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

use crate::db::queries;
use crate::errors::AppError;
use crate::git::status as git_status;
use crate::models::{
    ActivityState, ActivityThresholds, Project, ProjectDetail, ProjectSummary,
};

/// Register a new project from a filesystem path.
/// Validates that the path exists and contains a `.git` directory.
pub fn register_project(
    conn: &Connection,
    path: &str,
    name: Option<&str>,
) -> Result<ProjectDetail, AppError> {
    let dir = Path::new(path);

    if !dir.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Path does not exist or is not a directory: {path}"
        )));
    }

    let now = Utc::now().to_rfc3339();
    let project_name = name
        .map(|n| n.to_string())
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unnamed".to_string());

    let project = Project {
        id: Uuid::new_v4().to_string(),
        name: project_name,
        path: path.to_string(),
        description: None,
        last_activity_at: Some(now.clone()),
        hero_preference: None,
        created_at: now.clone(),
        updated_at: now,
    };

    queries::insert_project(conn, &project).map_err(|e| {
        if let rusqlite::Error::SqliteFailure(err, _) = &e {
            if err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE {
                return AppError::AlreadyExists(format!(
                    "Project already registered at path: {path}"
                ));
            }
        }
        AppError::Database(e)
    })?;

    let thresholds = ActivityThresholds::default();
    Ok(to_project_detail(&project, &thresholds))
}

/// List all registered projects with computed activity states.
pub fn list_projects(
    conn: &Connection,
    thresholds: &ActivityThresholds,
) -> Result<Vec<ProjectSummary>, AppError> {
    let projects = queries::list_projects(conn)?;
    let now = Utc::now();

    Ok(projects
        .iter()
        .map(|p| {
            let mut summary = ProjectSummary::from_project(p, thresholds, now);
            if let Ok(status) = git_status::get_status(Path::new(&p.path)) {
                summary.branch = status.branch;
                summary.is_dirty = Some(status.is_dirty);
            }
            summary
        })
        .collect())
}

/// Get full project details by UUID.
pub fn get_project(
    conn: &Connection,
    id: &str,
    thresholds: &ActivityThresholds,
) -> Result<ProjectDetail, AppError> {
    let project = queries::get_project(conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {id}")))?;

    Ok(to_project_detail(&project, thresholds))
}

/// Update a project's mutable fields.
pub fn update_project(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    description: Option<Option<&str>>,
    hero_preference: Option<Option<&str>>,
) -> Result<(), AppError> {
    let changed = queries::update_project(conn, id, name, None, description, None, hero_preference)?;
    if !changed {
        return Err(AppError::NotFound(format!("Project not found: {id}")));
    }
    Ok(())
}

/// Remove a project and all associated data (sessions, relationships via CASCADE).
pub fn remove_project(conn: &Connection, id: &str) -> Result<(), AppError> {
    let deleted = queries::delete_project(conn, id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("Project not found: {id}")));
    }
    Ok(())
}

fn to_project_detail(project: &Project, thresholds: &ActivityThresholds) -> ProjectDetail {
    let now = Utc::now();
    let (branch, is_dirty) = git_status::get_status(Path::new(&project.path))
        .map(|s| (s.branch, Some(s.is_dirty)))
        .unwrap_or((None, None));

    ProjectDetail {
        id: project.id.clone(),
        name: project.name.clone(),
        path: project.path.clone(),
        description: project.description.clone(),
        activity_state: ActivityState::compute(
            project.last_activity_at.as_deref(),
            thresholds,
            now,
        ),
        last_activity_at: project.last_activity_at.clone(),
        hero_preference: project.hero_preference.clone(),
        created_at: project.created_at.clone(),
        updated_at: project.updated_at.clone(),
        branch,
        is_dirty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use pretty_assertions::assert_eq;
    use tempfile::{NamedTempFile, TempDir};

    fn test_db() -> (Connection, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();
        (conn, tmp)
    }

    /// Create a temp directory that looks like a git repo.
    fn temp_project_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        dir
    }

    fn default_thresholds() -> ActivityThresholds {
        ActivityThresholds::default()
    }

    // AC-1: register_project with a valid path creates a project
    #[test]
    fn register_project_creates_project() {
        let (conn, _db) = test_db();
        let dir = temp_project_dir();

        let detail = register_project(&conn, dir.path().to_str().unwrap(), None).unwrap();

        assert!(!detail.id.is_empty());
        assert_eq!(detail.path, dir.path().to_str().unwrap());
        assert_eq!(detail.activity_state, ActivityState::Active);
        assert!(!detail.created_at.is_empty());
    }

    // AC-1: register_project uses directory name as default project name
    #[test]
    fn register_project_uses_dir_name() {
        let (conn, _db) = test_db();
        let dir = temp_project_dir();
        let expected_name = dir.path().file_name().unwrap().to_str().unwrap().to_string();

        let detail = register_project(&conn, dir.path().to_str().unwrap(), None).unwrap();
        assert_eq!(detail.name, expected_name);
    }

    // AC-1: register_project with custom name
    #[test]
    fn register_project_custom_name() {
        let (conn, _db) = test_db();
        let dir = temp_project_dir();

        let detail =
            register_project(&conn, dir.path().to_str().unwrap(), Some("my-project")).unwrap();
        assert_eq!(detail.name, "my-project");
    }

    // AC-2: register_project with non-existent path returns error
    #[test]
    fn register_project_nonexistent_path() {
        let (conn, _db) = test_db();
        let result = register_project(&conn, "/nonexistent/path/to/nowhere", None);

        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidPath(msg) => assert!(msg.contains("/nonexistent")),
            e => panic!("Expected InvalidPath, got: {e:?}"),
        }
    }

    // AC-3: register_project with already-registered path returns error
    #[test]
    fn register_project_duplicate_path() {
        let (conn, _db) = test_db();
        let dir = temp_project_dir();
        let path = dir.path().to_str().unwrap();

        register_project(&conn, path, None).unwrap();
        let result = register_project(&conn, path, None);

        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::AlreadyExists(msg) => assert!(msg.contains(path)),
            e => panic!("Expected AlreadyExists, got: {e:?}"),
        }
    }

    // AC-4: list_projects sorted by last_activity_at with activity states
    #[test]
    fn list_projects_sorted_with_states() {
        let (conn, _db) = test_db();
        let dir1 = temp_project_dir();
        let dir2 = temp_project_dir();

        register_project(&conn, dir1.path().to_str().unwrap(), Some("project-1")).unwrap();
        register_project(&conn, dir2.path().to_str().unwrap(), Some("project-2")).unwrap();

        let projects = list_projects(&conn, &default_thresholds()).unwrap();
        assert_eq!(projects.len(), 2);
        // Both just registered, so both active
        assert_eq!(projects[0].activity_state, ActivityState::Active);
        assert_eq!(projects[1].activity_state, ActivityState::Active);
    }

    // AC-5: list_projects with no projects returns empty
    #[test]
    fn list_projects_empty() {
        let (conn, _db) = test_db();
        let projects = list_projects(&conn, &default_thresholds()).unwrap();
        assert!(projects.is_empty());
    }

    // AC-6: get_project returns ProjectDetail with activity state
    #[test]
    fn get_project_returns_detail() {
        let (conn, _db) = test_db();
        let dir = temp_project_dir();

        let created = register_project(&conn, dir.path().to_str().unwrap(), Some("test")).unwrap();
        let fetched = get_project(&conn, &created.id, &default_thresholds()).unwrap();

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "test");
        assert_eq!(fetched.activity_state, ActivityState::Active);
    }

    // AC-7: get_project with invalid UUID returns NotFound
    #[test]
    fn get_project_not_found() {
        let (conn, _db) = test_db();
        let result = get_project(&conn, "nonexistent-uuid", &default_thresholds());

        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::NotFound(msg) => assert!(msg.contains("nonexistent-uuid")),
            e => panic!("Expected NotFound, got: {e:?}"),
        }
    }

    // AC-8: update_project modifies fields and bumps updated_at
    #[test]
    fn update_project_modifies_fields() {
        let (conn, _db) = test_db();
        let dir = temp_project_dir();

        let created =
            register_project(&conn, dir.path().to_str().unwrap(), Some("original")).unwrap();

        update_project(
            &conn,
            &created.id,
            Some("renamed"),
            Some(Some("A description")),
            Some(Some("readme")),
        )
        .unwrap();

        let fetched = get_project(&conn, &created.id, &default_thresholds()).unwrap();
        assert_eq!(fetched.name, "renamed");
        assert_eq!(fetched.description, Some("A description".into()));
        assert_eq!(fetched.hero_preference, Some("readme".into()));
        assert_ne!(fetched.updated_at, created.updated_at);
    }

    // AC-9: remove_project deletes project (cascades to sessions/relationships)
    #[test]
    fn remove_project_deletes() {
        let (conn, _db) = test_db();
        let dir = temp_project_dir();

        let created = register_project(&conn, dir.path().to_str().unwrap(), None).unwrap();
        remove_project(&conn, &created.id).unwrap();

        let result = get_project(&conn, &created.id, &default_thresholds());
        assert!(result.is_err());
    }

    // AC-10: Typed errors
    #[test]
    fn errors_are_typed() {
        let (conn, _db) = test_db();

        // InvalidPath
        let err = register_project(&conn, "/no/such/path", None).unwrap_err();
        assert!(matches!(err, AppError::InvalidPath(_)));

        // NotFound
        let err = get_project(&conn, "bad-id", &default_thresholds()).unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));

        let err = remove_project(&conn, "bad-id").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }
}
