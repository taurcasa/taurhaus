// Regression: e3ace37 pinned this clock to the authoring date
// ('2026-03-06T12:00:00.000Z'). HoverCard measures session freshness against
// the real `Date.now()` with a 7-day window, so once wall-clock time passed
// that window every "fresh session" scenario silently fell back to the commit
// row and the spec's `Session: …` expectations went stale. The scenarios are
// anchored to the run's own clock instead; deliberately stale fixtures use
// `isoDaysAgo`.
const NOW = new Date()
const DEFAULT_ANCHOR_RECT = {
  left: 220,
  right: 252,
  top: 164,
  height: 28,
  width: 32,
}

function isoDaysAgo(days) {
  return new Date(NOW.getTime() - days * 24 * 60 * 60 * 1000).toISOString()
}

function createProject(overrides = {}) {
  return {
    id: 'proj-hovercard',
    path: '/projects/taurhaus',
    name: 'taurhaus',
    branch: 'main',
    activityState: 'active',
    isDirty: false,
    ...overrides,
  }
}

function createLiveSession(overrides = {}) {
  return {
    id: 'live-session-1',
    live: true,
    state: 'active',
    tool: 'claude',
    cli_tool: 'claude',
    toolLabel: 'Claude',
    _duration: 12 * 60_000,
    ...overrides,
  }
}

function createLatestSession(overrides = {}) {
  return {
    date: NOW.toISOString(),
    summary: 'Implement structured IPC logging',
    open_questions: ['Should request IDs be generated in the frontend or backend?'],
    next_steps: ['Verify JSONL sink rotation in stress mode'],
    ...overrides,
  }
}

function createCommit(overrides = {}) {
  return {
    hash: 'abc1234',
    message: 'Stabilize visual screenshot harness',
    date: 'today',
    ...overrides,
  }
}

function createRelationship(overrides = {}) {
  return {
    source_project_id: 'proj-hovercard',
    target_project_id: 'proj-sidebar',
    relationship_type: 'depends_on',
    detection_source: 'cargo_toml',
    ...overrides,
  }
}

function createScenario({
  name,
  theme,
  project,
  sessions = [],
  latestSession = null,
  recentCommits = [],
  relationships = [],
  expected,
  compareAgainst = null,
  anchorRect = DEFAULT_ANCHOR_RECT,
}) {
  return {
    name,
    theme,
    project,
    sessions,
    ipc: {
      getLatestSession: latestSession,
      getRecentCommits: recentCommits,
      getRelationships: relationships,
    },
    anchorRect,
    expected,
    compareAgainst,
  }
}

const active_claudeWorking_dirty_dark = createScenario({
  name: 'active_claudeWorking_dirty_dark',
  theme: 'dark',
  project: createProject({ isDirty: true }),
  sessions: [createLiveSession()],
  latestSession: createLatestSession(),
  recentCommits: [createCommit({ message: 'Refresh hover card data model' })],
  expected: {
    verdict: 'Active work in progress',
    motion: 'Claude is working now',
    latestChange: 'Session: Implement structured IPC logging',
    unresolved: 'Open question: Should request IDs be generated in the frontend or backend?',
    relationshipChip: null,
  },
})

const active_claudeWorking_dirty_light = createScenario({
  name: 'active_claudeWorking_dirty_light',
  theme: 'light',
  project: createProject({ isDirty: true }),
  sessions: [createLiveSession()],
  latestSession: createLatestSession(),
  recentCommits: [createCommit({ message: 'Refresh hover card data model' })],
  expected: {
    verdict: 'Active work in progress',
    motion: 'Claude is working now',
    latestChange: 'Session: Implement structured IPC logging',
    unresolved: 'Open question: Should request IDs be generated in the frontend or backend?',
    relationshipChip: null,
  },
  compareAgainst: 'active_claudeWorking_dirty_dark',
})

const idle_waitingInput_dark = createScenario({
  name: 'idle_waitingInput_dark',
  theme: 'dark',
  project: createProject(),
  sessions: [
    createLiveSession({
      state: 'idle',
      tool: 'codex',
      cli_tool: 'codex',
      toolLabel: 'Codex',
      _duration: 9 * 60_000,
      _lastTransition: null,
    }),
  ],
  latestSession: createLatestSession({
    summary: 'Prepare dependency cleanup plan',
    open_questions: [],
    next_steps: ['Confirm whether to batch the removals'],
  }),
  recentCommits: [createCommit({ message: 'Add batch delete guardrails' })],
  expected: {
    verdict: 'Waiting on user input',
    motion: 'Codex is waiting on input',
    latestChange: 'Session: Prepare dependency cleanup plan',
    unresolved: 'Next: Confirm whether to batch the removals',
    relationshipChip: null,
  },
})

const recentHandoff_withQuestion_dark = createScenario({
  name: 'recentHandoff_withQuestion_dark',
  theme: 'dark',
  project: createProject({ activityState: 'recent' }),
  sessions: [],
  latestSession: createLatestSession({
    summary: 'Handoff after daemon lifecycle review',
    open_questions: ['Should stalled sessions be auto-recovered on resume?'],
    next_steps: [],
  }),
  recentCommits: [createCommit({ message: 'Document daemon recovery flow' })],
  expected: {
    verdict: 'Recent handoff needs review',
    motion: 'No live session',
    latestChange: 'Session: Handoff after daemon lifecycle review',
    unresolved: 'Open question: Should stalled sessions be auto-recovered on resume?',
    relationshipChip: null,
  },
})

const recentHandoff_withQuestion_light = createScenario({
  name: 'recentHandoff_withQuestion_light',
  theme: 'light',
  project: createProject({ activityState: 'recent' }),
  sessions: [],
  latestSession: createLatestSession({
    summary: 'Handoff after daemon lifecycle review',
    open_questions: ['Should stalled sessions be auto-recovered on resume?'],
    next_steps: [],
  }),
  recentCommits: [createCommit({ message: 'Document daemon recovery flow' })],
  expected: {
    verdict: 'Recent handoff needs review',
    motion: 'No live session',
    latestChange: 'Session: Handoff after daemon lifecycle review',
    unresolved: 'Open question: Should stalled sessions be auto-recovered on resume?',
    relationshipChip: null,
  },
  compareAgainst: 'recentHandoff_withQuestion_dark',
})

const staleCommitOnly_dark = createScenario({
  name: 'staleCommitOnly_dark',
  theme: 'dark',
  project: createProject({ activityState: 'stale' }),
  sessions: [],
  latestSession: createLatestSession({
    date: isoDaysAgo(10),
    summary: 'Old handoff that should no longer win',
    open_questions: [],
    next_steps: [],
  }),
  recentCommits: [createCommit({ message: 'Fix search result ordering', date: '2d ago' })],
  expected: {
    verdict: 'Project may need attention',
    motion: 'No live session',
    latestChange: 'Commit: Fix search result ordering',
    unresolved: null,
    relationshipChip: null,
  },
})

const dormant_empty_dark = createScenario({
  name: 'dormant_empty_dark',
  theme: 'dark',
  project: createProject({ activityState: 'dormant' }),
  sessions: [],
  latestSession: null,
  recentCommits: [],
  expected: {
    verdict: 'Quiet project',
    motion: 'No live session',
    latestChange: 'No recent session or commit yet',
    unresolved: null,
    relationshipChip: null,
  },
})

const dormant_empty_light = createScenario({
  name: 'dormant_empty_light',
  theme: 'light',
  project: createProject({ activityState: 'dormant' }),
  sessions: [],
  latestSession: null,
  recentCommits: [],
  expected: {
    verdict: 'Quiet project',
    motion: 'No live session',
    latestChange: 'No recent session or commit yet',
    unresolved: null,
    relationshipChip: null,
  },
  compareAgainst: 'dormant_empty_dark',
})

const multiSession_threeLive_dark = createScenario({
  name: 'multiSession_threeLive_dark',
  theme: 'dark',
  project: createProject(),
  sessions: [
    createLiveSession({ state: 'idle', tool: 'claude', cli_tool: 'claude', toolLabel: 'Claude', _duration: 11 * 60_000 }),
    createLiveSession({ id: 'live-session-2', state: 'active', tool: 'codex', cli_tool: 'codex', toolLabel: 'Codex', _duration: 4 * 60_000 }),
    createLiveSession({ id: 'live-session-3', state: 'idle', tool: 'agy', cli_tool: 'agy', toolLabel: 'Antigravity', _duration: 8 * 60_000, project_unattributed_active: true }),
  ],
  latestSession: createLatestSession({
    summary: 'Coordinate the multi-session handoff',
    open_questions: ['Who owns the remaining verification pass?'],
  }),
  recentCommits: [createCommit({ message: 'Align session sorting rules' })],
  expected: {
    verdict: 'Active work in progress',
    motion: 'Codex is working now +2 more',
    latestChange: 'Session: Coordinate the multi-session handoff',
    unresolved: 'Open question: Who owns the remaining verification pass?',
    relationshipChip: null,
  },
})

const withRelationship_dependsOn_dark = createScenario({
  name: 'withRelationship_dependsOn_dark',
  theme: 'dark',
  project: createProject({ activityState: 'recent' }),
  sessions: [],
  latestSession: createLatestSession({
    summary: 'Map cross-project build ordering',
    open_questions: [],
    next_steps: [],
  }),
  recentCommits: [createCommit({ message: 'Track Cargo workspace dependency edges' })],
  relationships: [createRelationship()],
  expected: {
    verdict: 'Recent change, no live session',
    motion: 'No live session',
    latestChange: 'Session: Map cross-project build ordering',
    unresolved: null,
    relationshipChip: 'Depends on',
  },
})

export const hoverCardScenarios = [
  active_claudeWorking_dirty_dark,
  active_claudeWorking_dirty_light,
  idle_waitingInput_dark,
  recentHandoff_withQuestion_dark,
  recentHandoff_withQuestion_light,
  staleCommitOnly_dark,
  dormant_empty_dark,
  dormant_empty_light,
  multiSession_threeLive_dark,
  withRelationship_dependsOn_dark,
]
