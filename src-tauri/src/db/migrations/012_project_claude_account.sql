-- Claude subscription (config dir) a project launches on.
-- NULL means "use the global default account" — every existing row.

ALTER TABLE projects ADD COLUMN claude_account_id TEXT;
