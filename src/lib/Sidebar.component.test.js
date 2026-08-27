import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor, fireEvent, within } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./ipc.js', () => ({
  refreshAccountsUsage: vi.fn(() => Promise.resolve(true)),
  navigateToSession: vi.fn(),
  launchCliSession: vi.fn(),
  stopClaudeSession: vi.fn(),
  removeProject: vi.fn(),
  // A Claude launch asks the account store first, which detects before it
  // decides whether the subscription chooser has to open.
  listAccounts: vi.fn(() =>
    Promise.resolve({ accounts: [], source: 'native', degraded: false, error: null })
  ),
  setProjectAccount: vi.fn(() => Promise.resolve()),
  resolveLaunchAccount: vi.fn(() => Promise.resolve({ needsChoice: true })),
  getSettings: vi.fn(() => Promise.resolve({ terminal: {} })),
}))

vi.mock('./sessionStore.svelte.js', () => ({
  getSessionForProject: vi.fn(() => null),
  getSessionsForProject: vi.fn(() => []),
}))

vi.mock('./sessionIndicator.js', () => ({
  hasLiveSession: vi.fn((session) => session?.state === 'active' || session?.state === 'idle'),
  rowTintForSessions: vi.fn(() => ''),
  toolIndicators: vi.fn(() => []),
}))

const { navigateToSession, launchCliSession, stopClaudeSession, removeProject, listAccounts } = await import('./ipc.js')
const { accountState, resetAccountsForTest } = await import('./accounts.svelte.js')
const claudeAccounts = accountState('claude')
const { getSessionForProject, getSessionsForProject } = await import('./sessionStore.svelte.js')
const { toolIndicators } = await import('./sessionIndicator.js')
import Sidebar from './Sidebar.svelte'

function makeProjects(count) {
  const activityStates = ['active', 'recent', 'stale', 'dormant']
  return Array.from({ length: count }, (_, index) => ({
    id: `project-${index}`,
    name: `Project ${index}`,
    path: `/projects/project-${index}`,
    activityState: activityStates[index % activityStates.length],
    branch: index % 2 === 0 ? 'main' : null,
    isDirty: index % 3 === 0,
  }))
}

describe('Sidebar component branches', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  beforeEach(() => {
    vi.clearAllMocks()
    // The account store is module state shared by the whole app, detection
    // included: without this a test inherits the previous one's answer.
    resetAccountsForTest()
    listAccounts.mockResolvedValue({
      accounts: [],
      source: 'native',
      degraded: false,
      error: null,
    })
    removeProject.mockResolvedValue(undefined)
    launchCliSession.mockResolvedValue({ ok: true })
    stopClaudeSession.mockResolvedValue(undefined)
    navigateToSession.mockResolvedValue(undefined)

    if (!navigator.clipboard) {
      Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } })
    } else {
      navigator.clipboard.writeText = vi.fn().mockResolvedValue(undefined)
    }
  })

  it('renders loading, error, empty, and no-match states', async () => {
    const onRetry = vi.fn()
    const onAddProject = vi.fn()
    const { rerender } = render(Sidebar, {
      props: {
        projects: [],
        sidebarLoading: true,
        actions: { onRetry, onAddProject },
      },
    })

    expect(screen.getByTestId('sidebar-skeleton')).toBeInTheDocument()

    await rerender({ projects: [], sidebarLoading: false, sidebarError: 'boom', actions: { onRetry, onAddProject } })
    expect(screen.getByTestId('sidebar-error')).toBeInTheDocument()
    await fireEvent.click(screen.getByText('Retry'))
    expect(onRetry).toHaveBeenCalled()

    await rerender({ projects: [], sidebarLoading: false, sidebarError: null, actions: { onRetry, onAddProject } })
    expect(screen.getByTestId('sidebar-empty')).toBeInTheDocument()
    await fireEvent.click(screen.getByTestId('sidebar-empty-scan'))
    expect(onAddProject).toHaveBeenCalledTimes(1)

    await rerender({ projects: makeProjects(2), sidebarLoading: false, sidebarError: null, actions: { onRetry, onAddProject } })
    const input = screen.getByTestId('sidebar-filter')
    await fireEvent.input(input, { target: { value: 'does-not-exist' } })
    expect(screen.getByTestId('sidebar-no-matches')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('sidebar-filter-clear'))
    expect(screen.queryByTestId('sidebar-no-matches')).not.toBeInTheDocument()
  })

  it('fires selection and footer actions', async () => {
    const onSelectProject = vi.fn()
    const onAddProject = vi.fn()
    const onToggleSettings = vi.fn()
    const projects = makeProjects(3)

    render(Sidebar, {
      props: {
        projects,
        actions: {
          onSelectProject,
          onAddProject,
          onToggleSettings,
        },
      },
    })

    await waitFor(() => {
      expect(screen.getAllByTestId('project-item').length).toBeGreaterThan(0)
    })

    await fireEvent.click(screen.getAllByTestId('project-item')[0])
    expect(onSelectProject).toHaveBeenCalledWith(expect.objectContaining({ id: projects[0].id }))

    await fireEvent.click(screen.getByTestId('manage-projects-btn'))
    expect(onAddProject).toHaveBeenCalled()

    await fireEvent.click(screen.getByTestId('settings-toggle'))
    expect(onToggleSettings).toHaveBeenCalled()
  })

  it('keeps visible focus styling on the filter and project rows', async () => {
    const projects = makeProjects(2)

    render(Sidebar, {
      props: {
        projects,
      },
    })

    await waitFor(() => {
      expect(screen.getAllByTestId('project-item').length).toBe(2)
    })

    expect(screen.getByTestId('sidebar-filter').className).toContain('focus-visible:ring-1')
    expect(screen.getByTestId('sidebar-filter').className).toContain('focus-visible:ring-brand-500/70')
    expect(screen.getAllByTestId('project-item')[0].className).toContain('focus-visible:ring-1')
    expect(screen.getAllByTestId('project-item')[0].className).toContain('focus-visible:ring-brand-500')
  })

  it('renders a right-side foreground indicator when the project matches the foreground project id', async () => {
    const projects = makeProjects(2)

    render(Sidebar, {
      props: {
        projects,
        foregroundProjectId: projects[1].id,
      },
    })

    await waitFor(() => {
      expect(screen.getAllByTestId('project-item').length).toBe(2)
    })

    const row = document.querySelector(`[data-project-id="${projects[1].id}"]`)
    expect(row).toBeTruthy()
    expect(within(row).getByTestId('sidebar-foreground-indicator')).toBeInTheDocument()
  })

  it('does not render a right-side foreground indicator for non-matching projects', async () => {
    const projects = makeProjects(2)

    render(Sidebar, {
      props: {
        projects,
        foregroundProjectId: projects[1].id,
      },
    })

    await waitFor(() => {
      expect(screen.getAllByTestId('project-item').length).toBe(2)
    })

    const row = document.querySelector(`[data-project-id="${projects[0].id}"]`)
    expect(row).toBeTruthy()
    expect(within(row).queryByTestId('sidebar-foreground-indicator')).not.toBeInTheDocument()
  })

  it('shows both left selection and right foreground indicators on the same row when both states are active', async () => {
    const projects = makeProjects(2)

    render(Sidebar, {
      props: {
        projects,
        selectedProject: projects[0],
        foregroundProjectId: projects[0].id,
      },
    })

    await waitFor(() => {
      expect(screen.getAllByTestId('project-item').length).toBe(2)
    })

    const row = document.querySelector(`[data-project-id="${projects[0].id}"]`)
    expect(row).toBeTruthy()
    expect(within(row).getByTestId('sidebar-selection-indicator')).toBeInTheDocument()
    expect(within(row).getByTestId('sidebar-foreground-indicator')).toBeInTheDocument()
  })

  it('renders no right-side foreground indicator when foreground project id is null', async () => {
    const projects = makeProjects(2)

    render(Sidebar, {
      props: {
        projects,
        foregroundProjectId: null,
      },
    })

    await waitFor(() => {
      expect(screen.getAllByTestId('project-item').length).toBe(2)
    })

    expect(screen.queryByTestId('sidebar-foreground-indicator')).not.toBeInTheDocument()
  })

  it('renders projects with canonical camelCase activity fields', async () => {
    const projects = [
      {
        id: 'proj-recent',
        name: 'Recent Project',
        path: '/projects/recent',
        activityState: 'recent',
        branch: 'main',
        isDirty: true,
      },
    ]

    render(Sidebar, { props: { projects } })

    await waitFor(() => {
      expect(screen.getByText('RECENT')).toBeInTheDocument()
      expect(screen.getByText('Recent Project')).toBeInTheDocument()
    })
  })

  it('renders non-default branches on a second line and hides default branches', async () => {
    const projects = [
      {
        id: 'proj-main',
        name: 'Mainline Project',
        path: '/projects/mainline',
        activityState: 'active',
        branch: 'main',
        isDirty: false,
      },
      {
        id: 'proj-feature',
        name: 'Feature Project',
        path: '/projects/feature',
        activityState: 'active',
        branch: 'feature/clear-overhaul',
        isDirty: false,
      },
    ]

    render(Sidebar, { props: { projects } })

    await waitFor(() => {
      expect(screen.getAllByTestId('project-item').length).toBe(2)
    })

    const featureRow = document.querySelector('[data-project-id="proj-feature"]')
    const mainRow = document.querySelector('[data-project-id="proj-main"]')
    expect(featureRow).toBeTruthy()
    expect(mainRow).toBeTruthy()

    expect(within(featureRow).getByTestId('sidebar-branch-line')).toHaveTextContent('feature/clear-overhaul')
    expect(within(mainRow).queryByTestId('sidebar-branch-line')).not.toBeInTheDocument()
    expect(featureRow.className).toContain('h-[50px]')
    expect(mainRow.className).toContain('h-[36px]')
  })

  it('renders daemon status variants and hides not_configured', async () => {
    const { rerender } = render(Sidebar, {
      props: {
        projects: makeProjects(1),
        daemonStatus: 'connected',
      },
    })
    expect(screen.getByTestId('daemon-status')).toHaveTextContent('Connected')

    await rerender({ projects: makeProjects(1), daemonStatus: 'busy' })
    expect(screen.getByTestId('daemon-status')).toHaveTextContent('Daemon busy')

    await rerender({ projects: makeProjects(1), daemonStatus: 'reconnecting' })
    expect(screen.getByTestId('daemon-status')).toHaveTextContent('Reconnecting')

    await rerender({ projects: makeProjects(1), daemonStatus: 'disconnected' })
    expect(screen.getByTestId('daemon-status')).toHaveTextContent('Daemon offline')

    await rerender({ projects: makeProjects(1), daemonStatus: 'failed' })
    expect(screen.getByTestId('daemon-status')).toHaveTextContent('Daemon failed')

    await rerender({ projects: makeProjects(1), daemonStatus: 'not_configured' })
    expect(screen.queryByTestId('daemon-status')).not.toBeInTheDocument()
  })

  it('navigates through interactive session indicator pills only when session has tmux fields', async () => {
    const projects = [makeProjects(1)[0]]
    const onForegroundProjectChange = vi.fn()
    const interactiveSession = {
      tmux_session: 'team',
      tmux_window: '1',
      tmux_pane: '%3',
      cli_tool: 'codex',
      state: 'active',
      project_path: projects[0].path,
    }

    getSessionsForProject.mockImplementation(() => [interactiveSession])
    toolIndicators.mockImplementation(() => ([
      {
        kind: 'session',
        interactive: true,
        colorClass: 'text-success-400',
        isActive: true,
        ariaLabel: 'Codex active',
        icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' },
        session: interactiveSession,
      },
      {
        kind: 'session',
        interactive: false,
        colorClass: 'text-zinc-400',
        isActive: false,
        ariaLabel: 'Claude idle',
        icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' },
        session: { state: 'idle' },
      },
    ]))

    render(Sidebar, { props: { projects, onForegroundProjectChange } })

    await waitFor(() => {
      expect(screen.getByLabelText('Codex active')).toBeInTheDocument()
      expect(screen.getByLabelText('Claude idle')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByLabelText('Codex active'))
    expect(onForegroundProjectChange).toHaveBeenCalledWith(projects[0].id)
    expect(navigateToSession).toHaveBeenCalledWith('team', '1', '%3')
  })

  it('shows a sidebar notice when session navigation fails', async () => {
    const projects = [makeProjects(1)[0]]
    navigateToSession.mockRejectedValueOnce(new Error('pane not found'))

    const interactiveSession = {
      tmux_session: 'team',
      tmux_window: '1',
      tmux_pane: '%3',
      cli_tool: 'codex',
      state: 'active',
      project_path: projects[0].path,
    }

    getSessionsForProject.mockImplementation(() => [interactiveSession])
    toolIndicators.mockImplementation(() => ([
      {
        kind: 'session',
        interactive: true,
        colorClass: 'text-success-400',
        isActive: true,
        ariaLabel: 'Codex active',
        icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' },
        session: interactiveSession,
      },
    ]))

    render(Sidebar, { props: { projects } })

    await fireEvent.click(await screen.findByLabelText('Codex active'))

    await waitFor(() => {
      expect(screen.getByTestId('sidebar-notice-message')).toHaveTextContent(
        'Could not open that terminal. The session may have already closed.'
      )
    })

    expect(screen.getByTestId('sidebar-notice')).toHaveAttribute('role', 'status')
    expect(screen.getByTestId('sidebar-notice')).toHaveAttribute('aria-live', 'polite')
  })

  it('ignores rapid repeated standalone session clicks while navigation is in flight', async () => {
    const projects = [makeProjects(1)[0]]
    const onForegroundProjectChange = vi.fn()
    let resolveNavigation
    navigateToSession.mockReturnValue(new Promise((resolve) => {
      resolveNavigation = resolve
    }))

    const interactiveSession = {
      tmux_session: 'team',
      tmux_window: '1',
      tmux_pane: '%3',
      cli_tool: 'codex',
      state: 'active',
      project_path: projects[0].path,
    }

    getSessionsForProject.mockImplementation(() => [interactiveSession])
    toolIndicators.mockImplementation(() => ([
      {
        kind: 'session',
        interactive: true,
        colorClass: 'text-success-400',
        isActive: true,
        ariaLabel: 'Codex active',
        icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' },
        session: interactiveSession,
      },
    ]))

    render(Sidebar, { props: { projects, onForegroundProjectChange } })

    const indicator = await screen.findByLabelText('Codex active')
    await fireEvent.click(indicator)
    await fireEvent.click(indicator)

    expect(navigateToSession).toHaveBeenCalledTimes(1)
    expect(onForegroundProjectChange).toHaveBeenCalledTimes(1)

    resolveNavigation(undefined)
    await waitFor(() => {
      expect(navigateToSession).toHaveBeenCalledTimes(1)
    })

    await fireEvent.click(indicator)
    expect(navigateToSession).toHaveBeenCalledTimes(2)
  })

  it('renders grouped team token before standalone session icons', async () => {
    const projects = [makeProjects(1)[0]]
    getSessionsForProject.mockImplementation(() => [
      { state: 'active', group_kind: 'mesh_team' },
      { state: 'idle', group_kind: 'mesh_team' },
      { state: 'idle', group_kind: 'standalone' },
      { state: 'active', group_kind: 'standalone' },
    ])
    toolIndicators.mockImplementation(() => ([
      {
        kind: 'team',
        layout: 'rail',
        count: 2,
        tone: 'active',
        isActive: true,
        memberTools: [
          { tool: 'claude', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'default', isActive: true, colorClass: 'text-success-300' },
          { tool: 'codex', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'default', isActive: false, colorClass: 'text-warning-300' },
        ],
        ariaLabel: 'team-a: 2 team sessions active',
      },
      {
        kind: 'session',
        interactive: false,
        colorClass: 'text-warning-300',
        isActive: false,
        ariaLabel: 'Gemini idle',
        icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' },
        session: { state: 'idle' },
      },
    ]))

    render(Sidebar, { props: { projects } })

    await waitFor(() => {
      expect(screen.getByTestId('sidebar-team-indicator')).toBeInTheDocument()
      expect(screen.getByLabelText('Gemini idle')).toBeInTheDocument()
    })

    expect(screen.getByTestId('sidebar-team-indicator').className).toContain('sidebar-session-team-rail')
    expect(document.querySelectorAll('.sidebar-session-team-rail .session-pill-active')).toHaveLength(1)
    expect(document.querySelectorAll('.sidebar-session-team-rail .session-pill-idle')).toHaveLength(1)
  })

  it('navigates grouped rail indicators to the lead tmux pane even when the lead has a custom name', async () => {
    const projects = [makeProjects(1)[0]]
    toolIndicators.mockImplementation(() => ([
      {
        kind: 'team',
        layout: 'rail',
        groupId: 'team-a',
        count: 2,
        tone: 'active',
        memberTools: [
          { tool: 'claude', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'default', isActive: true, colorClass: 'text-success-300' },
          { tool: 'codex', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'default', isActive: false, colorClass: 'text-warning-300' },
        ],
        members: [
          {
            member_name: 'orchestrator',
            role: 'lead',
            tmux_session: 'mesh',
            tmux_window: '4',
            tmux_pane: '%12',
          },
          {
            member_name: 'developer2',
            role: 'member',
            tmux_session: 'mesh',
            tmux_window: '4',
            tmux_pane: '%13',
          },
        ],
        ariaLabel: 'team-a: 2 team sessions active',
      },
    ]))

    render(Sidebar, { props: { projects } })

    const indicator = await screen.findByTestId('sidebar-team-indicator')
    await fireEvent.click(indicator)

    expect(navigateToSession).toHaveBeenCalledWith('mesh', '4', '%12')
  })

  it('ignores rapid repeated grouped session clicks while navigation is in flight', async () => {
    const projects = [makeProjects(1)[0]]
    let resolveNavigation
    navigateToSession.mockReturnValue(new Promise((resolve) => {
      resolveNavigation = resolve
    }))

    toolIndicators.mockImplementation(() => ([
      {
        kind: 'team',
        layout: 'rail',
        groupId: 'team-a',
        count: 2,
        tone: 'active',
        memberTools: [
          { tool: 'claude', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'default', isActive: true, colorClass: 'text-success-300' },
          { tool: 'codex', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'default', isActive: false, colorClass: 'text-warning-300' },
        ],
        members: [
          {
            member_name: 'orchestrator',
            role: 'lead',
            tmux_session: 'mesh',
            tmux_window: '4',
            tmux_pane: '%12',
          },
          {
            member_name: 'developer2',
            role: 'member',
            tmux_session: 'mesh',
            tmux_window: '4',
            tmux_pane: '%13',
          },
        ],
        ariaLabel: 'team-a: 2 team sessions active',
      },
    ]))

    render(Sidebar, { props: { projects } })

    const indicator = await screen.findByTestId('sidebar-team-indicator')
    await fireEvent.click(indicator)
    await fireEvent.click(indicator)

    expect(navigateToSession).toHaveBeenCalledTimes(1)

    resolveNavigation(undefined)
    await waitFor(() => {
      expect(navigateToSession).toHaveBeenCalledTimes(1)
    })

    await fireEvent.click(indicator)
    expect(navigateToSession).toHaveBeenCalledTimes(2)
  })

  it('navigates grouped stack indicators to the lead tmux pane on keyboard activation', async () => {
    const projects = [makeProjects(1)[0]]
    toolIndicators.mockImplementation(() => ([
      {
        kind: 'team',
        layout: 'stack',
        groupId: 'team-b',
        count: 4,
        tone: 'idle',
        tools: [
          { tool: 'claude', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, isActive: false, colorClass: 'text-warning-300' },
          { tool: 'codex', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, isActive: false, colorClass: 'text-warning-300' },
        ],
        members: [
          {
            member_name: 'architect',
            role: 'lead',
            tmux_session: 'mesh',
            tmux_window: '7',
            tmux_pane: '%21',
          },
          {
            member_name: 'developer2',
            role: 'member',
            tmux_session: 'mesh',
            tmux_window: '7',
            tmux_pane: '%22',
          },
        ],
        ariaLabel: 'team-b: 4 team sessions idle',
      },
    ]))

    render(Sidebar, { props: { projects } })

    const indicator = await screen.findByTestId('sidebar-team-indicator')
    expect(document.querySelectorAll('.sidebar-session-team-stack .session-pill-idle')).toHaveLength(2)
    expect(document.querySelector('.sidebar-session-team-count')).toHaveTextContent('4')
    await fireEvent.keyDown(indicator, { key: 'Enter' })

    expect(navigateToSession).toHaveBeenCalledWith('mesh', '7', '%21')
  })

  it('falls back to the first grouped member pane when no lead member is present', async () => {
    const projects = [makeProjects(1)[0]]
    toolIndicators.mockImplementation(() => ([
      {
        kind: 'team',
        layout: 'rail',
        groupId: 'team-c',
        count: 2,
        tone: 'active',
        memberTools: [
          { tool: 'codex', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'default', isActive: true, colorClass: 'text-success-300' },
          { tool: 'gemini', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'default', isActive: false, colorClass: 'text-warning-300' },
        ],
        members: [
          {
            member_name: 'developer2',
            role: 'member',
            tmux_session: 'mesh',
            tmux_window: '9',
            tmux_pane: '%31',
          },
          {
            member_name: 'developer3',
            role: 'member',
            tmux_session: 'mesh',
            tmux_window: '9',
            tmux_pane: '%32',
          },
        ],
        ariaLabel: 'team-c: 2 team sessions active',
      },
    ]))

    render(Sidebar, { props: { projects } })

    const indicator = await screen.findByTestId('sidebar-team-indicator')
    await fireEvent.click(indicator)

    expect(navigateToSession).toHaveBeenCalledWith('mesh', '9', '%31')
  })

  it('context menu supports copy path and two-click remove confirmation', async () => {
    const onProjectRemoved = vi.fn()
    const projects = [makeProjects(1)[0]]

    render(Sidebar, {
      props: {
        projects,
        actions: {
          onProjectRemoved,
        },
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('project-item')).toBeInTheDocument()
    })

    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await fireEvent.mouseDown(screen.getByText('Copy Path'))
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(projects[0].path)

    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await fireEvent.mouseDown(screen.getByText('Remove from taurhaus'))
    expect(screen.getByText('Confirm remove?')).toBeInTheDocument()

    await fireEvent.mouseDown(screen.getByText('Confirm remove?'))
    await waitFor(() => {
      expect(removeProject).toHaveBeenCalledWith(projects[0].id)
      expect(onProjectRemoved).toHaveBeenCalledWith(projects[0].id)
    })
  })

  it('opens the project context menu with Shift+F10', async () => {
    const projects = [makeProjects(1)[0]]

    render(Sidebar, {
      props: {
        projects,
      },
    })

    const projectItem = await screen.findByTestId('project-item')
    projectItem.focus()

    await fireEvent.keyDown(projectItem, { key: 'F10', shiftKey: true })

    expect(screen.getByTestId('context-menu')).toBeInTheDocument()
    expect(screen.getByText('Copy Path')).toBeInTheDocument()
  })

  it('context menu hides terminal navigation until a tmux target exists and still supports restart/stop', async () => {
    const project = makeProjects(1)[0]
    const session = {
      state: 'active',
      cli_tool: 'codex',
      tmux_pane: '%9',
      tmux_session: 'team',
      tmux_window: '2',
    }

    getSessionsForProject.mockImplementation(() => [session])
    getSessionForProject.mockImplementation(() => null)

    render(Sidebar, {
      props: {
        projects: [project],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('project-item')).toBeInTheDocument()
    })

    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    expect(screen.queryByText('Open in Terminal')).not.toBeInTheDocument()
    expect(screen.queryByText('Continue Codex')).not.toBeInTheDocument()
    expect(screen.queryByText('Continue Gemini')).not.toBeInTheDocument()
    expect(screen.getByText('New Codex Session')).toBeInTheDocument()
    expect(screen.getByText('Resume Gemini')).toBeInTheDocument()

    getSessionForProject.mockImplementation(() => session)
    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await fireEvent.mouseDown(screen.getByText('Open in Terminal'))
    await waitFor(() => {
      expect(navigateToSession).toHaveBeenCalledWith('team', '2', '%9', true)
    })

    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await fireEvent.mouseDown(screen.getByText('Restart Codex'))
    await waitFor(() => {
      expect(stopClaudeSession).toHaveBeenCalledWith('%9', 'codex')
      expect(launchCliSession).toHaveBeenCalledWith(project.id, 'fresh', 'codex', null)
    })

    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await fireEvent.mouseDown(screen.getByText('Stop Codex'))
    expect(screen.getByText('Confirm stop Codex?')).toBeInTheDocument()

    await fireEvent.mouseDown(screen.getByText('Confirm stop Codex?'))
    await waitFor(() => {
      expect(stopClaudeSession).toHaveBeenCalledWith('%9', 'codex')
    })
  })

  // Regression: c982822 routed the sidebar's ordinary launches through
  // requestClaudeLaunch but left Restart calling launchCliSession directly.
  // On a host with two signed-in subscriptions and a project pinned to neither,
  // that path can never open the chooser: it stopped the pane and took the
  // backend fallback, so a one-off session could come back on the other
  // subscription — and cancelling was not an option, the session was gone.
  it('asks which subscription a restart runs on before stopping the session', async () => {
    const project = makeProjects(1)[0]
    const session = {
      state: 'active',
      cli_tool: 'claude',
      tmux_pane: '%9',
      tmux_session: 'team',
      tmux_window: '2',
    }
    listAccounts.mockResolvedValue({
      accounts: [
        { id: 'account-1', email: 'a@example.com', logged_in: true, is_default: true },
        { id: 'account-2', email: 'b@example.com', logged_in: true, is_default: false },
      ],
      source: 'native',
      degraded: false,
      error: null,
    })
    getSessionsForProject.mockImplementation(() => [session])

    render(Sidebar, { props: { projects: [project] } })

    await waitFor(() => {
      expect(screen.getByTestId('project-item')).toBeInTheDocument()
    })

    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await fireEvent.mouseDown(screen.getByText('Restart Claude'))

    await waitFor(() => {
      expect(claudeAccounts.pending).toMatchObject({ projectId: project.id, mode: 'fresh' })
    })
    // The pane is still alive: cancelling here must cost the user nothing.
    expect(stopClaudeSession).not.toHaveBeenCalled()

    await claudeAccounts.pending.confirm('account-2', true)

    expect(stopClaudeSession).toHaveBeenCalledWith('%9', 'claude')
    expect(launchCliSession).toHaveBeenCalledWith(project.id, 'fresh', 'claude', 'account-2')
  })

  it('surfaces launch and stop failures from the session context menu', async () => {
    const project = makeProjects(1)[0]
    const session = {
      state: 'active',
      cli_tool: 'codex',
      tmux_pane: '%9',
      tmux_session: 'team',
      tmux_window: '2',
    }

    getSessionsForProject.mockImplementation(() => [session])
    launchCliSession.mockRejectedValueOnce(new Error('boom'))
    stopClaudeSession.mockRejectedValueOnce(new Error('boom'))

    render(Sidebar, {
      props: {
        projects: [project],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('project-item')).toBeInTheDocument()
    })

    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await fireEvent.mouseDown(screen.getByText('Continue Claude'))

    await waitFor(() => {
      expect(screen.getByTestId('sidebar-notice-message')).toHaveTextContent(
        'Could not start Claude. Please try again.'
      )
    })

    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await fireEvent.mouseDown(screen.getByText('Stop Codex'))
    await fireEvent.mouseDown(screen.getByText('Confirm stop Codex?'))

    await waitFor(() => {
      expect(screen.getByTestId('sidebar-notice-message')).toHaveTextContent(
        'Could not stop Codex. Please try again.'
      )
    })
  })

  it('virtualizes large project lists and clears pending timers on unmount', async () => {
    vi.useFakeTimers()
    const clearTimeoutSpy = vi.spyOn(globalThis, 'clearTimeout')

    const { unmount } = render(Sidebar, {
      props: {
        projects: makeProjects(220),
      },
    })

    await waitFor(() => {
      expect(screen.getAllByTestId('project-item').length).toBeGreaterThan(0)
    })

    expect(screen.getAllByTestId('project-item').length).toBeLessThan(150)

    await fireEvent.mouseEnter(screen.getAllByTestId('project-item')[0])

    unmount()
    expect(clearTimeoutSpy).toHaveBeenCalled()
  })
})
