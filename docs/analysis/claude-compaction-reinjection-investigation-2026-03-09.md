# Claude Compaction Reinjection Investigation

Date: 2026-03-09
Task: #845

## Question

Does post-compaction session continuity actually work for Claude Code, and if so through which path?

## Short Answer

Yes, Claude post-compaction continuity works in the current codebase, but it does **not** use the Codex JSONL compaction pipeline.

The shipped split is:

- `Codex`: JSONL `compacted/context_compacted` detection -> canonical signal log -> watcher -> processor -> mesh inbox delivery
- `Claude`: `SessionStart(source=compact)` hook bridge -> managed member resolution -> operational context render -> Claude `additionalContext` response

So the Claude answer is:

1. JSONL compaction detection: not implemented/used for Claude
2. `CompactionSignalExtractor`: does not process Claude sessions
3. watcher -> extractor -> processor pipeline: Codex-only today
4. tmux text injection: not the current Claude path, and no longer the current Codex path either
5. Claude hook bridge: functional, with passing tempdir-backed tests

## Evidence

## 1. Claude is excluded from the extractor path

`src-tauri/src/session_scanner/compaction_extractor.rs` is explicitly Codex-oriented.

Observed behavior in source:

- active runtime sessions are filtered to `CliTool::Codex`
- startup initialization also seeds the extractor from Codex-only sessions
- the tracked transcript type is `ManagedCodexTranscript`

That means a Claude runtime session never enters the canonical compaction signal log path.

## 2. The watcher/processor chain is wired to the Codex signal log

`src-tauri/src/session_scanner/compaction_watcher.rs` and `src-tauri/src/coordination/compaction_processor.rs` are aligned with the same Codex path:

- watcher is described as the canonical Codex compaction signal log watcher
- processor resolves managed Codex signals
- delivery appends a Codex post-compaction message into the mesh inbox

This is consistent with the current Codex-only pipeline and inconsistent with Claude using that same path.

## 3. Claude has a separate hook bridge

`src-tauri/src/coordination/claude_hooks.rs` implements a dedicated Claude reinjection path:

- input payload is `SessionStart`
- only `source == "compact"` is accepted
- member resolution is by managed Claude runtime `session_id` and matching `cwd`
- it loads the operational snapshot
- it renders Claude `additionalContext`
- it records delivery state as `Injected` or `Skipped`

`src-tauri/src/lib.rs` exposes a dedicated CLI entrypoint:

- `--claude-compact-hook`

That path reads hook JSON from stdin and prints a serialized response for Claude Code to consume.

## 4. Current compaction delivery is not tmux text injection

The older tmux send-keys style is no longer the active post-compaction delivery mechanism.

Current state:

- `Codex`: delivery goes to mesh inbox
- `Claude`: delivery goes through the Claude hook response `additionalContext`

So the correct answer to "does tmux injection work for Claude or only Codex?" is:

- neither is using tmux text injection for compaction reinjection now

## Synthetic Test Evidence

All evidence below uses tempdir-backed tests or synthetic test fixtures. No live-session validation was used.

Passing targeted tests:

- `cargo test --manifest-path src-tauri/Cargo.toml coordination::claude_hooks::tests::compact_hook_returns_additional_context_for_matching_member -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml coordination::claude_hooks::tests::compact_hook_additional_context_is_well_formed_and_contains_expected_fields -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml coordination::claude_hooks::tests::compact_hook_skips_non_compact_session_start -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml coordination::claude_hooks::tests::ensure_compact_hook_installed_writes_script_and_settings_entry -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml coordination::claude_hooks::tests::compact_hook_emits_received_resolved_and_delivered_events -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml startup::compaction -- --nocapture`

What those tests prove:

- Claude compact hook returns reinjection context for a matching managed member
- returned payload is structured and contains the expected operational fields
- non-compact SessionStart events are ignored
- hook installation into Claude settings is covered
- structured Claude hook lifecycle logs are emitted on successful delivery
- the watcher/extractor/processor startup pipeline works end-to-end, but that test covers the Codex path

## What Actually Works Today

Claude continuity works **if** all of the following are true:

- the Claude compact hook is installed in `settings.json`
- Claude Code emits `SessionStart(source=compact)`
- the managed Claude member has a runtime `session_id` matching the hook payload
- `cwd` still matches the managed member project
- an operational snapshot exists for that member

When those hold, Taurhaus returns `additionalContext` to Claude Code and records successful delivery.

## What Is Not Implemented

These are not implemented for Claude today:

- JSONL compaction detection
- canonical compaction signal log emission from Claude sessions
- watcher/processor handling of Claude compaction signals
- mesh inbox delivery for Claude compaction reinjection

If Claude ever stopped firing `SessionStart(source=compact)`, current continuity would fail because there is no Claude fallback on the Codex signal pipeline.

## Conclusion

Claude post-compaction continuity is real, but it is a separate hook-driven subsystem.

The current architecture is not "one compaction pipeline for both tools." It is:

- Codex: signal-log pipeline
- Claude: hook bridge

That means the right product statement is:

"Claude compaction reinjection works through the SessionStart compact hook bridge, not through the JSONL extractor/watcher/processor pipeline."
