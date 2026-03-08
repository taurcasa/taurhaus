# Compaction Pipeline Health - 2026-03-08

## Scope

Validate the live event-driven Codex compaction pipeline for `taurhaus-team`:

- tracked transcript ownership
- raw signal correctness
- watcher consumption
- extractor heartbeat and offsets
- end-to-end outcome for at least one signal

## Snapshot Verified

Inspection time: `2026-03-08` around `22:35-22:36Z`.

Files inspected:

- `~/.claude/teams/taurhaus-team/state/compaction/signals/codex-compaction-signals.jsonl`
- `~/.claude/teams/taurhaus-team/state/compaction/extractor-state.json`
- `~/.claude/teams/taurhaus-team/state/compaction/signal-watcher-state.json`
- `~/.claude/teams/taurhaus-team/state/compaction/*.json`
- `~/.claude/teams/taurhaus-team/runtime/*.json`
- `~/.claude/teams/taurhaus-team/config.json`
- `~/.local/share/com.taurhaus.dev/taurhaus.log.jsonl`
- referenced Codex transcript JSONLs under `~/.codex/sessions/`

## What Is Healthy

### 1. All 7 tracked transcript files currently belong to live managed members

There are 9 live Codex members in `taurhaus-team`, but only 7 unique transcript files because three members currently share the same Codex session/transcript:

- `architect` -> `019cbddb...`
- `developer1` -> `019cbddb...`
- `developer2` -> `019cbddb...`
- `mesh-expert` -> `019cbead...`
- `asset-generator` -> `019cc5b5...`
- `communication-analyst` -> `019cc822...`
- `code-quality-auditor` -> `019cc9b8...`
- `security-auditor` -> `019cc9b9...`
- `developer3` -> `019cce4e...`

So the extractor's `7 active files tracked` is consistent with the current runtime attachment state. There are no currently tracked files that are clearly stale or non-member.

### 2. The signal log is internally consistent

Current Taurhaus-team signal log state:

- total signals: `4`
- file size: `1898` bytes

Signals present:

1. `fa445792-6e46-4271-a5a7-74032f4cfc8c`
   - session `019cbddb...`
   - pane `%219`
   - timestamp `2026-03-08T21:40:33.665Z`
2. `ff5867eb-2e06-4b99-9068-6ee64a6ebf2f`
   - session `019cbddb...`
   - pane `%219`
   - timestamp `2026-03-08T21:48:17.634Z`
3. `07996f17-190c-4078-b7d4-259b2279a75a`
   - session `019cbead...`
   - pane `%158`
   - timestamp `2026-03-08T22:10:50.211Z`
4. `0d961399-d103-4876-8327-96d34b28c368`
   - session `019cbddb...`
   - pane `%217`
   - timestamp `2026-03-08T22:34:57.669Z`

Note: the user snapshot listed 3 signals. There is now a 4th valid later signal at `22:34:57Z`.

### 3. Raw JSONL transcript boundaries match the recorded signals

For all four signals, the referenced transcript and offset neighborhood contain a real:

- `{"type":"event_msg","payload":{"type":"context_compacted"}}`

Examples verified directly:

- `019cbddb...` at offset `123647140`:
  - transcript contains `2026-03-08T21:40:33.665Z` + `context_compacted`
- `019cbddb...` at offset `124698881`:
  - transcript contains `2026-03-08T21:48:17.634Z` + `context_compacted`
- `019cbead...` at offset `9255435`:
  - transcript contains `2026-03-08T22:10:50.211Z` + `context_compacted`
- `019cbddb...` at offset `126142418`:
  - transcript contains `2026-03-08T22:34:57.669Z` + `context_compacted`

This confirms the extractor is not inventing signals or reading the wrong files for these records.

### 4. Watcher consumption is clean

Watcher state:

- `last_consumed_offset = 1898`
- signal log size = `1898`
- `last_event_at = 2026-03-08T22:34:57.848904058+00:00`
- `reconciliation_poll_count = 854`
- `missed_event_recovery_count = 0`

So:

- all Taurhaus-team signals are consumed
- there are `0` unconsumed signals
- watcher consumption is keeping up with the signal log
- no replay/recovery path was needed

### 5. Extractor heartbeat is healthy

Extractor state:

- `heartbeat_at = 2026-03-08T22:35:22.501646492Z`
- `last_error_by_file = {}`
- `last_processed_signal = 0d961399-d103-4876-8327-96d34b28c368`

Tracked offsets:

- 6 files were exactly at EOF at inspection time
- 1 live active file (`019cbddb...`) was only `676` bytes behind EOF, which is consistent with normal new transcript writes between heartbeat and inspection

This is healthy. There is no evidence of a stuck extractor or a file-specific parse failure.

## End-to-End Trace

### Signal `07996f17-190c-4078-b7d4-259b2279a75a`

This is the cleanest full-chain example.

1. Raw transcript:
   - file: `~/.codex/sessions/2026/03/05/rollout-2026-03-05T16-46-39-019cbead-fb7d-7e83-a3b0-361407c6b336.jsonl`
   - offset neighborhood contains:
     - `{"timestamp":"2026-03-08T22:10:50.211Z","type":"event_msg","payload":{"type":"context_compacted"}}`

2. Signal emission:
   - signal id `07996f17-190c-4078-b7d4-259b2279a75a`
   - appended to `codex-compaction-signals.jsonl`
   - emitted at `2026-03-08T22:10:50.413628045Z`

3. Watcher/app consumption:
   - app log contains `compaction.signal_consumed` at `2026-03-08T22:10:50.416Z`

4. Managed member resolution:
   - app log contains `compaction.detected`
   - resolved member: `mesh-expert`
   - team: `taurhaus-team`

5. Terminal outcome:
   - app log contains `compaction.skipped` at `2026-03-08T22:10:50.420Z`
   - member compaction state file `state/compaction/mesh-expert.json` records:
     - `last_session_id = 019cbead...`
     - `last_compaction_timestamp = 2026-03-08T22:10:50.211Z`
     - `last_delivery_result = skipped`

Result: the pipeline completed end to end and reached a terminal outcome. The pipeline is functioning structurally. The business outcome was `skipped`, not `injected`.

## What Is Suspicious

### 1. All detected Taurhaus-team compactions are skipping

In the selected recent window:

- detected: `4`
- injected: `0`
- skipped: `4`
- stale: `0`
- failed: `0`

That means the event-driven pipeline itself is live, but the effective user-facing reinjection behavior is not yet succeeding for these Taurhaus-team members.

### 2. Skip reason is not recorded

Current events record only terminal class:

- `compaction.injected`
- `compaction.skipped`
- `compaction.stale`
- `compaction.failed`

But the skip event does not include a concrete reason field. This makes it impossible to distinguish:

- prompt-boundary guard
- session mismatch
- missing operational snapshot
- empty card payload
- member no longer eligible

without extra manual debugging.

### 3. Shared session ownership is real and still fragile

`architect`, `developer1`, and `developer2` currently share the same Codex session/transcript `019cbddb...`.

The current pipeline did still attribute the observed signals to the correct pane/member in the inspected cases:

- `%219` -> `developer2`
- `%217` -> `architect`

But this remains a high-friction operating mode. When multiple members share one session, correctness depends on exact pane-bound attachment state and on downstream safety guards.

## Bottom Line

The compaction detection pipeline for `taurhaus-team` is structurally healthy:

- extractor is alive
- tracked transcript set matches current runtime attachments
- raw transcript boundaries are real
- signals are emitted correctly
- watcher consumption is complete
- managed member resolution is succeeding

The remaining problem is not detection. It is delivery effectiveness:

- every recent Taurhaus-team compaction reached a terminal outcome
- every one of those terminal outcomes was `skipped`
- current telemetry is not detailed enough to say why from logs alone

## Recommended Follow-Up

1. Add explicit `skip_reason` and `fail_reason` fields to terminal compaction events.
2. Add a structured audit field showing which guard rejected delivery.
3. Keep using runtime attachment state as transcript authority; the current tracked set now looks correct.
4. Investigate why recent Taurhaus-team reinjections are all skipping despite healthy end-to-end detection.
