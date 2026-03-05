-- Improve query plans for session timeline ordering and archived task history.

CREATE INDEX IF NOT EXISTS idx_sessions_project_date_desc
    ON sessions (project_id, date DESC);

CREATE INDEX IF NOT EXISTS idx_tasks_archived_session_timeline
    ON tasks (project_path, session_id, source, source_key, source_task_id)
    WHERE archived_at IS NOT NULL;
