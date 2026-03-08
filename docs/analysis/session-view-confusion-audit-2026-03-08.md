# Session View Confusion Audit

Date: 2026-03-08
Owner: architect
Task: #704

## Summary

We have repeated the same bug class three times because the codebase still exposes two semantically different session views through the same `ClaudeSession` shape:

- a **display-safe** view that may intentionally clear `session_id` and `jsonl_path`
- a **runtime** view that must preserve those fields for coordination logic

The immediate regressions from `#698`, `#699`, and `#703` are fixed, and this audit did **not** find another currently-active runtime consumer still wired to the display-safe path. The remaining problem is structural: the naming and shared type make the mistake easy to reintroduce.

## Recommended Guard

Recommended pattern: **split the types and rename the display path**.

Keep it simple:

1. Introduce two concrete types:
   - `RuntimeSession`
   - `DisplaySession`
2. Make runtime producers return `RuntimeSession`.
3. Make display producers explicitly sanitize `RuntimeSession -> DisplaySession`.
4. Rename ambiguous display APIs to include `display` in the name.

Why this is the simplest effective guard:

- comments alone have already failed three times
- naming alone helps, but still allows accidental reuse when both paths return the same struct
- full trait/newtype/generic marker systems would be heavier than necessary
- two concrete structs plus one-way conversion gives a real compile-time boundary with low conceptual overhead

Concrete recommendation:

- rename `scan_sessions()` -> `scan_sessions_for_display()`
- keep `scan_sessions_for_runtime()` as the metadata-preserving path
- rename daemon `LIST_CLAUDE_SESSIONS` -> `LIST_DISPLAY_SESSIONS`
- keep `LIST_RUNTIME_SESSIONS` for runtime consumers
- deprecate the old names for one release cycle, then remove them

Implementation note:

- `DisplaySession` should omit `session_id` and `jsonl_path` entirely instead of keeping nullable fields. That is the real guard. If those fields still exist on the display type, downstream misuse remains easy.

## Current Boundary

Current display-safe behavior lives in [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:528), where `scan_sessions()` conditionally strips `session_id` and `jsonl_path` when activity attribution is ambiguous.

Current runtime path lives in [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:653), where `scan_sessions_for_runtime()` preserves metadata explicitly.

Current daemon display handler is [handlers.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/handlers.rs:320).

Current daemon runtime handler is [handlers.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/handlers.rs:327).

## Consumer Audit

### Display-safe consumers

These are correct as written:

- [session_listing.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/command_center/session_listing.rs:12)
  - Uses daemon `LIST_CLAUDE_SESSIONS` and local fallback `scan_sessions()`
  - Purpose is session list UI / hover / activity promotion
  - Does not require `session_id` or `jsonl_path`

- [session_activity.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/session_activity.rs:199)
  - Populates `SessionActivityHub` from `scan_sessions()`
  - Purpose is UI polling / event stream
  - Correct place for the display-safe view

- [handlers.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/handlers.rs:320)
  - `handle_list_claude_sessions()` returns `SessionActivityHub` snapshot
  - Correct for display consumers, but the method name is misleading

- [handlers.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/handlers.rs:338)
  - `WAIT_SESSION_UPDATES` returns `SessionActivityHub` updates
  - Correct for display/event consumers

- [stall_detector.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/stall_detector.rs:1807)
  - Uses `scan_sessions()` only for pane/project/activity signals
  - Does not depend on transcript metadata

- [claude_index.rs](/home/mstie/projects/taurhaus/src-tauri/src/task_scanner/claude_index.rs:28)
  - Uses `scan_sessions()` only to enrich live Claude task source mapping
  - Also merges offline filesystem session data, so it is not relying exclusively on live transcript metadata

- [bootstrap.rs](/home/mstie/projects/taurhaus/src-tauri/src/bootstrap.rs:262)
  - Same task-scan context path as above
  - Not a runtime coordination consumer

### Runtime consumers

These are correct after `#703`:

- [runtime.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/runtime.rs:268)
  - `detect_session_id()` uses `collect_runtime_sessions()`

- [runtime.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/runtime.rs:558)
  - `collect_runtime_sessions()` uses `scan_sessions_for_runtime()`

- [orchestrator.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/orchestrator.rs:844)
  - Liveness reconciliation calls runtime `detect_session_id()`
  - This is the correct runtime path

- [members.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/members.rs:604)
- [members.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/members.rs:653)
- [members.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/members.rs:683)
  - Member add/resume flows call runtime `detect_session_id()`
  - Correct

### Windows daemon bridge

Current state after `#703`:

- [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:216)
  - `scan_sessions_via_daemon_snapshot()` still uses the display-safe daemon method
  - Correct for UI-facing `scan_sessions()`

- [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:221)
  - `scan_runtime_sessions_via_daemon()` uses the runtime daemon method
  - Correct for `scan_sessions_for_runtime()`

Audit result:

- no remaining active runtime consumer is still reading `SessionActivityHub` or `LIST_CLAUDE_SESSIONS`
- the bug class is currently fixed in behavior
- the bug class is **not** fixed structurally

## Additional Bugs / Design Problems Found

### 1. Misleading daemon method name

Severity: medium

Problem:

- `LIST_CLAUDE_SESSIONS` is not Claude-only
- it returns Codex and Gemini sessions too
- it is also the **display-safe** view, not a canonical runtime session list

Evidence:

- constant name: [protocol.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/protocol.rs:86)
- handler implementation: [handlers.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/handlers.rs:320)
- UI session listing consumer: [session_listing.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/command_center/session_listing.rs:17)

Fix needed:

- rename to `LIST_DISPLAY_SESSIONS`
- keep compatibility alias temporarily if needed

### 2. Shared struct still allows future misuse

Severity: high

Problem:

- both display and runtime paths still deserialize into the same `ClaudeSession` struct
- nothing in the type system tells a caller whether metadata is guaranteed or best-effort

Evidence:

- shared type: [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:66)
- display path strips metadata in-place: [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:528)
- runtime path returns the same struct shape: [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:653)

Fix needed:

- split `ClaudeSession` into `DisplaySession` and `RuntimeSession`
- make sanitization a one-way conversion

### 3. Codex runtime resolution is still project-scoped

Severity: high

Problem:

- Codex session resolution still maps project path -> session JSONL
- same-project Codex panes can therefore share the same session candidate
- this is separate from the null-session regression, but it still weakens runtime correctness for compaction/reinjection

Evidence:

- resolver matches by project `cwd`: [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs:146)
- file match predicate is project-only: [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs:200)
- runtime test currently encodes shared metadata in multi-session Codex projects: [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:1047)

Fix needed:

- redesign Codex runtime session resolution around pane/process-attached evidence, not only project `cwd`
- likely candidates:
  - inspect open JSONL path per PID
  - persist pane -> session_id correlation once observed
  - only fall back to project-level inference when there is exactly one Codex candidate for that project

### 4. `scan_sessions()` name is too neutral

Severity: medium

Problem:

- `scan_sessions()` sounds canonical
- in reality it is the display-safe, ambiguity-aware classification view
- that naming mismatch is a direct contributor to the repeated regressions

Evidence:

- UI-safe stripping logic sits inside the function itself: [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:528)
- runtime-safe alternative exists separately: [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:653)

Fix needed:

- rename to `scan_sessions_for_display()`
- reserve neutral names like `scan_runtime_sessions()` or `collect_runtime_sessions()` for metadata-preserving paths only

## Recommended Implementation Tasks

1. Split session types into `DisplaySession` and `RuntimeSession`, with explicit sanitizer conversion.
2. Rename display APIs:
   - `scan_sessions()` -> `scan_sessions_for_display()`
   - `LIST_CLAUDE_SESSIONS` -> `LIST_DISPLAY_SESSIONS`
3. Add module-level warning docs in `session_scanner/mod.rs` and `daemon/protocol.rs`:
   - display view may strip transcript metadata
   - runtime view must be used for coordination/session-id logic
4. Redesign Codex runtime correlation so same-project Codex panes do not share one project-level session candidate.

## Bottom Line

No remaining active consumer is currently wired to the wrong session view after `#703`.

The next real prevention step should be:

- **type split + explicit display naming**

That is the smallest change that makes this bug class hard to reintroduce without adding unnecessary abstraction.
