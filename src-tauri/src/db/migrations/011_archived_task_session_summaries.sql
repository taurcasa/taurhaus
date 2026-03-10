CREATE TABLE archived_task_session_summaries (
    project_path         TEXT NOT NULL,
    session_key          TEXT NOT NULL,
    session_id           TEXT,
    started_at           TEXT,
    ended_at             TEXT,
    duration_ms          INTEGER,
    commit_count         INTEGER NOT NULL DEFAULT 0,
    file_count           INTEGER NOT NULL DEFAULT 0,
    sources_json         TEXT NOT NULL DEFAULT '[]',
    last_archived_at     TEXT,
    enrichment_warnings  TEXT NOT NULL DEFAULT '[]',
    updated_at           TEXT NOT NULL,
    PRIMARY KEY (project_path, session_key)
);

CREATE INDEX idx_archived_task_session_summaries_project_timeline
    ON archived_task_session_summaries (project_path, last_archived_at DESC, ended_at DESC);
