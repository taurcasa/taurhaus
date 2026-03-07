import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./ipc.js', () => ({
  navigateToSession: vi.fn(),
  launchClaudeSession: vi.fn(),
  stopClaudeSession: vi.fn(),
  removeProject: vi.fn(),
}))

vi.mock('./sessionStore.svelte.js', () => ({
  getSessionForProject: vi.fn(() => null),
  getSessionsForProject: vi.fn(() => []),
}))

vi.mock('./sessionIndicator.js', () => ({
  rowTintForSessions: vi.fn(() => ''),
  toolIndicators: vi.fn(() => []),
}))

const { navigateToSession, launchClaudeSession, stopClaudeSession, removeProject } = await import('./ipc.js')
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
    removeProject.mockResolvedValue(undefined)
    launchClaudeSession.mockResolvedValue({ ok: true })
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

  it('renders daemon status variants and hides not_configured', async () => {
    const { rerender } = render(Sidebar, {
      props: {
        projects: makeProjects(1),
        daemonStatus: 'connected',
      },
    })
    expect(screen.getByTestId('daemon-status')).toHaveTextContent('Connected')

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
    const interactiveSession = {
      tmux_session: 'team',
      tmux_window: '1',
      tmux_pane: '%3',
      cli_tool: 'codex',
      state: 'active',
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

    render(Sidebar, { props: { projects } })

    await waitFor(() => {
      expect(screen.getByLabelText('Codex active')).toBeInTheDocument()
      expect(screen.getByLabelText('Claude idle')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByLabelText('Codex active'))
    expect(navigateToSession).toHaveBeenCalledWith('team', '1', '%3')
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
          { tool: 'claude', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'sidebarSmall' },
          { tool: 'codex', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'sidebarSmall' },
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
    expect(screen.getByTestId('sidebar-team-indicator').querySelectorAll('.sidebar-session-team-rail-logo')).toHaveLength(2)
  })

  it('navigates grouped rail indicators to the lead tmux pane', async () => {
    const projects = [makeProjects(1)[0]]
    toolIndicators.mockImplementation(() => ([
      {
        kind: 'team',
        layout: 'rail',
        groupId: 'team-a',
        count: 2,
        tone: 'active',
        memberTools: [
          { tool: 'claude', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'sidebarSmall' },
          { tool: 'codex', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'sidebarSmall' },
        ],
        members: [
          {
            member_name: 'team-lead',
            tmux_session: 'mesh',
            tmux_window: '4',
            tmux_pane: '%12',
          },
          {
            member_name: 'developer2',
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
          { tool: 'claude', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' } },
          { tool: 'codex', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' } },
        ],
        members: [
          {
            member_name: 'lead-architecture',
            tmux_session: 'mesh',
            tmux_window: '7',
            tmux_pane: '%21',
          },
          {
            member_name: 'developer2',
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
          { tool: 'codex', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'sidebarSmall' },
          { tool: 'gemini', icon: { viewBox: '0 0 10 10', path: 'M0 0h10v10z' }, iconVariant: 'sidebarSmall' },
        ],
        members: [
          {
            member_name: 'developer2',
            tmux_session: 'mesh',
            tmux_window: '9',
            tmux_pane: '%31',
          },
          {
            member_name: 'developer3',
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

  it('context menu supports open/restart/stop session flows and stop confirmation', async () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})
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
    await fireEvent.mouseDown(screen.getByText('Open in Terminal'))
    expect(navigateToSession).not.toHaveBeenCalled()
    expect(warnSpy).toHaveBeenCalled()

    getSessionForProject.mockImplementation(() => session)
    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await fireEvent.mouseDown(screen.getByText('Restart Codex'))
    await waitFor(() => {
      expect(stopClaudeSession).toHaveBeenCalledWith('%9', 'codex')
      expect(launchClaudeSession).toHaveBeenCalledWith(project.id, 'continue', 'codex')
    })

    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await fireEvent.mouseDown(screen.getByText('Stop Codex'))
    expect(screen.getByText('Confirm stop Codex?')).toBeInTheDocument()

    await fireEvent.mouseDown(screen.getByText('Confirm stop Codex?'))
    await waitFor(() => {
      expect(stopClaudeSession).toHaveBeenCalledWith('%9', 'codex')
    })

    warnSpy.mockRestore()
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
