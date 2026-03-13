# Mesh Claude-Side Task Delivery Fix

## Scope

Task `#1271` asked for the real actionable-delivery path for Claude-side team members, specifically `design-taurhaus` and `product-check-1`, and a fix so assignment/update delivery reaches the file location Claude-side roles actually consume.

## Live path comparison

On the live `taurhaus-team` state, the relevant files are:

- `~/.claude/teams/taurhaus-team/inboxes/design-taurhaus.json`
- `~/.claude/teams/taurhaus-team/inboxes/product-check-1.json`
- `~/.claude/teams/taurhaus-team/runtime/design-taurhaus.json`
- `~/.claude/teams/taurhaus-team/runtime/product-check-1.json`

The inbox files already contain prior actionable notices for both members, including:

- onboarding `role_context`
- communication checks
- task assignments from `team-lead`

The runtime files show both members are Claude-side roles (`"cli_tool": "claude"`) with no Mesh daemon (`"daemon_pid": null`).

So the live delivery contract for these members is not tmux/daemon delivery. It is file-based delivery through the shared member inbox:

- `~/.claude/teams/<team>/inboxes/<member>.json`

That matches the documented Taurhaus contract in:

- `docs/coordination-architecture.md`
- `docs/architecture/data-architecture.md`

Both documents already describe Claude-side delivery as inbox-file write plus Claude-native polling/hook behavior.

## Root cause

There were two separate bugs in the coordination delivery stack:

1. `src-tauri/src/coordination/backend/claude.rs` was still a placeholder.
   - `ClaudeNativeBackend.deliver(...)` returned `Ok(DeliveryResult { delivered: false, method: NativeMessageApi })`
   - it did not write the notice into `teams/<team>/inboxes/<member>.json`
   - for Claude-side members, the actionable assignment/update payload was therefore dropped

2. `src-tauri/src/coordination/orchestrator.rs` treated any `Ok(DeliveryResult)` as success.
   - even when `delivered == false`
   - it emitted `delivery_succeeded`
   - it updated runtime `last_seen_at`
   - it could therefore make the system look healthy while no actionable file delivery had happened

That combination explains the observed failure pattern:

- Codex-side roles received actionable assignments through Mesh/tmux
- Claude-side roles got no actual file write from Taurhaus on the native backend path
- the system could still report delivery success internally

## Fix

The fix keeps the existing architecture and changes only the broken delivery surface.

### 1. Real Claude-native delivery now appends to the shared inbox file

`src-tauri/src/coordination/backend/claude.rs`

- `ClaudeNativeBackend` now carries the resolved teams directory
- `deliver(operator_notice)` now creates a `MeshInboxMessage`
- it appends that message to `teams/<team>/inboxes/<member>.json` through `MeshInboxStore`
- it returns `DeliveryResult { delivered: true, method: InboxFile }`

This matches the path already used by the live Claude-side members and the documented Taurhaus data model.

### 2. False delivery results no longer count as success

`src-tauri/src/coordination/orchestrator.rs`

- if a backend returns `Ok(...)` with `delivered == false`, the orchestrator now treats that as a delivery failure
- it emits `delivery_failed`
- it does not update runtime `last_seen_at`
- it returns a backend error instead of silently auditing success

### 3. Default backend wiring now resolves the real teams directory

`src-tauri/src/coordination/state.rs`

- the default Claude-native backend is now created with the actual teams root instead of a zero-state placeholder instance

## Regression coverage

Added tests:

- `coordination::backend::claude::tests::deliver_operator_notice_appends_to_claude_member_inbox`
- `coordination::backend::claude::tests::deliver_operator_notice_falls_back_to_default_sender_name`
- `coordination::orchestrator::tests::deliver_false_result_is_treated_as_failure`

Existing behavior guard still passing:

- `coordination::pipelines::tests::initialize_pipeline_claude_template_agent_receives_role_context_message`

## Exact verification run

- `cargo fmt --manifest-path src-tauri/Cargo.toml`
- `cargo test deliver_operator_notice_appends_to_claude_member_inbox --manifest-path src-tauri/Cargo.toml`
- `cargo test deliver_operator_notice_falls_back_to_default_sender_name --manifest-path src-tauri/Cargo.toml`
- `cargo test deliver_false_result_is_treated_as_failure --manifest-path src-tauri/Cargo.toml`
- `cargo test initialize_pipeline_claude_template_agent_receives_role_context_message --manifest-path src-tauri/Cargo.toml`
- `cargo check --tests --manifest-path src-tauri/Cargo.toml`

All passed.

## Outcome

Yes, Claude-side roles now receive actionable assignments and updates through the same real Taurhaus-managed delivery queue that they actually consume: the shared member inbox file under `~/.claude/teams/<team>/inboxes/<member>.json`.

This is not the same transport as Codex-side tmux delivery, and it should not be. The correct equivalence is:

- Codex-side roles: actionable delivery via Mesh/tmux
- Claude-side roles: actionable delivery via shared inbox file append

After this fix, the Claude-side path is no longer a no-op placeholder.

## Remaining risk

This task fixes the broken native delivery write path for operator notices. It does not change Claude-side polling/hook behavior itself.

So the remaining risk is downstream from Taurhaus:

- if Claude-side clients stop reading `teams/<team>/inboxes/<member>.json`
- or if their native poller changes format expectations later

then delivery could regress again even though Taurhaus is writing the correct inbox file. The current live file state for `design-taurhaus` and `product-check-1` strongly indicates that this inbox path is the correct contract today.
