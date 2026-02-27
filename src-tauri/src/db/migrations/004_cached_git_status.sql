-- Cache git status in the projects table so list_projects doesn't need I/O.
-- NULL means "not yet scanned" — displayed gracefully in the frontend.

ALTER TABLE projects ADD COLUMN cached_branch TEXT;
ALTER TABLE projects ADD COLUMN cached_is_dirty INTEGER;
