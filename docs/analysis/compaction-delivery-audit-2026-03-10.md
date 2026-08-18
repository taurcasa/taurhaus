# Compaction Delivery Audit — 2026-03-10

## Scope

This audit answers one concrete question:

- Do compaction signals reach managed agents end to end?

The pipeline was checked separately for:

- Codex members: transcript boundary -> Taurhaus detection -> inbox append -> mesh daemon wake prompt -> `mesh read` surfacing the card
- Claude members: Claude compact event -> Claude hook fire -> Taurhaus bridge resolution -> `additionalContext` return

This document is evidence-based. It does not treat log-only success as proof of final model uptake unless transcript/debug evidence also exists.

## Sources

Runtime and state:
- `~/.claude/teams/taurhaus-team/runtime/*.json`
- `~/.claude/teams/taurhaus-team/inboxes/*.json`
- `~/.claude/teams/taurhaus-team/state/compaction/*`
- `~/.claude/teams/taurhaus-team/state/operational/*`

Codex transcripts:
- `~/.codex/sessions/2026/03/05/rollout-2026-03-05T12-56-33-019cbddb-5527-77a0-a457-7908cf7d790b.jsonl`

Claude diagnostics:
- `~/.claude/debug/47fb0840-8a3e-4877-b512-72d133d44386.txt`

Taurhaus app logs:
- Windows canonical app log root:
  - `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.log*.jsonl`
- WSL/local root exists too:
  - `~/.local/share/com.taurhaus.dev/taurhaus.log*.jsonl`

Code paths reviewed:
- `src-tauri/src/coordination/compaction_processor.rs`
- `src-tauri/src/coordination/reinjection.rs`
- `src-tauri/src/coordination/claude_hooks.rs`
- `src-tauri/src/session_scanner/compaction_extractor.rs`
- `scripts/analyze-compaction.py`
- `/home/user/projects/mesh/src/daemon.rs`
- `/home/user/projects/mesh/src/main.rs`

## Executive Summary

### Codex

The Codex delivery transport works.

What is proven:
- compaction boundaries are present in Codex JSONL transcripts
- Taurhaus detects them and records `compaction.detected`
- Taurhaus appends a post-compaction card into the correct inbox and records `compaction.injected`
- the mesh member daemon injects a wake prompt into the Codex tmux pane
- the Codex transcript records that wake prompt as a real user message
- `mesh read` later surfaces the Taurhaus compaction card content from the inbox

What is not guaranteed:
- immediate consumption while Codex is already busy on another turn
- immediate pivot from the wake prompt into `mesh read`

So the correct statement is:
- delivery works
- immediate reaction is opportunistic / next-turn, not guaranteed

### Claude

The Claude compact-hook path works at the bridge level.

What is proven:
- Claude compact events happen
- Claude matches and executes the `SessionStart:compact` hook
- after the runtime-environment hook fix, the hook now succeeds instead of erroring
- Taurhaus resolves the managed member and logs `compaction.claude_hook.received -> resolved -> delivered`
- Taurhaus logs `compaction.injected` for the Claude lead session

What is not directly provable from current telemetry:
- whether Claude actually used the returned `additionalContext` well inside the next turn

So the correct statement is:
- the Claude bridge is functioning
- final model uptake is still inferred, not directly observable

## Stage-by-Stage Findings

## 1. Codex boundary detection

Confidence: high

Evidence:
- signal journal contains canonical records for the active architect session:
  - `~/.claude/teams/taurhaus-team/state/compaction/signals/codex-compaction-signals.jsonl`
- example architect signals:
  - `2026-03-09T22:37:10.842Z`
  - `2026-03-09T22:49:34.609Z`
  - `2026-03-09T23:19:57.546Z`
  - `2026-03-10T09:05:14.475Z`

The transcript also contains the actual compaction boundary:
- `2026-03-09T23:19:57.546Z` `event_msg.payload.type = context_compacted`
- same session file:
  - `~/.codex/sessions/2026/03/05/rollout-2026-03-05T12-56-33-019cbddb-5527-77a0-a457-7908cf7d790b.jsonl`

Conclusion:
- the extractor is seeing real Codex compaction boundaries and emitting canonical signal records

## 2. Taurhaus Codex delivery decision

Confidence: high

Evidence from the Windows app log:
- `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`
- architect examples:
  - `2026-03-09T22:37:10.850Z` `compaction.detected`
  - `2026-03-09T22:37:10.860Z` `compaction.injected`
  - `2026-03-09T22:49:34.614Z` `compaction.detected`
  - `2026-03-09T22:49:34.622Z` `compaction.injected`
  - `2026-03-09T23:19:57.549Z` `compaction.detected`
  - `2026-03-09T23:19:57.555Z` `compaction.injected`

24h analyzer result against the Windows log, taurhaus-team only:
- detected: `25`
- injected: `22`
- stale: `5`
- failed: `0`

Interpretation:
- `stale` is a real, explicit terminal outcome, not a silent drop
- there is no evidence here of transport silently failing after detection

Conclusion:
- Taurhaus delivery decisioning is working and mostly succeeds for Codex

## 3. Codex inbox append

Confidence: high

Evidence:
- architect inbox contains real post-compaction entries:
  - `2026-03-09T22:37:10.845Z`
  - `2026-03-09T22:49:34.613Z`
  - `2026-03-09T23:19:57.548Z`
  - `2026-03-10T09:05:14.478Z`
- file:
  - `~/.claude/teams/taurhaus-team/inboxes/architect.json`

These entries have:
- `from = "taurhaus"`
- `summary = "post_compaction_context"`

Conclusion:
- the card is definitely being appended to the correct inbox

## 4. Mesh member-daemon wake prompt for Codex

Confidence: medium-high

Code path:
- `/home/user/projects/mesh/src/daemon.rs`
- `check_inbox(...)` compares inbox length
- if new non-empty, non-low-priority messages arrived, it calls:
  - `format_notification(...)`
  - `deliver_to_tmux(...)`

Wake prompt format is generic and sender-based:
- `[mesh] You are "<name>" on team "<team>". Message from <sender>. Read: mesh read --unread --mark-read --team <team> --name <name>`

Direct transcript proof exists for architect:
- `2026-03-09T00:20:09.435Z`
- transcript recorded:
  - `[mesh] You are "architect" on team "taurhaus-team". Message from taurhaus. Read: mesh read --unread --mark-read --team taurhaus-team --name architect`
- same transcript also shows another explicit Taurhaus wake prompt later:
  - `2026-03-09T23:20:11.404Z`

Conclusion:
- the inbox append does trigger the member-daemon wake prompt path
- this is not a hypothetical path; it is recorded in the live Codex transcript as a real user turn

## 5. Codex consumption after wake prompt

Confidence: medium

This is the subtle stage.

What is proven:
- after compaction at `2026-03-09T00:20:08.814Z`, the Taurhaus wake prompt appears at `00:20:09.435Z`
- the same transcript then shows `mesh read` surfacing the compaction card content at `00:20:26.378Z`
- after compaction at `2026-03-09T18:22:36.845Z`, the transcript shows `mesh read` surfacing the Taurhaus card at `18:22:49.433Z`
- after compaction at `2026-03-09T23:19:57.546Z`, the wake prompt appears at `23:20:11.404Z`

What is not always true:
- the agent does not always immediately pivot on the wake prompt if it is already inside another active turn
- example: the `23:20:11` Taurhaus wake prompt landed while the session was already continuing task work

Additional evidence of delayed pickup:
- older Taurhaus cards from `22:37:10` and `22:49:34` were later surfaced during a `mesh read` triggered by unrelated message handling at `23:16:49`

Conclusion:
- Codex transport and prompt delivery work
- immediate consumption is not guaranteed under active-turn contention
- the current behavior is effectively next-turn / opportunistic, not interruptive

## 6. Claude compact hook fire

Confidence: high

Claude debug log evidence:
- `~/.claude/debug/47fb0840-8a3e-4877-b512-72d133d44386.txt`

Historical state before the hook fix:
- many repeats of:
  - `Getting matching hook commands for SessionStart with query: compact`
  - `Matched 1 unique hooks for query "compact"`
  - `Hook SessionStart:compact (SessionStart) error:`

After the runtime-environment fix:
- success lines appear instead:
  - `2026-03-09T20:23:31.935Z` `Hook SessionStart:compact (SessionStart) success:`
  - `2026-03-09T23:03:13.884Z` `Hook SessionStart:compact (SessionStart) success:`

Conclusion:
- Claude really is compacting
- Claude really is firing the hook
- after the fix, the hook execution succeeds instead of failing on the old bad command path

## 7. Taurhaus Claude bridge delivery

Confidence: high

Windows app log evidence around the live team-lead hook run:
- `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.log.20260309T220416Z.jsonl`

Observed sequence:
- `2026-03-09T20:58:00.079Z` `compaction.claude_hook.received`
- `2026-03-09T20:58:00.165Z` `compaction.claude_hook.resolved`
- `2026-03-09T20:58:00.320Z` `compaction.injected`
- `2026-03-09T20:58:00.320Z` `compaction.claude_hook.delivered`

Second successful cycle:
- `2026-03-09T20:58:11.761Z` `received`
- `2026-03-09T20:58:11.826Z` `resolved`
- `2026-03-09T20:58:11.991Z` `injected`
- `2026-03-09T20:58:11.991Z` `delivered`

Conclusion:
- the Claude bridge is functioning end to end up to `additionalContext` return

## 8. Claude final model uptake

Confidence: low-medium

What we can prove:
- Claude executed the hook successfully
- Taurhaus returned non-empty `additionalContext`
- app logs include `additional_context_bytes`

What we cannot prove from the current telemetry:
- whether the next Claude turn actually used that returned context well
- there is no equivalent to the Codex transcript-level wake prompt + `mesh read` proof here because Claude uses native hook return, not inbox wake prompts

Conclusion:
- hook delivery is proven
- final model uptake is still not directly observable

## Cross-Team Sample

For `2ksim-team`, the Windows log also supports healthy Codex delivery:
- analyzer result against the Windows log:
  - detected: `12`
  - injected: `12`
  - stale: `0`
  - failed: `0`
- members with successful injections in that window:
  - `dev-1`
  - `dev-2`
  - `developer3`
  - `lead-2ksim`

Caveat:
- the live `~/.claude/teams/2ksim-team` folder is not present anymore in this workspace, so current on-disk inbox/runtime spot checks were not possible there
- for `2ksim-team`, the evidence is log-level rather than current filesystem/runtime-level

## Real Gaps / Problems Found

## 1. Immediate Codex reaction is not guaranteed

This is the main operational limitation.

The wake prompt lands, but if Codex is already in an active turn, the prompt becomes queued user input. It does not interrupt the current turn and force immediate `mesh read`.

Impact:
- delivery can be correct while reachability still feels flaky
- a compaction card may sit unread until the next prompt boundary or another message causes a read

## 2. Mesh wake delivery lacks durable structured telemetry

`mesh daemon` currently has the mechanics, but not durable per-delivery evidence.

What exists:
- code path in `/home/user/projects/mesh/src/daemon.rs`
- tmux injection happens through `deliver_to_tmux(...)`

What is missing:
- structured event stream like:
  - inbox growth seen
  - wake prompt composed
  - tmux injection succeeded/failed
  - pane current command at send time
  - sender list

Impact:
- proving delivery currently requires transcript archaeology or transient stderr access

## 3. Analyzer default log selection is risky in mixed WSL/Windows usage

The default analyzer run selected:
- `~/.local/share/com.taurhaus.dev/taurhaus.log.jsonl`

That produced an incomplete picture.

The real operational evidence was in:
- `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.log*.jsonl`

Impact:
- a default run can under-report compaction success/failure unless `--log` is explicitly set

## 4. Compaction member-state diagnostics are polluted / untrustworthy

Current live file:
- `~/.claude/teams/taurhaus-team/state/compaction/architect.json`

Current contents:
- `last_session_id = "sess-123"`
- `last_compaction_timestamp = 2026-03-10T09:24:36.892867608Z`
- `last_delivery_result = "skipped"`

That does not match the real live architect session id:
- `019cbddb-5527-77a0-a457-7908cf7d790b`

This means at least one of these is true:
- a synthetic/manual test path wrote into the live default teams dir
- a test/runtime isolation boundary is leaking
- the member-state diagnostic file is not trustworthy as production evidence

Impact:
- this can mislead later investigations even when actual delivery is working

## 5. Historical compaction card wording remains in inbox history

Recent live inbox entries still contain:
- `[taurhaus] resume_work_after_compaction`

Current code renders:
- `[taurhaus] restored_working_context_after_compaction`

This is not a current delivery bug; it is just historical inbox state remaining on disk.

## What is actually working today

### Codex
- transcript boundary detection: yes
- Taurhaus detection: yes
- inbox append: yes
- mesh wake prompt injection: yes
- wake prompt appears as real transcript user input: yes
- `mesh read` can surface the compaction card: yes
- guaranteed immediate reaction during active turn: no

### Claude
- compact event exists: yes
- hook matches/fires: yes
- hook command now succeeds: yes
- Taurhaus resolves member and returns `additionalContext`: yes
- final model uptake directly observable: no

## Recommended Follow-up Tasks

## A. Add durable mesh wake-delivery telemetry

Objective:
- make wake delivery provable without transcript archaeology

Add structured events for member daemon delivery attempts, including:
- team/member
- senders list
- inbox delta count
- pane id
- pane current command
- tmux injection result
- error if any

Owner split:
- `mesh`

## B. Add explicit “agent consumed reinjection” semantics only where provable

Do not treat `compaction.injected` as “agent consumed context”.

Instead separate:
- `bridge_delivered` / `inbox_appended`
- `wake_prompt_injected`
- `agent_consumed` only if a provable signal exists

For Codex, possible proof signals:
- subsequent `mesh read` within a bounded window
- transcript contains the wake prompt and then a `mesh read` action

For Claude, there may be no trustworthy direct proof without additional hook/client instrumentation.

## C. Fix analyzer default log-source selection

The analyzer should either:
- prefer the active Windows log when running from WSL on this deployment model, or
- print a loud warning that two canonical roots exist and the default may be incomplete

## D. Fix test/runtime isolation for compaction state

The live `architect.json` compaction state file is polluted.

This needs one of:
- strict non-default teams dir for tests and synthetic CLI smoke runs
- explicit guardrail preventing test/synthetic writes into the default live teams root
- cleanup/repair of already-polluted member compaction state files

## E. Optional: stronger Codex post-compaction wake semantics

If the product requirement is “compaction should resume work immediately”, then current generic queued user-message delivery may be insufficient.

Possible next design question:
- is the current generic wake prompt enough, or do we need a more explicit/interrupt-oriented mechanism?

This is product/behavioral work, not just a transport bug fix.

## Final Assessment

The compaction system is not globally broken.

The correct picture is:
- Codex end-to-end delivery transport works
- Claude compact-hook bridge works
- the biggest remaining weakness is not signal loss; it is the difference between successful delivery and guaranteed immediate agent reaction
- the second real weakness is observability: proving wake injection and final uptake still requires too much manual forensic work
