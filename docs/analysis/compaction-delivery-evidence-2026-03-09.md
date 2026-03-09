# Compaction Delivery Evidence — 2026-03-09

## Question

Does `compaction.injected` represent a real delivery to agents, or only a logging event with no usable content behind it?

## Answer

It represents a real delivery artifact, but the delivery channel is:

1. Taurhaus appends a JSON reinjection card into the target member inbox file.
2. The mesh daemon wakes the pane with a short `[mesh] ... Read: mesh read ...` prompt.
3. The actual reinjection content lives in the inbox JSON, not inline in the pane.

So:

- this is **not** direct tmux keystroke injection of the full reinjection payload
- the pane normally shows only the wake-up prompt
- the user would not see the role/task JSON appear inline unless the agent explicitly reads and echoes it

## Delivery Channel

Code path:

- [compaction_processor.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/compaction_processor.rs)
  - `append_codex_inbox_message(...)`
  - `MeshInboxStore::append(...)`
- [reinjection.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/reinjection.rs)
  - `render_codex_inbox_text(...)`
  - serializes the reinjection card as pretty JSON

This means the `compaction.injected` event is emitted after appending the JSON card to the member inbox file, not after any pane-side display of that JSON.

## Which Members/Sessions Received Injections

Current confirmed injected members from the fresh-deploy evidence:

### `taurhaus-team`

- `architect`
  - pane: `%217`
  - session: `019cbddb-5527-77a0-a457-7908cf7d790b`
  - runtime: [architect.json](/home/mstie/.claude/teams/taurhaus-team/runtime/architect.json)

### `2ksim-team`

- `developer3`
  - pane: `%266`
  - session: `019ccf84-1fc0-7b72-ab25-e5dab689619b`
  - runtime: [developer3.json](/home/mstie/.claude/teams/2ksim-team/runtime/developer3.json)

- `dev-2`
  - pane: `%263`
  - session: `019ccd25-b718-7313-b64a-f2ded2b54ca9`
  - runtime: [dev-2.json](/home/mstie/.claude/teams/2ksim-team/runtime/dev-2.json)

- `dev-1`
  - pane: `%262`
  - session: `019ccd25-b110-7f70-9172-d93f7e165f9b`
  - runtime: [dev-1.json](/home/mstie/.claude/teams/2ksim-team/runtime/dev-1.json)

## Actual Injected Content

### `architect`

Inbox file:

- [architect.json](/home/mstie/.claude/teams/taurhaus-team/inboxes/architect.json)

Recent injected entries are real JSON cards, for example the `2026-03-09T01:14:54.767Z` entry:

- `reason`: `post_compaction`
- `team_name`: `taurhaus-team`
- `member_name`: `architect`
- `task.id`: `761`
- `task.subject`: `Audit and fix ALL polling loops in daemon — replace with event-driven or diff-based`
- `working_set.project_path`: `/home/mstie/projects/taurhaus`

But the payload is under-populated:

- `role.role_id`: `null`
- `role.role_name`: `null`
- `role.focus_area`: `null`
- `role.behavior_summary`: `null`
- `task.execution_mode`: `""`
- `task.validation_expectation`: `""`
- `boundaries.file_ownership_boundary`: `[]`
- `working_set.focal_files`: `[]`

So `architect` received a real reinjection card, but it is mostly placeholder-level operational content.

### `2ksim-team` Codex members

Inbox files:

- [dev-1.json](/home/mstie/.claude/teams/2ksim-team/inboxes/dev-1.json)
- [dev-2.json](/home/mstie/.claude/teams/2ksim-team/inboxes/dev-2.json)
- [developer3.json](/home/mstie/.claude/teams/2ksim-team/inboxes/developer3.json)

These contain materially better cards. Example fields present in recent entries:

- `role.role_id`: `codex-developer`
- `role.role_name`: `Codex Developer`
- `role.focus_area`: `Scoped implementation and delivery`
- `role.behavior_summary`: populated
- `task.id`: populated (`68`, `72`, `74`)
- `task.subject`: populated
- `working_set.project_path`: populated

But they are still incomplete operationally:

- `task.execution_mode`: `""`
- `task.validation_expectation`: `""`
- `boundaries.file_ownership_boundary`: `[]`
- `working_set.focal_files`: `[]`

So the reinjection cards are not empty, but they are not yet full operational snapshots.

## Did Agents Consume The Content?

### Strong evidence

All the recent reinjection entries inspected above are marked:

- `"read": true`

That means the inbox entry was consumed through the mesh read path at some point after delivery.

### Pane evidence

Recent pane captures show mesh wake-up prompts for some recipients:

- `developer3` pane `%266`
  - contains repeated `[mesh] ... Read: mesh read --unread --mark-read ...`
- `dev-1` pane `%262`
  - contains the same mesh wake-up/read prompt pattern
- `architect` pane `%217`
  - contains recent mesh wake-up prompts as well

This is consistent with:

- Taurhaus writing to the inbox
- mesh daemon nudging the pane to read messages
- the agent then consuming via `mesh read`

### Important limitation

I do **not** have strong evidence that the agent then incorporated the reinjection JSON into its visible reasoning/output.

What I can prove:

- inbox entry exists
- inbox entry contains real JSON content
- inbox entry is marked `read: true`
- some panes show the mesh wake-up prompt

What I cannot prove from current instrumentation:

- that the agent semantically used the card in its next response
- that the full JSON was shown inline in the pane
- that the card changed the agent’s behavior in a measurable way

## Why The User Did Not See Role/Task Context In The Pane

Because the current non-Claude compaction path does **not** inject the full reinjection JSON directly into tmux as visible inline text.

Instead:

- Taurhaus writes the JSON card to inbox storage
- mesh wakes the pane with a short prompt
- the agent then reads the inbox

So if the user expected to see a rich role/task card printed directly into the terminal, the current implementation does not do that.

## Bottom Line

1. `compaction.injected` is not fake; it corresponds to a real inbox write.
2. The delivery channel is inbox append plus mesh wake-up prompt, not direct full-payload tmux injection.
3. `2ksim-team` Codex members received meaningful role/task cards.
4. `architect` received a real card, but it was under-populated and mostly placeholder-level.
5. There is evidence of inbox consumption (`read: true`) and some pane wake-up prompts.
6. There is **not yet** strong evidence that agents visibly processed the reinjection content beyond reading the inbox entry.

## Actionable Next Steps

1. Add explicit observability for post-compaction consumption:
   - log when a `post_compaction_context` message is read
   - include message ID and member name
2. Add a bounded “consumed and acknowledged” marker in runtime state so Taurhaus can distinguish:
   - injected
   - read
   - actually acknowledged by the agent
3. Fix the under-populated Taurhaus cards so `execution_mode`, `validation_expectation`, boundaries, and focal files are present.
4. Decide whether the product should keep inbox-only delivery or also show a short visible summary inline in the pane after compaction.
