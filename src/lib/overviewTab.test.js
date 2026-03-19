/**
 * OverviewTab component tests.
 *
 * Tests quick actions, commit display, session rendering,
 * relationships, conditional sections, callbacks, and layout ordering.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

// Mock markdown renderer — just renders the source as text
vi.mock('./MarkdownRenderer.svelte', () => {
  return {
    default: function MockMarkdown(target, props) {
      const el = document.createElement('div')
      el.setAttribute('data-testid', 'markdown-renderer')
      el.textContent = props?.source || ''
      // Svelte 5 may pass a comment anchor node inside {#if} blocks
      if (target.nodeType === Node.COMMENT_NODE) {
        target.parentNode.insertBefore(el, target)
      } else {
        target.appendChild(el)
      }
      return { $set() {}, $destroy() { el.remove() } }
    },
  }
})

import OverviewTab from './OverviewTab.svelte'
import OverviewTabContextHarness from './OverviewTabContextHarness.svelte'

/** Minimal project for rendering. */
function makeProject(overrides = {}) {
  return {
    id: 'p1',
    name: 'taurhaus',
    path: '~/projects/taurhaus',
    branch: 'main',
    isDirty: false,
    activityState: 'active',
    description: 'Desktop tool for AI project management',
    createdAt: '2025-01-01T00:00:00Z',
    ...overrides,
  }
}

/** Default props for OverviewTab. */
function defaultProps(overrides = {}) {
  const dataOverrideKeys = new Set([
    'selectedProject',
    'projects',
    'recentCommits',
    'commitsLoading',
    'latestSession',
    'sessionHistory',
    'sessionLoading',
    'readmeContent',
    'relationships',
    'relationshipsLoading',
  ])
  const actionOverrideKeys = new Set([
    'onNavigateToCommit',
    'onSelectProject',
    'onLaunchSession',
    'onOpenTerminal',
  ])
  const baseData = {
    selectedProject: makeProject(),
    projects: [makeProject()],
    recentCommits: [],
    commitsLoading: false,
    latestSession: null,
    sessionHistory: [],
    sessionLoading: false,
    readmeContent: null,
    relationships: [],
    relationshipsLoading: false,
  }
  const baseActions = {
    onNavigateToCommit: vi.fn(),
    onSelectProject: vi.fn(),
    onLaunchSession: vi.fn(),
    onOpenTerminal: vi.fn(),
  }
  const data = { ...baseData, ...(overrides.data || {}) }
  const actions = { ...baseActions, ...(overrides.actions || {}) }
  const rest = {}

  for (const [key, value] of Object.entries(overrides)) {
    if (key === 'data' || key === 'actions') continue
    if (dataOverrideKeys.has(key)) {
      data[key] = value
      continue
    }
    if (actionOverrideKeys.has(key)) {
      actions[key] = value
      continue
    }
    rest[key] = value
  }

  return {
    dark: false,
    codeTheme: 'github-light',
    data,
    actions,
    onViewAllCommits: vi.fn(),
    onDismissRelationship: vi.fn(),
    onMarkdownNavigate: vi.fn(),
    ...rest,
  }
}

describe('OverviewTab', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  // --- Project header ---

  it('renders project name in header', () => {
    render(OverviewTab, { props: defaultProps() })
    expect(screen.getByText('taurhaus')).toBeTruthy()
  })

  it('shows branch name', () => {
    render(OverviewTab, { props: defaultProps() })
    expect(screen.getByText('main')).toBeTruthy()
  })

  it('shows activity state', () => {
    render(OverviewTab, { props: defaultProps() })
    expect(screen.getByText('active')).toBeTruthy()
  })

  it('shows description when present', () => {
    render(OverviewTab, { props: defaultProps() })
    expect(screen.getByText('Desktop tool for AI project management')).toBeTruthy()
  })

  it('shows dirty indicator when isDirty is true', () => {
    render(OverviewTab, {
      props: defaultProps({ selectedProject: makeProject({ isDirty: true }) }),
    })
    const dot = document.querySelector('[title="Uncommitted changes"]')
    expect(dot).toBeTruthy()
  })

  it('hides dirty indicator when isDirty is false', () => {
    render(OverviewTab, { props: defaultProps() })
    const dot = document.querySelector('[title="Uncommitted changes"]')
    expect(dot).toBeNull()
  })

  // --- Quick actions ---

  it('renders Claude, Codex, and Gemini launch buttons', () => {
    render(OverviewTab, { props: defaultProps() })
    expect(screen.getByTestId('action-launch-claude')).toBeTruthy()
    expect(screen.getByTestId('action-launch-codex')).toBeTruthy()
    expect(screen.getByTestId('action-launch-gemini')).toBeTruthy()
  })

  it('renders Terminal button', () => {
    render(OverviewTab, { props: defaultProps() })
    expect(screen.getByTestId('action-open-terminal')).toBeTruthy()
  })

  it('clicking Claude button calls onLaunchSession("claude")', async () => {
    const onLaunchSession = vi.fn()
    render(OverviewTab, { props: defaultProps({ onLaunchSession }) })
    await fireEvent.click(screen.getByTestId('action-launch-claude'))
    expect(onLaunchSession).toHaveBeenCalledWith('claude')
  })

  it('clicking Codex button calls onLaunchSession("codex")', async () => {
    const onLaunchSession = vi.fn()
    render(OverviewTab, { props: defaultProps({ onLaunchSession }) })
    await fireEvent.click(screen.getByTestId('action-launch-codex'))
    expect(onLaunchSession).toHaveBeenCalledWith('codex')
  })

  it('clicking Gemini button calls onLaunchSession("gemini")', async () => {
    const onLaunchSession = vi.fn()
    render(OverviewTab, { props: defaultProps({ onLaunchSession }) })
    await fireEvent.click(screen.getByTestId('action-launch-gemini'))
    expect(onLaunchSession).toHaveBeenCalledWith('gemini')
  })

  it('clicking Terminal button calls onOpenTerminal', async () => {
    const onOpenTerminal = vi.fn()
    render(OverviewTab, { props: defaultProps({ onOpenTerminal }) })
    await fireEvent.click(screen.getByTestId('action-open-terminal'))
    expect(onOpenTerminal).toHaveBeenCalled()
  })

  // --- Commits (Recent Activity only — no separate Last Commit) ---

  it('shows loading skeleton for commits', () => {
    render(OverviewTab, { props: defaultProps({ commitsLoading: true }) })
    expect(screen.getByTestId('commits-loading')).toBeTruthy()
  })

  it('shows "No commits found" when no commits available', () => {
    render(OverviewTab, { props: defaultProps({ recentCommits: [] }) })
    expect(screen.getByText('No commits found.')).toBeTruthy()
  })

  // --- Recent activity ---

  it('renders commit rows in recent activity', () => {
    const commits = [
      { hash: 'abc1234', message: 'First', date: '1h' },
      { hash: 'def5678', message: 'Second', date: '2h' },
    ]
    render(OverviewTab, { props: defaultProps({ recentCommits: commits }) })
    const rows = screen.getAllByTestId('overview-commit-row')
    expect(rows).toHaveLength(2)
  })

  it('shows "View all" button for recent commits', () => {
    const commits = [{ hash: 'abc1234', message: 'Commit', date: '1h' }]
    render(OverviewTab, { props: defaultProps({ recentCommits: commits }) })
    expect(screen.getByText(/View all/)).toBeTruthy()
  })

  it('clicking "View all" calls onViewAllCommits', async () => {
    const onViewAllCommits = vi.fn()
    const commits = [{ hash: 'abc1234', message: 'Commit', date: '1h' }]
    render(OverviewTab, { props: defaultProps({ recentCommits: commits, onViewAllCommits }) })
    await fireEvent.click(screen.getByText(/View all/))
    expect(onViewAllCommits).toHaveBeenCalled()
  })

  // --- Sessions (combined, conditional) ---

  it('hides sessions section when no data exists', () => {
    render(OverviewTab, { props: defaultProps({ latestSession: null, sessionHistory: [], sessionLoading: false }) })
    expect(screen.queryByTestId('overview-sessions')).toBeNull()
  })

  it('shows sessions section when latestSession exists', () => {
    const session = {
      summary: 'Completed Phase 5B implementation',
      date: new Date().toISOString(),
      next_steps: ['Implement file watcher'],
      open_questions: ['Virtual scrolling?'],
    }
    render(OverviewTab, { props: defaultProps({ latestSession: session }) })
    expect(screen.getByText('Completed Phase 5B implementation')).toBeTruthy()
  })

  it('shows next steps in session', () => {
    const session = {
      summary: 'Session summary',
      date: new Date().toISOString(),
      next_steps: ['Step one', 'Step two'],
      open_questions: [],
    }
    render(OverviewTab, { props: defaultProps({ latestSession: session }) })
    expect(screen.getByText('Step one')).toBeTruthy()
    expect(screen.getByText('Step two')).toBeTruthy()
  })

  it('shows open questions in session', () => {
    const session = {
      summary: 'Session summary',
      date: new Date().toISOString(),
      next_steps: [],
      open_questions: ['How to handle X?'],
    }
    render(OverviewTab, { props: defaultProps({ latestSession: session }) })
    expect(screen.getByText('How to handle X?')).toBeTruthy()
  })

  it('shows sessions section with loading skeleton', () => {
    render(OverviewTab, { props: defaultProps({ sessionLoading: true }) })
    expect(screen.getByTestId('overview-sessions')).toBeTruthy()
    expect(screen.getByTestId('sessions-loading')).toBeTruthy()
  })

  // --- Relationships (conditional) ---

  it('hides relationships section when no data exists', () => {
    render(OverviewTab, { props: defaultProps({ relationships: [], relationshipsLoading: false }) })
    expect(screen.queryByTestId('overview-relationships')).toBeNull()
  })

  it('shows relationships section with loading skeleton', () => {
    render(OverviewTab, { props: defaultProps({ relationshipsLoading: true }) })
    expect(screen.getByTestId('overview-relationships')).toBeTruthy()
    expect(screen.getByTestId('relationships-loading')).toBeTruthy()
  })

  it('renders relationship rows', () => {
    const projects = [
      makeProject(),
      makeProject({ id: 'p2', name: 'taurui' }),
    ]
    const rels = [
      {
        id: 'rel-1',
        source_project_id: 'p1',
        target_project_id: 'p2',
        relationship_type: 'references',
        detection_source: 'claude_md',
        dismissed: false,
      },
    ]
    render(OverviewTab, { props: defaultProps({ projects, relationships: rels }) })
    const rows = screen.getAllByTestId('relationship-row')
    expect(rows).toHaveLength(1)
    expect(rows[0].textContent).toContain('taurui')
    expect(rows[0].textContent).toContain('references')
  })

  it('shows dismiss button for auto-detected relationships', () => {
    const projects = [makeProject(), makeProject({ id: 'p2', name: 'taurui' })]
    const rels = [{
      id: 'rel-1', source_project_id: 'p1', target_project_id: 'p2',
      relationship_type: 'references', detection_source: 'claude_md', dismissed: false,
    }]
    render(OverviewTab, { props: defaultProps({ projects, relationships: rels }) })
    expect(screen.getByTestId('dismiss-relationship')).toBeTruthy()
  })

  it('clicking dismiss calls onDismissRelationship', async () => {
    const onDismissRelationship = vi.fn()
    const projects = [makeProject(), makeProject({ id: 'p2', name: 'taurui' })]
    const rels = [{
      id: 'rel-1', source_project_id: 'p1', target_project_id: 'p2',
      relationship_type: 'references', detection_source: 'claude_md', dismissed: false,
    }]
    render(OverviewTab, { props: defaultProps({ projects, relationships: rels, onDismissRelationship }) })
    await fireEvent.click(screen.getByTestId('dismiss-relationship'))
    expect(onDismissRelationship).toHaveBeenCalledWith('rel-1')
  })

  it('shows relationship count', () => {
    const projects = [makeProject(), makeProject({ id: 'p2', name: 'taurui' })]
    const rels = [{
      id: 'rel-1', source_project_id: 'p1', target_project_id: 'p2',
      relationship_type: 'references', detection_source: 'claude_md', dismissed: false,
    }]
    render(OverviewTab, { props: defaultProps({ projects, relationships: rels }) })
    expect(screen.getByText('1 connection')).toBeTruthy()
  })

  // --- Session history (combined into sessions section) ---

  it('shows sessions section when sessionHistory has entries', () => {
    const history = [
      { id: 's1', date: new Date().toISOString(), summary: 'Some session' },
    ]
    render(OverviewTab, { props: defaultProps({ sessionHistory: history }) })
    expect(screen.getByTestId('overview-sessions')).toBeTruthy()
  })

  it('renders session history entries', () => {
    const history = [
      { id: 's1', date: new Date().toISOString(), summary: 'First session' },
      { id: 's2', date: new Date(Date.now() - 86400000).toISOString(), summary: 'Second session' },
    ]
    render(OverviewTab, { props: defaultProps({ sessionHistory: history }) })
    expect(screen.getByText('First session')).toBeTruthy()
    expect(screen.getByText('Second session')).toBeTruthy()
  })

  // --- Project info ---

  it('shows project path', () => {
    render(OverviewTab, { props: defaultProps() })
    expect(screen.getByText('~/projects/taurhaus')).toBeTruthy()
  })

  it('shows created date', () => {
    render(OverviewTab, { props: defaultProps() })
    // createdAt: '2025-01-01T00:00:00Z'
    const dateStr = new Date('2025-01-01T00:00:00Z').toLocaleDateString()
    expect(screen.getByText(dateStr)).toBeTruthy()
  })

  it('does not render dead project info action buttons', () => {
    render(OverviewTab, { props: defaultProps() })
    expect(screen.queryByRole('button', { name: 'Edit' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Remove' })).toBeNull()
  })

  // --- Layout ordering ---

  it('quick actions render in header area', () => {
    render(OverviewTab, { props: defaultProps() })
    const quickActions = screen.getByTestId('quick-actions')
    expect(quickActions).toBeTruthy()
    // Should be within the header (not in scrollable content)
    expect(quickActions.closest('h1')?.parentElement || quickActions.closest('[class*="shrink-0"]')).toBeTruthy()
  })

  it('README renders before Recent Activity in DOM order', () => {
    const commits = [{ hash: 'abc1234', message: 'Commit', date: '1h' }]
    const { container } = render(OverviewTab, {
      props: defaultProps({
        recentCommits: commits,
        readmeContent: { content: '# Title\nSome readme content' },
      }),
    })
    // Both sections should exist
    const readme = container.querySelector('[data-testid="overview-readme"]')
    const commitRow = container.querySelector('[data-testid="overview-commit-row"]')
    expect(readme).toBeTruthy()
    expect(commitRow).toBeTruthy()
    // Check DOM order via innerHTML — README section appears first
    const html = container.innerHTML
    const readmePos = html.indexOf('overview-readme')
    const commitPos = html.indexOf('overview-commit-row')
    expect(readmePos).toBeLessThan(commitPos)
  })

  it('README section hidden when no content', () => {
    render(OverviewTab, { props: defaultProps({ readmeContent: null }) })
    expect(screen.queryByTestId('overview-readme')).toBeNull()
  })

  it('uses and updates project data from context when selectedProject/projects are omitted', async () => {
    const alpha = makeProject({ id: 'p1', name: 'alpha' })
    const beta = makeProject({ id: 'p2', name: 'beta' })
    const gamma = makeProject({ id: 'p3', name: 'gamma' })

    const { rerender } = render(OverviewTabContextHarness, {
      props: {
        contextSelectedProject: alpha,
        contextProjects: [alpha, beta],
        data: {
          relationships: [{
            id: 'rel-1',
            source_project_id: 'p1',
            target_project_id: 'p2',
            relationship_type: 'references',
            detection_source: 'manual',
            dismissed: false,
          }],
        },
      },
    })

    expect(screen.getByText('alpha')).toBeTruthy()
    expect(screen.getByText('beta')).toBeTruthy()

    await rerender({
      contextSelectedProject: beta,
      contextProjects: [beta, gamma],
      data: {
        relationships: [{
          id: 'rel-2',
          source_project_id: 'p2',
          target_project_id: 'p3',
          relationship_type: 'references',
          detection_source: 'manual',
          dismissed: false,
        }],
      },
    })

    await waitFor(() => {
      expect(screen.queryByText('alpha')).toBeNull()
      expect(screen.getByText('gamma')).toBeTruthy()
    })
  })

  // --- Dark mode ---

  it('renders without errors in dark mode', () => {
    const { container } = render(OverviewTab, { props: defaultProps({ dark: true }) })
    expect(container.querySelector('h1')).toBeTruthy()
  })
})
