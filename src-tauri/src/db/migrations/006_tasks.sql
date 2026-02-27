-- Persistent task storage for compound task board.
-- Tasks are imported from Claude Code, Codex, and Gemini session data
-- and persisted so they survive session cleanup.

CREATE TABLE tasks (
    -- Composite natural key: project + source + source-specific ID
    project_path   TEXT NOT NULL,
    source         TEXT NOT NULL,  -- 'claude', 'codex', 'gemini'
    source_task_id TEXT NOT NULL,  -- Original ID within the source (e.g. '1', 'codex-0', 'todo-3')

    subject        TEXT NOT NULL,
    description    TEXT,
    active_form    TEXT,
    status         TEXT NOT NULL DEFAULT 'pending',  -- 'pending', 'in_progress', 'completed'
    blocks         TEXT NOT NULL DEFAULT '[]',        -- JSON array of source_task_ids
    blocked_by     TEXT NOT NULL DEFAULT '[]',        -- JSON array of source_task_ids
    owner          TEXT,

    -- Session attribution
    session_id     TEXT,           -- Claude session UUID, Codex JSONL filename, etc.

    -- Timestamps
    first_seen_at  TEXT NOT NULL,
    updated_at     TEXT NOT NULL,

    PRIMARY KEY (project_path, source, source_task_id)
);

CREATE INDEX idx_tasks_project ON tasks (project_path);
CREATE INDEX idx_tasks_project_source ON tasks (project_path, source);
