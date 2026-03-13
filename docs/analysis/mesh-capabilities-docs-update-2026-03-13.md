# Mesh Capabilities Docs Update — 2026-03-13

**Task:** #1261
**Scope:** Update operator-facing docs across both repos to match the current
Mesh command surface. Consolidates gaps identified in tasks #1247 and #1250.

---

## Changes Applied

### 1. Mesh USAGE.md — New "Lead-Admin Operations" section

Added a full section (after task lifecycle examples, before tmux communication
guidance) covering:

- **Reassignment** — `task assign` to different owner with `--admin-reason`
  for audit trail, explaining auto-supersede + lifecycle state reset
- **Lane guardrail override** — `--override-lane-limit` + `--override-reason`
  on `task assign` and `task start`
- **Lead repair mutations** — `--as-lead` + `--admin-reason` on `block`,
  `review`, `complete` with examples for each
- **Orchestration metadata on task create** — full table of 9 flags
  (`--lane-id`, `--work-kind`, `--criticality`, `--parent`, `--anchor`,
  `--scaffold-class`, `--sunset-decision`, `--sunset-owner`, `--sunset-trigger`)
  with allowed values and purpose
- **Nudge** — manual `mesh nudge OWNER TASK_ID` with guidance on when to use
- **Cross-team messaging** — `mesh xteam send` / `mesh xteam relay`

### 2. Mesh USAGE.md — Quick Reference expanded

Updated all task lifecycle commands in the Quick Reference to include:

- `task create`: added all 9 orchestration metadata flags
- `task assign`: added `--override-lane-limit`, `--override-reason`,
  `--admin-reason`
- `task start`: added `--override-lane-limit`, `--override-reason`
- `task block`: added `--as-lead --admin-reason`
- `task review`: added `--as-lead --admin-reason`
- `task complete`: added `--as-lead --admin-reason`

### 3. Mesh USAGE.md — Assign example expanded

Updated the "Assign a task" example to include contract fields
(`--first-step`, `--deliverable`, `--completion-signal`) on `task assign`
(previously only shown on `task create`). Added a new "Reassign a task"
example immediately after.

### 4. Mesh README.md — Lead-admin commands in task section

Added 3 lead-admin examples to the `mesh task` code block:
- Reassignment with `--admin-reason`
- Complete on behalf with `--as-lead --admin-reason`
- Lane override with `--override-lane-limit --override-reason`

Added explanatory note pointing to USAGE.md for full documentation.

### 5. Migration guide — New "Lead-Admin Task Controls" section

Added after "Assignment Contract Fields" covering:
- Reassignment with audit trail
- Lane guardrail override
- Lead repair mutations (`--as-lead` + `--admin-reason`)
- Orchestration metadata summary

### 6. Migration guide — Command Reference expanded

Updated all task lifecycle commands in the Command Reference to include the
same flags as USAGE.md Quick Reference (orchestration metadata, lane override,
lead repair flags).

### 7. Taurhaus AGENTS.md — Lead vs agent command surface

Added a table after the environment variables section distinguishing:
- Lead commands: `task create`, `task assign`, `nudge`, `xteam`, repair
- Agent commands: `accept`, `start`, `progress`, `block`, `review`, `complete`
- Both: `send`, `read`, `tasks`, `task get`, `who`, `heartbeat`, `status`

Plus a note about `--override-lane-limit` and `--admin-reason`.

---

## Files Modified

| File | Repo | Changes |
|------|------|---------|
| `USAGE.md` | mesh | New Lead-Admin Operations section, Quick Reference expansion, assign example expansion |
| `README.md` | mesh | Lead-admin examples in task section |
| `docs/analysis/mesh-cli-migration-guide-for-operators-2026-03-12.md` | mesh | New Lead-Admin Task Controls section, Command Reference expansion |
| `AGENTS.md` | taurhaus | Lead vs agent command surface table |

---

## Gaps Closed (From #1247 and #1250)

| Gap (from #1247/#1250) | Status |
|------------------------|--------|
| Task reassignment undocumented | **Fixed** — USAGE.md, README.md, migration guide |
| Lane guardrails undocumented | **Fixed** — USAGE.md, README.md, migration guide |
| Lead repair flags (`--as-lead`, `--admin-reason`) | **Fixed** — all 4 files |
| Orchestration metadata on `task create` (9 flags) | **Fixed** — USAGE.md, migration guide |
| Nudge workflow guidance | **Fixed** — USAGE.md Lead-Admin section |
| Cross-team workflow guidance | **Fixed** — USAGE.md Lead-Admin section |
| Lead vs agent command surface | **Fixed** — AGENTS.md |
| Task close (no CLI subcommand) | **Still blocked** — needs CLI implementation |
| Machine-safe `--json` on mutations | **Still blocked** — needs code implementation |
| Exit code distinction for warnings | **Still blocked** — needs code implementation |

---

## Remaining Gaps (Require Code Changes)

1. **`task close` CLI subcommand** — `TaskClosedRecord` + `TaskLifecycleStage::Closed`
   exist internally but no `mesh task close` CLI command
2. **`--json` on mutation commands** — all 9 mutation commands output human-only
   text; no machine-parseable output
3. **Exit code for success-with-warnings** — guardrail warnings don't affect
   exit code
