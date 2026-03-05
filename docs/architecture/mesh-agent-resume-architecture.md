# Agent Resume Architecture for Mesh Team Lifecycle

Date: 2026-03-04  
Owner: task #68 (taurhaus-dev-1) with task #69 input (taurhaus-dev-2)

## Scope

Design a unified resume flow for offline team members that fits existing coordination patterns (`initialize`, `add-agent`, `remove`, `reonboard`) and supports:

1. Offline Claude lead resume
2. Offline non-Claude member resume (Codex/Gemini)
3. Offline Claude non-lead member resume

## Goals

- Reattach offline members without mutating team membership.
- Reuse existing pipeline/runtime primitives where possible.
- Keep pane handling safe (ownership checks before destructive actions).
- Provide a simple runtime UI action with context choice (`Continue` vs `Fresh`).
- Return step-structured reports matching existing IPC/report patterns.

## Non-goals

- No change to durable team membership semantics.
- No broad refactor of initialize/add-agent pipelines in this task.
- No introduction of session-id-as-authority (team lifecycle stays name/path based).

## Existing constraints and facts

- Claude resume in team context is flag-driven (`--team-name`, `--agent-name`, `--agent-id`, `--agent-type`) and does not require a `TeamCreate` event when `~/.claude/teams/<team>/` already exists (task #66).
- `CoordinationState::with_orchestrator` serializes operations via a mutex, which prevents true concurrent pipeline execution.
- Runtime is recoverable metadata (`teams/<team>/runtime/*.json`); config remains source of team membership.
- `add_agent` pipeline already has step reporting and rollback semantics we can mirror.

## New IPC contract

### Command

`coordination_resume_member`

### Request

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeContextMode {
    Continue,
    Fresh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeMemberRequest {
    pub team_name: String,
    pub member_name: String,
    pub context_mode: ResumeContextMode,
}
```

### Response

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeAgentReport {
    pub team_name: String,
    pub member_name: String,
    pub resumed: bool,
    pub succeeded_steps: Vec<String>,
    pub failed_step: Option<String>,
    pub retryable: bool,
    pub message: String,
    pub steps: Vec<StepProgress>,
    pub warnings: Vec<String>,
    pub pane_id: Option<String>,
    pub reused_pane: bool,
}
```

### Step names

`validate` -> `load_member` -> `resolve_pane` -> `launch_session` -> `join_mesh` -> `start_daemon` -> `send_onboarding` -> `update_runtime`

For skipped optional steps (`join_mesh`, `start_daemon`, `send_onboarding` for Claude), mark as succeeded with message `"not required for claude"` to keep deterministic reports.

## Backend pipeline design

Add orchestrator API:

```rust
pub fn resume_member_with_cli_commands_and_layout(
    &mut self,
    request: &ResumeMemberRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
) -> Result<ResumeAgentReport, CoordinationError>
```

### Pipeline algorithm

1. Validate request
- Team exists
- Member exists in config
- Member runtime status is offline (`SessionDead`/no runtime), else return conflict (`already active`)

2. Load member + runtime snapshot
- Load `Member` from config
- Load runtime if present; default runtime record if missing

3. Resolve pane (`resolve_or_create_pane_for_member`)
- Input: member project path, existing `runtime.pane_id`, `tmux_layout`
- Classification:
  - Missing: no pane id or pane lookup fails -> create
  - Dead: pane exists and tmux reports dead pane -> kill + create
  - Reusable: pane exists, not dead, ownership matches project -> reuse
  - Ownership mismatch/check failure -> warn, do not kill, create new
- Output: `{ pane_id, reused_pane, created_new_pane }`

4. Launch session in pane (mode-aware)
- Build command from configured CLI commands plus team context for Claude.
- Launch mode mapping:
  - `Fresh` -> fresh command for all tools
  - `Continue`:
    - Claude -> `continue_cmd`
    - Codex/Gemini -> resume command (Codex `resume --last`, Gemini `--resume`)
- Claude commands must pass through existing team flag injector (`with_claude_team_context`) with role-aware `--agent-type`:
  - lead: `orchestrator`
  - non-lead: `general-purpose`

5. Post-launch integration
- Claude lead: skip `join_mesh`, skip daemon, skip onboarding
- Claude non-lead: skip `join_mesh`, skip daemon, send onboarding (team context reminder + lead report-back)
- Non-Claude member: `join_mesh` + `start_daemon` + `send_onboarding`

6. Update runtime
- Preserve member identity record, overwrite runtime attachment fields:
  - `pane_id = resolved pane`
  - `session_id = detect_session_id()` for Claude (best-effort)
  - `daemon_pid = newly spawned pid` for non-Claude, else `None`
  - `health = Healthy`
  - `attached_at = now`
- If existing daemon pid was recorded for non-Claude, terminate before starting a new one.

7. Failure cleanup (resume-specific)
- Never remove member from team config.
- Clean only resources created during this resume attempt:
  - new daemon pid
  - temporary mesh join (if completed)
  - newly created pane
- Do not kill pre-existing pane unless this attempt explicitly replaced it due to `dead` and ownership-safe decision.

## Runtime/pane lifecycle handling

### Required runtime additions

Add pane-state helpers to `CoordinationRuntime` (or equivalent internal helper methods):

- `pane_exists(pane_id) -> Result<bool, CoordinationError>`
- `pane_is_dead(pane_id) -> Result<bool, CoordinationError>` using `tmux display-message -p '#{pane_dead}'`

`pane_belongs_to_project()` already exists and remains mandatory before any destructive pane action.

### Decision table

- No `pane_id` in runtime: create pane with configured layout
- `pane_id` exists but lookup fails: treat as missing, create
- `pane_id` exists and dead: ownership check; if safe kill and recreate
- `pane_id` exists and alive + ownership match: reuse
- `pane_id` exists + ownership mismatch/check error: warn + create new (do not kill)

## UI interaction design

### MeshRuntimeView + MeshNodeDetail

- Show `Resume` action only on offline rows (`statusToState(member.sessionStatus) === 'offline'`).
- Lead is resume-eligible; keep `Remove` hidden for lead.
- Add per-row mode selector (`Continue` default, `Fresh` alternate) next to `Resume`.
- During resume of a row:
  - button text `Resuming...`
  - disable row actions (`Focus`, `Re-onboard`, `Resume`, `Remove`) for that row

### MeshTab

- Maintain `resumingMembers: Set<string>` state (parallel to `removingMembers`).
- Add `coordinationResumeMember(teamName, memberName, contextMode)` IPC wrapper.
- Reuse runtime toast/error channels:
  - success: `Resumed '<member>'` (+ warning count when present)
  - failure: existing error banner path
- Trigger roster refresh nonce on successful resume.

## Context mode behavior

- UI labels: `Continue` and `Fresh`
- Default: `Continue` (action semantics match “Resume”; user can switch to Fresh when recovering from bad context)

## Reuse strategy

Direct reuse from add-agent pipeline:

- `launch_agent_in_pane`
- `join_mesh_for_agent` (non-Claude only)
- `start_daemon_for_agent` (non-Claude only)
- `send_onboarding_for_agent` (reuse payload shape; skip for lead Claude)
- `capture_session_id_for_member` / Claude detection pattern

Adapt/split:

- `validate_add_agent_request` -> new `validate_resume_request`
- `create_pane_for_agent` -> `resolve_or_create_pane_for_member`
- `update_roster_with_agent` -> `update_runtime_for_member` (no config mutation)
- `cleanup_add_agent_failure` -> resume cleanup that never removes member

New command-building helper:

- `build_team_launch_command` currently hardcodes Fresh.
- Introduce a mode-aware variant for resume flow, then pass result through existing Claude team-context injector.

## Data flow (logical)

1. UI row action (`Resume` + mode)
2. IPC `coordination_resume_member`
3. Commands layer resolves project path + cli settings + tmux layout
4. Orchestrator resume pipeline executes steps + report
5. Runtime record updated
6. IPC returns `ResumeAgentReport`
7. UI shows toast + refreshes live roster

## Edge case policy

- Offline but hung process in pane: treat non-dead alive pane as reusable by default; user can choose `Fresh` mode to reset context. Optional future enhancement: force-replace toggle.
- Manual pane close: treated as missing -> create new pane.
- Rapid duplicate resume clicks: command serialization plus offline validation prevents duplicate successful resumes.
- Team disband race: validate team existence at start and before final write; if disbanded mid-run, fail and cleanup created resources.
- Missing runtime file for existing member: allowed; reconstruct runtime from resume pipeline.

## Acceptance criteria

1. New IPC command exists and is registered with typed request/response.
2. Offline member rows (including lead) expose `Resume` with mode selector in runtime roster.
3. Resume in `Continue` mode launches:
- Claude with hidden team flags + role-specific `--agent-type`
- Codex/Gemini with resume-oriented command
4. Resume in `Fresh` mode launches tool fresh command.
5. Pane resolution follows classification rules and never kills ownership-mismatched panes.
6. Non-Claude resume performs mesh join + daemon start + onboarding.
7. Claude lead resume skips onboarding; Claude non-lead resume sends onboarding.
8. Runtime store updates reflect new attachment (`pane_id`, `health`, `attached_at`, `daemon_pid`, Claude `session_id` best-effort).
9. Failure rollback cleans only resources created by the failed resume attempt; member remains in config.
10. Step report is deterministic and includes warnings for non-fatal safety skips.

## TDD task split recommendation

1. Backend contracts + command registration (`coordination_types.rs`, `coordination.rs`, `lib.rs`).
2. Resume pipeline core + pane classification + cleanup semantics (`coordination/pipelines/`, `coordination/runtime.rs`).
3. Frontend IPC wrapper + MeshTab/MeshRuntimeView/MeshNodeDetail resume UX + row in-flight state.
4. Tests:
- Rust pipeline tests for three scenarios + edge cases
- Rust command tests for request validation + report mapping
- Vitest roster/tab tests for offline Resume UI, mode propagation, and in-flight behavior
