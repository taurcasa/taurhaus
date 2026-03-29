# Canonical Member Activation Stage Mapping

This document mirrors the authoritative Rust mapping in
`src-tauri/src/coordination/requests.rs`:
`LEGACY_MEMBER_ACTIVATION_STAGE_MAPPINGS`.

Canonical member-activation stages:

- `prepare_member`
- `acquire_pane`
- `launch_session`
- `capture_session_identity`
- `join_mesh`
- `start_member_daemon`
- `commit_runtime`
- `deliver_onboarding`

## Legacy-to-Canonical Mapping

| Wrapper | Legacy stage | Canonical stage(s) | Notes |
|---|---|---|---|
| initialize | `validate_configuration` | `prepare_member` | Team-wide/member-wide preparation before activation begins |
| initialize | `create_team` | none | Wrapper-scoped team creation |
| initialize | `add_lead` | none | Wrapper-scoped roster seeding |
| initialize | `create_panes` | `acquire_pane`, `launch_session` | Initialize currently opens panes and launches sessions in one batch stage |
| initialize | `launch_sessions` | `capture_session_identity` | Initialize captures runtime session identity after launch |
| initialize | `join_mesh` | `join_mesh` | Canonical mesh-join stage |
| initialize | `start_daemons` | `start_member_daemon` | Canonical daemon-start stage |
| initialize | `send_onboarding` | `deliver_onboarding` | Canonical onboarding stage |
| resume | `validate` | `prepare_member` | Part of member preparation |
| resume | `load_member` | `prepare_member` | Part of member preparation |
| resume | `resolve_pane` | `acquire_pane` | Canonical pane acquisition stage |
| resume | `launch_session` | `launch_session`, `capture_session_identity` | Legacy step spans launch and session capture |
| resume | `join_mesh` | `join_mesh` | Canonical mesh-join stage |
| resume | `start_daemon` | `start_member_daemon` | Canonical daemon-start stage |
| resume | `send_onboarding` | `deliver_onboarding` | Canonical onboarding stage |
| resume | `update_runtime` | `commit_runtime` | Canonical runtime commit stage |
| add-agent | `validate` | `prepare_member` | Part of member preparation |
| add-agent | `create_pane` | `acquire_pane`, `launch_session` | Add-agent currently opens pane and launches CLI in one step |
| add-agent | `launch_session` | `capture_session_identity` | Add-agent uses this step for runtime session identity capture |
| add-agent | `join_mesh` | `join_mesh` | Canonical mesh-join stage |
| add-agent | `start_daemon` | `start_member_daemon` | Canonical daemon-start stage |
| add-agent | `send_onboarding` | `deliver_onboarding` | Canonical onboarding stage |
| add-agent | `update_roster` | `commit_runtime` | Add-agent commits member/runtime state here |

## Usage Note

Wrapper-scoped stages such as `create_team` and `add_lead` intentionally remain
outside the shared member-activation vocabulary. They should continue to exist
at the wrapper layer even after the member-activation execution path is unified.
