# Compaction Reinjection Audit — 2026-03-10

## Scope

This audit uses live production data only. It traces the current compaction reinjection pipeline across:

- Codex transcript boundary detection
- signal emission and watcher consumption
- compaction processing and target resolution
- delivery into the Codex/Claude reinjection path
- end-to-end timing where the evidence exists

Evidence sources used:

- Windows app log: `/mnt/c/Users/mstie/AppData/Roaming/com.taurhaus.dev/taurhaus*.jsonl`
- Codex signal logs:
  - `~/.claude/teams/taurhaus-team/state/compaction/signals/codex-compaction-signals.jsonl`
  - `~/.claude/teams/2ksim-team/state/compaction/signals/codex-compaction-signals.jsonl`
- live team inboxes under `~/.claude/teams/*/inboxes/*.json`
- live Codex transcript JSONL under `~/.codex/sessions/`
- Claude compact-hook bridge events from the Windows app log

## Verdict

The compaction reinjection system is **partially working** in production.

- **Codex detection and signal delivery are working.**
- **Codex transport delivery is working, but not for every detected event.**
- **Codex wake prompting into tmux is working.**
- **Codex card surfacing to the model is proven in at least one live end-to-end case, but is not yet durably observable from current telemetry for recent natural compactions.**
- **Claude compact-hook delivery is working.**
- **The remaining weak points are skip/stale outcomes on Codex, missing skip reasons, and limited proof of final model uptake after transport.**

## Stage-By-Stage Status

### 1. Codex compaction detection

Status: **working**

Evidence:

- `taurhaus-team` signal log contains real Codex compaction signals, for example:
  - `2026-03-09T21:27:45.356090709Z`
  - `signal_id=606d58c2-bc88-4b85-b748-b6951eb7bb99`
  - `session_id=019cbddb-5527-77a0-a457-7908cf7d790b`
  - `signal_kind=context_compacted`
- the matching Codex transcript file is:
  - `~/.codex/sessions/2026/03/05/rollout-2026-03-05T12-56-33-019cbddb-5527-77a0-a457-7908cf7d790b.jsonl`
- that transcript contains:
  - `2026-03-09T21:27:45.345Z` `type="compacted"`
  - `2026-03-09T21:27:45.359Z` `payload.type="context_compacted"`

This confirms the extractor is reading the real Codex transcript boundary correctly.

### 2. Signal delivery / watcher pickup

Status: **working**

Evidence:

For the same `taurhaus-team/architect` event:

- `2026-03-09T21:27:45.366Z` `compaction.signal_emitted`
- `2026-03-09T21:27:45.371Z` `compaction.signal_consumed`

For a later duplicate/negative case on the same member:

- signal log contains two distinct records at the same transcript boundary:
  - `0725b6b9-6da5-4d57-90ad-0e5c85f0bec8`
  - `33ac7280-4755-47bd-a447-c8eba8007295`
  - both at `2026-03-10T12:39:12.547...Z`
- Windows log shows the watcher consuming them immediately:
  - `2026-03-10T12:39:12.578Z` `compaction.signal_consumed`
  - `2026-03-10T12:39:12.582Z` `compaction.signal_consumed`

So the signal path is not the broken stage.

### 3. Compaction processing / target resolution

Status: **partially working**

Positive example:

- `taurhaus-team/architect`
  - `2026-03-09T21:27:45.361Z` `compaction.detected`
  - `2026-03-09T21:27:45.371Z` `compaction.injected`

That path proves managed-member resolution and card composition succeeded.

Negative examples:

- `2ksim-team/team-lead`
  - `2026-03-10T12:26:56.347Z` `compaction.detected`
  - `2026-03-10T12:26:56.353Z` `compaction.skipped`
  - `2026-03-10T12:46:02.931Z` `compaction.detected`
  - `2026-03-10T12:46:02.931Z` `compaction.stale`
  - `2026-03-10T13:16:16.300Z` `compaction.detected`
  - `2026-03-10T13:16:16.307Z` `compaction.skipped`
  - `2026-03-10T13:41:26.958Z` `compaction.detected`
  - `2026-03-10T13:41:26.969Z` `compaction.skipped`

Aggregate outcome over the last 48h from the analyzer:

- `taurhaus-team`
  - detected: `57`
  - injected: `39`
  - skipped: `11`
  - stale: `9`
- `2ksim-team`
  - detected: `45`
  - injected: `38`
  - skipped: `4`
  - stale: `3`

This means the processor is functional, but not every real compaction ends in a delivered reinjection.

The main observability gap here is that `compaction.skipped` and `compaction.stale` do **not** currently record a machine-readable reason field, so real failures are measurable but not yet explainable from logs alone.

### 4. Reinjection delivery

#### Codex delivery

Status: **working, but only transport is fully provable**

Positive example: `taurhaus-team/architect`, `2026-03-09T21:27:45Z`

- transcript boundary:
  - `2026-03-09T21:27:45.359Z` `context_compacted`
- signal emitted:
  - `2026-03-09T21:27:45.356090709Z`
- app log detection:
  - `2026-03-09T21:27:45.361Z` `compaction.detected`
- app log transport delivery:
  - `2026-03-09T21:27:45.371Z` `compaction.injected`
- inbox evidence:
  - `~/.claude/teams/taurhaus-team/inboxes/architect.json`
  - message timestamp `2026-03-09T21:27:45.361Z`
  - sender `taurhaus`
  - summary `post_compaction_context`
- wake prompt reached the Codex session:
  - transcript contains a real user message at `2026-03-09T21:27:46.027Z`
  - message text: `[mesh] You are "architect" on team "taurhaus-team". Message from taurhaus. Read: mesh read --unread --mark-read --team taurhaus-team --name architect`
- card surfaced into the conversation:
  - transcript contains `mesh read` output at `2026-03-09T21:27:54.295Z`
  - that output includes the Taurhaus compaction card text

This is a real end-to-end successful Codex trace.

Important limitation:

- the currently deployed mesh telemetry now records `wake_delivery` and `compaction_read_surfaced`, but the recent natural compaction window still shows:
  - `wake_delivery`: present
  - `compaction_read_surfaced`: `0`
- so the current telemetry can prove transport, but it does **not yet** prove recent natural compaction cards were surfaced during the observed window
- the strongest proof of actual surfacing is still the live Codex transcript trace above

#### Claude delivery

Status: **working**

Positive example: `taurhaus-team/team-lead`, `2026-03-09T20:58:00Z`

Windows app log shows the whole hook bridge chain:

- `2026-03-09T20:58:00.079Z` `compaction.claude_hook.received`
- `2026-03-09T20:58:00.165Z` `compaction.claude_hook.resolved`
- `2026-03-09T20:58:00.320Z` `compaction.injected`
- `2026-03-09T20:58:00.320Z` `compaction.claude_hook.delivered`

A second successful event happened 11 seconds later:

- `2026-03-09T20:58:11.761Z` `compaction.claude_hook.received`
- `2026-03-09T20:58:11.826Z` `compaction.claude_hook.resolved`
- `2026-03-09T20:58:11.991Z` `compaction.injected`
- `2026-03-09T20:58:11.991Z` `compaction.claude_hook.delivered`

So the Claude path is now working at the Taurhaus bridge level.

Important boundary:

- `compaction.claude_hook.delivered` proves Taurhaus returned `additionalContext` to Claude's hook system
- it does **not** prove the model used the context well afterward
- that is the correct observability boundary for the Claude path today

### 5. End-to-end latency

Status: **good at transport level; model-surface timing still conditional**

#### Codex transport latency

From analyzer output:

- `taurhaus-team`
  - detected -> injected median: `12ms`
  - detected -> injected max: `29ms`
- `2ksim-team`
  - detected -> injected median: `10ms`
  - detected -> injected max: `51ms`

Concrete `taurhaus-team/architect` example:

- `2026-03-09T21:27:45.359Z` transcript compaction boundary
- `2026-03-09T21:27:45.371Z` `compaction.injected`
- transport latency: about `12ms`

#### Codex wake / surfacing latency

Concrete `taurhaus-team/architect` example:

- `2026-03-09T21:27:45.371Z` transport delivery
- `2026-03-09T21:27:46.027Z` wake prompt present as a real user message in the transcript
- `2026-03-09T21:27:54.295Z` `mesh read` output surfaces the card content into the session

So for this successful trace:

- transport delivery: ~`12ms`
- wake visible in session: ~`656ms`
- card surfaced in transcript: ~`8.9s` after transport

This supports the current architectural conclusion: Codex wake behavior is a **next-turn steer**, not a guaranteed immediate interrupt.

#### Claude hook latency

Concrete `taurhaus-team/team-lead` example:

- `2026-03-09T20:58:00.079Z` `compaction.claude_hook.received`
- `2026-03-09T20:58:00.320Z` `compaction.claude_hook.delivered`

End-to-end hook bridge latency: about `241ms`.

## Gaps, Failures, and Silent Drops

### What is clearly not broken

- Codex compaction boundaries are being detected.
- Signals are being written and consumed.
- The processor does reach terminal outcomes.
- Codex cards are being appended to inboxes when injection succeeds.
- Mesh wake prompts are reaching live tmux panes.
- Claude compact hook delivery is functioning.

### What is still weak

1. **Codex delivery is not 100%.**
   - `taurhaus-team`: `39/57` delivered (`68.4%`)
   - `2ksim-team`: `38/45` delivered (`84.4%`)

2. **Skip/stale outcomes are opaque.**
   - The logs show `compaction.skipped` and `compaction.stale`, but not why.
   - This makes debugging real production misses too manual.

3. **Recent surfaced-evidence telemetry is still missing.**
   - Current live telemetry shows wake transport, but `compaction_read_surfaced=0` in the selected 48h window.
   - That does not mean surfacing never happened; the live transcript above proves it did happen at least once.
   - It does mean the current instrumentation has not yet captured a natural post-rollout surfacing event in that window.

4. **Runtime session health is still partial for `taurhaus-team`.**
   - analyzer reports `7/9` runtime members with `session_id`
   - missing: `asset-generator`, `developer2`
   - this did not block the traced `architect` or `team-lead` paths, but it is still a structural risk for exact matching

5. **Claude final uptake remains unprovable.**
   - We can prove hook execution and delivery to Claude's hook interface.
   - We cannot directly prove model quality/usefulness after that point.

## Recommendations

1. **Add structured skip/stale reasons.**
   - `compaction.skipped` and `compaction.stale` should emit a `reason` field.
   - Without that, partial failure analysis stays too manual.

2. **Correlate wake telemetry with signal IDs or delivery IDs.**
   - Current `wake_delivery` telemetry proves tmux injection happened.
   - It does not yet tie a wake event back to a specific compaction signal cleanly.

3. **Keep transport vs consumption separate in all reporting.**
   - `compaction.injected` means delivery into the transport path.
   - `compaction_read_surfaced` or transcript evidence means the card actually surfaced.
   - Those should not be conflated.

4. **Treat Codex as working-but-not-fully-reliable.**
   - The transport chain works.
   - Real misses still exist.
   - The right next work is on skip/stale explainability and stronger surfacing evidence, not on re-litigating whether signals are being detected.

5. **Treat Claude as operationally healthy at the hook boundary.**
   - The recent live data does support that conclusion.
   - Further work there is observability polish, not emergency repair.

## Final Assessment

- **Codex detection:** working
- **Codex signal delivery:** working
- **Codex processing:** partially working
- **Codex transport delivery:** working but incomplete
- **Codex actual surfacing/consumption:** proven in at least one live end-to-end trace, but not yet durably observable from current telemetry in the recent window
- **Claude hook delivery:** working
- **Overall:** the pipeline is real and functional, but still not reliable enough to call fully solved
