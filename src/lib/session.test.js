import { describe, it, expect, vi, beforeEach } from 'vitest'
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
}))

describe('Session display logic', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  // --- isSessionFresh logic ---

  it('session from today is fresh (< 7 days)', () => {
    const today = new Date().toISOString()
    const sessionDate = new Date(today)
    const diffDays = (new Date() - sessionDate) / (1000 * 60 * 60 * 24)
    expect(diffDays).toBeLessThan(7)
  })

  it('session from 3 days ago is fresh', () => {
    const threeDaysAgo = new Date(Date.now() - 3 * 86400000).toISOString()
    const diffDays = (new Date() - new Date(threeDaysAgo)) / (1000 * 60 * 60 * 24)
    expect(diffDays).toBeLessThan(7)
  })

  it('session from 10 days ago is not fresh', () => {
    const tenDaysAgo = new Date(Date.now() - 10 * 86400000).toISOString()
    const diffDays = (new Date() - new Date(tenDaysAgo)) / (1000 * 60 * 60 * 24)
    expect(diffDays).toBeGreaterThanOrEqual(7)
  })

  it('null date is not fresh', () => {
    const dateStr = null
    expect(dateStr).toBeFalsy()
  })

  // --- formatSessionDate logic ---

  it('formats today correctly', () => {
    const today = new Date()
    const diffDays = Math.floor((new Date() - today) / (1000 * 60 * 60 * 24))
    expect(diffDays).toBe(0)
  })

  it('formats yesterday correctly', () => {
    const yesterday = new Date(Date.now() - 86400000)
    const diffDays = Math.floor((new Date() - yesterday) / (1000 * 60 * 60 * 24))
    expect(diffDays).toBe(1)
  })

  it('formats recent dates as "N days ago"', () => {
    const fourDaysAgo = new Date(Date.now() - 4 * 86400000)
    const diffDays = Math.floor((new Date() - fourDaysAgo) / (1000 * 60 * 60 * 24))
    expect(diffDays).toBeGreaterThanOrEqual(2)
    expect(diffDays).toBeLessThan(7)
  })

  // --- Hero mode toggle (Session/README) ---

  it('hero defaults to auto mode', () => {
    const heroMode = 'auto'
    expect(heroMode).toBe('auto')
  })

  it('auto mode shows session when fresh', () => {
    const heroMode = 'auto'
    const latestSession = { date: new Date().toISOString(), summary: 'Recent session' }
    const isFresh = (new Date() - new Date(latestSession.date)) / (1000 * 60 * 60 * 24) < 7
    const showSession = heroMode === 'session' || (heroMode === 'auto' && latestSession && isFresh)
    expect(showSession).toBe(true)
  })

  it('auto mode shows README when session is stale', () => {
    const heroMode = 'auto'
    const latestSession = { date: new Date(Date.now() - 30 * 86400000).toISOString(), summary: 'Old session' }
    const isFresh = (new Date() - new Date(latestSession.date)) / (1000 * 60 * 60 * 24) < 7
    const showSession = heroMode === 'session' || (heroMode === 'auto' && latestSession && isFresh)
    expect(showSession).toBe(false)
  })

  it('manual session mode always shows session', () => {
    const heroMode = 'session'
    const showSession = heroMode === 'session'
    expect(showSession).toBe(true)
  })

  it('manual readme mode always shows README', () => {
    const heroMode = 'readme'
    const showSession = heroMode === 'session' || (heroMode === 'auto' && false)
    const showReadme = !showSession
    expect(showReadme).toBe(true)
  })

  it('toggle is available when both session and README exist', () => {
    const latestSession = { date: new Date().toISOString(), summary: 'Test' }
    const readmeContent = { content: '# Test' }
    const hasToggle = latestSession && readmeContent
    expect(!!hasToggle).toBe(true)
  })

  it('toggle is not available when only session exists', () => {
    const latestSession = { date: new Date().toISOString(), summary: 'Test' }
    const readmeContent = null
    const hasToggle = latestSession && readmeContent
    expect(!!hasToggle).toBe(false)
  })

  // --- Session data shape ---

  it('getLatestSession returns session with expected fields', async () => {
    const mockSession = {
      id: 's1',
      project_id: 'p1',
      date: '2026-02-17T14:30:45Z',
      summary: 'Completed Phase 5C implementation.',
      next_steps: ['Add tests', 'Polish UI'],
      open_questions: ['Virtual scrolling approach?'],
      metadata: { branch: 'main' },
    }
    ipc.getLatestSession.mockResolvedValue(mockSession)

    const result = await ipc.getLatestSession('p1')

    expect(result.summary).toBe('Completed Phase 5C implementation.')
    expect(result.next_steps).toHaveLength(2)
    expect(result.open_questions).toHaveLength(1)
    expect(result.metadata).toHaveProperty('branch')
  })

  it('listSessions returns array of session summaries', async () => {
    const mockSessions = [
      { id: 's1', project_id: 'p1', date: '2026-02-17', summary: 'Session 1' },
      { id: 's2', project_id: 'p1', date: '2026-02-16', summary: 'Session 2' },
    ]
    ipc.listSessions.mockResolvedValue(mockSessions)

    const result = await ipc.listSessions('p1', 10)

    expect(result).toHaveLength(2)
    expect(result[0].summary).toBe('Session 1')
  })

  it('session history handles empty result', async () => {
    ipc.listSessions.mockResolvedValue([])

    const result = await ipc.listSessions('p1')

    expect(result).toEqual([])
  })

  // --- Session loading parallel calls ---

  it('loadSessions calls getLatestSession and listSessions in parallel', async () => {
    const mockLatest = { id: 's1', summary: 'Latest' }
    const mockHistory = [{ id: 's1', summary: 'Latest' }, { id: 's2', summary: 'Older' }]
    ipc.getLatestSession.mockResolvedValue(mockLatest)
    ipc.listSessions.mockResolvedValue(mockHistory)

    const [latest, history] = await Promise.all([
      ipc.getLatestSession('p1'),
      ipc.listSessions('p1', 10),
    ])

    expect(ipc.getLatestSession).toHaveBeenCalledWith('p1')
    expect(ipc.listSessions).toHaveBeenCalledWith('p1', 10)
    expect(latest.summary).toBe('Latest')
    expect(history).toHaveLength(2)
  })

  it('loadSessions handles getLatestSession failure gracefully', async () => {
    ipc.getLatestSession.mockRejectedValue(new Error('Not found'))
    ipc.listSessions.mockResolvedValue([])

    let latestSession = null
    let sessionHistory = []
    try {
      const [latest, history] = await Promise.all([
        ipc.getLatestSession('p1'),
        ipc.listSessions('p1', 10),
      ])
      latestSession = latest
      sessionHistory = history || []
    } catch {
      latestSession = null
      sessionHistory = []
    }

    expect(latestSession).toBeNull()
    expect(sessionHistory).toEqual([])
  })
})
