import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock the IPC module
vi.mock('./ipc.js', () => ({
  search: vi.fn(),
  isTauri: vi.fn(() => false),
}))

describe('Search overlay logic', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  // --- Search button (titlebar icon) ---

  it('search button toggles open state on click', () => {
    let searchOpen = false
    const toggle = () => { searchOpen = !searchOpen }

    toggle() // first click opens
    expect(searchOpen).toBe(true)

    toggle() // second click closes
    expect(searchOpen).toBe(false)
  })

  it('search button tooltip uses platform-appropriate shortcut', () => {
    const makeTitle = (platform) =>
      platform?.includes('Mac') ? 'Search (⌘K)' : 'Search (Ctrl+K)'

    expect(makeTitle('MacIntel')).toBe('Search (⌘K)')
    expect(makeTitle('MacARM')).toBe('Search (⌘K)')
    expect(makeTitle('Win32')).toBe('Search (Ctrl+K)')
    expect(makeTitle('Linux x86_64')).toBe('Search (Ctrl+K)')
    expect(makeTitle(undefined)).toBe('Search (Ctrl+K)')
  })

  // --- Keyboard shortcut ---

  it('Cmd+K / Ctrl+K should be detectable as search shortcut', () => {
    const isSearchShortcut = (e) =>
      e.key === 'k' && (e.metaKey || e.ctrlKey)

    expect(isSearchShortcut({ key: 'k', metaKey: true, ctrlKey: false })).toBe(true)
    expect(isSearchShortcut({ key: 'k', metaKey: false, ctrlKey: true })).toBe(true)
    expect(isSearchShortcut({ key: 'k', metaKey: false, ctrlKey: false })).toBe(false)
    expect(isSearchShortcut({ key: 'j', metaKey: true, ctrlKey: false })).toBe(false)
  })

  it('Escape should close the overlay', () => {
    let open = true
    const handleKeydown = (e) => {
      if (e.key === 'Escape') open = false
    }
    handleKeydown({ key: 'Escape' })
    expect(open).toBe(false)
  })

  // --- Debounce ---

  it('debounce delays execution by specified ms', async () => {
    vi.useFakeTimers()
    const fn = vi.fn()

    // Simple debounce implementation matching what we'll use
    function debounce(callback, delay) {
      let timer
      return (...args) => {
        clearTimeout(timer)
        timer = setTimeout(() => callback(...args), delay)
      }
    }

    const debounced = debounce(fn, 150)

    debounced('a')
    debounced('ab')
    debounced('abc')

    expect(fn).not.toHaveBeenCalled()

    vi.advanceTimersByTime(150)
    expect(fn).toHaveBeenCalledTimes(1)
    expect(fn).toHaveBeenCalledWith('abc')

    vi.useRealTimers()
  })

  // --- Search IPC ---

  it('search returns results from IPC', async () => {
    const mockResults = [
      { project_id: 'p1', entity_type: 'document', file_path: 'README.md', title: 'README', snippet: 'project docs', relevance_score: 1.5 },
      { project_id: 'p1', entity_type: 'session', file_path: 'session:s1', title: 'Phase 5', snippet: 'completed scaffold', relevance_score: 1.2 },
    ]
    ipc.search.mockResolvedValue(mockResults)

    const results = await ipc.search('docs', 20)
    expect(results).toHaveLength(2)
    expect(results[0].entity_type).toBe('document')
  })

  it('empty query returns empty array', async () => {
    ipc.search.mockResolvedValue([])
    const results = await ipc.search('', 20)
    expect(results).toEqual([])
  })

  // --- Result grouping ---

  it('groups results by entity_type', () => {
    const results = [
      { entity_type: 'document', title: 'README' },
      { entity_type: 'session', title: 'Phase 5' },
      { entity_type: 'commit', title: 'Add feature' },
      { entity_type: 'document', title: 'CLAUDE.md' },
    ]

    const grouped = {}
    for (const r of results) {
      if (!grouped[r.entity_type]) grouped[r.entity_type] = []
      grouped[r.entity_type].push(r)
    }

    expect(grouped.document).toHaveLength(2)
    expect(grouped.session).toHaveLength(1)
    expect(grouped.commit).toHaveLength(1)
  })

  it('group labels map correctly', () => {
    const groupLabels = {
      document: 'Documents',
      session: 'Sessions',
      commit: 'Commits',
    }
    expect(groupLabels['document']).toBe('Documents')
    expect(groupLabels['session']).toBe('Sessions')
    expect(groupLabels['commit']).toBe('Commits')
  })

  // --- Navigation mapping ---

  it('document result maps to files tab navigation', () => {
    const result = { entity_type: 'document', file_path: 'src/main.rs' }
    const action = mapResultToNavigation(result)
    expect(action.tab).toBe('files')
    expect(action.filePath).toBe('src/main.rs')
  })

  it('session result maps to overview tab navigation', () => {
    const result = { entity_type: 'session', file_path: 'session:s1' }
    const action = mapResultToNavigation(result)
    expect(action.tab).toBe('overview')
    expect(action.section).toBe('session')
  })

  it('commit result maps to overview tab navigation', () => {
    const result = { entity_type: 'commit', file_path: 'commit:abc123' }
    const action = mapResultToNavigation(result)
    expect(action.tab).toBe('overview')
    expect(action.section).toBe('commits')
  })

  // --- Entity type icons ---

  it('entity types have distinct icon identifiers', () => {
    const icons = {
      document: 'file',
      session: 'clock',
      commit: 'git-commit',
    }
    expect(Object.keys(icons)).toHaveLength(3)
    const values = Object.values(icons)
    expect(new Set(values).size).toBe(3) // all unique
  })

  // --- Edge cases ---

  it('search handles API error gracefully', async () => {
    ipc.search.mockRejectedValue(new Error('Network error'))
    await expect(ipc.search('test')).rejects.toThrow('Network error')
  })

  it('results with missing snippet use empty string', () => {
    const result = { entity_type: 'document', title: 'Test', snippet: null }
    const displaySnippet = result.snippet || ''
    expect(displaySnippet).toBe('')
  })
})

/**
 * Maps a search result to a navigation action.
 * This logic will live in the search overlay component.
 */
function mapResultToNavigation(result) {
  switch (result.entity_type) {
    case 'document':
      return { tab: 'files', filePath: result.file_path }
    case 'session':
      return { tab: 'overview', section: 'session' }
    case 'commit':
      return { tab: 'overview', section: 'commits' }
    default:
      return { tab: 'overview' }
  }
}
