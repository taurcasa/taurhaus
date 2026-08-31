use rusqlite::Connection;

/// Embedded SQL migration files.  Each entry is `(version, name, sql)`.
/// New migrations are appended — never modify existing ones.
pub(crate) const MIGRATIONS: &[(i64, &str, &str)] = &[
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
    (
        10,
        "session_and_task_timeline_indexes",
        include_str!("migrations/010_session_and_task_timeline_indexes.sql"),
    ),
    (
        11,
        "archived_task_session_summaries",
        include_str!("migrations/011_archived_task_session_summaries.sql"),
    ),
    (
        12,
        "project_claude_account",
        include_str!("migrations/012_project_claude_account.sql"),
    ),
    (
        13,
        "project_tool_accounts",
        include_str!("migrations/013_project_tool_accounts.sql"),
    ),
    (
        14,
        "task_assignment_effort",
        include_str!("migrations/014_task_assignment_effort.sql"),
    ),
    (
        15,
        "task_assignment_deadline",
        include_str!("migrations/015_task_assignment_deadline.sql"),
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
            apply_migration(conn, version, name, sql)?;
            tracing::info!(version, name, "applied migration");
        }
    }

    Ok(())
}

/// Migrations that open and close their own transaction.
///
/// SQLite refuses a nested `BEGIN`, and a table rebuild has to turn foreign
/// keys off outside one, so these run exactly as they are written and take
/// responsibility for their own atomicity.
const SELF_MANAGED_TRANSACTION_VERSIONS: &[i64] = &[9];

/// Apply one migration and record it as one unit of work.
///
/// SQLite auto-commits statement by statement, so a batch that stops partway
/// used to leave half a schema behind with no version recorded — and the next
/// start ran the whole batch again and aborted on what the first half had
/// already done. One transaction covers the statements and the version row
/// together, so a migration either happened or did not.
fn apply_migration(
    conn: &Connection,
    version: i64,
    name: &str,
    sql: &str,
) -> Result<(), rusqlite::Error> {
    const RECORD: &str =
        "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, datetime('now'))";

    if SELF_MANAGED_TRANSACTION_VERSIONS.contains(&version) {
        conn.execute_batch(sql)?;
        conn.execute(RECORD, rusqlite::params![version, name])?;
        return Ok(());
    }

    let transaction = conn.unchecked_transaction()?;
    transaction.execute_batch(sql)?;
    transaction.execute(RECORD, rusqlite::params![version, name])?;
    transaction.commit()
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

    // Regression: 2529309 shipped migration 014 as two unconditional
    // `ALTER TABLE ... ADD COLUMN` statements run through `execute_batch`,
    // which SQLite auto-commits one statement at a time, and recorded the
    // version afterwards. A crash between the two left a column added and no
    // version recorded, and the next start aborted for good on the duplicate.
    #[test]
    fn a_migration_that_fails_partway_leaves_neither_schema_nor_version_behind() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE _migrations (version INTEGER PRIMARY KEY NOT NULL, name TEXT NOT NULL, applied_at TEXT NOT NULL);
             CREATE TABLE tasks (id INTEGER PRIMARY KEY);",
        )
        .unwrap();

        let outcome = apply_migration(
            &conn,
            999,
            "half_applied",
            "ALTER TABLE tasks ADD COLUMN effort TEXT;\n\
             ALTER TABLE no_such_table ADD COLUMN effort_why TEXT;",
        );

        assert!(outcome.is_err(), "the second statement cannot succeed");
        let columns: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('tasks')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(
            !columns.iter().any(|column| column == "effort"),
            "a half-applied migration must not leave a column behind: {columns:?}"
        );
        let recorded: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM _migrations WHERE version = 999)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!recorded, "a migration that failed is not recorded");
    }

    #[test]
    fn migration_013_copies_the_legacy_claude_pin() {
        // Regression: commit d6839a3 stored the account only in
        // `projects.claude_account_id`; generic account memory must preserve
        // that 0.6.8 choice without continuing to read the legacy column.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE _migrations (version INTEGER PRIMARY KEY NOT NULL, name TEXT NOT NULL, applied_at TEXT NOT NULL);",
        )
        .unwrap();
        for &(version, name, sql) in &MIGRATIONS[..12] {
            conn.execute_batch(sql).unwrap();
            conn.execute(
                "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, datetime('now'))",
                rusqlite::params![version, name],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO projects (id, name, path, created_at, updated_at, claude_account_id) VALUES ('p1', 'one', '/tmp/one', datetime('now'), datetime('now'), 'account-2')",
            [],
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let memory: (String, String, String) = conn
            .query_row(
                "SELECT tool, account_id, origin FROM project_tool_accounts WHERE project_id = 'p1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            memory,
            ("claude".into(), "account-2".into(), "pinned".into())
        );
    }
}
