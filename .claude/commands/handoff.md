---
name: handoff
description: Create a session handoff document for taurhaus to import
allowed-tools:
  - Read
  - Write
  - Bash
  - Glob
  - Grep
---

# Session Handoff

Create a structured session handoff that taurhaus can automatically import. This produces two files in `docs/sessions/` that capture the current session's context.

## Instructions

### Step 1: Gather Context

Analyze the current conversation to extract:
- **Summary**: What was accomplished in this session (2-3 concise sentences)
- **Next steps**: 3-5 specific, actionable items for the next session
- **Open questions**: Any unresolved decisions or questions (omit if none)
- **Key context**: Important decisions, changes made, and technical details

### Step 2: Determine Metadata

Use Bash to gather:
```bash
# Current timestamp for filename
date -u +"%Y-%m-%dT%H-%M-%S"

# ISO date for frontmatter
date -u +"%Y-%m-%dT%H:%M:%SZ"

# Project name
basename "$(pwd)"

# Current git branch
git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown"
```

### Step 3: Create Output Directory

```bash
mkdir -p docs/sessions
```

### Step 4: Write Handoff Markdown

Write to `docs/sessions/session-{TIMESTAMP}.md` using this **exact** YAML frontmatter format:

```markdown
---
date: {ISO_DATE}
project: {PROJECT_NAME}
session_id: {generate a UUID or use session context}
summary: >
  {2-3 sentence summary of what was accomplished}
next_steps:
  - {specific actionable step 1}
  - {specific actionable step 2}
  - {specific actionable step 3}
open_questions:
  - {unresolved question, or omit this field if none}
metadata:
  branch: {current git branch}
  exit_reason: manual_handoff
---

## Session Notes

{1-3 paragraphs capturing key decisions, changes made, important context,
and anything someone needs to know to continue this work effectively}
```

### Step 5: Write Metadata Sidecar

Write to `docs/sessions/session-{TIMESTAMP}.meta.json`:

```json
{
  "session_id": "{same session_id as markdown}",
  "ended_at": "{ISO_DATE}",
  "exit_reason": "manual_handoff",
  "model": "{model being used, e.g. claude-opus-4-6}",
  "tools_used": {},
  "files_modified": [],
  "tokens": {}
}
```

### Step 6: Confirm

After writing both files, report:
- The paths of both files created
- A brief preview of the summary

## Format Requirements

- YAML frontmatter MUST be valid YAML (parseable by serde_yaml)
- JSON sidecar MUST be valid JSON
- `date` field must be ISO 8601 format
- `next_steps` must be a YAML list (not inline)
- `summary` should use YAML folded scalar (`>`) for multi-line
- Filename uses dashes not colons: `session-YYYY-MM-DDTHH-MM-SS`

## Important

- This skill is the **manual fallback** for the automatic SessionEnd hook
- Both produce identical file formats that taurhaus imports
- Multiple handoffs per session are fine (each gets a unique timestamp)
- taurhaus watches `docs/sessions/` and auto-imports new files
