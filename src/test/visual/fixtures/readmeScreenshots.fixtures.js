import { createAgentMembers, createLeadMember } from './builders.js'

function project({
  id,
  name,
  path,
  activityState,
  branch,
  isDirty = false,
  description = '',
}) {
  return { id, name, path, activityState, branch, isDirty, description }
}

function session({
  cli_tool,
  state,
  tmux_session,
  tmux_window,
  tmux_pane,
  pid,
  group_kind = 'standalone',
  group_id = null,
  group_label = null,
  member_name = null,
  _duration = 14 * 60_000,
  _lastTransition = Date.now() - 90_000,
  project_unattributed_active = false,
}) {
  return {
    cli_tool,
    state,
    tmux_session,
    tmux_window,
    tmux_pane,
    pid,
    group_kind,
    group_id,
    group_label,
    member_name,
    _duration,
    _lastTransition,
    project_unattributed_active,
  }
}

const selectedProject = project({
  id: 'proj-taurhaus',
  name: 'taurhaus',
  path: '/projects/taurhaus',
  activityState: 'active',
  branch: 'main',
  isDirty: true,
  description: 'Desktop control surface for supervising multi-project AI work.',
})

const projects = [
  selectedProject,
  project({
    id: 'proj-mesh',
    name: 'mesh',
    path: '/projects/mesh',
    activityState: 'active',
    branch: 'team-daemon',
  }),
  project({
    id: 'proj-ragged',
    name: 'ragged-api',
    path: '/projects/ragged-api',
    activityState: 'recent',
    branch: 'resume-flow',
  }),
  project({
    id: 'proj-docs',
    name: 'docs-site',
    path: '/projects/docs-site',
    activityState: 'recent',
    branch: 'search-copy',
    isDirty: true,
  }),
  project({
    id: 'proj-runner',
    name: 'runner-kit',
    path: '/projects/runner-kit',
    activityState: 'stale',
    branch: 'fix/windows-ipc',
  }),
  project({
    id: 'proj-index',
    name: 'indexer-lab',
    path: '/projects/indexer-lab',
    activityState: 'dormant',
    branch: 'main',
  }),
  project({
    id: 'proj-shared',
    name: 'shared-prompts',
    path: '/projects/shared-prompts',
    activityState: 'active',
    branch: 'mesh/runtime',
  }),
]

const sidebarSessionsByProject = {
  '/projects/taurhaus': [
    session({
      cli_tool: 'claude',
      state: 'active',
      tmux_session: 'taurhaus',
      tmux_window: '1',
      tmux_pane: '%11',
      pid: 101,
    }),
    session({
      cli_tool: 'codex',
      state: 'idle',
      tmux_session: 'taurhaus',
      tmux_window: '2',
      tmux_pane: '%12',
      pid: 102,
    }),
  ],
  '/projects/mesh': [
    session({
      cli_tool: 'claude',
      state: 'active',
      tmux_session: 'mesh',
      tmux_window: '3',
      tmux_pane: '%31',
      pid: 201,
      group_kind: 'team',
      group_id: 'mesh-runtime',
      group_label: 'mesh-runtime',
      member_name: 'lead',
    }),
    session({
      cli_tool: 'codex',
      state: 'active',
      tmux_session: 'mesh',
      tmux_window: '4',
      tmux_pane: '%41',
      pid: 202,
      group_kind: 'team',
      group_id: 'mesh-runtime',
      group_label: 'mesh-runtime',
      member_name: 'developer1',
    }),
    session({
      cli_tool: 'agy',
      state: 'idle',
      tmux_session: 'mesh',
      tmux_window: '5',
      tmux_pane: '%51',
      pid: 203,
      group_kind: 'team',
      group_id: 'mesh-runtime',
      group_label: 'mesh-runtime',
      member_name: 'ui',
    }),
    session({
      cli_tool: 'claude',
      state: 'idle',
      tmux_session: 'mesh',
      tmux_window: '6',
      tmux_pane: '%61',
      pid: 204,
      group_kind: 'team',
      group_id: 'mesh-runtime',
      group_label: 'mesh-runtime',
      member_name: 'research',
    }),
  ],
  '/projects/shared-prompts': [
    session({
      cli_tool: 'agy',
      state: 'active',
      tmux_session: 'shared-prompts',
      tmux_window: '2',
      tmux_pane: '%21',
      pid: 301,
    }),
  ],
}

const overviewData = {
  projects,
  selectedProject,
  recentCommits: [
    { hash: 'c30ffee', message: 'Make mesh live status refresh non-blocking', date: '2m ago' },
    { hash: '82e7b04', message: 'Split command_center into domain modules', date: '19m ago' },
    { hash: 'a3db2f9', message: 'Fix clippy lint in live status reconcile', date: '1h ago' },
    { hash: '30fcf08', message: 'Fix onboarding E2E mesh daemon harness', date: '2h ago' },
  ],
  commitsLoading: false,
  latestSession: {
    id: 'sess-taurhaus-latest',
    date: new Date(Date.now() - 45 * 60_000).toISOString(),
    summary: 'Wrapped the Windows Mesh stall fixes and verified the last runtime poll regression path.',
    next_steps: [
      'Capture README screenshots with reproducible visual fixtures.',
      'Regenerate architecture diagrams flagged as stale.',
    ],
    open_questions: [
      'Does the README need an additional onboarding confidence shot?',
    ],
  },
  sessionHistory: [
    { date: new Date(Date.now() - 4 * 60 * 60_000).toISOString(), summary: 'Closed the command_center split refactor with import fixes.' },
    { date: new Date(Date.now() - 28 * 60 * 60_000).toISOString(), summary: 'Validated hot-swap drift detection and WSL daemon identity handling.' },
  ],
  sessionLoading: false,
  readmeContent: {
    content: `# taurhaus

taurhaus is a native desktop operations surface for developers running Claude, Codex, Antigravity, and Mesh teams across many projects.

## Why teams use it

- watch live session state without living in tmux
- recover context from README, commits, tasks, and handoffs
- launch, resume, and supervise Mesh teams with visible runtime state`,
  },
  relationships: [
    {
      source_project_id: 'proj-taurhaus',
      target_project_id: 'proj-mesh',
      relationship_type: 'depends_on',
      detection_source: 'claude_md',
    },
    {
      source_project_id: 'proj-taurhaus',
      target_project_id: 'proj-shared',
      relationship_type: 'references',
      detection_source: 'session_mention',
    },
  ],
  relationshipsLoading: false,
}

const tasks = [
  {
    id: '598',
    source: 'claude',
    source_key: 'claude:#598',
    subject: 'Capture README screenshots',
    description: 'Create reproducible README-ready screenshots using the visual lane.',
    active_form: 'capturing screenshot fixtures',
    status: 'in_progress',
    blocked_by: [],
    blocks: ['599'],
    owner: 'developer1',
  },
  {
    id: '600',
    source: 'codex',
    source_key: 'codex:#600',
    subject: 'Fix ARCHITECTURE.md drift',
    description: 'Apply the findings from the architecture accuracy review.',
    active_form: null,
    status: 'completed',
    blocked_by: ['594'],
    blocks: [],
    owner: 'developer1',
  },
  {
    id: '597',
    source: 'agy',
    source_key: 'agy:#597',
    subject: 'Finalize README shot list',
    description: 'Define the screenshots required for the new README.',
    active_form: null,
    status: 'completed',
    blocked_by: [],
    blocks: ['598'],
    owner: 'architect',
  },
  {
    id: '575',
    source: 'claude',
    source_key: 'claude:#575',
    subject: 'Project selection debounce investigation',
    description: 'Assess whether project switching needs true request cancellation.',
    active_form: null,
    status: 'pending',
    blocked_by: [],
    blocks: [],
    owner: 'developer1',
  },
  {
    id: '577',
    source: 'codex',
    source_key: 'codex:#577',
    subject: 'command_center modularization follow-up',
    description: 'Split the command surface into behavior-preserving domain modules.',
    active_form: null,
    status: 'pending',
    blocked_by: [],
    blocks: [],
    owner: 'developer1',
  },
]

const taskDetail = {
  task: {
    ...tasks[0],
    archived_at: null,
    archived_reason: null,
    last_status: 'in_progress',
  },
  session: {
    id: 'sess-screenshots',
    started_at: new Date(Date.now() - 38 * 60_000).toISOString(),
    ended_at: new Date(Date.now() - 8 * 60_000).toISOString(),
  },
  commits: [
    { hash: '03a650c', message: 'Make mesh live status refresh non-blocking', date: 'today' },
    { hash: '6ff5019', message: 'Delay mesh live refresh off initial tab switch', date: 'today' },
  ],
  files_changed: [
    'src/test/visual/specs/readme.visual.test.js',
    'src/test/visual/fixtures/readmeScreenshots.fixtures.js',
    'docs/screenshots/mesh-runtime-canvas.png',
  ],
}

const searchResults = [
  {
    project_id: 'proj-taurhaus',
    entity_type: 'document',
    file_path: 'README.md',
    title: 'README hero copy',
    snippet: 'Operational visibility, context recovery, and Mesh team control in one place.',
  },
  {
    project_id: 'proj-mesh',
    entity_type: 'document',
    file_path: 'src/team_daemon.rs',
    title: 'team_daemon hot-swap identity check',
    snippet: 'Restart drifted daemons when the running executable no longer matches ~/.local/bin/mesh.',
  },
  {
    project_id: 'proj-taurhaus',
    entity_type: 'session',
    file_path: 'session:mesh-recovery',
    title: 'Mesh recovery verification',
    snippet: 'Validated Resume Team and degraded recovery affordances against live runtime state.',
  },
  {
    project_id: 'proj-docs',
    entity_type: 'session',
    file_path: 'session:readme-refresh',
    title: 'README rewrite planning',
    snippet: 'Reorganized the README around live supervision, context recovery, and Mesh teams.',
  },
  {
    project_id: 'proj-taurhaus',
    entity_type: 'commit',
    file_path: 'commit:03a650c',
    title: '03a650c Make mesh live status refresh non-blocking',
    snippet: 'Moved live team status onto spawn_blocking so tab switching no longer stalls.',
  },
  {
    project_id: 'proj-mesh',
    entity_type: 'commit',
    file_path: 'commit:cdb4314',
    title: 'cdb4314 Make team daemon process checks macOS-safe',
    snippet: 'Replaced Linux-only /proc identity checks with shared ps and kill handling.',
  },
]

const gitCommits = [
  {
    hash: '03a650c',
    short_hash: '03a650c',
    message: 'Make mesh live status refresh non-blocking',
    author: 'developer1',
    date: '2026-03-07 21:12',
  },
  {
    hash: '008cd93',
    short_hash: '008cd93',
    message: 'Drop stale mesh refresh state on reactivation',
    author: 'developer1',
    date: '2026-03-07 20:58',
  },
  {
    hash: '6ff5019',
    short_hash: '6ff5019',
    message: 'Delay mesh live refresh off initial tab switch',
    author: 'developer1',
    date: '2026-03-07 20:33',
  },
]

const gitCommitFiles = [
  { path: 'src-tauri/src/commands/coordination.rs', status: 'modified' },
  { path: 'src/lib/components/meshTabController.svelte.js', status: 'modified' },
  { path: 'src/lib/components/MeshTab.test.js', status: 'modified' },
]

const gitDiffHunks = [
  {
    header: '@@ -159,7 +159,14 @@',
    lines: [
      { origin: ' ', old_lineno: 159, new_lineno: 159, content: ' pub async fn coordination_get_live_team_status(' },
      { origin: '-', old_lineno: 160, new_lineno: null, content: '    state: State<\'_, CoordinationState>,' },
      { origin: '+', old_lineno: null, new_lineno: 160, content: '    app: tauri::AppHandle,' },
      { origin: '+', old_lineno: null, new_lineno: 161, content: '    state: State<\'_, CoordinationState>,' },
      { origin: '+', old_lineno: null, new_lineno: 162, content: '    team_name: String,' },
      { origin: ' ', old_lineno: 161, new_lineno: 163, content: ' ) -> IpcResult<LiveTeamStatus> {' },
      { origin: '+', old_lineno: null, new_lineno: 164, content: '    let result = tauri::async_runtime::spawn_blocking(move || {' },
      { origin: '+', old_lineno: null, new_lineno: 165, content: '        coordination_get_live_team_status_impl(state.inner(), team_name).ipc()' },
    ],
  },
]

function withRoleDetails(member, overrides = {}) {
  return {
    roleName: overrides.roleName ?? (member.role === 'lead' ? 'orchestrator' : 'specialist'),
    focusArea: overrides.focusArea ?? 'Keep the active project plan, runtime, and blockers coherent.',
    contextSummary: overrides.contextSummary ?? 'Tracks current work, open issues, and recent handoffs.',
    behaviorSummary: overrides.behaviorSummary ?? 'Acts independently on local edits, escalates on scope or ownership ambiguity.',
    description: overrides.description ?? 'Context-steering role for focused execution.',
    projectLabel: overrides.projectLabel ?? (member.isCrossProject ? 'mesh' : 'taurhaus'),
    ...member,
    ...overrides,
  }
}

const meshLead = withRoleDetails(
  createLeadMember({
    id: 'lead',
    name: 'team-lead',
    tool: 'claude',
    toolLabel: 'Claude',
    model: 'opus',
    status: 'active',
  }),
  {
    roleName: 'orchestrator',
    focusArea: 'Keep the release plan, assignments, and runtime coordination aligned.',
    contextSummary: 'Maintains the README refresh, release readiness, and cross-agent dependencies.',
    behaviorSummary: 'Coordinates task flow, nudges stalled members, and escalates blockers quickly.',
    projectLabel: 'taurhaus',
  }
)

const meshAgents = createAgentMembers(5).map((member, index) => withRoleDetails(member, {
  name: ['developer1', 'architect', 'reviewer', 'ui-specialist', 'mesh-expert'][index],
  projectId: index === 4 ? 'proj-mesh' : 'proj-taurhaus',
  isCrossProject: index === 4,
  projectLabel: index === 4 ? 'mesh' : 'taurhaus',
  status: ['active', 'idle', 'active', 'offline', 'idle'][index],
  tool: ['codex', 'claude', 'agy', 'codex', 'claude'][index],
  toolLabel: ['Codex', 'Claude', 'Antigravity', 'Codex', 'Claude'][index],
  model: ['gpt-5.4 high', 'opus', '2.5-pro', 'gpt-5.4 high', 'sonnet'][index],
  roleName: ['developer', 'architect', 'reviewer', 'ui-specialist', 'mesh-expert'][index],
  focusArea: [
    'Implement task-owned fixes and keep regressions covered.',
    'Preserve system boundaries and architecture intent.',
    'Audit risky chains for regressions and missing tests.',
    'Shape the visible workflow and screenshot polish.',
    'Maintain mesh daemon/versioning correctness.',
  ][index],
}))

const meshSetupTeam = {
  lead: meshLead,
  agents: meshAgents.slice(0, 3),
}

const meshRuntimeTeam = {
  lead: meshLead,
  agents: meshAgents,
}

export const readmeScreenshotScenarios = [
  {
    name: 'hero-overview',
    fileName: 'readme-hero-overview.png',
    mode: 'overview',
    layout: 'window',
    theme: 'dark',
    viewport: { width: 1600, height: 1000 },
  },
  {
    name: 'sidebar-live-supervision',
    fileName: 'readme-sidebar-live-supervision.png',
    mode: 'overview',
    layout: 'window',
    theme: 'dark',
    viewport: { width: 1200, height: 900 },
    hoverProjectName: 'mesh',
  },
  {
    name: 'task-board-context',
    fileName: 'readme-task-board-context.png',
    mode: 'tasks',
    layout: 'window',
    theme: 'dark',
    viewport: { width: 1400, height: 900 },
  },
  {
    name: 'search-overlay',
    fileName: 'readme-search-overlay.png',
    mode: 'search',
    layout: 'window',
    theme: 'dark',
    viewport: { width: 1200, height: 800 },
    searchQuery: 'mesh runtime',
  },
  {
    name: 'mesh-setup-composition',
    fileName: 'readme-mesh-setup-composition.png',
    mode: 'mesh-setup',
    layout: 'window',
    theme: 'dark',
    viewport: { width: 1400, height: 900 },
  },
  {
    name: 'mesh-runtime-canvas',
    fileName: 'readme-mesh-runtime-canvas.png',
    mode: 'mesh-runtime',
    layout: 'window',
    theme: 'dark',
    viewport: { width: 1600, height: 1000 },
  },
  {
    name: 'mesh-recovery-resume',
    fileName: 'readme-mesh-recovery-resume.png',
    mode: 'mesh-recovery',
    layout: 'window',
    theme: 'dark',
    viewport: { width: 1400, height: 900 },
  },
  {
    name: 'git-context-inspection',
    fileName: 'readme-git-context-inspection.png',
    mode: 'git',
    layout: 'window',
    theme: 'dark',
    viewport: { width: 1400, height: 900 },
  },
]

export const readmeScreenshotFixtureData = {
  projects,
  selectedProject,
  overviewData,
  sidebarSessionsByProject,
  tasks,
  taskDetail,
  searchResults,
  availableProjects: projects.map((item) => ({
    id: item.id,
    name: item.name,
    path: item.path,
  })),
  mesh: {
    teamName: 'taurhaus-team',
    setupTeam: meshSetupTeam,
    runtimeTeam: meshRuntimeTeam,
    selectedSetupNode: meshSetupTeam.agents[1],
    selectedRuntimeNode: meshRuntimeTeam.agents[0],
  },
  git: {
    commits: gitCommits,
    commitFiles: gitCommitFiles,
    diffHunks: gitDiffHunks,
  },
}
