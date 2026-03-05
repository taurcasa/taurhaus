# Mesh View — Design Document

## Vision

One-click multi-agent team setup inside taurhaus. Define agents, assign projects and tools, initialize, then work directly in CLI sessions. taurhaus handles all the tmux/mesh plumbing that currently requires manual commands.

**Core value proposition:** "Define a team once, launch it reliably, then work directly in your preferred CLI sessions."

---

## 1. UI Placement

### Mesh Tab

A new top-level tab alongside Files, Git, Tasks, Overview. Anchored to whichever project the team lead lives in.

- Projects without a team show an empty mesh tab with a "Create Team" prompt
- Projects with a running team show the live team view
- On app reopen, taurhaus detects existing teams by scanning `~/.claude/teams/` and restores the team view automatically

### Why Per-Project

Every other tab in taurhaus is per-project. A global mesh view would break the mental model. The anchor is the team lead's project — switch projects in the sidebar, see that project's mesh tab (if it has one).

### Secondary Entry Points (Nice-to-Have)

- `+ New Team` in sidebar header (opens mesh tab pre-filled for selected project)
- Taskboard banner: "Need parallel agents? Set up a Mesh team."

---

## 2. Setup Flow

### Step 0: Empty State

User opens the Mesh tab and sees:
- Title: **Mesh Team Setup**
- Short explanation: "Define agents, assign projects and tools, initialize once, then coordinate in CLI."
- CTA: **Create Team**

### Step 1: Team Basics

- **Team name** (required, unique) — defaults to project name + suffix (e.g., "taurhaus-team"), editable
- **Team description** (optional)

### Step 2: Team Lead Configuration

A dedicated card, visually distinct from member agents (starred/highlighted).

Fields:
- **Name** (default: `team-lead`)
- **CLI tool** — fixed to Claude Code (shown explicitly, not hidden)
- **Model selector** — opus, sonnet, haiku
- **Project** — defaults to current project
- **Session mode** — currently fixed to `Launch new session` in setup UI (no mode toggle yet)

Why a separate card: the lead role has different constraints and importance. Reduces misconfiguration.

### Step 3: Add Member Agents

User adds agent cards. Each card:

- **Name** (required) — e.g., `frontend-dev`, `backend-dev`, `qa-reviewer`
- **CLI tool** — Claude Code / Codex / Gemini CLI
- **Model selector** — filtered by chosen tool
- **Target project** — any loaded project from the sidebar
- **Description** (optional) — helps the lead know who does what

UX details:
- Duplicate name detection with inline errors
- Tool/model/project are compact select controls in setup
- Tool brand icons are shown in runtime roster cards

### Step 4: Launch

Primary CTA: **Initialize Team**

Current implementation starts initialization directly from setup form. There is no separate review screen or save-draft flow in V1.

### Naming Convention

Agents should be named by function (frontend-dev, reviewer, etc.) so the orchestrating agent knows who to assign what. The UI encourages this with placeholder text.

---

## 3. Initialize

User clicks **Initialize Team**. The button transforms into a progress view.

### Progress Steps

```
  ✓ Configuration validated
  ✓ Team created
  ✓ Pane opened: team-lead
  ✓ Pane opened: frontend-dev
  ✓ Pane opened: reviewer
  ✓ CLI sessions launched
  ✓ Agents joined mesh
  ● Sending onboarding messages...
```

Expected steps in current UI (in order): validate configuration, create team, create panes, launch sessions, join mesh, start daemons, send onboarding.

### Failure Handling

If a step fails:
- Show exact failed step with plain-language error
- Show what succeeded already
- Buttons: `Retry` and `Back`
- If failure is "team already exists" at create-team step, show recovery actions: `Open Existing Team` or `Disband Existing Team`

One-click setup is powerful, but opaque failure destroys trust. The user needs operational visibility, not technical logs.

### What Happens Behind the Scenes

One backend IPC command (`coordination_initialize_team`) handles the full sequence:
1. Validate configuration
2. Create Claude Code team (TeamCreate)
3. Pre-create inboxes for non-Claude agents
4. Create tmux panes via terminal management
5. Launch CLIs with proper flags (`--dangerously-skip-permissions` for Claude, `--yolo` for Codex/Gemini)
6. Run `mesh join` for non-Claude agents
7. Start `mesh daemon` per non-Claude agent
8. Send onboarding message to each non-Claude agent

The frontend calls one command and applies the returned `InitializeReport` step list. It also listens to `coordination-step-progress` events when emitted. Frontend does not orchestrate individual backend steps.

---

## 4. Live Team View

After initialization, the mesh tab switches from setup mode to runtime mode:

```
┌──────────────────────────────────────────────┐
│ Team: taurhaus-team              [+ Add Agent]│
│                                   [Disband]   │
│                                               │
│  ★ team-lead          ● Active                │
│    Claude · opus · taurhaus                   │
│    [Focus Pane]                               │
│                                               │
│  ◦ frontend-dev       ● Active                │
│    Codex · gpt-5.3 · taurhaus                │
│    [Focus Pane]                               │
│                                               │
│  ◦ reviewer           ○ Idle                  │
│    Gemini · pro · mesh                        │
│    [Focus Pane]  [Re-onboard]                 │
│                                               │
└──────────────────────────────────────────────┘
```

### Agent Cards Show

- Name and role
- Tool brand icon + model + target project
- Session status (active/idle/offline) — pulled from existing session scanner
- "Focus Pane" button — primary action, jumps to agent's tmux pane
- "Resume" button on offline rows (Continue/Fresh mode selector)

### Sidebar Integration

- Existing session indicators show active/idle per tool — no new status system needed
- Mesh-owned sessions get a subtle badge for discoverability (team membership indicator)

---

## 5. Post-Setup Interactions

### Must-Have

- **Focus pane** — jump to any agent's tmux session
- **Resume offline member** — relaunch an offline member without removing/re-adding.
  - Continue mode: resume previous context when the CLI supports it.
  - Fresh mode: launch a clean session for that member identity.
- **Add agent to running team** — opens a card form, creates pane, launches CLI, joins mesh, sends onboarding. Teams evolve as needs become clear mid-sprint.
- **Re-send onboarding** — if an agent loses context, resend the mesh instructions
- **Disband** — removes team state and stops managed resources (sessions/panes/daemons/mesh membership). Confirms before acting.
- **Team cleanup panel (setup mode)** — discover/disband existing teams before launch

### Nice-to-Have (Not V1)

- Save team config as template for reuse
- Rename agent function label mid-session
- Copy mesh identity snippet for manual debugging

### Explicit Non-Goals

- No chat UI — users interact with agents in CLI tools
- No runtime per-agent remove action in V1 roster
- No real-time message log from mesh

---

## 6. Edge Cases

### Configuration

- Duplicate agent names → inline error, block Initialize
- Team lead not Claude → not selectable (UI prevents it)
- Zero agents beyond lead → allowed (can add later)
- Missing mesh binary → show setup prompt: "Mesh CLI not found. Install it to enable multi-agent collaboration."
- Missing CLI binary (Codex/Gemini) → warn on the specific agent card

### Runtime

- tmux unavailable → block Initialize with clear error
- Agent session dies or exits back to shell prompt → live-status liveness reconciliation marks the member offline (pane missing/dead/shell)
- Partial initialization failure → show what succeeded, offer retry
- Team create conflict (`already exists`) → offer `Open Existing Team` or `Disband Existing Team`
- Same project assigned to many agents → allowed, but warn about resource load
- Session launched but scanner hasn't detected it yet → show "Starting..." state with brief timeout

### Returning to Existing Teams

User closes taurhaus, reopens. Mesh tab detects existing teams by scanning `~/.claude/teams/` and shows the live team view. Session scanner picks up running agents. No re-initialization needed.

### Multiple Teams

V1: one team at a time per project. Multiple concurrent teams adds complexity without clear benefit yet.

---

## 7. What Doesn't Make Sense

1. **In-app chat interface** — high complexity, low value vs. existing CLI workflow
2. **Real-time message log** — noisy, user sees conversations directly in terminals
3. **Dependency graph / visual dashboard** — attractive but distracts from launch reliability
4. **Auto-rebalancing tasks between agents** — over-automation before users trust basics
5. **Drag-and-drop agent placement** — sounds cool but a dropdown/badge selector is faster for "pick a project"
6. **Complex health dashboard** — existing session indicators (active/idle) are sufficient
7. **Disband without cleaning managed resources** — leaves stale sessions/daemons behind
8. **Too many "advanced settings" upfront** — increases friction for first successful launch

---

## 8. What Does Make Sense

1. **One-click setup** — the entire value proposition. Configure and go.
2. **Visual team roster** — at a glance: who's on the team, what tool, what project
3. **Focus pane as primary action** — mesh view is a launchpad to jump between agents
4. **Hot-add agents** — teams evolve mid-sprint, must support adding agents to running teams
5. **Existing session indicators** — no new status system, reuse what works
6. **Onboarding via mesh** — daemon notification delivers the command reference, reliable and non-intrusive
7. **Per-project anchoring** — consistent with taurhaus's project-centric model
8. **Progress transparency** — staged initialization feedback, not a spinner

---

## 9. Technical Architecture

### Orchestrator Wiring

Lazy singleton in AppState: `Arc<Mutex<Option<CoordinationOrchestrator>>>`. First coordination IPC call bootstraps with `teams_dir = ~/.claude/teams/`. App startup has no dependency on mesh availability.

### Team Lead Mode

Modeled as `LeadMode` in the initialize request:
- **LaunchNew** — current setup UI always uses this mode. Backend creates pane, launches Claude Code with lead bootstrap prompt, then TeamCreate.
- **AttachExisting** — supported by backend request model/tests, but not exposed in current setup UI.

Frontend only selects mode. Backend decides the execution branch.

### Initialize Command

Single backend IPC command: `coordination_initialize_team(config) -> InitializeReport`. Returns per-step status in report output; progress events may also be received during execution. No multi-step frontend orchestration.

If partial failure, returns what succeeded and what failed. Frontend shows retry options per failed step.

### Add Agent to Running Team

Single IPC command: `coordination_add_agent(team_name, agent_config) -> AddAgentReport`. Backend handles: create pane → launch CLI → mesh join → daemon → onboard. Same pattern as initialize but for one agent.

### Resume Offline Member

Single IPC command: `coordination_resume_member(team_name, member_name, mode) -> ResumeAgentReport`.

- `mode = Continue` resumes prior context when supported (`ResumeContextMode`)
- `mode = Fresh` launches a new context for the same team member identity
- Runtime output includes step-level progress (`ResumeMemberRequest` / `ResumeAgentReport`)
- Frontend uses the command from `MeshNodeDetail` resume actions on offline nodes and refreshes roster state on success

Pipeline behavior mirrors initialize/add-agent for one member:
1. Validate request + current member state
2. Resolve or create pane
3. Launch CLI with mode-aware command
4. Join mesh/start daemon for non-Claude members
5. Send onboarding when applicable
6. Persist runtime state with pane/session/health updates

### Live Status Liveness Reconciliation

`coordination_get_live_team_status` performs write-on-drift reconciliation before returning roster status:

- Missing pane id => offline drift
- Pane target missing => offline drift
- `pane_dead = true` => offline drift
- `pane_current_command` resolves to shell (`bash`/`zsh`/`sh`/`fish`) => offline drift

On drift:
- runtime health becomes `SessionDead`
- `session_id` is cleared
- non-Claude `daemon_pid` is checked and terminated/cleared when still running

### Onboarding Message

Template rendered by the delivery module. Contains:
- **Identity**: "You are `{name}` on team `{team}`. Your team lead is `{lead_name}`."
- **Read loop**: `mesh read --unread --mark-read --team {team} --name {name}`
- **Reply**: `mesh send {recipient} "{msg}" --team {team} --name {name} --summary "brief"`
- **Tasks**: `mesh task list/get/update --team {team} --name {name}`
- **Work contract**: Acknowledge assignment → execute → report completion with artifacts/test results
- **Escalation**: When blocked, send blocker description to lead. Do not stall silently.

### Feature Gating

All coordination code behind `mesh-bridged-backend` Cargo feature (default enabled). The mesh tab in the frontend checks for mesh binary availability before showing the setup flow.

### What We Use From M0

- **TeamConfigStore / MemberRuntimeStore** — team and agent persistence
- **Backend selector + MeshBridged adapter** — mesh CLI integration
- **Delivery renderer** — onboarding message formatting
- **Orchestrator lifecycle** — create_team, add_member, deliver_message
- **IPC command surface** — wired to real orchestrator instead of stubs
- **Event producer/consumer** — inbox and config change detection

### What Stays Dormant

- **Advanced health escalation policy** — `SessionDead` drift handling is active; multi-stage escalation remains out of scope
- **Advanced audit projections** — basic step logging is sufficient
- **Lease reclaim semantics** — premature optimization

---

## 10. UX Principles

1. **Configuration clarity over flexibility** — simple choices, not a wall of options
2. **Progress transparency over "magic"** — show what's happening, not a spinner
3. **CLI-first interaction model** — taurhaus launches and monitors, users work in terminals
4. **Recoverability over atomicity** — partial states are fine if the user can fix them
5. **Fast repeatability** — teams users run often should be quick to recreate
