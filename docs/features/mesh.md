# Mesh view

The mesh view is taurhaus's multi-agent team coordination tab. It provides one-click team setup, a live team roster, and agent lifecycle management — all backed by mesh CLI and tmux.

## Overview

Mesh view responsibilities:
- check mesh CLI and tmux prerequisites before enabling team features
- define and launch multi-agent teams with a single click
- display a live roster of team members with session status
- hot-add agents to running teams
- re-send onboarding instructions to agents that lose context
- disband teams with full resource cleanup (panes, daemons, mesh state)
- discover and clean up stale teams from previous sessions

## Prerequisites

Mesh view requires two external tools:

| Tool | Purpose | Install location |
|------|---------|-----------------|
| mesh CLI | Agent coordination protocol | `~/.local/bin/mesh` |
| tmux | Terminal multiplexer for agent sessions | System package manager |

If either is missing, the availability gate blocks the setup flow with actionable guidance.

## Availability gate

`MeshAvailabilityGate.svelte` runs two checks before showing the setup form:

1. **Mesh install status** (`check_mesh_install_status`) — detects whether mesh CLI is installed, its version, and whether an update is needed. Compares installed version against the bundled version shipped with taurhaus.
2. **Preflight check** (`coordination_preflight_check`) — validates that all requested agent tools (Claude, Codex, Gemini) are available in the environment. Returns blocking errors and per-agent warnings.

If mesh is missing or outdated, the gate shows an "Install Bundled Mesh" button that copies the bundled binary to `~/.local/bin/mesh` and verifies the installation. On Windows, this operates through WSL interop.

Agent warnings (e.g., a specific CLI tool not installed) are non-blocking — the user can still proceed, and agents will report issues at runtime.

## Tab placement

Mesh is a top-level tab alongside Overview, Files, Tasks, and Git. It is per-project — the team lead's project anchors the mesh tab. Switching projects in the sidebar shows that project's mesh state.

On app reopen, taurhaus auto-detects existing teams by scanning `~/.claude/teams/` and restores the runtime view if a matching team exists for the current project.

## Setup flow

When no team exists for the current project, the mesh tab shows the setup form.

### Team roster builder

The setup form (`MeshSetupForm.svelte`) presents a visual roster:

- **Team lead** — fixed to Claude Code with the `opus` model. Displayed as a highlighted "You" card with a Lead badge. Not editable (reduces misconfiguration).
- **Agent cards** — each agent has:
  - Name input (placeholder auto-generates from project name, e.g., `taurhaus-dev-1`)
  - CLI tool selector (Claude, Codex, Gemini)
  - Model selector (filtered by tool: opus/sonnet/haiku for Claude, gpt-5.3-codex/gpt-5-mini for Codex, gemini-2.5-pro/gemini-2.0-flash for Gemini)
  - Target project selector (any loaded project)
- **Add agent** button to add more agent cards
- **Remove** button per agent (when more than one exists)
- Duplicate name detection with inline error

### Customize options

Below the roster, a "Customize..." toggle reveals:
- **Team name** — defaults to `{project-name}-team`, editable
- **Team description** — optional

### Onboarding banner

First-time users see a dismissable info banner explaining what Mesh does. Dismissed state persists in localStorage.

### Start Team

Clicking "Start Team" submits the full configuration to the initialization pipeline. There is no separate review step — the roster builder IS the review.

## Initialization

`MeshInitProgress.svelte` shows a 7-step progress view with per-step status tracking:

| Step | Label | Description |
|------|-------|-------------|
| `validate_configuration` | Validating configuration | Checks team name, agent tools, and project assignments |
| `create_team` | Creating team | Writes team config to `~/.claude/teams/` |
| `create_panes` | Opening terminal panes | Creates tmux panes for each agent |
| `launch_sessions` | Launching agent sessions | Starts CLI tools in each pane |
| `join_mesh` | Connecting agents to mesh | Registers agents with mesh protocol |
| `start_daemons` | Starting coordination daemons | Launches file watchers for each agent inbox |
| `send_onboarding` | Sending agent instructions | Delivers initial instructions to each agent |

Each step shows a status glyph (pending/running/succeeded/failed), a human-readable label, and a description while running. An elapsed-time counter tracks total duration.

Progress events are streamed from the backend via the `coordination-step-progress` Tauri event channel — the frontend calls one IPC command (`coordination_initialize_team`) and receives real-time updates.

### Failure handling

On failure:
- The exact failed step is highlighted with a plain-language error
- Succeeded steps are listed for context
- A "What went wrong?" expandable section shows details
- Actions: **Retry** (re-runs from the beginning), **Back** (returns to setup form)

### Conflict recovery

If initialization fails because the team already exists (`create_team` step), three recovery options appear:
- **Open Existing Team** — skip initialization, switch to the runtime roster
- **Disband Existing Team** — disband the conflicting team, then retry initialization
- **Retry** — retry as-is

## Runtime roster

After successful initialization, the mesh tab switches to runtime mode showing `MeshTeamRoster.svelte`.

### Roster display

Each member card shows:
- **Status badge** — Active (green, pulsing), Idle (amber), or Offline (gray)
- **Name** — with star prefix for the team lead
- **Tool icon** — brand SVG (Anthropic starburst, OpenAI blossom, Gemini sparkle) using `sessionToolIcon()`
- **Metadata line** — tool name, model, target project
- **Description** (if set)

### Auto-refresh

The roster polls `coordination_get_live_team_status` every 5 seconds. A manual refresh button is also available. The header shows member count, active count, idle count, and the refresh interval.

### Session status mapping

Backend health states map to frontend display:

| Backend HealthState | Frontend SessionStatus | Display |
|---|---|---|
| Healthy | Active | Green badge, pulse animation |
| AwaitingRead, SuspectedStuck, Rebriefed, Suppressed | Idle | Amber badge |
| SessionDead | Offline | Gray badge |

### Available actions

**Per-member actions:**
- **Focus** — jump to the agent's tmux pane (only shown if `pane_id` exists)
- **Re-onboard** — re-send onboarding instructions via `coordination_reonboard` (non-lead members only). Shows brief "Sent!" confirmation.

**Team actions (header):**
- **+ Agent** — opens the hot-add form
- **Disband team** — via overflow menu (⋯), with confirmation dialog

## Hot-add agents

From the runtime view, clicking "+ Agent" opens an inline form with:
- Agent name
- CLI tool selector
- Model selector
- Target project selector
- Description (optional)

Submission calls `coordination_add_agent`, which handles: create pane, launch CLI, mesh join, start daemon, send onboarding. Progress is shown inline with per-step status.

## Team cleanup panel

When in setup mode and existing teams are discovered, a "Team Cleanup" section appears below the setup form. It allows users to review and disband teams before starting a new one.

Each discovered team card shows:
- Team name
- Lead project path
- "Current project" or "Different project" badge
- Disband button (with confirmation dialog)

Discovery warnings (e.g., unparseable team directories) are shown in a separate warning block.

## Disband

Disbanding a team:
1. Shows a confirmation dialog with the team name
2. Calls `coordination_disband_team`
3. Backend tears down: stops daemons (by PID), leaves mesh, kills tmux panes
4. Removes team config from `~/.claude/teams/`
5. Returns to setup mode

Disbanding is idempotent — if the team was already disbanded, the response indicates this.

## IPC commands

Mesh view uses these backend commands:

| Command | Purpose |
|---------|---------|
| `check_mesh_install_status` | Detect mesh CLI installation and version |
| `install_mesh` | Install bundled mesh binary to `~/.local/bin/` |
| `coordination_preflight_check` | Validate prerequisites before initialization |
| `coordination_initialize_team` | Execute full team bootstrap pipeline |
| `coordination_add_agent` | Hot-add one agent to a running team |
| `coordination_reonboard` | Re-send onboarding to one member |
| `coordination_get_live_team_status` | Get runtime roster with session status |
| `coordination_list_teams` | Discover existing teams for auto-restore and cleanup |
| `coordination_disband_team` | Disband a team and clean up resources |

See [IPC command reference](../architecture/ipc-reference.md) for full signatures and return types.

## Key files

| File | Purpose |
|------|---------|
| `src/lib/components/MeshTab.svelte` | Top-level mesh tab: mode switching (setup/runtime), team discovery, cleanup panel |
| `src/lib/components/MeshSetupForm.svelte` | Team roster builder with agent cards and team config |
| `src/lib/components/MeshTeamRoster.svelte` | Live runtime roster with status badges and member actions |
| `src/lib/components/MeshInitProgress.svelte` | 7-step initialization progress with failure recovery |
| `src/lib/components/MeshAvailabilityGate.svelte` | Prerequisite checks (mesh install + preflight) |
| `src/lib/components/CoordinationPanel.svelte` | Low-level coordination CRUD panel (internal/debug) |
| `src-tauri/src/commands/coordination.rs` | Coordination IPC command handlers |
| `src-tauri/src/commands/coordination_types.rs` | Request/response types for coordination IPC |
| `src-tauri/src/commands/mesh.rs` | Mesh install status and bundled install commands |
| `src-tauri/src/coordination/orchestrator.rs` | Core team lifecycle (create, disband, add/remove member) |
| `src-tauri/src/coordination/pipelines.rs` | Multi-step initialize and hot-add pipelines |
| `src-tauri/src/coordination/mesh_cli.rs` | Mesh binary resolution and WSL command helpers |

## Related documents

- [Coordination architecture](../coordination-architecture.md) — design decisions and backend structure
- [Mesh view design](../mesh-view-design.md) — original design document
- [IPC command reference](../architecture/ipc-reference.md) — command signatures
- [Command center](command-center.md) — CLI tool launch and terminal management
- [Session management](session-management.md) — session detection that feeds roster status
