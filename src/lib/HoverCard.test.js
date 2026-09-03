import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./ipc.js', () => ({
  getLatestSession: vi.fn(),
  getRecentCommits: vi.fn(),
  getRelationships: vi.fn(),
  listWorkflowRuns: vi.fn(),
  getWorkflowRun: vi.fn(),
}))

vi.mock('./sessionIndicator.js', () => ({
  sessionBadge: vi.fn((session) => ({ toolLabel: session.toolLabel || 'Codex' })),
  hasLiveSession: vi.fn((session) => Boolean(session.live)),
  toolIcon: vi.fn(() => ({ viewBox: '0 0 10 10', path: 'M0 0h10v10z' })),
  groupedSessionIndicators: vi.fn(() => []),
}))

vi.mock('./format.js', () => ({
  formatDuration: vi.fn((ms) => `${Math.round(ms)}ms`),
}))

vi.mock('./accounts.svelte.js', () => ({
  previewAccount: vi.fn(),
}))

const {
  getLatestSession,
  getRecentCommits,
  getRelationships,
  getWorkflowRun,
  listWorkflowRuns,
} = await import('./ipc.js')
const { resetWorkflowRunsForTest } = await import('./workflowRunStore.svelte.js')
const { previewAccount } = await import('./accounts.svelte.js')

import HoverCard from './HoverCard.svelte'

function createProject(overrides = {}) {
  return {
    id: 'proj-1',
    path: '/projects/taurhaus',
    name: 'taurhaus',
    branch: 'main',
    activityState: 'active',
    isDirty: false,
    ...overrides,
  }
}

function createLatestSession(overrides = {}) {
  return {
    date: new Date().toISOString(),
    summary: 'Implement IPC error envelope fix',
    open_questions: ['Should retry stay frontend-side?'],
    next_steps: ['Verify daemon startup logs'],
    ...overrides,
  }
}

describe('HoverCard', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getLatestSession.mockResolvedValue(createLatestSession())
    getRecentCommits.mockResolvedValue([
      { hash: 'abc1234', message: 'Fix tests', date: 'today' },
    ])
    getRelationships.mockResolvedValue([])
    previewAccount.mockResolvedValue(null)
  })

  it('does not render when project is missing', () => {
    render(HoverCard, {
      props: {
        project: null,
        sessions: [],
      },
    })

    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument()
  })

  it('shows the visible session account and why it will be used', async () => {
    previewAccount.mockResolvedValue({
      account: { id: 'work', display_name: 'Work' },
      origin: 'session',
    })

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [{ live: true, cli_tool: 'claude', state: 'active' }],
        visible: true,
      },
    })

    await waitFor(() => {
      expect(previewAccount).toHaveBeenCalledWith(
        expect.objectContaining({ id: 'proj-1' }),
        'claude',
        { visible: true, mode: 'continue' },
      )
      expect(screen.getByTestId('hovercard-account')).toHaveTextContent(
        "Claude · Work · resumes this session's account"
      )
    })
  })

  it('renders verdict-first layout with dirty chip, session summary, and unresolved item', async () => {
    render(HoverCard, {
      props: {
        project: createProject({ isDirty: true }),
        sessions: [{ live: false }],
        dark: true,
      },
    })

    await waitFor(() => {
      expect(getLatestSession).toHaveBeenCalledWith('proj-1')
      expect(getRecentCommits).toHaveBeenCalledWith('proj-1', 1)
      expect(getRelationships).toHaveBeenCalledWith('proj-1')
    })

    expect(screen.getByText('taurhaus')).toBeInTheDocument()
    expect(screen.getByText('main')).toBeInTheDocument()
    expect(screen.getByText('Dirty')).toBeInTheDocument()
    expect(screen.getByText('Recent handoff needs review')).toBeInTheDocument()
    expect(screen.getByText('No live session')).toBeInTheDocument()
    expect(screen.getByText('Session: Implement IPC error envelope fix')).toBeInTheDocument()
    expect(screen.getByText('Open question: Should retry stay frontend-side?')).toBeInTheDocument()
  })

  it('prioritizes the most relevant live session and appends +N more', async () => {
    const now = Date.now()

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [
          {
            live: true,
            state: 'idle',
            _duration: 10_000,
            _lastTransition: now - 500,
            toolLabel: 'Claude',
          },
          {
            live: true,
            state: 'active',
            _duration: 4_000,
            toolLabel: 'Codex',
          },
          {
            live: true,
            state: 'idle',
            project_unattributed_active: true,
            _duration: 9_000,
            toolLabel: 'Antigravity',
          },
        ],
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('tooltip')).toBeInTheDocument()
    })

    expect(screen.getByText('Active work in progress')).toBeInTheDocument()
    expect(screen.getByText('Codex is working now +2 more')).toBeInTheDocument()
    expect(screen.getByText('active 4000ms')).toBeInTheDocument()
  })

  it('renders grouped team roster when grouped token metadata is present', async () => {
    const { groupedSessionIndicators } = await import('./sessionIndicator.js')
    groupedSessionIndicators.mockReturnValueOnce([
      {
        kind: 'team',
        groupId: 'team-a',
        groupLabel: 'team-a',
        count: 2,
        isActive: true,
        members: [
          { member_name: 'team-lead', toolLabel: 'Claude', state: 'active' },
          { member_name: 'developer2', toolLabel: 'Codex', state: 'idle' },
        ],
      },
    ])

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [
          { live: true, state: 'active', toolLabel: 'Claude' },
          { live: true, state: 'idle', toolLabel: 'Codex' },
          { live: true, state: 'active', toolLabel: 'Antigravity' },
          { live: true, state: 'idle', toolLabel: 'Claude' },
        ],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('hovercard-team-roster')).toBeInTheDocument()
    })

    expect(screen.getByText('team-a')).toBeInTheDocument()
    expect(screen.getByText('team-lead')).toBeInTheDocument()
    expect(screen.getByText('developer2')).toBeInTheDocument()
    expect(screen.getByText('Active')).toBeInTheDocument()
    expect(screen.getByText('Idle')).toBeInTheDocument()
    expect(screen.getByTestId('hovercard-team-badge')).toHaveClass('text-success-300')
  })

  it('gives a retained-stale team header the uncertain tone, not the idle one', async () => {
    // Regression: 6c6f1cb routed every member row through the canonical
    // uncertain/info tone but left the team header on `isActive ? success :
    // warning`, so a team whose members were all retained-stale wore the same
    // amber header as a team that was simply idle.
    const { groupedSessionIndicators } = await import('./sessionIndicator.js')
    groupedSessionIndicators.mockReturnValueOnce([
      {
        kind: 'team',
        groupId: 'team-stale',
        groupLabel: 'team-stale',
        count: 2,
        isActive: false,
        tone: 'stale',
        members: [
          { member_name: 'team-lead', toolLabel: 'Claude', state: 'active', _presenceStale: true },
          { member_name: 'developer2', toolLabel: 'Codex', state: 'idle', _presenceStale: true },
        ],
      },
    ])

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [{ live: true, state: 'active', toolLabel: 'Claude' }],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('hovercard-team-roster')).toBeInTheDocument()
    })

    const badge = screen.getByTestId('hovercard-team-badge')
    expect(badge).toHaveClass('text-info-300')
    expect(badge).not.toHaveClass('text-warning-300')
  })

  it('falls back to commit summary when latest session is stale', async () => {
    getLatestSession.mockResolvedValueOnce(createLatestSession({
      date: new Date(Date.now() - 10 * 24 * 60 * 60 * 1000).toISOString(),
      summary: 'Old session summary',
      open_questions: [],
      next_steps: [],
    }))

    render(HoverCard, {
      props: {
        project: createProject({ activityState: 'recent' }),
        sessions: [],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('hovercard-latest-change')).toBeInTheDocument()
    })

    expect(screen.getByText('Commit: Fix tests')).toBeInTheDocument()
    expect(screen.queryByText('Session: Old session summary')).not.toBeInTheDocument()
    expect(screen.queryByTestId('hovercard-unresolved')).not.toBeInTheDocument()
  })

  it('renders relationship cue when a strong relationship exists', async () => {
    getRelationships.mockResolvedValueOnce([
      {
        source_project_id: 'proj-1',
        target_project_id: 'proj-2',
        relationship_type: 'depends_on',
        detection_source: 'cargo_toml',
      },
    ])

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('hovercard-relationship')).toBeInTheDocument()
    })

    expect(screen.getByText('Depends on')).toBeInTheDocument()
    expect(screen.getByText(/This project depends on another project via Cargo.toml/)).toBeInTheDocument()
  })

  it('handles fetch errors by falling back to quiet empty-state copy', async () => {
    getLatestSession.mockRejectedValueOnce(new Error('session failed'))
    getRecentCommits.mockRejectedValueOnce(new Error('commits failed'))
    getRelationships.mockRejectedValueOnce(new Error('relationships failed'))

    render(HoverCard, {
      props: {
        project: createProject({ activityState: 'dormant' }),
        sessions: [],
      },
    })

    await waitFor(() => {
      expect(getLatestSession).toHaveBeenCalled()
      expect(getRecentCommits).toHaveBeenCalled()
      expect(getRelationships).toHaveBeenCalled()
    })

    expect(screen.getByText('Quiet project')).toBeInTheDocument()
    expect(screen.getByText('No recent session or commit yet')).toBeInTheDocument()
    expect(screen.queryByTestId('hovercard-relationship')).not.toBeInTheDocument()
  })

  it('applies dark and light theme surface classes', async () => {
    const { rerender } = render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [],
        dark: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('tooltip')).toBeInTheDocument()
    })
    expect(screen.getByRole('tooltip').className).toContain('bg-brand-950/96')

    await rerender({
      project: createProject(),
      sessions: [],
      dark: false,
    })

    await waitFor(() => {
      expect(screen.getByRole('tooltip').className).toContain('bg-white/96')
    })
  })

  it('positions card to stay in viewport when anchor would overflow right and bottom', async () => {
    const anchorEl = {
      getBoundingClientRect: () => ({
        left: 760,
        right: 790,
        top: 580,
        height: 24,
      }),
    }

    const previousWidth = window.innerWidth
    const previousHeight = window.innerHeight
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 800 })
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 600 })

    const rectSpy = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function getRect() {
        if (this.getAttribute && this.getAttribute('role') === 'tooltip') {
          return {
            left: 0,
            top: 0,
            right: 288,
            bottom: 220,
            width: 288,
            height: 220,
            x: 0,
            y: 0,
            toJSON() {},
          }
        }
        return {
          left: 0,
          top: 0,
          right: 0,
          bottom: 0,
          width: 0,
          height: 0,
          x: 0,
          y: 0,
          toJSON() {},
        }
      })

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [],
        anchorEl,
      },
    })

    await waitFor(() => {
      const tooltip = screen.getByRole('tooltip')
      expect(tooltip.style.left).toBe('462px')
      expect(tooltip.style.top).toBe('368px')
    })

    rectSpy.mockRestore()
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: previousWidth })
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: previousHeight })
  })

  it('clamps top position to minimum viewport inset', async () => {
    const anchorEl = {
      getBoundingClientRect: () => ({
        left: 10,
        right: 40,
        top: -40,
        height: 20,
      }),
    }

    const rectSpy = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function getRect() {
        if (this.getAttribute && this.getAttribute('role') === 'tooltip') {
          return {
            left: 0,
            top: 0,
            right: 288,
            bottom: 260,
            width: 288,
            height: 260,
            x: 0,
            y: 0,
            toJSON() {},
          }
        }
        return {
          left: 0,
          top: 0,
          right: 0,
          bottom: 0,
          width: 0,
          height: 0,
          x: 0,
          y: 0,
          toJSON() {},
        }
      })

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [],
        anchorEl,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('tooltip').style.top).toBe('12px')
    })

    rectSpy.mockRestore()
  })
})

describe('HoverCard workflow runs', () => {
  const liveRun = {
    run_id: 'wf_live',
    name: 'feature-pr',
    phases: ['Implement', 'Review'],
    status: 'live',
    started_at: 1000,
    finished_at: null,
    agents: [
      {
        agent_id: 'a',
        label: 'reviewer',
        phase: 'Review',
        model: 'gpt-5.6',
        state: 'running',
        prompt_preview: 'Review the diff',
        last_tool: 'Read',
        tokens: 2400,
        tool_calls: 2,
        last_write_at: 900,
      },
    ],
    totals: { agents: 2, done: 1, tokens: 2400, tool_calls: 2, duration_ms: null },
  }

  beforeEach(() => {
    vi.clearAllMocks()
    resetWorkflowRunsForTest()
    getLatestSession.mockResolvedValue(createLatestSession())
    getRecentCommits.mockResolvedValue([])
    getRelationships.mockResolvedValue([])
    listWorkflowRuns.mockResolvedValue([])
    getWorkflowRun.mockResolvedValue(liveRun)
  })

  it('shows no workflow section for a session running none', () => {
    render(HoverCard, {
      props: { project: createProject(), sessions: [{ live: true, state: 'active' }] },
    })

    expect(screen.queryByTestId('hovercard-workflow')).not.toBeInTheDocument()
  })

  it('reports the live run count and the last agent write without a session id', () => {
    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [{
          live: true,
          state: 'idle',
          workflow_activity: { live_runs: 2, last_write_at: Date.now() - 5000 },
        }],
      },
    })

    const section = screen.getByTestId('hovercard-workflow')
    expect(section).toHaveTextContent('2 workflow runs live')
    expect(section).toHaveTextContent('last agent write 5s ago')
    expect(listWorkflowRuns).not.toHaveBeenCalled()
  })

  // Regression: 1663e40 keyed this on `session_id`, which `DisplaySession`
  // strips before the sessions reach the frontend — so on the production shape
  // the card never asked for the run and never named it.
  it('names the run from the id the frontend session snapshot carries', async () => {
    listWorkflowRuns.mockResolvedValue([liveRun])

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [{
          live: true,
          state: 'idle',
          workflow_session_id: 'sess-display',
          workflow_activity: { live_runs: 1, last_write_at: Date.now() - 2000 },
        }],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('hovercard-workflow')).toHaveTextContent('feature-pr · Review')
    })
    expect(listWorkflowRuns).toHaveBeenCalledWith('sess-display')
  })

  it('names the run and the phase it is in once the run has been read', async () => {
    listWorkflowRuns.mockResolvedValue([liveRun])

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [{
          live: true,
          state: 'idle',
          session_id: 'sess-1',
          workflow_activity: { live_runs: 1, last_write_at: Date.now() - 2000 },
        }],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('hovercard-workflow')).toHaveTextContent('feature-pr · Review')
    })
    expect(listWorkflowRuns).toHaveBeenCalledWith('sess-1')
  })
})
