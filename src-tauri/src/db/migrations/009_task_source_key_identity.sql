-- Add source_key identity dimension for task disambiguation and rebuild
-- uniqueness so different Claude source directories cannot collide.

PRAGMA foreign_keys=OFF;

BEGIN TRANSACTION;

ALTER TABLE tasks RENAME TO tasks_legacy_009;

CREATE TABLE tasks (
    row_id            INTEGER PRIMARY KEY AUTOINCREMENT,
    project_path      TEXT NOT NULL,
    source            TEXT NOT NULL,
    source_key        TEXT NOT NULL,
    source_task_id    TEXT NOT NULL,
    subject           TEXT NOT NULL,
    description       TEXT,
    active_form       TEXT,
    status            TEXT NOT NULL DEFAULT 'pending',
    blocks            TEXT NOT NULL DEFAULT '[]',
    blocked_by        TEXT NOT NULL DEFAULT '[]',
    owner             TEXT,
    session_id        TEXT,
    first_seen_at     TEXT NOT NULL,
    state_changed_at  TEXT,
    updated_at        TEXT NOT NULL,
    archived_at       TEXT,
    last_status       TEXT,
    archived_reason   TEXT
);

INSERT INTO tasks (
    project_path,
    source,
    source_key,
    source_task_id,
    subject,
    description,
    active_form,
    status,
    blocks,
    blocked_by,
    owner,
    session_id,
    first_seen_at,
    state_changed_at,
    updated_at,
    archived_at,
    last_status,
    archived_reason
)
SELECT
    project_path,
    source,
    CASE
        WHEN source = 'claude' THEN COALESCE(session_id, 'legacy-claude')
        WHEN source = 'codex' THEN 'legacy-codex'
        WHEN source = 'gemini' THEN 'legacy-gemini'
        ELSE 'legacy-' || source
    END AS source_key,
    source_task_id,
    subject,
    description,
    active_form,
    status,
    blocks,
    blocked_by,
    owner,
    session_id,
    first_seen_at,
    COALESCE(state_changed_at, updated_at) AS state_changed_at,
    updated_at,
    archived_at,
    COALESCE(last_status, status) AS last_status,
    archived_reason
FROM tasks_legacy_009;

DROP TABLE tasks_legacy_009;

CREATE INDEX idx_tasks_project ON tasks (project_path);
CREATE INDEX idx_tasks_project_source ON tasks (project_path, source);
CREATE INDEX idx_tasks_project_source_key ON tasks (project_path, source, source_key);
CREATE UNIQUE INDEX idx_tasks_identity_active
    ON tasks (project_path, source, source_key, source_task_id)
    WHERE archived_at IS NULL;
CREATE INDEX idx_tasks_archived_timeline
    ON tasks (project_path, archived_at DESC, session_id, source, source_key, source_task_id);

COMMIT;

PRAGMA foreign_keys=ON;
