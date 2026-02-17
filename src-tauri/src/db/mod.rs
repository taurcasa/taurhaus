pub mod migrations;
pub mod queries;
pub mod session_queries;

use std::path::Path;

use rusqlite::Connection;

use crate::db::migrations::run_migrations;

/// Open (or create) the SQLite database at the given path and run all pending
/// migrations.  Returns an open connection ready for queries.
pub fn init_db(path: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path)?;

    // Enable WAL mode for better concurrent read performance.
    conn.pragma_update(None, "journal_mode", "WAL")?;

    // Enforce foreign key constraints — SQLite has them off by default.
    conn.pragma_update(None, "foreign_keys", "ON")?;

    run_migrations(&conn)?;

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn init_db_creates_all_tables() {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '\\__%' ESCAPE '\\' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(tables, vec!["projects", "relationships", "sessions", "settings"]);
    }

    #[test]
    fn init_db_is_idempotent() {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();
        drop(conn);

        // Opening again should not fail — migrations already applied.
        let conn2 = init_db(tmp.path()).unwrap();

        let count: i64 = conn2
            .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();

        let result = conn.execute(
            "INSERT INTO sessions (id, project_id, date, summary, file_path, created_at)
             VALUES ('s1', 'nonexistent', '2025-01-01', 'test', '/test', '2025-01-01')",
            [],
        );

        assert!(result.is_err(), "Should fail with FK constraint violation");
    }
}
