-- Archive completed tasks instead of deleting them when they go stale.
-- When a scan no longer includes a previously-completed task, we set
-- archived_at instead of deleting the row. Non-completed stale tasks
-- are still hard-deleted.

ALTER TABLE tasks ADD COLUMN archived_at TEXT;
