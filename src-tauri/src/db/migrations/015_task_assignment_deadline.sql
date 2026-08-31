-- Deadline in minutes from `mesh task create` / `mesh task assign` metadata.
-- NULL keeps tasks without the managed-stage contract inert.

ALTER TABLE tasks ADD COLUMN deadline_minutes INTEGER;
