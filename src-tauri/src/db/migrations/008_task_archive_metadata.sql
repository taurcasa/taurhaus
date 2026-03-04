-- Add task lifecycle metadata used by snapshot reconciliation/history views.
-- first_seen_at already exists. These columns track status transitions and
-- archival context for tasks that disappear from source snapshots.

ALTER TABLE tasks ADD COLUMN state_changed_at TEXT;
ALTER TABLE tasks ADD COLUMN last_status TEXT;
ALTER TABLE tasks ADD COLUMN archived_reason TEXT;

-- Backfill existing rows for deterministic behavior.
UPDATE tasks
SET state_changed_at = COALESCE(state_changed_at, updated_at),
    last_status = COALESCE(last_status, status);
