import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

vi.mock('./ipc.js', () => {
  const listClaudeSessions = vi.fn()
  return {
    listClaudeSessions,
    // The store polls the snapshot command, which answers with the list *and*
    // how it was obtained. Cases that only care about the list keep stubbing
    // `listClaudeSessions`; a bare array reads as an observed list.
    listCliSessionSnapshot: vi.fn(() => listClaudeSessions()),
    listProjects: vi.fn().mockResolvedValue([]),
    recordSessionActivity: vi.fn().mockResolvedValue(undefined),
  }
})

describe('sessionStore', () => {
  let store
  let ipc

  beforeEach(async () => {
    vi.useFakeTimers()
    vi.resetModules()
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
    // clearAllMocks() keeps implementations and the mocked module is shared
    // across cases, so re-arm the delegation: a case that stubs the snapshot
    // answer directly must not leak its stub into the next one.
    ipc.listCliSessionSnapshot.mockImplementation(() => ipc.listClaudeSessions())
    store = await import('./sessionStore.svelte.js')
  })

  afterEach(() => {
    store.stopPolling()
    vi.useRealTimers()
  })

  // Regression: when daemon `sessions-updated` events are delayed/missing,
  // polling fallback must keep session indicators updating instead of stalling.
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
    expect(sessions.get('/home/user/proj-a')?.[0]).toMatchObject(mockSessions[0])
    expect(sessions.get('/home/user/proj-b')?.[0]).toMatchObject(mockSessions[1])
  })

  // AC3: getSessionForProject returns first matching session or null
  it('getSessionForProject returns first session by path', async () => {
    const session = { pid: 100, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    expect(store.getSessionForProject('/home/user/proj-a')).toMatchObject(session)
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
      { pid: 300, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/3', args: 'agy', cli_tool: 'agy' },
    ]
    ipc.listClaudeSessions.mockResolvedValue(mockSessions)
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const sessions = store.getSessions()
    expect(sessions.size).toBe(1) // One project key
    const all = store.getSessionsForProject('/home/user/proj-a')
    expect(all).toHaveLength(3)
    expect(all.map(s => s.cli_tool)).toEqual(['claude', 'codex', 'agy'])

    // getSessionForProject returns the first one
    expect(store.getSessionForProject('/home/user/proj-a').cli_tool).toBe('claude')
  })

  // Multi-CLI tools across different projects
  it('coexists sessions from different CLI tools on different projects', async () => {
    const mockSessions = [
      { pid: 100, project_path: '/home/user/proj-a', state: 'active', tty: '/dev/pts/1', args: 'claude --continue', cli_tool: 'claude' },
      { pid: 200, project_path: '/home/user/proj-b', state: 'idle', tty: '/dev/pts/2', args: 'codex --yolo', cli_tool: 'codex' },
      { pid: 300, project_path: '/home/user/proj-c', state: 'active', tty: '/dev/pts/3', args: 'agy', cli_tool: 'agy' },
    ]
    ipc.listClaudeSessions.mockResolvedValue(mockSessions)
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const sessions = store.getSessions()
    expect(sessions.size).toBe(3)
    expect(store.getSessionForProject('/home/user/proj-a').cli_tool).toBe('claude')
    expect(store.getSessionForProject('/home/user/proj-b').cli_tool).toBe('codex')
    expect(store.getSessionForProject('/home/user/proj-c').cli_tool).toBe('agy')
  })

  // --- Activity Tracker Tests ---

  it('creates tracker on first poll with new PID', async () => {
    const session = { pid: 500, project_path: '/proj', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)

    const stats = store.getSessionStats(500)
    expect(stats).not.toBeNull()
    // First sighting measures no elapsed time yet.
    expect(stats.totalMs).toBe(0)
    expect(stats.activeMs).toBe(0)
    expect(stats.projectPath).toBe('/proj')
    expect(stats.cliTool).toBe('claude')
  })

  it('accrues no active time while the session stays idle', async () => {
    const session = { pid: 600, project_path: '/proj', state: 'idle', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)
    await vi.advanceTimersByTimeAsync(500)

    const stats = store.getSessionStats(600)
    expect(stats.totalMs).toBe(500)
    expect(stats.activeMs).toBe(0) // idle — no active time
  })

  it('accrues total time regardless of state', async () => {
    const session = { pid: 700, project_path: '/proj', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling()

    // 3 poll cycles: two measured 500ms intervals.
    await vi.advanceTimersByTimeAsync(0)
    await vi.advanceTimersByTimeAsync(500)
    await vi.advanceTimersByTimeAsync(500)

    const stats = store.getSessionStats(700)
    expect(stats.totalMs).toBe(1000)
    expect(stats.activeMs).toBe(1000)
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

  // Regression: `_activeMs` counted polls instead of time
  // (`activeTicks * activePollIntervalMs`, sessionStore.svelte.js:215-232,
  // shipped in 9a66d1c). Daemon updates are event-driven, so the gap between
  // two snapshots is whatever the daemon took to report a change — never one
  // fixed tick.
  it('accumulates wall-clock elapsed time between polls while active', async () => {
    const session = { pid: 910, project_path: '/proj-wall-clock', state: 'active', tty: '/dev/pts/14', args: 'claude', cli_tool: 'claude' }
    ipc.listClaudeSessions.mockResolvedValue([session])
    store.startPolling({ intervalMs: 500 })

    await vi.advanceTimersByTimeAsync(0)
    expect(store.getSessionForProject('/proj-wall-clock')._activeMs).toBe(0)

    await vi.advanceTimersByTimeAsync(500)
    expect(store.getSessionForProject('/proj-wall-clock')._activeMs).toBe(500)

    await vi.advanceTimersByTimeAsync(500)
    expect(store.getSessionForProject('/proj-wall-clock')._activeMs).toBe(1000)
  })

  // Regression: a 5s gap between two snapshots used to count as a single
  // 500ms tick (9a66d1c).
  it('counts a five second gap between snapshots as five seconds of activity', () => {
    const session = { pid: 920, project_path: '/proj-gap', state: 'active', tty: '/dev/pts/15', args: 'claude', cli_tool: 'claude' }

    store.applyDaemonSessionUpdate([session])
    expect(store.getSessionForProject('/proj-gap')._activeMs).toBe(0)

    vi.advanceTimersByTime(5000)
    store.applyDaemonSessionUpdate([session])

    expect(store.getSessionForProject('/proj-gap')._activeMs).toBe(5000)
    expect(store.getSessionStats(920).totalMs).toBe(5000)
  })

  it('computes _activePercent as ratio of active to measured time', async () => {
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
    // Three measured 500ms intervals; the session was last seen active at the
    // start of the first two, so 1000ms active out of 1500ms measured.
    expect(s._activePercent).toBe(67)
  })

  it('triggers recordSessionActivity IPC when session disappears', async () => {
    const session = { pid: 1100, project_path: '/proj-x', state: 'active', tty: '/dev/pts/1', args: 'claude', cli_tool: 'claude' }
    ipc.listProjects.mockResolvedValueOnce([{ id: 'proj-x', path: '/proj-x' }])
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
      'proj-x',
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

    await store.stopPolling()
    expect(store.getSessionStats(1300)).toBeNull()
  })

  it('flushes accrued activity stats when stopPolling clears a live tracker', async () => {
    const session = { pid: 1310, project_path: '/proj-stop', state: 'active', tty: '/dev/pts/12', args: 'claude', cli_tool: 'claude' }
    ipc.listProjects.mockResolvedValueOnce([{ id: 'proj-stop-id', path: '/proj-stop' }])
    ipc.listClaudeSessions.mockResolvedValue([session])

    store.startPolling()
    await vi.advanceTimersByTimeAsync(0)
    await vi.advanceTimersByTimeAsync(500)

    await store.stopPolling()

    expect(ipc.recordSessionActivity).toHaveBeenCalledTimes(1)
    expect(ipc.recordSessionActivity).toHaveBeenCalledWith(
      'proj-stop-id',
      'claude',
      expect.any(String),
      expect.any(String),
      500,
      500,
    )
  })

  it('does not double-persist activity stats when stopPolling is called twice', async () => {
    const session = { pid: 1320, project_path: '/proj-double-stop', state: 'active', tty: '/dev/pts/13', args: 'claude', cli_tool: 'claude' }
    ipc.listProjects.mockResolvedValueOnce([{ id: 'proj-double-stop-id', path: '/proj-double-stop' }])
    ipc.listClaudeSessions.mockResolvedValue([session])

    store.startPolling()
    await vi.advanceTimersByTimeAsync(0)
    await vi.advanceTimersByTimeAsync(500)

    await store.stopPolling()
    await store.stopPolling()

    expect(ipc.recordSessionActivity).toHaveBeenCalledTimes(1)
    expect(ipc.recordSessionActivity).toHaveBeenCalledWith(
      'proj-double-stop-id',
      'claude',
      expect.any(String),
      expect.any(String),
      500,
      500,
    )
  })

  it('persists the measured active duration when a session disappears', async () => {
    // Regression: 6c6f1cb moved the tracker from tick counting to wall-clock
    // accrual but only accrued for sessions present in the new snapshot, so the
    // last interval — the one that ends at the observation which found the
    // session gone — was dropped. Q-PRD-02's acceptance case (observed active
    // at 0 s and 5 s, gone at 10 s) persisted 5000 ms instead of 10000 ms, and
    // 6c6f1cb rewrote this very assertion down to the undercount.
    const session = { pid: 1330, project_path: '/proj-tauri-persist', state: 'active', tty: '/dev/pts/16', args: 'claude', cli_tool: 'claude' }
    ipc.listProjects.mockResolvedValueOnce([{ id: 'proj-tauri-persist-id', path: '/proj-tauri-persist' }])
    ipc.listClaudeSessions
      .mockResolvedValueOnce([session])
      .mockResolvedValueOnce([session])
      .mockResolvedValue([])

    store.startPolling({ intervalMs: 5000 })

    await vi.advanceTimersByTimeAsync(0)
    await vi.advanceTimersByTimeAsync(5000)
    await vi.advanceTimersByTimeAsync(5000)

    expect(ipc.recordSessionActivity).toHaveBeenCalledTimes(1)
    expect(ipc.recordSessionActivity).toHaveBeenCalledWith(
      'proj-tauri-persist-id',
      'claude',
      expect.any(String),
      expect.any(String),
      10000,
      10000,
    )
  })

  it('credits the interval between the last observation and an explicit stop', async () => {
    // Regression: 6c6f1cb accrued elapsed time only inside applySessions, so
    // stopPolling() persisted the totals as of the last snapshot and threw away
    // everything measured since it.
    ipc.listProjects.mockResolvedValueOnce([{ id: 'proj-stop-gap-id', path: '/proj-stop-gap' }])
    store.applyDaemonSessionUpdate({
      version: 1,
      sessions: [
        { pid: 1340, project_path: '/proj-stop-gap', state: 'active', tty: '/dev/pts/17', args: 'claude', cli_tool: 'claude' },
      ],
    })

    vi.advanceTimersByTime(5000)
    await store.stopPolling()

    expect(ipc.recordSessionActivity).toHaveBeenCalledWith(
      'proj-stop-gap-id',
      'claude',
      expect.any(String),
      expect.any(String),
      5000,
      5000,
    )
  })

  it('does not credit the unobserved gap of a degraded snapshot to the last state', async () => {
    // Regression: 6c6f1cb credited the whole interval between two observations
    // to the state in effect at the earlier one. A degraded snapshot is not an
    // observation (PR 2, 06b432d), so a scanner blackout was silently backfilled
    // as activity: an active session that went dark for 10 s and came back
    // counted those 10 s as work nobody ever saw.
    const session = { pid: 1350, project_path: '/proj-degraded-gap', state: 'active', tty: '/dev/pts/18', args: 'claude', cli_tool: 'claude' }
    ipc.listProjects.mockResolvedValueOnce([{ id: 'proj-degraded-gap-id', path: '/proj-degraded-gap' }])

    store.applyDaemonSessionUpdate({ version: 1, sessions: [session] })
    vi.advanceTimersByTime(5000)

    // Observation lost: the hub keeps reporting its last good snapshot.
    store.applyDaemonSessionUpdate({ version: 1, sessions: [session], degraded: true })
    vi.advanceTimersByTime(10000)

    // Observation restored, then the session is gone.
    store.applyDaemonSessionUpdate({ version: 2, sessions: [session] })
    vi.advanceTimersByTime(5000)
    store.applyDaemonSessionUpdate({ version: 3, sessions: [] })
    await vi.advanceTimersByTimeAsync(0)

    expect(ipc.recordSessionActivity).toHaveBeenCalledWith(
      'proj-degraded-gap-id',
      'claude',
      expect.any(String),
      expect.any(String),
      10000,
      10000,
    )
  })

  it('does not credit the gap after presence went stale', async () => {
    // Regression: 6c6f1cb left trackers running while the daemon bridge was
    // down. markSessionPresenceStale() is the app admitting it stopped
    // observing, so the outage belongs to no state.
    const session = { pid: 1360, project_path: '/proj-stale-gap-ms', state: 'active', tty: '/dev/pts/19', args: 'claude', cli_tool: 'claude' }
    ipc.listProjects.mockResolvedValueOnce([{ id: 'proj-stale-gap-ms-id', path: '/proj-stale-gap-ms' }])

    store.applyDaemonSessionUpdate({ version: 1, sessions: [session] })
    vi.advanceTimersByTime(3000)
    store.markSessionPresenceStale()
    vi.advanceTimersByTime(7000)

    store.applyDaemonSessionUpdate({ version: 2, sessions: [session] })
    vi.advanceTimersByTime(2000)
    store.applyDaemonSessionUpdate({ version: 3, sessions: [] })
    await vi.advanceTimersByTimeAsync(0)

    expect(ipc.recordSessionActivity).toHaveBeenCalledWith(
      'proj-stale-gap-ms-id',
      'claude',
      expect.any(String),
      expect.any(String),
      5000,
      5000,
    )
  })

  it('stamps sessions from a degraded snapshot so the indicator reads uncertain', async () => {
    // Regression: 6c6f1cb taught activitySignal.js to downgrade a `degraded`
    // record to uncertain, but nothing on the frontend ever set the field —
    // the daemon hub's degradation stopped at the wire. A retained snapshot
    // stayed green for as long as the scanner was blind.
    const { activitySignal } = await import('./activitySignal.js')
    const session = { pid: 1370, project_path: '/proj-degraded-stamp', state: 'active', tty: '/dev/pts/20', args: 'claude', cli_tool: 'claude' }

    store.applyDaemonSessionUpdate({ version: 4, sessions: [session], degraded: true })
    expect(activitySignal(store.getSessionForProject('/proj-degraded-stamp')).level).toBe('uncertain')

    store.applyDaemonSessionUpdate({ version: 5, sessions: [session] })
    expect(activitySignal(store.getSessionForProject('/proj-degraded-stamp')).level).toBe('active')
  })

  it('does not retire a tracker on a degraded snapshot that omits it', async () => {
    // A blind scanner is not evidence that a session ended (PR 2, 06b432d):
    // only a real observation may flush activity stats.
    const session = { pid: 1380, project_path: '/proj-degraded-omit', state: 'active', tty: '/dev/pts/21', args: 'claude', cli_tool: 'claude' }
    store.applyDaemonSessionUpdate({ version: 1, sessions: [session] })

    store.applyDaemonSessionUpdate({ version: 1, sessions: [], degraded: true })
    await vi.advanceTimersByTimeAsync(0)

    expect(ipc.recordSessionActivity).not.toHaveBeenCalled()
    expect(store.getSessionStats(1380)).toBeTruthy()
  })

  it('applies daemon session updates without polling', () => {
    store.applyDaemonSessionUpdate({
      version: 1,
      sessions: [
        { pid: 1400, project_path: '/proj-daemon', state: 'active', tty: '/dev/pts/2', args: 'codex', cli_tool: 'codex' },
      ],
    })

    const session = store.getSessionForProject('/proj-daemon')
    expect(session).toBeTruthy()
    expect(session.cli_tool).toBe('codex')
    expect(session._activePercent).toBe(100)
  })

  it('accepts raw session arrays in applyDaemonSessionUpdate', () => {
    store.applyDaemonSessionUpdate([
      { pid: 1500, project_path: '/proj-array', state: 'idle', tty: '/dev/pts/3', args: 'claude', cli_tool: 'claude' },
    ])

    const session = store.getSessionForProject('/proj-array')
    expect(session).toBeTruthy()
    expect(session.state).toBe('idle')
  })

  it('normalizes camelCase session payloads from IPC polling', async () => {
    // Regression: IPC normalization drift returned camelCase fields while
    // sessionStore consumed snake_case only, causing sessions to be dropped.
    ipc.listClaudeSessions.mockResolvedValue([
      {
        pid: 1600,
        projectPath: '/proj-camel',
        state: 'active',
        tty: '/dev/pts/9',
        args: 'codex',
        cliTool: 'codex',
        tmuxSession: 'taurhaus',
        tmuxWindow: '2',
        tmuxPane: '%9',
      },
    ])

    store.startPolling()
    await vi.advanceTimersByTimeAsync(0)

    const session = store.getSessionForProject('/proj-camel')
    expect(session).toBeTruthy()
    expect(session.project_path).toBe('/proj-camel')
    expect(session.cli_tool).toBe('codex')
    expect(session.tmux_pane).toBe('%9')
  })

  it('normalizes camelCase session payloads from daemon updates', () => {
    // Regression: daemon bridge payloads with camelCase fields were not
    // normalized before grouping/tracking, hiding sidebar indicators.
    store.applyDaemonSessionUpdate({
      version: 2,
      sessions: [
        {
          pid: 1700,
          projectPath: '/proj-daemon-camel',
          state: 'idle',
          tty: '/dev/pts/10',
          args: 'agy',
          cliTool: 'agy',
          tmuxSession: 'taurhaus',
          tmuxWindow: '4',
          tmuxPane: '%10',
        },
      ],
    })

    const session = store.getSessionForProject('/proj-daemon-camel')
    expect(session).toBeTruthy()
    expect(session.project_path).toBe('/proj-daemon-camel')
    expect(session.cli_tool).toBe('agy')
    expect(session.tmux_pane).toBe('%10')
  })

  it('marks retained daemon sessions stale across a daemon gap without clearing them', () => {
    store.applyDaemonSessionUpdate({
      version: 1,
      sessions: [
        {
          pid: 1900,
          project_path: '/proj-stale-gap',
          state: 'active',
          tty: '/dev/pts/12',
          args: 'claude',
          cli_tool: 'claude',
        },
      ],
    })

    store.markSessionPresenceStale()

    const session = store.getSessionForProject('/proj-stale-gap')
    expect(session).toBeTruthy()
    expect(session._presenceStale).toBe(true)
    expect(session._presenceStatus).toBe('stale')
  })

  it('clears stale retained presence when a fresh snapshot arrives', () => {
    store.applyDaemonSessionUpdate({
      version: 1,
      sessions: [
        {
          pid: 1950,
          project_path: '/proj-stale-gap',
          state: 'idle',
          tty: '/dev/pts/13',
          args: 'codex',
          cli_tool: 'codex',
        },
      ],
    })
    store.markSessionPresenceStale()

    store.applyDaemonSessionUpdate({
      version: 2,
      sessions: [
        {
          pid: 1950,
          project_path: '/proj-stale-gap',
          state: 'idle',
          tty: '/dev/pts/13',
          args: 'codex',
          cli_tool: 'codex',
        },
      ],
    })

    const session = store.getSessionForProject('/proj-stale-gap')
    expect(session._presenceStale).toBe(false)
    expect(session._presenceStatus).toBe('live')
  })

  it('keeps trackers and sessions when a degraded snapshot without a sessions array arrives', async () => {
    // Regression: latent since 9a66d1c (a timed-out `ps` made the backend report
    // zero sessions); on the frontend a snapshot without a sessions array was
    // coerced to `[]`, which flushed every tracker (recordSessionActivity +
    // "skipping session activity persistence" warnings) and reset _lastTransition.
    // A degraded snapshot carries no session change and must be inert.
    const session = {
      pid: 2100,
      project_path: '/proj-degraded',
      state: 'active',
      tty: '/dev/pts/14',
      args: 'claude',
      cli_tool: 'claude',
    }
    ipc.listClaudeSessions
      .mockResolvedValueOnce([session])
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce(null)
      .mockResolvedValue([session])

    store.startPolling()
    await vi.advanceTimersByTimeAsync(0)
    const before = store.getSessionStats(2100)
    expect(before).toBeTruthy()
    const lastTransitionBefore = store.getSessionForProject('/proj-degraded')._lastTransition

    // Two degraded polls (undefined, null) must not flush or clear anything.
    await vi.advanceTimersByTimeAsync(500)
    await vi.advanceTimersByTimeAsync(500)
    expect(ipc.recordSessionActivity).not.toHaveBeenCalled()
    expect(store.getSessionStats(2100)).toBe(before)
    expect(store.getSessionForProject('/proj-degraded')).toBeTruthy()
    expect(store.getSessionForProject('/proj-degraded')._lastTransition).toBe(lastTransitionBefore)

    // A daemon payload without a sessions array is the same degraded shape.
    store.applyDaemonSessionUpdate({ version: 7 })
    expect(ipc.recordSessionActivity).not.toHaveBeenCalled()
    expect(store.getSessionStats(2100)).toBe(before)

    // The next good snapshot continues the same tracker (no reset of firstSeen).
    await vi.advanceTimersByTimeAsync(500)
    expect(store.getSessionStats(2100)).toBe(before)
    expect(store.getSessionForProject('/proj-degraded')._lastTransition).toBe(lastTransitionBefore)
  })

  it('measures nothing across an outage that fallback polling papers over', async () => {
    // Regression: fa572d4 suspended the trackers on a degraded daemon snapshot
    // and on markSessionPresenceStale(), but the fallback poll — the path the
    // app runs *because* the bridge is down — handed every returned array to
    // applySessions as a healthy observation. The suspension was cleared on the
    // first poll and the whole outage was credited to the last state seen, even
    // though the backend was serving its on-disk cache the entire time.
    const session = { pid: 1900, project_path: '/proj-fallback-gap', state: 'active', tty: '/dev/pts/30', args: 'claude', cli_tool: 'claude' }
    ipc.listProjects.mockResolvedValue([{ id: 'proj-fallback-gap-id', path: '/proj-fallback-gap' }])
    ipc.listCliSessionSnapshot.mockResolvedValue({ sessions: [session], freshness: 'fresh' })

    store.startPolling({ intervalMs: 1000 })
    await vi.advanceTimersByTimeAsync(0)
    await vi.advanceTimersByTimeAsync(1000)
    expect(store.getSessionStats(1900).activeMs).toBe(1000)

    // Daemon gone: the bridge dies and the backend answers from its cache.
    store.markSessionPresenceStale()
    ipc.listCliSessionSnapshot.mockResolvedValue({ sessions: [session], freshness: 'cached' })
    await vi.advanceTimersByTimeAsync(5000)
    expect(store.getSessionStats(1900).activeMs).toBe(1000)

    // Daemon back. The first fresh poll closes the blind interval without
    // crediting it; measurement resumes from there.
    ipc.listCliSessionSnapshot.mockResolvedValue({ sessions: [session], freshness: 'fresh' })
    await vi.advanceTimersByTimeAsync(1000)
    await vi.advanceTimersByTimeAsync(1000)

    const stats = store.getSessionStats(1900)
    expect(stats.activeMs).toBe(2000)
    expect(stats.totalMs).toBe(2000)
  })

  it('keeps measuring suspended while polls are rejected', async () => {
    // Regression: fa572d4 — a rejected poll only logged a warning, so the
    // trackers kept running through an outage the app knew it could not see.
    const session = { pid: 1910, project_path: '/proj-poll-rejected', state: 'active', tty: '/dev/pts/31', args: 'claude', cli_tool: 'claude' }
    ipc.listCliSessionSnapshot.mockResolvedValue({ sessions: [session], freshness: 'fresh' })

    store.startPolling({ intervalMs: 1000 })
    await vi.advanceTimersByTimeAsync(0)

    ipc.listCliSessionSnapshot.mockRejectedValue(new Error('daemon gone'))
    await vi.advanceTimersByTimeAsync(4000)

    ipc.listCliSessionSnapshot.mockResolvedValue({ sessions: [session], freshness: 'fresh' })
    await vi.advanceTimersByTimeAsync(1000)
    await vi.advanceTimersByTimeAsync(1000)

    // The interval that ended with the first failed poll is still credited —
    // the app was seeing until then. The three that follow, and the one that
    // ends with the first answer after the outage, are not.
    expect(store.getSessionStats(1910).activeMs).toBe(2000)
  })

  it('retains sessions when a poll has nothing to observe', async () => {
    // Regression: fa572d4 — with the daemon unreachable and no cache, the
    // backend answered with an empty list, which the store read as "every
    // session ended": it flushed the trackers and blanked the sidebar.
    const session = { pid: 1920, project_path: '/proj-poll-unavailable', state: 'active', tty: '/dev/pts/32', args: 'claude', cli_tool: 'claude' }
    ipc.listCliSessionSnapshot.mockResolvedValue({ sessions: [session], freshness: 'fresh' })

    store.startPolling({ intervalMs: 1000 })
    await vi.advanceTimersByTimeAsync(0)

    ipc.listCliSessionSnapshot.mockResolvedValue({ sessions: [], freshness: 'unavailable' })
    await vi.advanceTimersByTimeAsync(2000)

    expect(store.getSessionForProject('/proj-poll-unavailable')).toBeTruthy()
    expect(store.getSessionStats(1920)).toBeTruthy()
    expect(ipc.recordSessionActivity).not.toHaveBeenCalled()
  })

  it('does not credit an interval the daemon reports as unobserved', async () => {
    // Regression: fa572d4 carried the hub's `degraded` flag to the app, but a
    // blackout that started and ended between two long-poll answers left the
    // flag false on both, so the blind interval was measured as work. The hub's
    // degradation revision now travels with the answer and the bridge reports
    // the gap.
    const session = { pid: 1930, project_path: '/proj-observation-gap', state: 'active', tty: '/dev/pts/33', args: 'claude', cli_tool: 'claude' }
    ipc.listProjects.mockResolvedValue([{ id: 'proj-observation-gap-id', path: '/proj-observation-gap' }])

    store.applyDaemonSessionUpdate({ version: 1, sessions: [session] })
    vi.advanceTimersByTime(5000)

    // Scanner went blind and recovered inside one wait: healthy sessions, but
    // the interval that just ended contains a stretch nobody watched.
    store.applyDaemonSessionUpdate({ version: 1, sessions: [session], degraded: false, observation_gap: true })
    vi.advanceTimersByTime(5000)

    store.applyDaemonSessionUpdate({ version: 2, sessions: [] })
    await vi.advanceTimersByTimeAsync(0)

    expect(ipc.recordSessionActivity).toHaveBeenCalledWith(
      'proj-observation-gap-id',
      'claude',
      expect.any(String),
      expect.any(String),
      5000,
      5000,
    )
  })

  it('applies polling updates even without daemon bridge events', async () => {
    // Regression: indicators could stay stale when bridge events were absent;
    // polling fallback must still hydrate and refresh sessions.
    ipc.listClaudeSessions
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([
        {
          pid: 1800,
          project_path: '/proj-fallback',
          state: 'active',
          tty: '/dev/pts/11',
          args: 'claude',
          cli_tool: 'claude',
        },
      ])

    store.startPolling()

    await vi.advanceTimersByTimeAsync(0)
    expect(store.getSessionForProject('/proj-fallback')).toBeNull()

    await vi.advanceTimersByTimeAsync(500)
    expect(store.getSessionForProject('/proj-fallback')).toBeTruthy()
  })
})
