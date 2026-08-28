# Compaction Pipeline Flow

> **Retired design (2026-08, C3).** This document describes the deleted Codex transcript extractor/watcher/processor pipeline and is kept only as historical context. Native hooks are now the only compaction owner; use [`docs/operations/compaction-testing.md`](../operations/compaction-testing.md) for the live diagnostic flow.

Task: `#728`
Owner: `architect`
Date: `2026-03-08`

This document maps the end-to-end compaction detection and reinjection pipeline for both supported runtime shapes:
- Linux/macOS native app scan path
- Windows app via WSL daemon proxy

Use this document to answer one question systematically:

> "The pipeline broke. Exactly which boundary still works, and where does it stop working?"

For checkpoint automation, use:
- `python3 scripts/analyze-compaction.py --team <team> --last 30m`

The analyzer now prints a `Checkpoint Matrix` section that maps to the checkpoints below.

## Scope

This flow covers:
1. Codex transcript compaction detection
2. managed-member resolution
3. post-compaction reinjection delivery
4. delivery bookkeeping and audit logging
5. compact-hook readiness as the parallel delivery path (Claude always; Codex when `harness.codex_compaction=hooks`)

Core source files:
- `src-tauri/src/session_scanner/compaction_extractor.rs`
- `src-tauri/src/session_scanner/compaction_watcher.rs`
- `src-tauri/src/coordination/compaction_processor.rs`
- `src-tauri/src/coordination/stores/compaction_signal.rs`
- `src-tauri/src/coordination/stores/compaction.rs`
- `src-tauri/src/coordination/reinjection.rs`
- `src-tauri/src/coordination/compact_hook.rs` (one hook bridge for Claude, Codex and Grok)
- `src-tauri/src/coordination/compaction_events.rs` (event emitters)
- `src-tauri/src/startup/compaction.rs` (owner selection)
- `src-tauri/src/daemon/compaction.rs` (daemon-owned runtime + mode switch)
- `src-tauri/src/commands/terminal_settings.rs` (`harness.codex_compaction` reconcile/installer)
- `scripts/analyze-compaction.py`

## High-Level Pipeline

### Common logical flow

1. Codex writes `compacted` and/or `context_compacted` into its JSONL transcript.
2. The active-session registry tells the extractor which Codex JSONLs are managed and worth tracking.
3. `CompactionSignalExtractor` reads appended bytes and emits canonical signal records to the per-team signal log.
4. `CompactionSignalWatcher` consumes newly appended signal-log records from its persisted offset.
5. `CompactionSignalProcessor` resolves the managed member from team config + runtime attachment state.
6. Taurhaus loads the member's `OperationalContextSnapshot`.
7. Taurhaus composes an `OperationalReinjectionCard`.
8. Delivery is attempted:
   - Codex in `transcript` mode (default): append the reinjection card to the mesh inbox for that member
   - Claude, and Codex in `hooks` mode: return the card from the hook bridge as `hookSpecificOutput.additionalContext` (registry `compaction_delivery: HookStdout`)
   - Grok: the hook bridge fires, but the card is appended to the member's mesh inbox (registry `compaction_delivery: MeshInbox`) because grok documents passive-hook stdout as ignored
9. Delivery state is persisted.
10. Audit events are emitted to the structured JSONL log.

One bridge serves all three tools. `coordination/compact_hook.rs` parses a single `CompactHookInput`, infers the tool from the reserved `GROK_*` env names grok injects into every hook process and otherwise from `transcript_path` (Codex rollout vs Claude JSONL), resolves the member by runtime `session_id` and then normalized/canonical `cwd` across all teams (ambiguous matches are skipped), and books the result through `record_delivery_at(teams_dir, tool, …)`. For Codex the compaction timestamp is taken from the transcript tail.

The accepted hook events differ by delivery. `SessionStart` with `source=compact` is the reinjection event everywhere. `PostCompact` is accepted only for a harness whose registry delivery is `MeshInbox` — grok, whose session-start source never reports `compact`; for a stdout-answered harness a `PostCompact` payload is skipped as `post_compact_signal_only`, because that event has no documented stdout contract. grok also loads `~/.claude/settings.json` hooks, so one compaction can reach the bridge twice; the registry declares `compaction_hook_compat_import` and the bridge deduplicates inside a bounded window, logging `compaction.hook.compat_import` once per run.

Antigravity has no compaction hook (`compaction_hook: false`) and no transcript parser, so an `agy` compaction is not observed.

Codex uses that path only when `harness.codex_compaction=hooks` — opt-in, default `transcript`, requires Codex ≥ 0.147, and managed launches get `--dangerously-bypass-hook-trust`. Grok's hook directory (`<GROK_HOME>/hooks`) is always trusted, so its installer needs no equivalent flag. Events: `compaction.{claude,codex,grok}_hook.{received,resolved,delivered,skipped,failed}`, `compaction.hook.compat_import` and `compaction.compact_hook.failed`.

## Platform Flow Map

### Ownership before platform

Compaction has exactly one owner on every platform, logged as `compaction.owner.selected {owner, status, reason}`:

1. **hooks** when `harness.codex_compaction=hooks` is active — the extractor and watcher are stopped entirely
2. otherwise **daemon**, when a daemon is configured *and* connected
3. otherwise **app**, as a fallback that is released again when the daemon recovers

The native daemon runs on Linux and macOS as well as Windows, so "daemon-owned" is the normal case on all three. The flows below are the transcript-mode topologies for an app-owned and a daemon-owned pipeline.

### App-owned flow (no daemon configured or reachable)

```text
Codex JSONL append
  -> app runtime scan updates active Codex transcript set
  -> CompactionSignalExtractor reads appended bytes
  -> codex-compaction-signals.jsonl append
  -> CompactionSignalWatcher consumes new signal records
  -> CompactionSignalProcessor resolves managed member
  -> compose reinjection card
  -> MeshInboxStore::append(...)
  -> record_delivery_at(...)
  -> compaction.injected / skipped / stale / failed
```

Key boundary facts:
- the app owns the local runtime scan loop only while no daemon is available
- the scan loop only maintains the active transcript registry; it is no longer the compaction trigger
- compaction detection/delivery is extractor -> signal log -> watcher -> processor

### Daemon-owned flow (the normal case on Linux, macOS, and Windows)

```text
Codex JSONL append inside WSL
  -> daemon runtime scan updates active Codex transcript set
  -> daemon CompactionSignalExtractor reads appended bytes
  -> daemon codex-compaction-signals.jsonl append
  -> daemon CompactionSignalWatcher consumes new signal records
  -> daemon-owned CompactionSignalProcessor resolves member
  -> MeshInboxStore::append(...)
  -> record_delivery_at(...)
  -> compaction.injected / skipped / stale / failed
  -> Windows app observes resulting audit/runtime state
```

Key boundary facts:
- the daemon owns both runtime scanning and the Codex compaction pipeline
- the app does not watch transcript files while the daemon owns the pipeline
- the app pushes the mode with `set_codex_compaction_mode` (protocol v9); the daemon flips hooks↔transcript at runtime (`request_mode_and_wait`) and starts its compaction runtime on its own thread after bind
- `LIST_DISPLAY_SESSIONS` remains UI-safe and strips transcript metadata
- `LIST_RUNTIME_SESSIONS` remains the runtime-authoritative view for app-side status/debugging

## Ownership Divergence Points

| Boundary | App-owned fallback | Daemon-owned (normal) | Why it matters |
|---|---|---|---|
| Scan loop owner | app process | daemon `SessionActivityHub` thread | scanner health can fail in different places |
| Display session source | local `scan_sessions_for_display()` | daemon snapshot / `LIST_DISPLAY_SESSIONS` | the app falls back to a local scan only when the daemon is unavailable |
| Runtime session source for compaction | same local scan cycle | daemon `LIST_RUNTIME_SESSIONS` RPC | a daemon-fed path needs an explicit runtime metadata fetch |
| Transcript metadata availability | directly present in local `RuntimeSession` | absent from display view, present only in runtime view | using the wrong view breaks compaction |
| Where display updates are cached | app memory | daemon hub snapshot/versioned state | the UI can look healthy while runtime metadata is wrong |
| Where compaction extractor/watcher/processor run | app process | daemon process | ownership is a runtime decision, not a platform constant |

## Detailed Integration Boundaries

### B1. Transcript boundary appears

Implementation:
- Codex writes either:
  - `{"type":"compacted", ...}`
  - or `{"type":"event_msg", "payload":{"type":"context_compacted"}}`
- parsing and paired-boundary normalization live in `read_appended_compaction_boundaries(...)`

Key file:
- `src-tauri/src/session_scanner/compaction_extractor.rs`

### B2. Session scanner discovers runtime transcript metadata

Implementation:
- the scan cycle publishes `RuntimeSession` values through `publish_compaction_runtime_sessions(...)` before stripping them to the display-safe view
- proxied path: runtime metadata must come from `LIST_RUNTIME_SESSIONS`, not `LIST_DISPLAY_SESSIONS`
- a degraded scan (process inventory unreadable) publishes nothing: the extractor keeps its previous transcript set and offsets

Key files:
- `src-tauri/src/session_scanner/cache.rs`
- `src-tauri/src/daemon/handlers.rs`
- `src-tauri/src/daemon/protocol.rs`

### B3. Display scan finalization triggers compaction watcher

Implementation:
- `finalize_display_scan()` calls `publish_compaction_runtime_sessions(runtime_sessions)` when runtime sessions are available
- this is the boundary that was bypassed by the Windows early-return bug

Key files:
- `src-tauri/src/session_scanner/cache.rs`
- `src-tauri/src/session_scanner/scans.rs`

### B4. Compaction lines are read incrementally

Implementation:
- extractor tracks per-JSONL offsets
- reads only appended committed lines
- skips partial trailing lines until complete

Key functions:
- `read_appended_compaction_boundaries()`
- `extract_compaction_boundary()`
- `normalize_cross_pass_pair()`

Key file:
- `src-tauri/src/session_scanner/compaction_extractor.rs`

### B5. Parsed compaction event resolves to a managed member

Implementation:
- `resolve_managed_codex_signal()` matches by:
  - normalized project path
  - runtime `session_id`
  - runtime `pane_id`
  - recent activity tie-breakers
- `compaction.detected` is only emitted after this resolution succeeds

Key file:
- `src-tauri/src/coordination/compaction_processor.rs`

### B6. Operational reinjection payload is composed

Implementation:
- loads `OperationalContextSnapshot`
- loads role/task/boundary data
- builds `OperationalReinjectionCard`

Key file:
- `src-tauri/src/coordination/reinjection.rs`

### B7. Delivery attempt reaches terminal outcome

Codex path:
- validate current attachment, runtime match, prompt boundary, live Codex pane, and resumable task context
- append message to `MeshInboxStore`
- persist delivery result

Hook path (Claude always; Codex when `harness.codex_compaction=hooks`):
- hook bridge receives `SessionStart(source=compact)`
- infers the tool from `transcript_path`
- resolves the managed member by runtime `session_id`, then normalized `cwd` (ambiguous ⇒ skipped)
- loads snapshot
- returns `hookSpecificOutput.additionalContext`
- persists delivery result via `record_delivery_at(teams_dir, tool, …)`

Key files:
- `src-tauri/src/coordination/compaction_processor.rs`
- `src-tauri/src/coordination/compact_hook.rs`

### B8. Delivery bookkeeping and audit logging are persisted

Implementation:
- `MemberCompactionStore` persists `last_session_id`, timestamp, and terminal result
- emits one of:
  - `compaction.detected`
  - `compaction.injected`
  - `compaction.skipped`
  - `compaction.stale`
  - `compaction.failed`

Upstream of the terminal result, the pipeline also emits `compaction.owner.selected` / `.failed`, `compaction.signal_emitted` / `_consumed` / `_replayed` / `_failed`, `compaction.unresolved`, `compaction.extractor.heartbeat` / `.failed`, `compaction.watcher.missed_event_recovered`, and `compaction.<tool>_hook.resolved` on the hook path.

Key files:
- `src-tauri/src/coordination/stores/compaction.rs`
- `src-tauri/src/coordination/compaction_events.rs`

## Verification Checkpoints

Use these checkpoints in order. Do not skip ahead.

### CP0. Which process owns the pipeline

What this proves:
- you are inspecting the process that actually runs detection and delivery

Verify:
```bash
grep 'compaction.owner.selected' <taurhaus.log.jsonl> | tail -n 5
```

Working looks like:
- one `owner` value — `hooks`, `daemon`, or `app` — with a `reason`

Broken looks like:
- `compaction.owner.failed`, or an `app` owner while a daemon is connected

### CP1. Codex JSONL contains compaction boundary

What this proves:
- Codex actually compacted
- Taurhaus is not being asked to detect a nonexistent event

Verify:
```bash
grep -R 'context_compacted\|"type":"compacted"' ~/.codex/sessions | tail
```

Working looks like:
- at least one matching line in the target session file
- ideally includes the session you expect to have compacted

Broken looks like:
- no matching transcript boundary at all
- or you are inspecting the wrong transcript file

Notes:
- current automated analyzer can only infer CP1 from downstream events if detection already succeeded
- direct transcript inspection is still the strongest check

### CP2. Session scanner ran scan cycles

What this proves:
- scanner loop is alive on the relevant platform path

Verify:
```bash
grep 'session_scanner.scan.completed' <taurhaus.log.jsonl> | tail
python3 scripts/analyze-compaction.py --team <team> --last 30m
```

Working looks like:
- `session_scanner.scan.completed` events exist
- latest run has positive `session_count`

Broken looks like:
- no `session_scanner.scan.completed` events
- or latest run reports `session_count=0` for all cycles

### CP3. Compaction record was parsed

What this proves:
- watcher read the appended lines and recognized a compaction signal

Verify:
```bash
grep 'compaction.detected' <taurhaus.log.jsonl> | tail
python3 scripts/analyze-compaction.py --team <team> --last 30m
```

Working looks like:
- `compaction.detected` for the expected team/member/session

Broken looks like:
- CP1 is true, CP2 is true, but no `compaction.detected` event exists

### CP4. Event resolved to a managed member

What this proves:
- Taurhaus successfully mapped the compaction to a configured team member with usable runtime metadata

Verify:
```bash
grep 'compaction.detected' <taurhaus.log.jsonl> | tail -n 20
```

Working looks like:
- `compaction.detected` exists with `team_name`, `member_name`, `tool`, `session_id`

Broken looks like:
- transcript boundary exists, but there is no `compaction.detected`
- or runtime member state is missing `session_id`, making resolution ambiguous

Important current limitation:
- there is no dedicated `compaction.resolved` event today
- `compaction.detected` is the effective proof because it is emitted only after resolution succeeds

### CP5. Delivery attempt reached a terminal outcome

What this proves:
- Taurhaus got past resolution and tried to complete the delivery path

Verify:
```bash
grep 'compaction.injected\|compaction.skipped\|compaction.stale\|compaction.failed' <taurhaus.log.jsonl> | tail
python3 scripts/analyze-compaction.py --team <team> --last 30m
```

Working looks like:
- one terminal event appears after `compaction.detected`

Broken looks like:
- `compaction.detected` exists, but no terminal delivery event follows

### CP6. Runtime session records are healthy enough for exact correlation

What this proves:
- managed-member resolution and delayed delivery guards have the runtime metadata they need

Verify:
```bash
python3 scripts/analyze-compaction.py --team <team> --last 30m
jq . ~/.claude/teams/<team>/runtime/<member>.json
```

Working looks like:
- analyzer reports strong runtime `session_id` health
- runtime record has matching `session_id`, `pane_id`, `cli_tool`

Broken looks like:
- runtime record has `session_id: null`
- multiple Codex members share the same project and cannot be disambiguated cleanly

### CP7. Codex delivery persisted to inbox and compaction state

What this proves:
- reinjection was recorded durably, not just attempted in memory

Verify:
```bash
jq . ~/.claude/teams/<team>/state/compaction/<member>.json
jq . ~/.claude/teams/<team>/inboxes/<member>.json | tail
```

Working looks like:
- compaction state shows the expected `last_session_id`, timestamp, and delivery result
- Codex inbox contains the post-compaction payload when result is `injected`

Broken looks like:
- terminal event exists in logs but state/inbox does not reflect it
- or stale bookkeeping reappears after disband/remove

### CP8. Compact hook bridge is ready

What this proves:
- members can receive post-compaction operational context through the hook path

Paths follow `PlatformPaths::hook_settings_path()` and `hook_script_dir()`, so they move with `TAURHAUS_CLAUDE_DIR`. Substitute your resolved Claude root for `~/.claude` below.

Verify (Claude):
```bash
python3 scripts/analyze-compaction.py --team <team> --last 30m
jq . ~/.claude/settings.json
ls ~/.claude/hooks/taurhaus-session-start-compact.*
```

Working looks like:
- analyzer reports hook installed (it checks the Claude hook only)
- `settings.json` has `SessionStart` matcher `compact`
- hook script exists, plus the `taurhaus-session-start-compact.executable` marker — the installer re-installs when the app exe path changes

Verify (Codex, only in `hooks` mode):
```bash
jq . ${CODEX_HOME:-~/.codex}/hooks.json
grep 'compaction.codex_hook' <taurhaus.log.jsonl> | tail
```

Working looks like:
- `hooks.json` has `SessionStart`, matcher `compact`, `additionalContextLimit` 12000
- `${CODEX_HOME:-~/.codex}/hooks/taurhaus-session-start-compact.*` exists

`compaction.codex_hook.reconciled` is **not** required evidence. It is emitted only when reconciliation actually changed files, so a hook that was already correct logs nothing at all. An empty log with a correct `hooks.json` is the steady state, not a failure.

Broken looks like:
- matcher missing
- script missing
- `compaction.codex_hook.degraded` / `.unsupported`
- no hook fire evidence when compactions should have fired

Indeterminate, not broken:
- `compaction.codex_hook.version_unknown` — the Codex version could not be resolved, so reconciliation left the hook exactly as it found it. An installed hook stays the active compaction owner in that state (`startup/compaction.rs`); check `hooks.json` to see which case you are in.

## Diagnostic Use Order

When a new failure is reported, use this order:

1. CP1 transcript boundary exists
2. CP2 scanner alive
3. CP3 parsed signal emitted
4. CP4 managed resolution succeeded
5. CP5 terminal delivery event exists
6. CP6 runtime state healthy
7. CP7 durable Codex delivery state present
8. CP8 hook ready if the affected member uses the hook path

This order matters. If CP2 is false, investigating CP5 is wasted effort.

## Known Failure Modes

### FM1. Windows daemon proxy early return bypassed compaction processing

Tracking:
- `#715`

Symptom:
- real compactions exist in Codex JSONL
- no `compaction.detected` events appear on Windows

Root cause:
- Windows display-session path returned daemon sessions early and skipped app-side compaction processing

Checkpoint signature:
- CP1 passes
- CP2 may pass
- CP3 fails

### FM2. Scanner not running scan cycles

Tracking:
- `#726`

Symptom:
- no fresh `session_scanner.scan.completed` events
- analyzer shows no scanner events or all-zero cycles

Checkpoint signature:
- CP2 fails

### FM3. Null runtime member records cause ambiguous resolution

Tracking:
- `#727`

Symptom:
- active Codex members exist, but runtime `session_id` is null
- delayed delivery and managed resolution fall back to ambiguous project/pane matching

Checkpoint signature:
- CP4 unstable or failing
- CP6 warns/fails

### FM4. UI-safe display-session view stripped metadata used by runtime logic

Tracking:
- `#710`

Symptom:
- session listing looks alive in UI
- runtime/session correlation silently loses `session_id` or `jsonl_path`

Root cause:
- using `LIST_DISPLAY_SESSIONS` where `LIST_RUNTIME_SESSIONS` was required

Checkpoint signature:
- CP2 passes
- CP3 or CP4 fails
- CP6 warns/fails

### FM5. Project-scoped instead of PID-scoped Codex resolution

Tracking:
- `#709`

Symptom:
- same-project Codex panes interfere with one another
- compaction or activity attribution lands on the wrong member/session

Checkpoint signature:
- CP4 flaky or ambiguous
- CP6 may appear partially healthy but still mis-correlate

### FM6. Prompt-boundary drift before delayed Codex injection

Tracking:
- fixed under `#690`

Symptom:
- reinjection lands after the next turn started

Current guard:
- pending delivery stores JSONL path + observed length
- delivery skips if the JSONL grew before injection

Checkpoint signature:
- CP5 ends as `compaction.skipped`

### FM7. Compaction bookkeeping recreated after team disband/member removal

Tracking:
- fixed under `#691`

Symptom:
- stale `state/compaction/...` reappears after teardown

Current guard:
- delivery bookkeeping checks active team config before persisting state

Checkpoint signature:
- CP7 would show unexpected state recreation after teardown

## Current Gaps In Observability

Most of the gaps recorded here have since shipped: resolution is proved by `compaction.<tool>_hook.resolved` on the hook path and by `compaction.signal_consumed` / `compaction.unresolved` on the transcript path, and ownership by `compaction.owner.selected`.

Still worth adding later:
- explicit event for `runtime session fetched from daemon` vs `display session fetched from daemon`
- checkpoint-specific skip/fail reason fields in terminal compaction events

## Companion Diagnostic

The existing analyzer is the current checkpoint companion:

```bash
python3 scripts/analyze-compaction.py --team taurhaus-team --last 30m
```

It prints a `Checkpoint Matrix` with a pass/warn/fail/unknown status per row. **The matrix uses the analyzer's own numbering, which diverges from this document's from CP6 onward.** Read it through this map:

| Analyzer row | Analyzer's own label | Section in this document |
|---|---|---|
| CP1–CP5 | same subjects, same order | CP1–CP5 |
| CP6 | Mesh wake prompt transport happened | no numbered section here |
| CP7 | Compaction card was surfaced by mesh read | CP7 — the analyzer checks read-side surfacing, the section checks the on-disk state |
| CP8 | Runtime member records can support exact session resolution | **CP6** |
| CP9 | Claude compact hook bridge is installed | **CP8**, Claude half only |

The hook row is the one to read carefully: it inspects `~/.claude/settings.json` and `~/.claude/hooks/taurhaus-session-start-compact.*` and nothing else. The analyzer never opens Codex `hooks.json`, so a `hooks`-mode Codex hook has no checkpoint at all.

What still remains manual:
- strongest-form CP1 transcript inspection with `grep` on the actual Codex JSONL
- detailed inbox/state inspection for CP7 when chasing one specific member
- Codex hook installation in `hooks` mode — use the Codex block in CP8 above

## Bottom Line

The compaction pipeline is not one linear path. It is a set of platform-specific scan sources that must converge back into the same app-side reinjection pipeline.

The main debugging rule is:
- verify the transcript boundary first
- then verify scanner liveness
- then verify parsed detection
- then verify managed resolution
- only then investigate delivery bookkeeping

If we debug in that order, new failures become isolatable instead of ad-hoc.
