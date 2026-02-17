-- Add UNIQUE constraint on sessions.file_path for dedup safety
-- and an index for efficient lookups during import.

CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_file_path ON sessions(file_path);
