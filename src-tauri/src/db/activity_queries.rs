use rusqlite::{params, Connection};
use serde::Serialize;

/// Aggregated activity statistics for a project.
#[derive(Debug, Serialize)]
pub struct ProjectActivityStats {
    pub total_active_ms: i64,
    pub total_duration_ms: i64,
    pub session_count: i64,
    pub last_session_at: Option<String>,
}

/// Record a completed session's activity stats.
pub fn insert_session_activity(
    conn: &Connection,
    project_path: &str,
    cli_tool: &str,
    started_at: &str,
    ended_at: &str,
    active_duration_ms: i64,
    total_duration_ms: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO session_activity (project_path, cli_tool, started_at, ended_at, active_duration_ms, total_duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            project_path,
            cli_tool,
            started_at,
            ended_at,
            active_duration_ms,
            total_duration_ms,
        ],
    )?;
    Ok(())
}

/// Aggregate activity stats for a project path.
pub fn get_project_activity(
    conn: &Connection,
    project_path: &str,
) -> Result<ProjectActivityStats, rusqlite::Error> {
    conn.query_row(
        "SELECT
            COALESCE(SUM(active_duration_ms), 0),
            COALESCE(SUM(total_duration_ms), 0),
            COUNT(*),
            MAX(ended_at)
         FROM session_activity
         WHERE project_path = ?1",
        [project_path],
        |row| {
            Ok(ProjectActivityStats {
                total_active_ms: row.get(0)?,
                total_duration_ms: row.get(1)?,
                session_count: row.get(2)?,
                last_session_at: row.get(3)?,
            })
        },
    )
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

    #[test]
    fn insert_and_aggregate_round_trip() {
        let (conn, _tmp) = test_db();

        insert_session_activity(
            &conn,
            "/projects/foo",
            "claude",
            "2026-02-20T10:00:00Z",
            "2026-02-20T11:00:00Z",
            1_800_000, // 30 min active
            3_600_000, // 60 min total
        )
        .unwrap();

        let stats = get_project_activity(&conn, "/projects/foo").unwrap();
        assert_eq!(stats.total_active_ms, 1_800_000);
        assert_eq!(stats.total_duration_ms, 3_600_000);
        assert_eq!(stats.session_count, 1);
        assert_eq!(
            stats.last_session_at.as_deref(),
            Some("2026-02-20T11:00:00Z")
        );
    }

    #[test]
    fn multiple_sessions_accumulate() {
        let (conn, _tmp) = test_db();

        insert_session_activity(
            &conn,
            "/projects/foo",
            "claude",
            "2026-02-20T10:00:00Z",
            "2026-02-20T11:00:00Z",
            1_800_000,
            3_600_000,
        )
        .unwrap();

        insert_session_activity(
            &conn,
            "/projects/foo",
            "codex",
            "2026-02-20T12:00:00Z",
            "2026-02-20T13:00:00Z",
            2_400_000,
            3_600_000,
        )
        .unwrap();

        let stats = get_project_activity(&conn, "/projects/foo").unwrap();
        assert_eq!(stats.total_active_ms, 4_200_000); // 30 + 40 min
        assert_eq!(stats.total_duration_ms, 7_200_000); // 60 + 60 min
        assert_eq!(stats.session_count, 2);
        assert_eq!(
            stats.last_session_at.as_deref(),
            Some("2026-02-20T13:00:00Z")
        );
    }

    #[test]
    fn different_projects_dont_interfere() {
        let (conn, _tmp) = test_db();

        insert_session_activity(
            &conn,
            "/projects/foo",
            "claude",
            "2026-02-20T10:00:00Z",
            "2026-02-20T11:00:00Z",
            1_800_000,
            3_600_000,
        )
        .unwrap();

        insert_session_activity(
            &conn,
            "/projects/bar",
            "claude",
            "2026-02-20T10:00:00Z",
            "2026-02-20T12:00:00Z",
            5_000_000,
            7_200_000,
        )
        .unwrap();

        let foo_stats = get_project_activity(&conn, "/projects/foo").unwrap();
        assert_eq!(foo_stats.session_count, 1);
        assert_eq!(foo_stats.total_active_ms, 1_800_000);

        let bar_stats = get_project_activity(&conn, "/projects/bar").unwrap();
        assert_eq!(bar_stats.session_count, 1);
        assert_eq!(bar_stats.total_active_ms, 5_000_000);
    }

    #[test]
    fn empty_project_returns_zero_stats() {
        let (conn, _tmp) = test_db();

        let stats = get_project_activity(&conn, "/projects/nonexistent").unwrap();
        assert_eq!(stats.total_active_ms, 0);
        assert_eq!(stats.total_duration_ms, 0);
        assert_eq!(stats.session_count, 0);
        assert!(stats.last_session_at.is_none());
    }
}
