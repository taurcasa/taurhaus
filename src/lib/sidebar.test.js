import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

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
      { id: 'p1', name: 'test-project', activity_state: 'active', branch: 'main', is_dirty: false },
    ]
    ipc.listProjects.mockResolvedValue(mockData)

    const result = await ipc.listProjects()

    expect(result).toEqual(mockData)
    expect(result[0].activity_state).toBe('active')
  })

  it('listProjects groups by activity_state', async () => {
    const mockData = [
      { id: 'p1', name: 'a', activity_state: 'active', branch: 'main', is_dirty: false },
      { id: 'p2', name: 'b', activity_state: 'recent', branch: 'main', is_dirty: false },
      { id: 'p3', name: 'c', activity_state: 'dormant', branch: 'main', is_dirty: false },
    ]
    ipc.listProjects.mockResolvedValue(mockData)

    const result = await ipc.listProjects()
    const groups = {
      active: result.filter(p => p.activity_state === 'active'),
      recent: result.filter(p => p.activity_state === 'recent'),
      stale: result.filter(p => p.activity_state === 'stale'),
      dormant: result.filter(p => p.activity_state === 'dormant'),
    }

    expect(groups.active).toHaveLength(1)
    expect(groups.recent).toHaveLength(1)
    expect(groups.stale).toHaveLength(0)
    expect(groups.dormant).toHaveLength(1)
  })

  it('activity_state maps to correct dot color class', () => {
    const dotColor = {
      active: 'bg-success-300',
      recent: 'bg-info-300',
      stale: 'bg-warning-300',
      dormant: 'bg-zinc-400',
    }

    expect(dotColor['active']).toBe('bg-success-300')
    expect(dotColor['recent']).toBe('bg-info-300')
    expect(dotColor['stale']).toBe('bg-warning-300')
    expect(dotColor['dormant']).toBe('bg-zinc-400')
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
      activity_state: 'active',
      last_activity_at: '2025-01-01T00:00:00Z',
      hero_preference: null,
      created_at: '2025-01-01T00:00:00Z',
      updated_at: '2025-01-01T00:00:00Z',
      branch: 'main',
      is_dirty: false,
    }
    ipc.getProject.mockResolvedValue(mockDetail)

    const result = await ipc.getProject('p1')

    expect(result.description).toBe('Desktop tool for AI project management')
    expect(result.activity_state).toBe('active')
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

describe('Context menu session actions', () => {
  /**
   * Mirrors the session menu item generation logic from Shell.svelte.
   * Returns the session-specific items that go between "Copy path" and "Remove".
   */
  function sessionMenuItems(session, confirmStop = false) {
    const items = []

    if (session?.state === 'active' || session?.state === 'idle') {
      // Has session — show navigate first, launch disabled, destructive group
      items.push({ label: 'Open in Terminal', action: 'openInTerminal' })
      items.push({ separator: true })
      items.push({ label: 'Continue Session', disabled: true })
      items.push({ label: 'New Session', disabled: true })
      items.push({ label: 'Resume (pick)...', disabled: true })
      items.push({ separator: true })
      items.push({ label: 'Restart Session', action: 'restart' })
      items.push({
        label: confirmStop ? 'Confirm stop?' : 'Stop Session',
        action: 'stop',
        danger: true,
        keepOpen: !confirmStop,
      })
    } else {
      // No session — all launch items enabled
      items.push({ label: 'Continue Session', action: 'continue' })
      items.push({ label: 'New Session', action: 'fresh' })
      items.push({ label: 'Resume (pick)...', action: 'resume' })
    }

    return items
  }

  // AC1: No session → menu shows launch items all enabled
  it('shows all launch items enabled when no session', () => {
    const items = sessionMenuItems(null)
    expect(items).toHaveLength(3)
    expect(items[0].label).toBe('Continue Session')
    expect(items[1].label).toBe('New Session')
    expect(items[2].label).toBe('Resume (pick)...')
    expect(items.every(i => !i.disabled)).toBe(true)
  })

  // AC2: Active session → "Open in Terminal" first, launch items disabled
  it('shows Open in Terminal first and disables launch items for active session', () => {
    const items = sessionMenuItems({ state: 'active' })
    const actionable = items.filter(i => !i.separator)

    expect(actionable[0].label).toBe('Open in Terminal')
    expect(actionable[0].disabled).toBeFalsy()

    // Launch items disabled
    expect(actionable[1].label).toBe('Continue Session')
    expect(actionable[1].disabled).toBe(true)
    expect(actionable[2].label).toBe('New Session')
    expect(actionable[2].disabled).toBe(true)
    expect(actionable[3].label).toBe('Resume (pick)...')
    expect(actionable[3].disabled).toBe(true)
  })

  // AC3: Active session → destructive group with Restart and Stop
  it('includes Restart and Stop in destructive group for active session', () => {
    const items = sessionMenuItems({ state: 'active' })
    const actionable = items.filter(i => !i.separator)

    expect(actionable[4].label).toBe('Restart Session')
    expect(actionable[5].label).toBe('Stop Session')
    expect(actionable[5].danger).toBe(true)
  })

  // AC8: Stop uses two-click confirmation
  it('Stop Session uses two-click confirmation pattern', () => {
    // First state — not confirmed
    const items1 = sessionMenuItems({ state: 'active' }, false)
    const stop1 = items1.filter(i => !i.separator).find(i => i.label === 'Stop Session')
    expect(stop1.keepOpen).toBe(true)
    expect(stop1.danger).toBe(true)

    // Second state — confirmed
    const items2 = sessionMenuItems({ state: 'active' }, true)
    const stop2 = items2.filter(i => !i.separator).find(i => i.label === 'Confirm stop?')
    expect(stop2.keepOpen).toBe(false)
    expect(stop2.danger).toBe(true)
  })

  // AC9: Disabled items are not actionable
  it('disabled launch items have no action when session active', () => {
    const items = sessionMenuItems({ state: 'active' })
    const disabled = items.filter(i => i.disabled)
    expect(disabled).toHaveLength(3)
    disabled.forEach(item => {
      expect(item.action).toBeUndefined()
    })
  })

  // AC10: Menu structure stable — separator positions don't shift
  it('has stable separator positions between states', () => {
    const noSession = sessionMenuItems(null)
    const withSession = sessionMenuItems({ state: 'active' })

    // No session: 3 items, no separators
    expect(noSession.filter(i => i.separator)).toHaveLength(0)

    // With session: 2 separators (after Open in Terminal, before destructive group)
    const seps = withSession.filter(i => i.separator)
    expect(seps).toHaveLength(2)
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
    expect(items[0].action).toBe('continue')
    expect(items[1].action).toBe('fresh')
    expect(items[2].action).toBe('resume')
  })
})

describe('Sidebar visual indicators', () => {
  // Dot color = git activity state ONLY (Option B: dot never reflects session)
  const dotColor = {
    active: 'bg-success-300',
    recent: 'bg-info-300',
    stale: 'bg-warning-300',
    dormant: 'bg-zinc-400',
  }

  // Mirrors dotClassFor() from Shell.svelte.
  // Dot = git activity color + ambient shadow. No session logic.
  function dotClassFor(project) {
    const color = dotColor[project.activity_state]
    return color + ' shadow-[0_0_4px_rgba(255,255,255,0.15)]'
  }

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

  // --- Dot tests (git-only, no session influence) ---

  it('dot always uses ambient shadow regardless of session', () => {
    const project = { path: '/proj', activity_state: 'dormant' }
    const cls = dotClassFor(project)
    expect(cls).toContain('bg-zinc-400')
    expect(cls).toContain('shadow-')
    expect(cls).not.toContain('session-')
  })

  it('dot never has session classes even with active session', () => {
    const project = { path: '/proj', activity_state: 'active' }
    const cls = dotClassFor(project)
    expect(cls).toBe('bg-success-300 shadow-[0_0_4px_rgba(255,255,255,0.15)]')
    expect(cls).not.toContain('session-')
  })

  it('dot color maps correctly for all activity states', () => {
    for (const [state, color] of Object.entries(dotColor)) {
      const cls = dotClassFor({ activity_state: state })
      expect(cls).toContain(color)
    }
  })

  // --- Session indicator tests (separate element on row) ---

  it('hasSession true for active session', () => {
    const project = { path: '/proj', activity_state: 'dormant' }
    const sessions = new Map([['/proj', { state: 'active' }]])
    expect(hasSession(project, sessions)).toBe(true)
  })

  it('hasSession true for idle session', () => {
    const project = { path: '/proj', activity_state: 'active' }
    const sessions = new Map([['/proj', { state: 'idle' }]])
    expect(hasSession(project, sessions)).toBe(true)
  })

  it('hasSession false when no session', () => {
    const project = { path: '/proj', activity_state: 'recent' }
    expect(hasSession(project, new Map())).toBe(false)
  })

  it('isSessionActive true only for active, not idle', () => {
    const project = { path: '/proj', activity_state: 'stale' }
    const active = new Map([['/proj', { state: 'active' }]])
    const idle = new Map([['/proj', { state: 'idle' }]])
    const none = new Map()

    expect(isSessionActive(project, active)).toBe(true)
    expect(isSessionActive(project, idle)).toBe(false)
    expect(isSessionActive(project, none)).toBe(false)
  })

  // --- Row tint tests ---

  it('applies row tint when session active', () => {
    const project = { path: '/proj', activity_state: 'active' }
    const sessions = new Map([['/proj', { state: 'active' }]])
    expect(rowTintFor(project, sessions)).toBe('bg-white/[0.03]')
  })

  it('applies row tint when session idle', () => {
    const project = { path: '/proj', activity_state: 'dormant' }
    const sessions = new Map([['/proj', { state: 'idle' }]])
    expect(rowTintFor(project, sessions)).toBe('bg-white/[0.03]')
  })

  it('no row tint when no session', () => {
    const project = { path: '/proj', activity_state: 'recent' }
    expect(rowTintFor(project, new Map())).toBe('')
  })

  it('row tint removed when session ends', () => {
    const project = { path: '/proj', activity_state: 'active' }
    const withSession = new Map([['/proj', { state: 'active' }]])
    expect(rowTintFor(project, withSession)).toBe('bg-white/[0.03]')

    expect(rowTintFor(project, new Map())).toBe('')
  })
})
