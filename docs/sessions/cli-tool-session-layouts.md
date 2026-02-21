# CLI Tool Session Storage Layouts

Research snapshot from Task 1 — examining how each CLI tool stores session data.

## Claude Code

**Base dir:** `~/.claude/projects/`

**Layout:** Project-slug directories containing session files.

```
~/.claude/projects/
  -home-mstie-projects-taurhaus/        # slug = path with / -> -
    7fe74200-b2bb-4385-996f-e9db1b7939fe.jsonl   # session transcript
    7fe74200-b2bb-4385-996f-e9db1b7939fe/         # session dir (may exist)
    f5145e0f-c337-49ae-8f68-8f0507e05656.jsonl
    f5145e0f-c337-49ae-8f68-8f0507e05656/
  -home-mstie-projects-2ksim/
    ...
```

**Project path → session dir mapping:**
- Deterministic: replace `/` with `-` in project path to get slug
- Forward lookup: `path → slug → ls dir`
- Reverse lookup: `slug → replace leading `-` with `/` → path` (lossy but works for absolute paths)

**Session activity detection (idle.rs):**
- Reads last `.jsonl` file modification time
- Compares to `IDLE_THRESHOLD` (60s default)
- Hysteresis: requires N consecutive samples in same state before transitioning

**Key insight:** Claude organizes by project, sessions within project. One dir per project.

---

## Codex CLI

**Base dir:** `~/.codex/sessions/`

**Layout:** TWO formats (migration happened between 2025 and 2026):

### Old format (2025, .json, 640 files)
```
~/.codex/sessions/
  rollout-2025-04-18-009af1f0-4991-46b2-8e1d-153b80eb3314.json
  rollout-2025-04-18-00c017d3-f47f-4940-8de7-653e0f4d17c2.json
  ...
```

Structure: single JSON object with `session.id`, `session.timestamp`, `session.instructions`, `items[]` (messages).
**No `cwd` field in old format.** Project path is NOT stored.

### New format (2026, .jsonl, 46 files)
```
~/.codex/sessions/
  2026/02/21/
    rollout-2026-02-21T17-25-42-019c8105-6f11-7740-be54-d0a404f79eb6.jsonl
  2026/02/12/
    ...
```

Structure: JSONL with typed records:
- `session_meta`: Contains `payload.cwd` (project path!), `payload.id`, `payload.cli_version`, `payload.model_provider`, `payload.base_instructions`, `payload.git.commit_hash`, `payload.git.branch`
- `turn_context`: Contains `payload.cwd`, `payload.model`, `payload.approval_policy`
- `response_item`: Message content (user, assistant, system, developer)
- `event_msg`: Events

**Project path → session mapping:**
- NOT organized by project — organized by DATE
- Must scan `session_meta` lines to find which sessions belong to a project
- `cwd` in `session_meta` and `turn_context` contains the project path

**Other Codex files:**
- `~/.codex/history.jsonl`: User prompts with `session_id` + `ts` + `text`. Chat history, NOT project mapping.
- `~/.codex/history.json` (old): Array of `{command, timestamp}`. No session_id.
- `~/.codex/config.toml`: Global config, has `[projects."path"]` entries for trust level.
- `~/.codex/shell_snapshots/`: Empty in our case.

**Key insight:** Codex is date-organized, not project-organized. Finding sessions for a project requires scanning session_meta records. The `config.toml` `[projects]` table could serve as a project registry.

---

## Gemini CLI

**Base dir:** `~/.gemini/tmp/`

**Layout:** SHA-256 hash of project path as directory name.

```
~/.gemini/tmp/
  a3c1b806c429.../         # SHA-256("/home/mstie/projects/2ksim")
    chats/
      session-2026-02-10T19-57-4574fc66.json
      session-2026-02-10T19-59-62fe9348.json
    logs.json
  eaad0764870a.../         # SHA-256("/home/mstie/projects/missing_invoices_gemini_review")
    chats/
      session-2026-02-13T03-15-c77808b2.json
    logs.json
  bin/                     # Gemini CLI tools
```

**Chat file structure:** JSON with:
- `sessionId`: UUID
- `projectHash`: SHA-256 of project path (matches directory name)
- `startTime`, `lastUpdated`: ISO timestamps
- `messages[]`: Array of `{id, timestamp, type, content}` where type is `"user"` or `"gemini"`
- Gemini messages also have `thoughts[]` (reasoning), `tokens` (usage), `toolCalls[]`

**logs.json:** Array of `{sessionId, messageId, type, message, timestamp}` — flat log of all messages.

**Project path → session dir mapping:**
- Deterministic: `SHA-256(project_path)` → directory name
- Forward lookup: compute hash, check if dir exists, list `chats/`
- Reverse lookup: NOT possible from hash alone. Need `trustedFolders.json` or scan all known project paths.

**trustedFolders.json:** Maps project paths to trust level. Can serve as project registry.
```json
{
  "/home/mstie/projects/taurora_gemini_test": "TRUST_FOLDER",
  "/home/mstie/projects/missing_invoices_gemini_review": "TRUST_FOLDER"
}
```

**Session activity detection:**
- `lastUpdated` in chat JSON gives last activity time
- Could compare to threshold like Claude does

**Key insight:** Gemini is project-organized via hash. Deterministic forward lookup (project → hash → dir). No reverse lookup without external data.

---

## Comparison Matrix

| Aspect | Claude | Codex | Gemini |
|--------|--------|-------|--------|
| **Base dir** | `~/.claude/projects/` | `~/.codex/sessions/` | `~/.gemini/tmp/` |
| **Organization** | By project (slug) | By date (YYYY/MM/DD) | By project (SHA-256) |
| **File format** | JSONL | JSON (old) / JSONL (new) | JSON |
| **Project path in session** | In dir name (slug) | In `session_meta.cwd` (new only) | In `projectHash` (hash, not path) |
| **Forward lookup** (path → sessions) | slug → ls dir | scan all session_meta | SHA-256 → ls dir |
| **Reverse lookup** (session → path) | slug → path | `cwd` field | hash only (need external map) |
| **Activity timestamp** | `.jsonl` mtime | `timestamp` in records | `lastUpdated` in chat JSON |
| **Project registry** | Dir listing | `config.toml [projects]` | `trustedFolders.json` |
| **Session naming** | UUID.jsonl | `rollout-{date}-{uuid}.jsonl` | `session-{date}-{uuid_prefix}.json` |

## Implications for SessionResolver

1. **Forward lookup is universal** — all three tools can find sessions for a known project path
2. **Reverse lookup varies** — Claude is trivial, Codex has it in file content, Gemini needs external data
3. **Activity detection needs per-tool logic** — different timestamp sources
4. **Idle detection for Codex/Gemini** is fundamentally different from Claude:
   - Claude: reads last file mtime in project session dir
   - Codex: would need to find the right session file first (date-organized), then check mtime or content timestamp
   - Gemini: check `lastUpdated` in chat JSON, or dir/file mtime
5. **Old Codex format** has no project path — can't map those sessions to projects at all
