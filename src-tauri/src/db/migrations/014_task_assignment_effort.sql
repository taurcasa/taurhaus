-- The effort a lead attached to an assignment, and the reason for it.
--
-- `mesh task assign` requires both and writes them onto the task record it
-- persists, so the board can show what was asked for without reading the
-- assignment notice. Both stay NULL for a task no lead assigned and for every
-- source with no assignment contract.

ALTER TABLE tasks ADD COLUMN effort TEXT;
ALTER TABLE tasks ADD COLUMN effort_why TEXT;
