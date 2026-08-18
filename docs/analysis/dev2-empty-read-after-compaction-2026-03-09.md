# `dev-2` Empty `mesh read` After Compaction — 2026-03-09

## Question

For the live `2ksim-team/dev-2` compaction, did reinjection fail to reach the agent, or did it arrive and then disappear before the manual `mesh read --unread --mark-read` command printed anything?

## Conclusion

This was **not** a silent Taurhaus write failure.

The exact `dev-2` compaction at `2026-03-09T01:30:44Z` produced:

- canonical compaction signals
- `compaction.detected`
- `compaction.injected`
- two actual inbox entries in `dev-2.json`

The reason the pane then showed:

- mesh notification prompt
- followed by `mesh read --unread --mark-read`
- followed by no visible shell output

is most likely:

- the compaction messages had already been consumed/marked `read=true` by the time the shell command ran
- `mesh read --unread --mark-read` only prints currently unread messages

So the break is in **message presentation / read-state timing**, not in the compaction delivery write path.

## Exact evidence

### 1. Inbox entries exist for the exact compaction window

Inbox file:

- [dev-2.json](/home/user/.claude/teams/2ksim-team/inboxes/dev-2.json)

Relevant entries:

- index `135`
  - timestamp: `2026-03-09T01:30:44.565Z`
  - summary: `post_compaction_context`
  - `read: true`
- index `136`
  - timestamp: `2026-03-09T01:30:44.589Z`
  - summary: `post_compaction_context`
  - `read: true`

These are real Taurhaus reinjection cards, not placeholders. They contain:

- role:
  - `role_id: codex-developer`
  - `role_name: Codex Developer`
  - `focus_area: Scoped implementation and delivery`
  - `behavior_summary: ...`
- task:
  - `id: 72`
  - `subject: Add browser smoke coverage for Scenario Challenge and reproducibility bundle launch paths`
- working set:
  - `project_path: /home/user/projects/2ksim`

So the compaction card was definitely written.

### 2. App logs show detected -> injected for the same session

From the active Windows app log:

- [taurhaus.log.jsonl](/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl)

For `session_id = 019ccd25-b718-7313-b64a-f2ded2b54ca9`:

- `2026-03-09T01:30:44.567Z` `compaction.detected`
  - `compaction_timestamp: 2026-03-09T01:30:44.562+00:00`
- `2026-03-09T01:30:44.585Z` `compaction.injected`
  - `delivery_result: injected`
- `2026-03-09T01:30:44.590Z` `compaction.detected`
  - `compaction_timestamp: 2026-03-09T01:30:44.566+00:00`
- `2026-03-09T01:30:44.599Z` `compaction.injected`
  - `delivery_result: injected`

So the app believed the write succeeded, and the inbox file confirms that it did.

### 3. Canonical signal log shows emitted signals for the same pane/session

Signal log:

- [codex-compaction-signals.jsonl](/home/user/.claude/teams/2ksim-team/state/compaction/signals/codex-compaction-signals.jsonl)

Matching records for `dev-2` / pane `%263` / session `019ccd25-b718-7313-b64a-f2ded2b54ca9`:

- `2026-03-09T01:30:44.563924155Z`
  - `signal_kind: compacted`
- `2026-03-09T01:30:44.584996024Z`
  - `signal_kind: context_compacted`

So the pipeline is complete all the way up to canonical signal emission.

## Why the shell command could print nothing anyway

`mesh read --unread --mark-read` behavior in the mesh CLI:

- [main.rs](/home/user/projects/mesh/src/main.rs)

Relevant logic:

- it filters messages with `!m.read` when `--unread` is set
- if nothing unread remains, `display_is_empty == true`
- then it prints the empty branch rather than the actual messages

Important point:

- the command only shows **currently unread** inbox entries
- if something else already marked the relevant entries as read, the command will not print them

That aligns with what we observed:

- by inspection time, the two relevant Taurhaus entries were already `read: true`
- yet the agent still had enough context to say:
  - “It’s informational only: taurhaus sent post_compaction task context for active task #72”

That combination strongly suggests:

- the content had already been consumed through another layer/path
- the manual shell command then found no unread messages left to print

## Most likely explanation

The empty read symptom is caused by a **read-state race / dual-consumption path**:

1. Taurhaus writes compaction cards to inbox successfully.
2. A mesh notification is emitted to the pane.
3. Some path already consumes or marks those inbox messages read.
4. The explicit shell command `mesh read --unread --mark-read` runs after that and sees no unread messages.

Contributing factor:

- duplicate paired compaction boundaries still generate duplicate compaction cards
- that makes the wake/read timing more confusing and increases the chance of presentation mismatch

## What this means

### What is **not** broken

- signal emission
- app-level detection
- app-level `injected` delivery record
- inbox append itself

### What **is** broken

- the agent-facing UX/instrumentation around compaction delivery
- specifically, the system can tell the agent “read this Taurhaus message” while the manual unread-only CLI path no longer has anything visible to print

## Actionable next steps

1. Add explicit compaction message consumption telemetry:
   - message ID
   - member name
   - read timestamp
   - whether it was consumed by shell `mesh read` or another path
2. Fix duplicate paired-boundary deliveries first so one compaction produces one card.
3. Decide on one source of truth for post-compaction delivery UX:
   - inbox-only
   - or explicit inline pane summary
4. If the prompt layer is pre-consuming messages, stop telling the agent to manually run `mesh read --unread --mark-read` for the same event.

## Bottom line

The `dev-2` case is concrete proof of a **delivery visibility problem**, but **not** of a missing compaction write.

The message was written. The logs and inbox both prove that.

The actual failure is:

- by the time the agent ran `mesh read --unread --mark-read`, the message was already `read`
- so the unread-only shell command had nothing left to display
