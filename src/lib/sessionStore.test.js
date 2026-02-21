import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

vi.mock('./ipc.js', () => ({
  listClaudeSessions: vi.fn(),
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

  // AC2: Sessions are keyed by project path
  it('stores sessions keyed by project path', async () => {
    const mockSessions = [
      { pid: 100, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude' },
      { pid: 200, project_path: '/home/user/proj-b', state: 'idle', tty: '/dev/pts/2', args: 'claude' },
    ]
    ipc.listClaudeSessions.mockResolvedValue(mockSessions)
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const sessions = store.getSessions()
    expect(sessions.size).toBe(2)
    expect(sessions.get('/home/user/proj-a')).toEqual(mockSessions[0])
    expect(sessions.get('/home/user/proj-b')).toEqual(mockSessions[1])
  })

  // AC3: getSessionForProject returns matching session or null
  it('getSessionForProject returns session by path', async () => {
    const session = { pid: 100, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    expect(store.getSessionForProject('/home/user/proj-a')).toEqual(session)
    expect(store.getSessionForProject('/home/user/nonexistent')).toBeNull()
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
    const session = { pid: 100, project_path: '/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude' }
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
    const session = { pid: 100, project_path: '/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude' }
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
    const session = { pid: 100, project_path: '/home/user/proj-a/', state: 'active', tty: '/dev/pts/1', args: 'claude' }
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
    }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    // DB stores \\wsl$\ form — should still match
    expect(store.getSessionForProject('\\\\wsl$\\Ubuntu\\home\\user\\proj')).toBeTruthy()
    // And the \\wsl.localhost\ form should also match
    expect(store.getSessionForProject('\\\\wsl.localhost\\Ubuntu\\home\\user\\proj')).toBeTruthy()
  })
})
