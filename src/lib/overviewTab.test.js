/**
 * OverviewTab component tests.
 *
 * Tests quick actions, last commit display, session rendering,
 * relationships, callbacks, and helper functions.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

// Mock markdown renderer — just renders the source as text
vi.mock('./MarkdownRenderer.svelte', () => {
  const { mount } = require('svelte')
  return {
    default: function MockMarkdown(target, props) {
      const el = document.createElement('div')
      el.setAttribute('data-testid', 'markdown-renderer')
      el.textContent = props?.source || ''
      target.appendChild(el)
      return { $set() {}, $destroy() { el.remove() } }
    },
  }
})

import OverviewTab from './OverviewTab.svelte'

/** Minimal project for rendering. */
function makeProject(overrides = {}) {
  return {
    id: 'p1',
    name: 'taurhaus',
    path: '~/projects/taurhaus',
    branch: 'main',
    is_dirty: false,
    activity_state: 'active',
    description: 'Desktop tool for AI project management',
    created_at: '2025-01-01T00:00:00Z',
    ...overrides,
  }
}

/** Default props for OverviewTab. */
function defaultProps(overrides = {}) {
  return {
    dark: false,
    codeTheme: 'github-light',
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
    onNavigateToCommit: vi.fn(),
    onViewAllCommits: vi.fn(),
    onDismissRelationship: vi.fn(),
    onSelectProject: vi.fn(),
    onMarkdownNavigate: vi.fn(),
    onLaunchSession: vi.fn(),
    onOpenTerminal: vi.fn(),
    ...overrides,
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

  it('shows dirty indicator when is_dirty is true', () => {
    render(OverviewTab, {
      props: defaultProps({ selectedProject: makeProject({ is_dirty: true }) }),
    })
    const dot = document.querySelector('[title="Uncommitted changes"]')
    expect(dot).toBeTruthy()
  })

  it('hides dirty indicator when is_dirty is false', () => {
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

  // --- Last commit ---

  it('shows loading state for commits', () => {
    render(OverviewTab, { props: defaultProps({ commitsLoading: true }) })
    // Should show pulse animation, not the commit row
    expect(screen.queryByTestId('overview-last-commit')).toBeNull()
  })

  it('shows last commit when available', () => {
    const commits = [
      { hash: 'abc1234', message: 'Fix bug in parser', date: '2h' },
    ]
    render(OverviewTab, { props: defaultProps({ recentCommits: commits }) })
    expect(screen.getByTestId('overview-last-commit')).toBeTruthy()
    expect(screen.getByTestId('overview-last-commit').textContent).toContain('abc1234')
    expect(screen.getByTestId('overview-last-commit').textContent).toContain('Fix bug in parser')
  })

  it('clicking last commit calls onNavigateToCommit', async () => {
    const onNavigateToCommit = vi.fn()
    const commits = [{ hash: 'abc1234', message: 'Fix bug', date: '2h' }]
    render(OverviewTab, { props: defaultProps({ recentCommits: commits, onNavigateToCommit }) })
    await fireEvent.click(screen.getByTestId('overview-last-commit'))
    expect(onNavigateToCommit).toHaveBeenCalledWith('abc1234')
  })

  it('shows "No commits found" when no commits available', () => {
    render(OverviewTab, { props: defaultProps({ recentCommits: [] }) })
    // Appears in both Last Commit and Recent Activity sections
    const matches = screen.getAllByText('No commits found.')
    expect(matches.length).toBeGreaterThanOrEqual(1)
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

  // --- Latest session ---

  it('hides session section when no session exists', () => {
    render(OverviewTab, { props: defaultProps({ latestSession: null, sessionLoading: false }) })
    expect(screen.queryByText('Latest session')).toBeNull()
  })

  it('shows session section when latestSession exists', () => {
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

  it('shows session loading skeleton', () => {
    render(OverviewTab, { props: defaultProps({ sessionLoading: true }) })
    expect(screen.getByText('Latest session')).toBeTruthy()
  })

  // --- Relationships ---

  it('shows "No connections detected" when empty', () => {
    render(OverviewTab, { props: defaultProps({ relationships: [] }) })
    expect(screen.getByText('No connections detected yet.')).toBeTruthy()
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

  // --- Session history ---

  it('shows "No sessions imported" when empty', () => {
    render(OverviewTab, { props: defaultProps({ sessionHistory: [] }) })
    expect(screen.getByText('No sessions imported yet.')).toBeTruthy()
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
    // created_at: '2025-01-01T00:00:00Z'
    const dateStr = new Date('2025-01-01T00:00:00Z').toLocaleDateString()
    expect(screen.getByText(dateStr)).toBeTruthy()
  })

  // --- Dark mode ---

  it('renders without errors in dark mode', () => {
    const { container } = render(OverviewTab, { props: defaultProps({ dark: true }) })
    expect(container.querySelector('h1')).toBeTruthy()
  })
})
