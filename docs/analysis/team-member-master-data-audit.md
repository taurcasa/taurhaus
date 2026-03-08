# Team/Member Master Data Audit

Date: 2026-03-08
Task: #740

## Executive Summary

Taurhaus does not currently have one authoritative store that answers the full question:

- this is team `X`
- these are its members
- this is each member's current tool
- this is each member's current pane/session/transcript
- this is whether each member is currently active or idle

Instead, the system is split across four layers:

1. **Logical roster**: `teams/<team>/config.json`
2. **Attachment/runtime binding**: `teams/<team>/runtime/<member>.json`
3. **Observed live sessions**: session scanner runtime/display views
4. **Derived compaction tracking**: extractor/watcher state under `teams/<team>/state/compaction/`

That split is reasonable, but the boundaries are not explicit enough, and some derived layers still reconstruct membership by heuristic instead of consuming authoritative attachment state. The recent compaction extractor issue is exactly that: it routed transcripts from scanner sessions back to teams by project-path matching, even though team membership already existed elsewhere.

## Current Stores

### 1. Taurhaus team config: `teams/<team>/config.json`

Files:
- `src-tauri/src/coordination/stores/config.rs`
- `src-tauri/src/coordination/domain.rs`

What it stores:
- team identity
- authoritative member list
- member role/role metadata
- canonical member `project_path`
- canonical member `cli_tool`
- mesh-compatible aliases on write (`projectPath`, `cwd`, `tmuxPaneId`, etc.)

Important implementation details:
- `TeamConfig.members` is the authoritative Taurhaus roster in memory.
- On save, Taurhaus serializes a mesh-compatible wire format and pulls selected runtime fields into the file (`tmuxPaneId`, lead session aliasing) (`config.rs:209-243`).
- This means `config.json` is both a logical roster and a compatibility surface for mesh.

What it answers well:
- who belongs to team `X`
- what tool member `Y` is supposed to use
- what project member `Y` belongs to

What it does **not** answer well:
- member `Y`'s current session id
- member `Y`'s current transcript path
- member `Y`'s current active/idle state

### 2. Taurhaus runtime records: `teams/<team>/runtime/<member>.json`

File:
- `src-tauri/src/coordination/stores/runtime.rs`

What it stores:
- `member_name`
- `cli_tool`
- `project_path`
- `pane_id`
- `session_id`
- `daemon_pid`
- health / lease / timestamps

This is the current Taurhaus attachment state per member. It is the only persisted place where Taurhaus records:
- which pane a member is attached to
- which session id Taurhaus believes that member currently owns
- current daemon pid / health metadata

What it answers well:
- member `Y`'s current session binding, if runtime is fresh
- member `Y`'s current pane
- member `Y`'s current tool/project attachment

What it does **not** answer well:
- the authoritative team roster by itself
- transcript path
- UI activity state

### 3. Mesh config view: shared `config.json`

Files:
- `/home/mstie/projects/mesh/src/types.rs`
- `/home/mstie/projects/mesh/src/config.rs`

Mesh reads the same `teams/<team>/config.json`, but through its own schema:
- `TeamConfig.lead_agent_id`
- `TeamConfig.lead_session_id`
- `members[*].name`
- `members[*].agent_id`
- `members[*].agent_type`
- `members[*].model`
- `members[*].cwd`
- `members[*].tmux_pane_id`
- status/activity flags like `is_active`, `last_activity_at`, `status_state`
- unknown fields preserved through `serde(flatten)`

Important boundary:
- mesh does **not** maintain the richer Taurhaus role/context fields semantically; it just preserves unknown fields.
- mesh has no first-class per-member `session_id` field in its `Member` schema.
- mesh only has `lead_session_id` at top level.

So mesh owns messaging/orchestration semantics on the shared team config, but not the full Taurhaus attachment model.

### 4. Session scanner runtime/display views

File:
- `src-tauri/src/session_scanner/mod.rs`

Two different scanner products exist:
- `DisplaySession`: UI-safe, strips transcript metadata
- `RuntimeSession`: keeps `session_id` and `jsonl_path`

Important details:
- `scan_sessions_for_runtime()` returns a **live observed process list**, not a team roster (`mod.rs:844-895`).
- `RuntimeSession` currently includes no `team_name` and no authoritative `member_name`; those remain `None` in the scanner path (`mod.rs:882-885`, `mod.rs:988-991`).
- scanner sessions are deduplicated by `(tty, cli_tool)`, so this is an observation layer, not a member-binding store.

What it answers well:
- what CLI sessions are live right now
- their current `session_id` / `jsonl_path` / pane / state
- active vs idle observations

What it does **not** answer well:
- which managed team member a session belongs to
- whether an observed session should be considered part of a particular team

### 5. Compaction extractor state

File:
- `src-tauri/src/session_scanner/compaction_extractor.rs`

What it stores:
- per-team file offsets under `teams/<team>/state/compaction/extractor-state.json`
- last processed signal metadata
- heartbeat / per-file errors

What it is:
- a derived processing cache for tailing transcript files
- not a roster
- not an attachment registry

Current problem:
- extractor active files are still derived from scanner sessions routed back to teams by heuristics (`compaction_extractor.rs:420-470`), rather than from an authoritative member->attachment roster.
- specifically, routing gives a non-zero score even when neither pane nor session matches, if project path matches (`score = 1`) (`compaction_extractor.rs:447-452`).
- that is why historical or unrelated transcripts can enter the tracked set.

### 6. Compaction watcher state

File:
- `src-tauri/src/session_scanner/compaction_watcher.rs`

What it stores:
- last consumed signal-log offset
- watcher health counters
- recent signal ids

What it is:
- pure downstream consumption state for the signal log
- not authoritative for team membership or attachment

## Canonical Answer Matrix

### Who are the members of team `X`?

Current canonical source:
- **`teams/<team>/config.json` via Taurhaus `TeamConfigStore`**

Reason:
- this is the only persisted logical roster with complete member list semantics.
- runtime records may be missing or stale for some members, but config remains the authoritative intended roster.

### What is member `Y`'s current `session_id`?

Current canonical source:
- **`teams/<team>/runtime/<member>.json` via `MemberRuntimeStore`**

Reason:
- scanner only sees observed sessions; it does not own member binding.
- config does not carry per-member current `session_id`.

Caveat:
- this is only canonical if runtime reconciliation is working and fresh.

### What CLI tool is member `Y` using?

Current canonical source:
- **logical tool**: `config.json`
- **currently attached tool**: `runtime/<member>.json`

Reason:
- config tells you what the member is configured to be.
- runtime tells you what the current attachment actually is.

These should normally agree, but they are not the same question.

### Is member `Y` currently active or idle?

Current canonical source:
- **no persisted canonical member-level store exists today**
- nearest answer is the **session scanner runtime/display observation layer**

Reason:
- active/idle is computed from live observation, not stored in config/runtime as authoritative attachment state.
- mesh config has `last_activity_at` and `status_state`, but those are orchestration signals, not the scanner's active/idle truth.

This is a genuine gap.

### What transcript file should we watch for member `Y`?

Current canonical source today:
- **none fully authoritative**
- inferred from `runtime/<member>.json` `session_id` plus scanner/runtime observation of `jsonl_path`

Reason:
- runtime does not persist `jsonl_path`
- scanner observes `jsonl_path` but does not bind it authoritatively to member identity
- extractor currently reconstructs this by routing heuristics

This is the second major gap.

## Gaps and Overlaps

### Gap 1: No single member attachment record includes transcript path

Today the attachment picture is split:
- runtime record has `session_id`, `pane_id`, tool, project
- scanner runtime session has `session_id`, `jsonl_path`, pane, state
- neither store alone gives a complete authoritative member attachment row

Impact:
- downstream systems rejoin these two worlds repeatedly.
- every rejoin point is a place for drift or heuristic mistakes.

### Gap 2: Scanner is authoritative for observation, but not for ownership

`RuntimeSession` answers “what processes exist now?”
It does **not** answer “which managed member owns this session?”

Impact:
- consumers like the extractor can be tempted to route sessions back to members by project path or pane heuristics.
- that is exactly what happened in `route_managed_codex_transcripts()`.

### Gap 3: Mesh and Taurhaus both read/write the same config file for different reasons

This is intentional, but the ownership boundary is blurry.

Taurhaus uses `config.json` as:
- logical roster
- role/tool/project identity store
- mesh compatibility export

Mesh uses the same file as:
- operational team/member list
- lead identity
- tmux pane metadata
- status/activity metadata

Impact:
- shared-file compatibility is good, but it encourages subtle duplication of meaning.
- some fields are Taurhaus-owned semantics, some are mesh-owned semantics, and some are merely preserved.

### Gap 4: No first-class “authoritative member attachment roster” exists

There is no single persisted Taurhaus document that says, per member:
- member exists in this team
- current tool
- current pane
- current session id
- current transcript path
- current attachment state
- current activity state source

Instead, callers reconstruct it from multiple stores.

### Gap 5: Activity/idle is not modeled as member master data

The active/idle answer is purely observational today.
That is correct conceptually, but it means the system needs an explicit rule:
- activity state should be treated as **derived volatile state**, not master data

Without that rule, teams can expect `config.json` or runtime to answer something they never promised to answer.

## Root Cause of the Extractor Tracking Wrong Files

The extractor bug is **not** that the team member list is missing.
The team member list exists in `config.json`.
The runtime attachment list exists in `runtime/<member>.json`.

The real root cause is:
- the extractor did not consume an authoritative per-member attachment roster.
- instead it took scanner-observed sessions and routed them back to teams by project-path/session/pane heuristics.
- the routing logic still accepted project-only matches (`score = 1`) (`compaction_extractor.rs:447-452`).

So the failure mode is:
1. scanner observes a live Codex transcript in project `P`
2. extractor looks up managed Codex members with project `P`
3. if no exact current session/pane match exists, it can still attach the transcript to a team member based on project alone
4. stale or unrelated transcripts become “tracked files”

That is a derived-view ownership problem, not a missing-roster problem.

## Ownership Boundary: Taurhaus vs Mesh

### What Taurhaus should own

Taurhaus should own:
- logical team roster for app semantics
- role/tool/project identity per member
- current member attachment state for managed sessions
- transcript binding (`jsonl_path`) for transcript-aware features
- scanner-derived activity/idle observation for UI/runtime features
- compaction extractor/watcher state

Reason:
- these are Taurhaus-specific concerns tied to session scanning, transcript processing, role-aware UX, and reinjection.
- mesh does not semantically understand transcript paths or member role metadata.

### What mesh should own

Mesh should own:
- lightweight cross-agent messaging semantics
- team daemon / member daemon orchestration
- inbox/task/idle-monitor workflows
- mesh-native status/activity fields in shared config (`status_state`, `last_activity_at`, `is_active`)

Reason:
- these are mesh's cross-tool operational concerns.
- mesh should not become the owner of Taurhaus transcript resolution or rich attachment metadata.

### Should Taurhaus consume mesh as the source of truth for team membership?

No.

Reason:
- mesh does not have the right semantic model for Taurhaus member identity.
- mesh `Member` lacks first-class per-member `session_id`, `cli_tool`, and transcript path semantics.
- mesh preserves unknown fields but does not own them.
- making mesh the canonical roster for Taurhaus would move app-specific semantics into a transport/orchestration layer that does not naturally own them.

The correct model is:
- shared `config.json` remains the interoperability surface
- Taurhaus remains the semantic owner of Taurhaus-specific roster/role/tool/project meaning
- mesh consumes that file for orchestration-compatible fields and preserves the rest

## Proposed Clean Master Data Model

### Recommended authoritative model

Introduce an explicit Taurhaus-owned **Team Member Roster View** with two layers:

1. **Logical roster (persisted)**
- source: `config.json`
- fields:
  - `team_name`
  - `member_name`
  - `role`
  - `role metadata`
  - `configured_cli_tool`
  - `configured_project_path`
  - `membership_state`

2. **Attachment/runtime row (persisted)**
- source: `runtime/<member>.json` expanded or replaced by a clearer attachment record
- fields:
  - `member_name`
  - `attached_cli_tool`
  - `project_path`
  - `pane_id`
  - `session_id`
  - `jsonl_path`
  - `attachment_state` (`attached`, `detached`, `session_dead`, etc.)
  - `last_seen_at`
  - `health`

Then derive:
- **activity state** from scanner observation
- **display grouping** from roster + activity
- **compaction tracking set** from current attachment rows, not scanner heuristics

### Minimum structural rule

The system should adopt one explicit rule:

> Any subsystem that needs to know which transcript belongs to which team member must consume the authoritative member attachment roster, not reconstruct it from scanner sessions by project-path heuristics.

That one rule would have prevented the extractor bug.

## Recommended Canonical Answers After Cleanup

If the model is cleaned up, the canonical answers should be:

- Who are the members of team `X`?
  - `config.json`
- What tool is member `Y` configured to use?
  - `config.json`
- What is member `Y` currently attached to?
  - attachment/runtime record
- What is member `Y`'s current `session_id`?
  - attachment/runtime record
- What transcript file should we watch for member `Y`?
  - attachment/runtime record
- Is member `Y` currently active/idle?
  - scanner-derived volatile observation layered onto the attachment record

## Concrete Recommendations

1. Keep `config.json` as the authoritative logical roster.
2. Keep runtime attachment state in Taurhaus, not mesh.
3. Extend Taurhaus runtime attachment state to persist `jsonl_path`.
4. Treat scanner sessions as observation-only, never as ownership truth.
5. Make extractor active-set construction consume current attachment rows directly.
6. Keep mesh as the orchestration/messaging consumer of shared team config, not the owner of Taurhaus attachment semantics.
7. Document explicitly that active/idle is derived volatile state, not master data.

## Bottom Line

The system already has most of the pieces. The problem is not absence of data; it is absence of one explicit ownership model.

Today:
- `config.json` is the logical roster
- `runtime/<member>.json` is the attachment cache
- scanner sessions are observed live sessions
- extractor state is derived processing state

What is missing is a strict rule that only the first two may answer ownership questions. The observed/derived layers should consume those stores, not infer membership on their own.
