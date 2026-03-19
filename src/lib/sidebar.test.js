import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import {
  __resetSidebarProjectionCacheForTests,
  buildSidebarProjection,
} from './sidebar.js'

// Mock the IPC module
vi.mock('./ipc.js', () => ({
  listProjects: vi.fn(),
  getProject: vi.fn(),
  getFileTree: vi.fn(),
  readFile: vi.fn(),
  getRecentCommits: vi.fn(),
  getAllCommits: vi.fn(),
  getReadme: vi.fn(),
  getLatestSession: vi.fn(),
  listSessions: vi.fn(),
  getSession: vi.fn(),
  isTauri: vi.fn(() => false),
  search: vi.fn(),
  getIndexStatus: vi.fn(),
  rebuildIndex: vi.fn(),
}))

describe('Sidebar data loading', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  it('listProjects returns mock data in array form', async () => {
    const mockData = [
      { id: 'p1', name: 'test-project', activityState: 'active', branch: 'main', isDirty: false },
    ]
    ipc.listProjects.mockResolvedValue(mockData)

    const result = await ipc.listProjects()

    expect(result).toEqual(mockData)
    expect(result[0].activityState).toBe('active')
  })

  it('listProjects groups by activityState', async () => {
    const mockData = [
      { id: 'p1', name: 'a', activityState: 'active', branch: 'main', isDirty: false },
      { id: 'p2', name: 'b', activityState: 'recent', branch: 'main', isDirty: false },
      { id: 'p3', name: 'c', activityState: 'dormant', branch: 'main', isDirty: false },
    ]
    ipc.listProjects.mockResolvedValue(mockData)

    const result = await ipc.listProjects()
    const groups = {
      active: result.filter(p => p.activityState === 'active'),
      recent: result.filter(p => p.activityState === 'recent'),
      stale: result.filter(p => p.activityState === 'stale'),
      dormant: result.filter(p => p.activityState === 'dormant'),
    }

    expect(groups.active).toHaveLength(1)
    expect(groups.recent).toHaveLength(1)
    expect(groups.stale).toHaveLength(0)
    expect(groups.dormant).toHaveLength(1)
  })

  it('listProjects handles error gracefully', async () => {
    ipc.listProjects.mockRejectedValue(new Error('Connection failed'))

    await expect(ipc.listProjects()).rejects.toThrow('Connection failed')
  })

  it('getProject returns full detail shape', async () => {
    const mockDetail = {
      id: 'p1',
      name: 'taurhaus',
      path: '~/projects/taurhaus',
      description: 'Desktop tool for AI project management',
      activityState: 'active',
      lastActivityAt: '2025-01-01T00:00:00Z',
      heroPreference: null,
      createdAt: '2025-01-01T00:00:00Z',
      updatedAt: '2025-01-01T00:00:00Z',
      branch: 'main',
      isDirty: false,
    }
    ipc.getProject.mockResolvedValue(mockDetail)

    const result = await ipc.getProject('p1')

    expect(result.description).toBe('Desktop tool for AI project management')
    expect(result.activityState).toBe('active')
    expect(result.branch).toBe('main')
  })

  it('getFileTree returns nested structure', async () => {
    const mockTree = [
      { name: 'src', path: 'src', is_dir: true, children: [
        { name: 'main.rs', path: 'src/main.rs', is_dir: false, children: [] },
      ]},
      { name: 'README.md', path: 'README.md', is_dir: false, children: [] },
    ]
    ipc.getFileTree.mockResolvedValue(mockTree)

    const result = await ipc.getFileTree('p1')

    expect(result).toHaveLength(2)
    expect(result[0].is_dir).toBe(true)
    expect(result[0].children).toHaveLength(1)
    expect(result[1].name).toBe('README.md')
  })

  it('readFile returns content with language', async () => {
    const mockContent = {
      path: 'src/main.rs',
      content: 'fn main() {}',
      language: 'rust',
    }
    ipc.readFile.mockResolvedValue(mockContent)

    const result = await ipc.readFile('p1', 'src/main.rs')

    expect(result.content).toBe('fn main() {}')
    expect(result.language).toBe('rust')
  })

  it('tab state defaults and can distinguish between overview and files', () => {
    // Tab state is just a string — verify the expected values
    const validTabs = ['overview', 'files']
    const defaultTab = 'overview'

    expect(validTabs).toContain(defaultTab)
    expect(validTabs).toHaveLength(2)
  })
})

describe('sidebar projection memoization', () => {
  beforeEach(() => {
    __resetSidebarProjectionCacheForTests()
  })

  it('returns same grouped projection reference for same input', () => {
    const projects = [
      { id: 'p1', name: 'alpha', activityState: 'active' },
      { id: 'p2', name: 'beta', activityState: 'recent' },
    ]

    const first = buildSidebarProjection(projects, '')
    const second = buildSidebarProjection(projects, '')

    expect(second).toBe(first)
    expect(second.grouped[0].items).toHaveLength(1)
    expect(second.grouped[1].items).toHaveLength(1)
  })

  it('groups canonical camelCase project shapes into sidebar buckets', () => {
    const projects = [
      { id: 'p1', name: 'alpha', activityState: 'active' },
      { id: 'p2', name: 'beta', activityState: 'recent' },
      { id: 'p3', name: 'gamma', activityState: 'stale' },
      { id: 'p4', name: 'delta', activityState: 'dormant' },
    ]

    const projection = buildSidebarProjection(projects, '')
    expect(projection.grouped.find((group) => group.key === 'active')?.items.map((project) => project.id)).toEqual(['p1'])
    expect(projection.grouped.find((group) => group.key === 'recent')?.items.map((project) => project.id)).toEqual(['p2'])
    expect(projection.grouped.find((group) => group.key === 'stale')?.items.map((project) => project.id)).toEqual(['p3'])
    expect(projection.grouped.find((group) => group.key === 'dormant')?.items.map((project) => project.id)).toEqual(['p4'])
  })
})

describe('Context menu session actions', () => {
  /**
   * Mirrors the session menu item generation logic from Shell.svelte.
   * Returns the session-specific items that go between "Copy path" and "Remove".
   */
  function sessionMenuItems(session, confirmStop = false) {
    const items = []
    const hasNavigableTmuxTarget = Boolean(
      session?.tmux_session && session?.tmux_window && session?.tmux_pane
    )
    const isLiveSession = session?.state === 'active' || session?.state === 'idle'

    if (hasNavigableTmuxTarget) {
      items.push({ label: 'Open in Terminal', action: 'openInTerminal' })
      items.push({ separator: true })
    }

    items.push({ label: 'Continue Claude', action: 'continue-claude' })
    items.push({ separator: true })
    items.push({ label: 'New Claude Session', action: 'fresh-claude' })
    items.push({ label: 'New Codex Session', action: 'fresh-codex' })
    items.push({ label: 'New Gemini Session', action: 'fresh-gemini' })
    items.push({ separator: true })
    items.push({ label: 'Resume Claude', action: 'resume-claude' })
    items.push({ label: 'Resume Codex', action: 'resume-codex' })
    items.push({ label: 'Resume Gemini', action: 'resume-gemini' })

    if (isLiveSession) {
      items.push({ separator: true })
      items.push({ label: 'Restart Session', action: 'restart' })
      items.push({
        label: confirmStop ? 'Confirm stop?' : 'Stop Session',
        action: 'stop',
        danger: true,
        keepOpen: !confirmStop,
      })
    }

    return items
  }

  it('shows only distinct launch actions when no session exists', () => {
    const actionable = sessionMenuItems(null).filter(i => !i.separator)
    expect(actionable.map((item) => item.label)).toEqual([
      'Continue Claude',
      'New Claude Session',
      'New Codex Session',
      'New Gemini Session',
      'Resume Claude',
      'Resume Codex',
      'Resume Gemini',
    ])
  })

  it('hides Open in Terminal when a live session has no tmux target', () => {
    const actionable = sessionMenuItems({ state: 'active', cli_tool: 'codex' }).filter(i => !i.separator)
    expect(actionable.map((item) => item.label)).not.toContain('Open in Terminal')
  })

  it('shows Open in Terminal first when a live session has tmux coordinates', () => {
    const actionable = sessionMenuItems({
      state: 'active',
      tmux_session: 'team',
      tmux_window: '2',
      tmux_pane: '%9',
    }).filter(i => !i.separator)
    expect(actionable[0].label).toBe('Open in Terminal')
  })

  it('includes Restart and Stop in destructive group for active session', () => {
    const actionable = sessionMenuItems({ state: 'active' }).filter(i => !i.separator)
    expect(actionable.at(-2)?.label).toBe('Restart Session')
    expect(actionable.at(-1)?.label).toBe('Stop Session')
    expect(actionable.at(-1)?.danger).toBe(true)
  })

  it('Stop Session uses two-click confirmation pattern', () => {
    const items1 = sessionMenuItems({ state: 'active' }, false)
    const stop1 = items1.filter(i => !i.separator).find(i => i.label === 'Stop Session')
    expect(stop1.keepOpen).toBe(true)
    expect(stop1.danger).toBe(true)

    const items2 = sessionMenuItems({ state: 'active' }, true)
    const stop2 = items2.filter(i => !i.separator).find(i => i.label === 'Confirm stop?')
    expect(stop2.keepOpen).toBe(false)
    expect(stop2.danger).toBe(true)
  })

  it('collapses duplicate Codex and Gemini continue labels from the menu', () => {
    const labels = sessionMenuItems(null).filter(i => !i.separator).map((item) => item.label)
    expect(labels).not.toContain('Continue Codex')
    expect(labels).not.toContain('Continue Gemini')
  })

  it('has stable separator positions between states', () => {
    const noSession = sessionMenuItems(null)
    const withSession = sessionMenuItems({
      state: 'active',
      tmux_session: 'team',
      tmux_window: '2',
      tmux_pane: '%9',
    })

    expect(noSession.filter(i => i.separator)).toHaveLength(2)
    expect(withSession.filter(i => i.separator)).toHaveLength(4)
  })

  // Idle session gets same treatment as active
  it('treats idle session same as active', () => {
    const active = sessionMenuItems({ state: 'active' })
    const idle = sessionMenuItems({ state: 'idle' })
    expect(active.map(i => i.label)).toEqual(idle.map(i => i.label))
  })

  // AC5-7: Launch actions use correct modes
  it('launch actions map to correct modes', () => {
    const items = sessionMenuItems(null)
    const actionable = items.filter(i => !i.separator)
    expect(actionable[0].action).toBe('continue-claude')
    expect(actionable[1].action).toBe('fresh-claude')
    expect(actionable[2].action).toBe('fresh-codex')
    expect(actionable[6].action).toBe('resume-gemini')
  })
})

describe('Sidebar visual indicators', () => {
  // Mirrors hasSession() from Shell.svelte.
  function hasSession(project, sessionMap) {
    const session = sessionMap.get(project.path) ?? null
    return session?.state === 'active' || session?.state === 'idle'
  }

  // Mirrors isSessionActive() from Shell.svelte.
  function isSessionActive(project, sessionMap) {
    const session = sessionMap.get(project.path) ?? null
    return session?.state === 'active'
  }

  // Mirrors rowTintFor() from Shell.svelte.
  // Row tint = session presence (active or idle).
  function rowTintFor(project, sessionMap) {
    const session = sessionMap.get(project.path) ?? null
    if (session?.state === 'active' || session?.state === 'idle') {
      return 'bg-white/[0.03]'
    }
    return ''
  }

  it('hasSession true for active session', () => {
    const project = { path: '/proj', activityState: 'dormant' }
    const sessions = new Map([['/proj', { state: 'active' }]])
    expect(hasSession(project, sessions)).toBe(true)
  })

  it('hasSession true for idle session', () => {
    const project = { path: '/proj', activityState: 'active' }
    const sessions = new Map([['/proj', { state: 'idle' }]])
    expect(hasSession(project, sessions)).toBe(true)
  })

  it('hasSession false when no session', () => {
    const project = { path: '/proj', activityState: 'recent' }
    expect(hasSession(project, new Map())).toBe(false)
  })

  it('isSessionActive true only for active, not idle', () => {
    const project = { path: '/proj', activityState: 'stale' }
    const active = new Map([['/proj', { state: 'active' }]])
    const idle = new Map([['/proj', { state: 'idle' }]])
    const none = new Map()

    expect(isSessionActive(project, active)).toBe(true)
    expect(isSessionActive(project, idle)).toBe(false)
    expect(isSessionActive(project, none)).toBe(false)
  })

  // --- Row tint tests ---

  it('applies row tint when session active', () => {
    const project = { path: '/proj', activityState: 'active' }
    const sessions = new Map([['/proj', { state: 'active' }]])
    expect(rowTintFor(project, sessions)).toBe('bg-white/[0.03]')
  })

  it('applies row tint when session idle', () => {
    const project = { path: '/proj', activityState: 'dormant' }
    const sessions = new Map([['/proj', { state: 'idle' }]])
    expect(rowTintFor(project, sessions)).toBe('bg-white/[0.03]')
  })

  it('no row tint when no session', () => {
    const project = { path: '/proj', activityState: 'recent' }
    expect(rowTintFor(project, new Map())).toBe('')
  })

  it('row tint removed when session ends', () => {
    const project = { path: '/proj', activityState: 'active' }
    const withSession = new Map([['/proj', { state: 'active' }]])
    expect(rowTintFor(project, withSession)).toBe('bg-white/[0.03]')

    expect(rowTintFor(project, new Map())).toBe('')
  })
})
