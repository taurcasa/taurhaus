# Taurhaus Mesh Usage Docs Update — 2026-03-13

**Task:** #1242
**Scope:** Update Taurhaus team-facing docs to teach current Mesh operator
model. AGENTS.md mandatory.

---

## Changes Applied

### AGENTS.md — New "Mesh Team Coordination" section

Added a full section after Team Messaging Conventions that teaches Codex/Gemini
agents the current Mesh workflow:

1. **Task lifecycle commands** — `accept`, `start`, `progress`, `block`,
   `review`, `complete` with exact CLI syntax. Explicit note that `task update`
   is legacy.
2. **Action-first reply behavior** — "start working immediately, do NOT send
   acknowledgments, summaries, or 'understood' replies before doing the work."
3. **Reading messages** — `mesh read --unread --mark-read` pattern with the
   note about checking `mesh tasks` when no inbox messages exist.
4. **Sending messages** — `mesh send` with `--summary` flag.
5. **Environment variables** — `MESH_TEAM` and `MESH_NAME` to skip repetitive
   flags.

### AGENTS.md — Sync drift from CLAUDE.md

Fixed entries that had drifted from CLAUDE.md:

| Change | Detail |
|--------|--------|
| IPC command count | 80 → 89 |
| Architecture: Team templates | Updated to match CLAUDE.md (`MeshTeamBuilder`-driven setup flow) |
| Architecture: Windows Mesh behavior | Added (was missing entirely) |
| Key Files: MeshSetupView.svelte | Added |
| Key Files: MeshTeamBuilder.svelte | Added |
| Key Files: TemplateBrowserPanel description | Updated to match CLAUDE.md |
| Key Files: TeamCustomizerPanel description | Updated to match CLAUDE.md |
| Key Files: platform_paths.rs | Added |
| Key Files: claude_hooks.rs | Added |
| Key Files: compaction_processor.rs | Added |
| Key Files: compaction_extractor.rs | Added |
| Key Files: compaction_watcher.rs | Added |
| Key Files: adapters.rs | Added |
| First File: compaction detection | Added task-type entry |
| First File: path/root resolution | Added task-type entry |

---

## Files NOT Changed (Already Current)

| File | Status | Notes |
|------|--------|-------|
| `CLAUDE.md` | Current | Already has Mesh integration section, correct version, JSONL references |
| `docs/coordination-architecture.md` | Current | Correctly positions `mesh task assign` + `mesh nudge` as enforced surface |
| `docs/team-templates.md` | Current | UI-focused, no CLI patterns |
| `ARCHITECTURE.md` | Current | Correct JSONL and mesh references |
| Role template YAMLs | Current | Use SendMessage + task system correctly |

---

## Stale Patterns (Archive/Analysis Only — No Action Needed)

All stale `mesh task update` / `mesh send` patterns are confined to:
- `docs/archive/` — explicitly archived design docs
- `docs/analysis/` — timestamped historical analysis snapshots
- `CHANGELOG.md` — release notes (historical record)

None of these are used for agent training or operator instruction.
