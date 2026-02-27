import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

vi.mock('./ipc.js', () => ({
  listClaudeSessions: vi.fn(),
  recordSessionActivity: vi.fn().mockResolvedValue(undefined),
}))

describe('sessionStore', () => {
  let store
  let ipc

  beforeEach(async () => {
    vi.useFakeTimers()
    vi.resetModules()
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
    store = await import('./sessionStore.svelte.js')
  })

  afterEach(() => {
    store.stopPolling()
    vi.useRealTimers()
  })

  // AC1: Polling calls listClaudeSessions every 500ms
  it('polls listClaudeSessions every 500ms after startPolling', async () => {
    ipc.listClaudeSessions.mockResolvedValue([])
    store.startPolling()

    // Should call immediately on start
    await vi.advanceTimersByTimeAsync(0)
    expect(ipc.listClaudeSessions).toHaveBeenCalledTimes(1)

    // After 500ms, should call again
    await vi.advanceTimersByTimeAsync(500)
    expect(ipc.listClaudeSessions).toHaveBeenCalledTimes(2)

    // After another 500ms
    await vi.advanceTimersByTimeAsync(500)
    expect(ipc.listClaudeSessions).toHaveBeenCalledTimes(3)
  })

  // AC2: Sessions are grouped by project path
  it('stores sessions grouped by project path', async () => {
    const mockSessions = [
      { pid: 100, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' },
      { pid: 200, project_path: '/home/user/proj-b', state: 'idle', tty: '/dev/pts/2', args: 'claude', cli_tool: 'claude' },
    ]
    ipc.listClaudeSessions.mockResolvedValue(mockSessions)
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const sessions = store.getSessions()
    expect(sessions.size).toBe(2)
    expect(sessions.get('/home/user/proj-a')).toEqual([mockSessions[0]])
    expect(sessions.get('/home/user/proj-b')).toEqual([mockSessions[1]])
  })

  // AC3: getSessionForProject returns first matching session or null
  it('getSessionForProject returns first session by path', async () => {
    const session = { pid: 100, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    expect(store.getSessionForProject('/home/user/proj-a')).toEqual(session)
    expect(store.getSessionForProject('/home/user/nonexistent')).toBeNull()
  })

  // getSessionsForProject returns all sessions for a project
  it('getSessionsForProject returns all sessions for a project', async () => {
    const mockSessions = [
      { pid: 100, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' },
      { pid: 200, project_path: '/home/user/proj-a', state: 'idle', tty: '/dev/pts/2', args: 'codex --yolo', cli_tool: 'codex' },
    ]
    ipc.listClaudeSessions.mockResolvedValue(mockSessions)
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const all = store.getSessionsForProject('/home/user/proj-a')
    expect(all).toHaveLength(2)
    expect(all[0].cli_tool).toBe('claude')
    expect(all[1].cli_tool).toBe('codex')
  })

  it('getSessionsForProject returns empty array when no sessions', async () => {
    ipc.listClaudeSessions.mockResolvedValue([])
    store.startPolling()
    await vi.advanceTimersByTimeAsync(0)

    expect(store.getSessionsForProject('/nonexistent')).toEqual([])
  })

  // AC4: Self-scheduling prevents overlapping polls
  it('does not start a new poll while the previous one is still in-flight', async () => {
    let resolveFirst
    const firstCall = new Promise(resolve => { resolveFirst = resolve })

    ipc.listClaudeSessions
      .mockReturnValueOnce(firstCall)
      .mockResolvedValue([])

    store.startPolling()

    // First call starts immediately
    await vi.advanceTimersByTimeAsync(0)
    expect(ipc.listClaudeSessions).toHaveBeenCalledTimes(1)

    // Advance well past the poll interval — should NOT start a second poll
    // because the first one hasn't resolved yet
    await vi.advanceTimersByTimeAsync(2000)
    expect(ipc.listClaudeSessions).toHaveBeenCalledTimes(1)

    // Now resolve the first call
    resolveFirst([])
    await vi.advanceTimersByTimeAsync(0)

    // After resolution + 500ms interval, the next poll fires
    await vi.advanceTimersByTimeAsync(500)
    expect(ipc.listClaudeSessions).toHaveBeenCalledTimes(2)
  })

  // AC5: Polling stops cleanly
  it('stops polling when stopPolling is called', async () => {
    ipc.listClaudeSessions.mockResolvedValue([])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)
    expect(ipc.listClaudeSessions).toHaveBeenCalledTimes(1)

    store.stopPolling()

    await vi.advanceTimersByTimeAsync(2000)
    // Should not have called again after stop
    expect(ipc.listClaudeSessions).toHaveBeenCalledTimes(1)
  })

  // AC5: No double polling if startPolling called twice
  it('does not double-poll if startPolling called twice', async () => {
    ipc.listClaudeSessions.mockResolvedValue([])
    store.startPolling()
    store.startPolling() // second call should be no-op

    await vi.advanceTimersByTimeAsync(500)
    // Should only call twice (once immediate + once at 500ms), not four times
    expect(ipc.listClaudeSessions).toHaveBeenCalledTimes(2)
  })

  // AC7: Empty response clears session state
  it('clears sessions when empty response received', async () => {
    const session = { pid: 100, project_path: '/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions
      .mockResolvedValueOnce([session])
      .mockResolvedValueOnce([])

    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)
    expect(store.getSessions().size).toBe(1)

    await vi.advanceTimersByTimeAsync(500)
    expect(store.getSessions().size).toBe(0)
  })

  // AC7: IPC error doesn't crash, keeps previous state
  it('keeps previous state on IPC error', async () => {
    const session = { pid: 100, project_path: '/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions
      .mockResolvedValueOnce([session])
      .mockRejectedValueOnce(new Error('daemon offline'))

    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)
    expect(store.getSessions().size).toBe(1)

    // Error on second poll — should keep previous state
    await vi.advanceTimersByTimeAsync(500)
    expect(store.getSessions().size).toBe(1)
  })

  // Path normalization: trailing slashes
  it('normalizes trailing slashes in project paths', async () => {
    const session = { pid: 100, project_path: '/home/user/proj-a/', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    // Should be findable both with and without trailing slash
    expect(store.getSessionForProject('/home/user/proj-a')).toBeTruthy()
    expect(store.getSessionForProject('/home/user/proj-a/')).toBeTruthy()
  })

  // WSL path normalization: \\wsl.localhost\ and \\wsl$\ should match
  it('matches sessions across wsl.localhost and wsl$ path forms', async () => {
    // Backend returns \\wsl.localhost\ form
    const session = {
      pid: 100,
      project_path: '\\\\wsl.localhost\\Ubuntu\\home\\user\\proj',
      state: 'active',
      tty: '/dev/pts/1',
      args: 'claude',
      cli_tool: 'claude',
    }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    // DB stores \\wsl$\ form — should still match
    expect(store.getSessionForProject('\\\\wsl$\\Ubuntu\\home\\user\\proj')).toBeTruthy()
    // And the \\wsl.localhost\ form should also match
    expect(store.getSessionForProject('\\\\wsl.localhost\\Ubuntu\\home\\user\\proj')).toBeTruthy()
  })

  // Windows drive path normalization
  it('matches sessions across Windows drive letter casing', async () => {
    const session = {
      pid: 100,
      project_path: 'D:\\projects\\foo',
      state: 'active',
      tty: '/dev/pts/1',
      args: 'claude',
      cli_tool: 'claude',
    }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    // Both uppercase and lowercase drive letters should match
    expect(store.getSessionForProject('D:\\projects\\foo')).toBeTruthy()
    expect(store.getSessionForProject('d:\\projects\\foo')).toBeTruthy()
    // Trailing backslash should also match
    expect(store.getSessionForProject('D:\\projects\\foo\\')).toBeTruthy()
  })

  // Multiple CLI tools on the same project
  it('groups multiple CLI tools on the same project', async () => {
    const mockSessions = [
      { pid: 100, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude --continue', cli_tool: 'claude' },
      { pid: 200, project_path: '/home/user/proj-a', state: 'idle', tty: '/dev/pts/2', args: 'codex --yolo', cli_tool: 'codex' },
      { pid: 300, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/3', args: 'gemini --yolo', cli_tool: 'gemini' },
    ]
    ipc.listClaudeSessions.mockResolvedValue(mockSessions)
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const sessions = store.getSessions()
    expect(sessions.size).toBe(1) // One project key
    const all = store.getSessionsForProject('/home/user/proj-a')
    expect(all).toHaveLength(3)
    expect(all.map(s => s.cli_tool)).toEqual(['claude', 'codex', 'gemini'])

    // getSessionForProject returns the first one
    expect(store.getSessionForProject('/home/user/proj-a').cli_tool).toBe('claude')
  })

  // Multi-CLI tools across different projects
  it('coexists sessions from different CLI tools on different projects', async () => {
    const mockSessions = [
      { pid: 100, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude --continue', cli_tool: 'claude' },
      { pid: 200, project_path: '/home/user/proj-b', state: 'idle', tty: '/dev/pts/2', args: 'codex --yolo', cli_tool: 'codex' },
      { pid: 300, project_path: '/home/user/proj-c', state: 'active', tty: '/dev/pts/3', args: 'gemini --yolo', cli_tool: 'gemini' },
    ]
    ipc.listClaudeSessions.mockResolvedValue(mockSessions)
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const sessions = store.getSessions()
    expect(sessions.size).toBe(3)
    expect(store.getSessionForProject('/home/user/proj-a').cli_tool).toBe('claude')
    expect(store.getSessionForProject('/home/user/proj-b').cli_tool).toBe('codex')
    expect(store.getSessionForProject('/home/user/proj-c').cli_tool).toBe('gemini')
  })

  // --- Activity Tracker Tests ---

  it('creates tracker on first poll with new PID', async () => {
    const session = { pid: 500, project_path: '/proj', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const stats = store.getSessionStats(500)
    expect(stats).not.toBeNull()
    expect(stats.totalTicks).toBe(1)
    expect(stats.activeTicks).toBe(1)
    expect(stats.projectPath).toBe('/proj')
    expect(stats.cliTool).toBe('claude')
  })

  it('increments activeTicks only when state is active', async () => {
    const session = { pid: 600, project_path: '/proj', state: 'idle', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const stats = store.getSessionStats(600)
    expect(stats.totalTicks).toBe(1)
    expect(stats.activeTicks).toBe(0) // idle — no active tick
  })

  it('increments totalTicks regardless of state', async () => {
    const session = { pid: 700, project_path: '/proj', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    // 3 poll cycles
    await vi.advanceTimersByTimeAsync(0)
    await vi.advanceTimersByTimeAsync(500)
    await vi.advanceTimersByTimeAsync(500)

    const stats = store.getSessionStats(700)
    expect(stats.totalTicks).toBe(3)
    expect(stats.activeTicks).toBe(3)
  })

  it('updates lastTransitionTime on state change', async () => {
    const session = { pid: 800, project_path: '/proj', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)
    const initialTransition = store.getSessionStats(800).lastTransitionTime

    // Still active — no transition
    await vi.advanceTimersByTimeAsync(500)
    expect(store.getSessionStats(800).lastTransitionTime).toBe(initialTransition)

    // Switch to idle — transition should update
    session.state = 'idle'
    await vi.advanceTimersByTimeAsync(500)
    expect(store.getSessionStats(800).lastTransitionTime).toBeGreaterThan(initialTransition)
    expect(store.getSessionStats(800).lastState).toBe('idle')
  })

  it('enriches session objects with computed fields', async () => {
    const session = { pid: 900, project_path: '/proj', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const s = store.getSessionForProject('/proj')
    expect(s._duration).toBeTypeOf('number')
    expect(s._activeMs).toBeTypeOf('number')
    expect(s._activePercent).toBeTypeOf('number')
    expect(s._lastTransition).toBeTypeOf('number')
    expect(s._activePercent).toBe(100) // all ticks were active
  })

  it('computes _activePercent as ratio of active to total ticks', async () => {
    const session = { pid: 1000, project_path: '/proj', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    // 2 active polls
    await vi.advanceTimersByTimeAsync(0)
    await vi.advanceTimersByTimeAsync(500)

    // Switch to idle for 2 polls
    session.state = 'idle'
    await vi.advanceTimersByTimeAsync(500)
    await vi.advanceTimersByTimeAsync(500)

    const s = store.getSessionForProject('/proj')
    // 2 active out of 4 total = 50%
    expect(s._activePercent).toBe(50)
  })

  it('triggers recordSessionActivity IPC when session disappears', async () => {
    const session = { pid: 1100, project_path: '/proj-x', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions
      .mockResolvedValueOnce([session])
      .mockResolvedValue([]) // session disappears

    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)
    expect(store.getSessionStats(1100)).not.toBeNull()

    // Session disappears on next poll
    await vi.advanceTimersByTimeAsync(500)

    expect(ipc.recordSessionActivity).toHaveBeenCalledTimes(1)
    expect(ipc.recordSessionActivity).toHaveBeenCalledWith(
      '/proj-x',
      'claude',
      expect.any(String), // startedAt
      expect.any(String), // endedAt
      expect.any(Number), // activeDurationMs
      expect.any(Number), // totalDurationMs
    )
  })

  it('cleans up tracker after session disappears', async () => {
    const session = { pid: 1200, project_path: '/proj', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions
      .mockResolvedValueOnce([session])
      .mockResolvedValue([])

    store.startPolling()
    await vi.advanceTimersByTimeAsync(0)
    expect(store.getSessionStats(1200)).not.toBeNull()

    await vi.advanceTimersByTimeAsync(500)
    expect(store.getSessionStats(1200)).toBeNull()
  })

  it('stopPolling clears all trackers', async () => {
    const session = { pid: 1300, project_path: '/proj', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)
    expect(store.getSessionStats(1300)).not.toBeNull()

    store.stopPolling()
    expect(store.getSessionStats(1300)).toBeNull()
  })
})
