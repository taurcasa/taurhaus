-- Initial schema: projects, sessions, relationships, settings

CREATE TABLE IF NOT EXISTS projects (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    path            TEXT NOT NULL UNIQUE,
    description     TEXT,
    last_activity_at TEXT,
    hero_preference TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id              TEXT PRIMARY KEY NOT NULL,
    project_id      TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    date            TEXT NOT NULL,
    summary         TEXT NOT NULL,
    next_steps      TEXT,
    open_questions  TEXT,
    metadata        TEXT,
    file_path       TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS relationships (
    id                  TEXT PRIMARY KEY NOT NULL,
    source_project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    target_project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    relationship_type   TEXT NOT NULL,
    detection_source    TEXT NOT NULL,
    dismissed           INTEGER NOT NULL DEFAULT 0,
    first_detected_at   TEXT NOT NULL,
    last_seen_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key     TEXT PRIMARY KEY NOT NULL,
    value   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_project_id ON sessions(project_id);
CREATE INDEX IF NOT EXISTS idx_sessions_date ON sessions(date);
CREATE INDEX IF NOT EXISTS idx_relationships_source ON relationships(source_project_id);
CREATE INDEX IF NOT EXISTS idx_relationships_target ON relationships(target_project_id);
CREATE INDEX IF NOT EXISTS idx_projects_last_activity ON projects(last_activity_at);
