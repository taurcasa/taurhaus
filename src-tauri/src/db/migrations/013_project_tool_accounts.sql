CREATE TABLE project_tool_accounts (
    project_id TEXT NOT NULL,
    tool TEXT NOT NULL,
    account_id TEXT NOT NULL,
    origin TEXT NOT NULL CHECK(origin IN ('pinned', 'last_used')),
    updated_at TEXT NOT NULL,
    PRIMARY KEY(project_id, tool),
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

INSERT INTO project_tool_accounts (project_id, tool, account_id, origin, updated_at)
SELECT id, 'claude', claude_account_id, 'pinned', datetime('now')
FROM projects
WHERE claude_account_id IS NOT NULL;
