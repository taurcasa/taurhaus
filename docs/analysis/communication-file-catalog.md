# Communication File Catalog

Observed on 2026-03-07 in the local `~/.claude` tree on this machine.

This catalog distinguishes between:

- Primary mesh communication storage: the files mesh appears to use directly for team messages and tasks.
- Communication-adjacent metadata: files that support delivery, agent presence, or task tracking.
- Indirect logs: larger Claude session/history logs that can duplicate mesh events but are not the canonical mesh store.

## Summary

The canonical mesh data for the active team lives in two places:

- `~/.claude/teams/<team>/`: team membership, per-agent inboxes, runtime presence, and team state.
- `~/.claude/tasks/<namespace>/`: task definitions keyed by numeric ID, plus small cursor/lock metadata files.

On this machine, the most relevant active paths are:

- `~/.claude/teams/taurhaus-team/` at about `1.9M`
- `~/.claude/tasks/taurhaus-team/` at about `1.7M`
- `~/.claude/history.jsonl` at about `6.1M`
- `~/.claude/projects/-home-mstie-projects-taurhaus/` at about `562M`

## Primary Mesh Communication Storage

| Path | Format | What it contains | Communication data captured | Observed volume |
| --- | --- | --- | --- | --- |
| `~/.claude/teams/<team>/config.json` | JSON | Team definition, lead agent, member list, working directories, pane IDs, agent backend metadata, recent activity fields | Team membership and routing context for agent-to-agent communication | Small, about `4K` per team config in sampled teams |
| `~/.claude/teams/<team>/inboxes/*.json` | JSON array | Per-agent inbox files. Each entry contains fields like `id`, `from`, `text`, `timestamp`, `read`, and optional `summary` | Canonical direct messages, operator notices, task-assignment messages, and unread/read state | Active `taurhaus-team/inboxes/` directory is about `1.8M` across `14` JSON files. Largest sampled inboxes: `team-lead.json` `664K` with `833` messages, `developer2.json` `328K`, `communication-analyst.json` `3.8K` with `4` messages |
| `~/.claude/teams/<team>/inboxes/unassigned.json` | JSON array | Same message schema as agent inboxes, but used as a shared/unassigned queue | Broadcast or not-yet-routed task assignment messages | Sampled `taurhaus-team/inboxes/unassigned.json` is `1.9K` with one task-assignment message |
| `~/.claude/tasks/<namespace>/*.json` | JSON | One task record per numeric ID. Sampled fields include `id`, `subject`, `description`, `status`, `blocks`, `blockedBy`, `metadata`, and `owner` | Canonical task assignments and task lifecycle state | `~/.claude/tasks/taurhaus-team/` contains `423` task JSON files and uses about `1.7M`. Entire `~/.claude/tasks/` tree uses about `3.9M` across `971` files |
| `~/.claude/teams/<team>/state/task_mutations.jsonl` | JSONL | One mutation event per line. Sampled fields: `taskId`, `actor`, `timestamp`, `changedFields` | Append-only audit trail of task updates such as status changes and owner changes | Sampled `taurhaus-team` file is about `16K` with `137` lines |

## Communication-Adjacent Mesh Metadata

| Path | Format | What it contains | Communication relevance | Observed volume |
| --- | --- | --- | --- | --- |
| `~/.claude/teams/<team>/runtime/*.json` | JSON | Live agent runtime state. Sampled fields: `schema_version`, `member_name`, `pane_id`, `session_id`, `daemon_pid`, `health`, `delivery_lease`, `attached_at`, `last_seen_at` | Presence and delivery bookkeeping for active agents, but not message bodies | Small, roughly `4K` per file. `taurhaus-team/runtime/` totals about `36K` |
| `~/.claude/teams/<team>/daemons/*.pid` | Plain text | PID files for running team daemons | Process presence only; no message content | Small, about `4K` per file due to filesystem block size. `taurhaus-team/daemons/` totals about `20K` |
| `~/.claude/tasks/<namespace>/.highwatermark` | Plain text integer | Numeric cursor file | Task-store metadata, likely a write/read cursor rather than the authoritative task inventory | Tiny, one short integer. Sampled `taurhaus-team/.highwatermark` contains `499` even though task files exist through `509`, so treat it as metadata, not a complete index |
| `~/.claude/tasks/<namespace>/.lock` | Empty file | Advisory lock file for task writes | Concurrency control only | Tiny, zero-byte file |
| `~/.claude/teams/<team>/.lock` | Empty file | Advisory lock file for team directory operations | Concurrency control only | Tiny, zero-byte file |
| `~/.claude/teams/<team>/state/*.idle_reminded` | Plain text flag | One-byte reminder markers, sampled content `1` | Evidence that reminder communication happened, but without message payload | Tiny, about `4K` allocated per file |
| `~/.claude/teams/<team>/state/activity/*.json` | JSON | Activity/stall state files in some teams. Sampled fields: `version`, `observed_at`, `stall_recent_activity`, `stall_no_output`, `stall_no_active_process` | Presence and stall-monitoring state related to agent liveness | Small, about `4K` per file in sampled teams |

## Indirect Logs That Can Capture Mesh Events

| Path | Format | What it contains | Communication relevance | Observed volume |
| --- | --- | --- | --- | --- |
| `~/.claude/history.jsonl` | JSONL | Global Claude command/prompt history. Sampled fields: `display`, `pastedContents`, `timestamp`, `project`, and sometimes `sessionId` | Not the canonical mesh store, but it does preserve mesh notifications, pasted `mesh` commands, and command output text | About `6.1M` on this machine |
| `~/.claude/projects/<project-slug>/*.jsonl` | JSONL | Per-session Claude transcripts. Sampled records include `teamName`, `agentName`, `sessionId`, `type`, `message`, tool invocations, tool results, and progress events | Indirect but rich copy of mesh/team communication when the session itself is team-enabled | Current project path `~/.claude/projects/-home-mstie-projects-taurhaus/` is about `562M` with `501` JSONL files. `64` files matched mesh/team markers in a quick scan |
| `~/.claude/projects/<project-slug>/*/subagents/*.jsonl` | JSONL | Subagent transcripts nested under a parent session | Can capture teammate-message blocks or mesh-related coordination that appears inside Claude subagent workflows | Included in the `562M` current-project total; file sizes range from tens of KB to multiple MB in sampled files |

## Sample Structure Notes

- Inbox files are JSON arrays, not JSONL. Each message carries the sender in `from`, full body in `text`, and read state in `read`.
- Task files are one JSON object per file, keyed by numeric filename such as `505.json`.
- Task mutation history is append-only JSONL and is the best compact source for answering "who changed what and when" without rereading every task file.
- Runtime and state files are mostly operational metadata rather than communication payloads.
- Claude session/history JSONL files can preserve the same communication in duplicated form, but they are secondary copies from the perspective of mesh itself.

## Paths Most Useful For Future Analysis

- `~/.claude/teams/taurhaus-team/inboxes/`
- `~/.claude/tasks/taurhaus-team/`
- `~/.claude/teams/taurhaus-team/state/task_mutations.jsonl`
- `~/.claude/history.jsonl`
- `~/.claude/projects/-home-mstie-projects-taurhaus/`
