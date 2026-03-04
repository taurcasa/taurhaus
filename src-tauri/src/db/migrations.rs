use rusqlite::Connection;

/// Embedded SQL migration files.  Each entry is `(version, name, sql)`.
/// New migrations are appended — never modify existing ones.
const MIGRATIONS: &[(i64, &str, &str)] = &[
    (1, "initial", include_str!("migrations/001_initial.sql")),
    (
        2,
        "session_file_path_unique",
        include_str!("migrations/002_session_file_path_unique.sql"),
    ),
    (
        3,
        "relationships_unique",
        include_str!("migrations/003_relationships_unique.sql"),
    ),
    (
        4,
        "cached_git_status",
        include_str!("migrations/004_cached_git_status.sql"),
    ),
    (
        5,
        "session_activity",
        include_str!("migrations/005_session_activity.sql"),
    ),
    (6, "tasks", include_str!("migrations/006_tasks.sql")),
    (
        7,
        "task_archived_at",
        include_str!("migrations/007_task_archived_at.sql"),
    ),
    (
        8,
        "task_archive_metadata",
        include_str!("migrations/008_task_archive_metadata.sql"),
    ),
    (
        9,
        "task_source_key_identity",
        include_str!("migrations/009_task_source_key_identity.sql"),
    ),
];

/// Ensure the `_migrations` tracking table exists, then apply any migrations
/// that haven't been run yet.  Safe to call on every app start.
pub fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version  INTEGER PRIMARY KEY NOT NULL,
            name     TEXT NOT NULL,
            applied_at TEXT NOT NULL
        )",
    )?;

    for &(version, name, sql) in MIGRATIONS {
        let already_applied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM _migrations WHERE version = ?1)",
            [version],
            |row| row.get(0),
        )?;

        if !already_applied {
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, datetime('now'))",
                rusqlite::params![version, name],
            )?;
            tracing::info!(version, name, "applied migration");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_sorted_by_version() {
        let versions: Vec<i64> = MIGRATIONS.iter().map(|(v, _, _)| *v).collect();
        let mut sorted = versions.clone();
        sorted.sort();
        assert_eq!(
            versions, sorted,
            "Migrations must be in ascending version order"
        );
    }

    #[test]
    fn all_migrations_have_unique_versions() {
        let mut versions: Vec<i64> = MIGRATIONS.iter().map(|(v, _, _)| *v).collect();
        versions.dedup();
        assert_eq!(
            versions.len(),
            MIGRATIONS.len(),
            "Migration versions must be unique"
        );
    }
}
