# Delivery Guard Audit

Task: `#754`  
Owner: `architect`  
Date: `2026-03-09`

## Scope

This audit reviewed guards, skip conditions, and safety checks across the current delivery and reinjection surface, with emphasis on places where older direct-tmux assumptions could still be shaping current file-based behavior.

Reviewed areas:

- `src-tauri/src/coordination/compaction_processor.rs`
- `src-tauri/src/session_scanner/compaction.rs`
- `src-tauri/src/coordination/reinjection.rs`
- `src-tauri/src/coordination/stores/compaction.rs`
- `src-tauri/src/coordination/stores/inbox.rs`
- `src-tauri/src/coordination/claude_hooks.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/coordination/pipelines/{initialize,members,lifecycle,helpers}.rs`
- `src-tauri/src/session_scanner/mod.rs`
- `src-tauri/src/session_scanner/idle/codex.rs`
- `src-tauri/src/daemon/handlers.rs`
- `src-tauri/src/daemon/compaction.rs`

This is analysis only. No code changes were made.

## Executive Summary

Main conclusion:

1. Most current guards in the active delivery path are valid for the current mechanisms.
2. The main wrong-guard pattern is not in the active processor anymore. It survives as stale logic and stale mental model in the legacy poll-based compaction module.
3. The clearest live bug in the audited read/write surface is not a tmux-era assumption, but an inbox corruption fallback: `MeshInboxStore::load()` treats a corrupt inbox as empty, which can hide real delivered messages and let later appends overwrite visibility.

Immediate follow-up candidates:

1. Fix inbox corruption handling in `MeshInboxStore::load()`.
2. Archive/remove or loudly mark `session_scanner/compaction.rs` as legacy/inactive so its guards do not keep contaminating current reasoning.
3. Remove or simplify dead defensive branches such as Claude hook `EmptyAdditionalContext`.

## Findings

| ID | Severity | Location | Guard / check | Assumption behind it | Valid now? | Recommendation |
|---|---|---|---|---|---|---|
| G1 | High | `src-tauri/src/coordination/stores/inbox.rs` | Corrupt inbox file loads as empty | “If inbox JSON is broken, best fallback is to act like there are no messages” | No | Modify immediately |
| G2 | Medium | `src-tauri/src/session_scanner/compaction.rs` | Entire legacy poll-based compaction flow remains compiled | “Old scanner-integrated guard set is still a reasonable reference for current delivery” | No | Remove/archive or mark inactive |
| G3 | Low | `src-tauri/src/coordination/claude_hooks.rs` | Skip if rendered `additional_context` is empty | “Renderer may legitimately produce blank output” | No practical value | Remove or collapse into assertion/log |
| G4 | Keep | `src-tauri/src/coordination/compaction_processor.rs` | `already_handled` idempotency skip | Same compaction event must not be delivered twice | Yes | Keep |
| G5 | Keep | `src-tauri/src/coordination/compaction_processor.rs` | `is_stale_compaction(...)` | Old compactions should not trigger new reinjection | Yes | Keep |
| G6 | Keep, clarify | `src-tauri/src/coordination/compaction_processor.rs` | `member_not_attached` via roster + pane existence + pane dead checks | Delivery should only happen when the managed member still has a valid live attachment | Yes | Keep, but document that this is attachment-validity, not tmux-send-key safety |
| G7 | Keep | `src-tauri/src/coordination/compaction_processor.rs` | `should_persist_delivery_state(...)` | Do not recreate compaction state after disband/remove | Yes | Keep |
| G8 | Keep | `src-tauri/src/coordination/claude_hooks.rs` | Skip non-compact `SessionStart` events | Hook bridge is only for compaction reinjection | Yes | Keep |
| G9 | Keep | `src-tauri/src/coordination/claude_hooks.rs` | Skip when no unique managed member match exists | Wrong-member reinjection is worse than no reinjection | Yes | Keep |
| G10 | Keep | `src-tauri/src/coordination/claude_hooks.rs` | Skip when operational snapshot is missing | Reinjection without bounded operational context is unsafe | Yes | Keep |
| G11 | Keep | `src-tauri/src/lib.rs` | On Claude hook bridge failure, print `{}` and exit `0` | Hook caller expects JSON-compatible output even on failure | Yes | Keep |
| G12 | Keep | `src-tauri/src/coordination/pipelines/helpers.rs` | `should_use_mesh_sidecar()` for non-Claude only | Only non-Claude agents need mesh delivery daemon sidecar | Yes | Keep |
| G13 | Keep | `src-tauri/src/coordination/pipelines/{initialize,members}.rs` | Skip Claude onboarding when no role context exists | Plain Claude native team context is enough unless Taurhaus has extra context to inject | Yes | Keep |
| G14 | Keep | `src-tauri/src/daemon/compaction.rs` | Only run daemon compaction runtime under WSL | This daemon-side transcript/watch path only makes sense where WSL-side tool files live | Yes | Keep |
| G15 | Keep, boundary warning | `src-tauri/src/session_scanner/idle/codex.rs` | Match Codex transcript to project by first-line `session_meta.payload.cwd` within lookback window | Project-level transcript discovery is acceptable for session observation | Yes for discovery only | Keep, but never reuse as authoritative member-delivery routing |

## Detailed Notes

## G1. Inbox corruption fallback hides real delivered messages

Location:
- [inbox.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/stores/inbox.rs)

What it checks:
- on JSON parse failure, `MeshInboxStore::load()` logs a warning and returns `Ok(Vec::new())`

Assumption:
- the safest fallback for a corrupt inbox file is to behave as though it has no messages

Why that assumption is wrong now:
- current delivery to Codex/Gemini members is file-append to inbox JSON
- treating corruption as empty does not just avoid a crash; it hides potentially real unread messages
- worse, a later `append()` call loads that synthetic empty state, pushes one new message, and writes a fresh file that has effectively discarded visibility of prior deliveries

Recommendation:
- **modify immediately**
- corruption should be surfaced as error state or quarantined, not silently reinterpreted as “empty inbox”

## G2. Legacy poll-based compaction module is still compiled and still encodes superseded assumptions

Location:
- [compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction.rs)

What it checks:
- old scanner-integrated transcript tailing and delayed pending-delivery flow

Current reality:
- current active architecture uses:
  - `compaction_extractor.rs`
  - canonical signal log
  - `compaction_watcher.rs`
  - `coordination/compaction_processor.rs`
- `process_codex_compaction_events(...)` has no live callers outside its own file/tests

Why this matters:
- the module is no longer the active implementation, but it still exists in the build and in prior docs/tests
- that makes it an easy place for stale guard logic to be copied forward again
- the three concrete wrong assumptions called out in the assignment all came from this older mental model family

Recommendation:
- **remove, archive, or visibly mark as inactive**
- at minimum, stop treating it as a valid behavioral reference for current delivery semantics

## G3. `EmptyAdditionalContext` is dead defensive logic

Location:
- [claude_hooks.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/claude_hooks.rs)
- [reinjection.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/reinjection.rs)

What it checks:
- after composing a card and serializing it with `render_claude_additional_context()`, skip if the result trims to empty

Assumption:
- serialization may produce a legitimately blank payload

Why that assumption no longer holds:
- the renderer is `serde_json::to_string_pretty(card)`
- for a valid card object, output is always non-empty JSON
- the branch is therefore not protecting a real current mechanism boundary; it is just dead defensive logic

Recommendation:
- **remove or collapse into internal invariant checking**
- it is low severity, but it adds noise and implies a failure mode that the renderer does not actually have

## G4. Idempotency skip is valid

Location:
- [compaction_processor.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/compaction_processor.rs)
- [compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/stores/compaction.rs)

What it checks:
- same `(member, session_id, compaction_timestamp)` should not be handled twice

Why it is still valid:
- file-based delivery does not make duplicate reinjection safe
- if anything, append-based delivery makes duplicate cards more visible to users

Recommendation:
- **keep**

## G5. Staleness window is valid

Location:
- [compaction_processor.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/compaction_processor.rs)
- [compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/stores/compaction.rs)

What it checks:
- skip or mark stale when the compaction timestamp is older than the freshness window

Assumption:
- delayed reinjection after the operator has already moved on is worse than no reinjection

Why it is still valid:
- this is not a tmux-era prompt-boundary guard
- it is a time-bounded relevance guard on the delivery itself

Recommendation:
- **keep**

## G6. `member_not_attached` is valid, but its wording should stay tied to attachment validity

Location:
- [compaction_processor.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/compaction_processor.rs)

What it checks:
- current managed member still exists in roster attachment view
- same pane is still attached
- pane exists
- pane is not dead

Assumption:
- reinjection only makes sense if the managed member still has the same live session attachment

Why it is still valid:
- even with inbox-file delivery, Taurhaus is not trying to queue indefinite reinjection for detached/dead members
- the guard is enforcing current attachment truth, not guarding `send-keys`

Risk:
- because it uses pane liveness terminology, it can look like a tmux-delivery relic

Recommendation:
- **keep**
- but document it as an attachment-validity check, not a direct-terminal-delivery check

## G7. Do not recreate compaction state after disband/remove

Location:
- [compaction_processor.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/compaction_processor.rs)

What it checks:
- `should_persist_delivery_state(...)` verifies the team and member still exist before saving state

Why it is valid:
- this prevents derived bookkeeping from resurrecting state after teardown
- this is exactly the kind of safety check that remains correct regardless of delivery transport

Recommendation:
- **keep**

## G8-G10. Claude hook skips are mostly correct for the current mechanism

Location:
- [claude_hooks.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/claude_hooks.rs)

### NonCompactSessionStart

Valid:
- hook bridge is scoped specifically to `SessionStart(source=compact)`

### NoManagedMemberMatch / MultipleManagedMembersMatched

Valid:
- wrong-member reinjection is materially worse than no reinjection
- this is current safety logic, not stale transport logic

### MissingOperationalSnapshot

Valid:
- the hook path is meant to deliver bounded operational context, not ad-hoc fallback prose
- if the snapshot is missing, silent “best effort” context generation would be riskier than skipping

Recommendation:
- **keep all three**

## G11. Claude hook CLI fallback to `{}` on error is valid compatibility behavior

Location:
- [lib.rs](/home/mstie/projects/taurhaus/src-tauri/src/lib.rs)

What it checks:
- if the hook bridge errors, print `{}` instead of propagating a non-JSON failure to Claude

Assumption:
- hook caller expects syntactically valid output more than process-level error semantics

Why it is still valid:
- the path already emits structured failure logs
- this guard preserves hook-call stability while keeping observability in logs

Recommendation:
- **keep**

## G12-G14. Pipeline and daemon guards are still correct

### Non-Claude sidecar only

Location:
- [helpers.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/helpers.rs)

Assessment:
- valid capability boundary, not outdated delivery logic

### Skip Claude onboarding when Taurhaus has no extra role context

Location:
- [initialize.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/initialize.rs)
- [members.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/members.rs)

Assessment:
- valid product policy
- it is not silently dropping current Taurhaus-owned context; it is skipping redundant delivery when there is no extra context to add

### Run daemon compaction runtime only in WSL

Location:
- [daemon/compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs)

Assessment:
- valid platform boundary
- this is not a stale guard; the daemon-side transcript/watch path is intentionally WSL-scoped

Recommendation:
- **keep**

## G15. Codex transcript discovery heuristics are acceptable for discovery, not for authority

Location:
- [idle/codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs)

What it checks:
- find Codex transcript candidates by date-window scan and first-line `session_meta.payload.cwd`

Assessment:
- valid for discovering likely current session files
- invalid if promoted into authoritative delivery/member binding logic

Current status:
- the current active compaction path has already moved toward runtime-attachment-based ownership rather than project-only transcript inference

Recommendation:
- **keep for discovery only**
- continue treating runtime attachment state as the authoritative binding layer

## Mesh Inbox Path Conclusion

### Append path

I did **not** find an active append-time guard in `MeshInboxStore::append()` that silently drops messages based on stale tmux assumptions.

The append path is simple:

1. load current inbox JSON
2. append message
3. atomically write back

The problem is the corrupt-load fallback described in `G1`, not an outdated transport check.

### Read path

The main read-path risk is also `G1`:

- corrupt inbox content is hidden as “empty”
- that can make correctly delivered messages appear absent

## Current-State Judgment

### Guards that should be removed or relaxed immediately

1. `MeshInboxStore::load()` corrupt-as-empty behavior
2. legacy `session_scanner/compaction.rs` as a compiled behavioral reference
3. Claude `EmptyAdditionalContext` dead branch

### Guards that should stay

1. idempotency (`already_handled`)
2. freshness window (`is_stale_compaction`)
3. attachment-validity checks before reinjection
4. team/member existence checks before persisting derived delivery state
5. Claude hook non-compact/member-resolution/snapshot guards
6. hook CLI JSON fallback
7. non-Claude sidecar boundary
8. platform-scoped daemon compaction startup

## Bottom Line

The active delivery path is not broadly full of wrong tmux-era guards.

The bigger risk is narrower:

1. one real inbox-path bug hides delivered messages on corruption
2. one legacy compaction module is still present and keeps outdated assumptions alive in the codebase
3. a few low-value defensive branches remain and should be simplified so the active path is easier to reason about

That means the right next fixes are targeted cleanup, not a broad rewrite of current active safety checks.
