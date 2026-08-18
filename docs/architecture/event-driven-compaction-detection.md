# Event-Driven Compaction Detection

Task: `#729`
Owner: `architect`
Date: `2026-03-08`

## Goal

Replace poll-based compaction detection with an event-driven architecture that is reliable across:
- Linux native app runtime
- macOS native app runtime
- Windows app using the WSL daemon path

This redesign is only for compaction detection and triggering.

It does **not** replace the broader poll-based session/activity machinery used for:
- UI session lists
- foreground activity / idle heuristics
- generic session snapshots

Those can remain poll-based for now.

## Problem Statement

The current compaction path is structurally fragile because it is downstream of the general session scanner loop:
- compaction detection only happens when the scan loop runs
- the same loop already broke once because scanner startup/liveness drifted
- the watcher reads busy Codex JSONL files opportunistically rather than from a dedicated compaction signal path
- Windows adds another boundary because display and runtime session views diverge

So even when one bug is fixed, the architecture is still prone to missed or delayed detection.

## Design Constraints

1. Do not watch Codex JSONL files directly with the main compaction trigger watcher.
2. The hot trigger path must be:
   - compaction record written
   - extractor notices
   - extractor writes compact signal file
   - low-traffic signal watcher fires
   - Taurhaus runs existing resolution -> card composition -> delivery pipeline
3. Existing downstream delivery logic stays:
   - managed-member resolution
   - operational snapshot load
   - reinjection card composition
   - Codex inbox delivery
   - Claude hook bridge
   - delivery bookkeeping + audit events
4. Windows must keep daemon vs app responsibilities explicit.

## Recommended Architecture

Current shipped note:
- the two-stage signal architecture landed
- the extractor is now `notify`-driven for active Codex transcript files with a `5s` reconciliation fallback
- Windows runs the extractor -> signal log -> watcher -> processor pipeline inside the WSL daemon rather than in the app process

## Summary

Use a two-stage design:

1. **Compaction Signal Extractor**
   - tails active Codex JSONL transcripts
   - extracts only `compacted` / `context_compacted` boundaries
   - writes canonical signal records to a dedicated append-only signal file

2. **Compaction Signal Watcher**
   - watches the low-traffic signal file, not the hot JSONL files
   - on append, loads only new signal records
   - invokes the existing downstream reinjection pipeline

This gives us:
- event-driven trigger semantics
- bounded parsing work on the hot path
- clear platform ownership
- reproducible diagnostics

## Why this is better than today's poll loop

The poll loop does too many jobs at once.

The new design splits responsibilities cleanly:
- session/activity scanner answers "what sessions exist?"
- extractor answers "did a compaction boundary appear in this transcript?"
- signal watcher answers "a compaction event exists; run reinjection now"

That separation is the architectural win.

## Evaluating Extractor Input Strategies

The user asked for an evaluation of two extractor approaches.

### Option A: Tight polling of active JSONL tails

Mechanism:
- keep an active set of Codex transcript files
- poll only those files at a short interval, for example `100-250ms`
- read only appended bytes from the last known offset
- emit a signal only when a compaction boundary is parsed

Pros:
- simple and portable
- no dependence on per-platform file event semantics for busy JSONL files
- resilient to write bursts because the extractor already reads incrementally
- easier to reason about partial-line handling and restart recovery

Cons:
- still technically polling at the extractor layer
- some wasted wakeups during steady active sessions

Assessment:
- this was the best phase-1 choice before active-file watching landed
- current code has since moved to active-file watch events plus periodic reconciliation

### Option B: JSONL-level file-event subscription inside extractor

Mechanism:
- use inotify/FSEvents/ReadDirectoryChangesW equivalent to wake the extractor whenever active JSONL files change
- extractor then reads appended bytes and emits signal records

Pros:
- lower idle overhead than polling
- faster wakeup on active files

Cons:
- Codex JSONLs are high-churn files; the extractor would receive constant events during ordinary non-compaction activity
- more platform-specific edge cases
- more risk of coalesced events / dropped edge semantics / backpressure under sustained writes
- Windows-via-WSL becomes especially awkward because the JSONLs live in Linux/WSL and the app does not

Assessment:
- not recommended for phase 1
- acceptable only as a later optimization after the signal pipeline is stable

## Recommendation

Keep the current shipped design:
- active Codex transcript files are watched inside the extractor
- the extractor persists offsets and emits canonical signal records
- the signal log remains the low-traffic watched handoff to downstream delivery
- a low-frequency reconciliation pass repairs missed watch drift without returning to the old poll-driven compaction path

## Proposed Components

## 1. `CompactionSignalExtractor`

Responsibility:
- maintain the active set of Codex transcript files
- tail only those files
- parse only compaction-relevant records
- write canonical signal records to the signal log
- persist offsets/checkpoints so restart recovery is deterministic

Non-responsibilities:
- no team-member resolution
- no inbox delivery
- no hook handling
- no UI-facing session display

### Inputs

Per active Codex session:
- `session_id`
- `jsonl_path`
- `pane_id` if available
- `project_path`
- `cli_tool`

Source of these inputs:
- Linux/macOS: local runtime session source
- Windows: daemon runtime session source (`LIST_RUNTIME_SESSIONS` or equivalent internal runtime provider)

### Extracted records

Only these transcript boundaries matter:
- `type == "compacted"`
- `type == "event_msg" && payload.type == "context_compacted"`

The extractor should normalize paired `compacted` / `context_compacted` records into one canonical signal.

### Output path

Per team root, append-only signal log:

```text
~/.claude/teams/<team>/state/compaction/signals/codex-compaction-signals.jsonl
```

Rationale:
- low traffic compared with transcripts
- lives under existing team state tree
- easy to inspect manually
- easy to watch with standard filesystem watchers

### Signal record format

```json
{
  "version": 1,
  "signal_id": "uuid",
  "emitted_at": "2026-03-08T20:00:00.123Z",
  "tool": "codex",
  "session_id": "sess-123",
  "pane_id": "%217",
  "project_path": "/home/user/projects/taurhaus",
  "jsonl_path": "/home/user/.codex/sessions/2026/03/08/rollout-...jsonl",
  "jsonl_offset": 18423,
  "transcript_timestamp": "2026-03-08T19:59:59.987Z",
  "signal_kind": "context_compacted"
}
```

Important fields:
- `signal_id`: idempotency key for downstream processing
- `jsonl_offset`: lets us correlate exactly where the boundary was seen
- `pane_id` and `session_id`: critical for runtime/member correlation
- `project_path`: fallback identity anchor

### Extractor state file

Per-host state:

```text
~/.claude/teams/<team>/state/compaction/extractor-state.json
```

Contains:
- active file offsets
- last processed `signal_id` or `(jsonl_path, offset)` tuple
- extractor heartbeat
- last error per file if any

This keeps restart recovery explicit and inspectable.

## 2. `CompactionSignalWatcher`

Responsibility:
- watch the signal log file, not the hot transcripts
- load newly appended signal records
- dispatch each signal into the downstream compaction delivery pipeline

Mechanism:
- Linux: inotify on signal file or containing directory
- macOS: FSEvents on signal directory
- Windows app: no native watcher on Linux files; WSL daemon owns the watcher and publishes resulting events to the app

### Downstream handoff

The watcher calls a new app-level service, conceptually:

```text
process_compaction_signal(signal)
  -> resolve managed member
  -> load operational snapshot
  -> compose reinjection card
  -> attempt delivery
  -> persist delivery state
  -> emit audit events
```

This is mostly the existing logic from `session_scanner/compaction.rs`, just fed by a signal record instead of a session-scan cycle.

## 3. `CompactionSignalProcessor`

Responsibility:
- pure downstream processing from canonical signal -> terminal outcome
- this replaces today's coupling to `process_codex_compaction_events(sessions)`

Inputs:
- canonical signal record
- runtime provider for pane/process liveness checks
- team config store
- runtime store
- operational snapshot store

Outputs:
- `compaction.detected`
- `compaction.injected` / `skipped` / `stale` / `failed`
- updated compaction state
- inbox append for Codex

This is the stable core that both Linux and Windows should share.

## End-to-End Data Flow

### Codex path

```text
active Codex transcript
  -> CompactionSignalExtractor tails appended bytes
  -> extractor parses compaction boundary
  -> extractor appends canonical signal to codex-compaction-signals.jsonl
  -> CompactionSignalWatcher receives file-change event
  -> watcher reads newly appended signal records
  -> CompactionSignalProcessor resolves managed member
  -> processor composes OperationalReinjectionCard
  -> processor appends MeshInboxStore message for Codex member
  -> processor records delivery result + emits audit events
```

### Claude path

Claude should remain hook-driven.

```text
Claude compaction
  -> Claude SessionStart(source=compact) hook fires
  -> taurhaus compact-hook bridge resolves member by runtime session_id
  -> bridge loads operational snapshot
  -> bridge renders additionalContext
  -> bridge records delivery result + emits audit events
```

Important boundary:
- this redesign does **not** replace the Claude hook path
- it only replaces Codex compaction detection

## ASCII Architecture Diagram

```text
                    +-----------------------------+
                    |  Poll-based session/activity|
                    |  scanner (still used)       |
                    |  - UI session list          |
                    |  - activity/idle state      |
                    +-------------+---------------+
                                  |
                                  | active Codex runtime sessions
                                  v
                    +-----------------------------+
                    | CompactionSignalExtractor   |
                    | - tracks active JSONLs      |
                    | - tails appended bytes      |
                    | - emits canonical signals   |
                    +-------------+---------------+
                                  |
                                  | append-only JSONL signal log
                                  v
                    +-----------------------------+
                    | codex-compaction-signals    |
                    | .jsonl                      |
                    +-------------+---------------+
                                  |
                                  | file change event
                                  v
                    +-----------------------------+
                    | CompactionSignalWatcher     |
                    | - watches signal file/dir   |
                    | - reads new signal records  |
                    +-------------+---------------+
                                  |
                                  v
                    +-----------------------------+
                    | CompactionSignalProcessor   |
                    | - member resolution         |
                    | - reinjection card compose  |
                    | - delivery + bookkeeping    |
                    +------+----------------------+
                           |
                 +---------+----------+
                 |                    |
                 v                    v
      +-------------------+   +----------------------+
      | Codex inbox append |   | Audit/state stores  |
      | MeshInboxStore     |   | compaction.* events |
      +-------------------+   +----------------------+

Claude path stays separate:
Claude compact -> Claude hook bridge -> additionalContext + delivery bookkeeping
```

## Platform Handling

## Linux native

Ownership:
- extractor runs in app process
- signal watcher runs in app process
- processor runs in app process

Path handling:
- all transcript paths are Linux-native
- signal file lives in local `~/.claude/teams/...`

Operational implication:
- simplest implementation path
- best platform to land first

## macOS native

Ownership:
- extractor runs in app process
- signal watcher runs in app process
- processor runs in app process

Path handling:
- same architecture as Linux
- transcript root differs by host home layout, but still native local files

Operational implication:
- same conceptual path as Linux
- only watcher implementation differs (`FSEvents` instead of inotify)

## Windows via WSL daemon

Ownership:
- extractor runs in the WSL daemon, not the Windows UI app
- signal watcher also runs in the WSL daemon
- processor should run in the WSL daemon or be daemon-owned with a narrow event RPC to the app

Recommendation:
- keep the entire Codex compaction detection pipeline daemon-owned on Windows until the terminal outcome event is produced
- the Windows app should consume resulting audit state and UI state, not own the raw Linux transcript watcher path

Reason:
- the transcripts live in WSL/Linux
- the signal file also lives in the Linux team-state tree
- pushing raw signal-file watching back into the Windows app reintroduces cross-boundary fragility

Recommended Windows path:

```text
WSL daemon:
  runtime sessions -> extractor -> signal file -> signal watcher -> processor -> inbox/state/events
Windows app:
  reads resulting state/events, not raw transcript signals
```

This is the cleanest platform split.

## Lifecycle

## Startup

On app/daemon startup:
1. load extractor state file
2. enumerate currently active Codex runtime sessions
3. register active transcript files
4. start extractor loop
5. start signal watcher
6. replay any unconsumed signal records after the last durable checkpoint

## Session discovery

New active Codex sessions are discovered from the existing runtime/session machinery.

Important point:
- general session scanning may stay poll-based
- compaction detection no longer depends on each scan cycle to parse transcripts

The session scanner now feeds only the extractor's active-file registry.

## Session end

When a Codex session disappears:
- extractor removes it from the active set
- its final offset/checkpoint remains in extractor state
- no more transcript tail work occurs for that session

## Recovery on restart

On restart:
- extractor loads saved offsets
- if transcript file still exists and has grown, extractor resumes from last committed offset
- signal watcher resumes from last consumed signal-log offset
- duplicate downstream processing is prevented by `signal_id` idempotency plus existing compaction-state checks

## Failure Modes and Recovery

### Extractor crashes

Effect:
- no new compaction signals are emitted

Recovery:
- supervised restart
- resume from saved `(jsonl_path, offset)` checkpoints
- emit extractor heartbeat and last-error status into a small status file

### Signal file corruption

Effect:
- watcher may fail to parse recent records

Recovery:
- use JSONL, not a mutable single JSON blob
- watcher should stop only at the last malformed line and keep prior good offset
- operator can truncate or rotate corrupted tail safely

### Watcher misses a file event

Effect:
- signal file changed but watcher was not notified

Recovery:
- watcher also performs a low-frequency reconciliation poll on signal-file size/offset, for example every `5s`
- this is a watcher self-heal, not the old transcript poll model

This is important: the hot path is event-driven, but the watcher can still have a cheap guardrail.

### Extractor sees duplicate paired records

Effect:
- `compacted` and `context_compacted` could double-trigger one compaction

Recovery:
- normalize paired records into one canonical signal before writing the signal log

### Runtime metadata is missing (`session_id`, `pane_id`)

Effect:
- downstream resolution can become ambiguous

Recovery:
- extractor should still emit the raw signal
- processor marks it unresolved and emits a new explicit failure event, for example `compaction.unresolved`
- do not silently drop the signal

This is a recommended observability improvement over current behavior.

### Team/member removed after signal emitted

Effect:
- delayed processing may target stale state

Recovery:
- keep current membership and stale-delivery guards
- skip state recreation after removal/disband

## What Stays Poll-Based vs Event-Driven

## Stays poll-based for now

- UI session list refresh
- session activity/idle heuristics
- foreground/active attribution
- generic daemon activity hub snapshots

## Becomes event-driven

- Codex compaction detection
- compaction trigger propagation
- downstream compaction processing kickoff

## Future possibility, not current scope

Session activity detection could later move toward more event-driven signals too, but that is a separate architecture problem. It should not be bundled into compaction redesign.

## Migration Path

### Phase 0: isolate downstream processor

Refactor current logic so resolution + delivery + bookkeeping can be called from a canonical signal record instead of from a scan cycle.

Goal:
- preserve working downstream behavior
- remove direct dependency on `process_codex_compaction_events(sessions)`

### Phase 1: add extractor and signal log on Linux/macOS

Implement:
- extractor loop
- signal log format
- signal watcher
- signal processor
- audit events

Keep current poll-based compaction path disabled or removed for Codex on these platforms.

### Phase 2: move Windows Codex compaction ownership fully into daemon

Implement:
- extractor in WSL daemon
- watcher in WSL daemon
- processor in daemon-owned compaction service
- narrow result/event propagation back to Windows app

This avoids repeating the display/runtime confusion that already broke the old architecture.

### Phase 3: observability hardening

Add explicit events:
- `compaction.signal_emitted`
- `compaction.signal_consumed`
- `compaction.unresolved`
- `compaction.processor.started`
- `compaction.processor.completed`
- extractor heartbeat/status metrics

## Recommended Implementation Tasks

1. Introduce `CompactionSignalRecord` schema and append-only signal-log store.
2. Extract current downstream logic into `CompactionSignalProcessor` with signal-record input.
3. Add `CompactionSignalExtractor` with offset persistence and paired-record normalization.
4. Add `CompactionSignalWatcher` on signal file / directory.
5. Add explicit unresolved/error events instead of silent drop paths.
6. Land Linux/macOS app-owned version first.
7. Move Windows Codex compaction handling into the WSL daemon end-to-end.
8. Extend diagnostics to inspect extractor state, signal-log offsets, and watcher health.

## Recommended Event Set

For this redesign, add these structured events:
- `compaction.signal_emitted`
- `compaction.signal_consumed`
- `compaction.signal_replayed`
- `compaction.unresolved`
- `compaction.extractor.heartbeat`
- `compaction.extractor.failed`
- `compaction.watcher.missed_event_recovered`

These events close the current observability gaps between transcript boundary, detection, and terminal outcome.

## Final Recommendation

Build the replacement around:
- **polling extractor + event-driven signal watcher**
- **canonical append-only compaction signal log**
- **shared downstream processor**
- **Windows daemon ownership for Codex compaction path**

That gives the cleanest architecture with the fewest platform traps.

The key decision is not "polling vs event-driven" in the abstract.

It is this:
- stop using the general session scan loop as the compaction trigger
- move compaction onto its own dedicated signal pipeline

That is the real architectural fix.
