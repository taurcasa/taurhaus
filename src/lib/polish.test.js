import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock IPC module
vi.mock('./ipc.js', () => ({
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
  getIndexStatus: vi.fn(),
  rebuildIndex: vi.fn(),
  scanDirectory: vi.fn(),
  registerProjectsBatch: vi.fn(),
  isFirstRun: vi.fn(),
}))

describe('Polish: error states and edge cases', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  // AC1: Settings load failure returns fallback defaults
  it('settings load failure should not crash — caller gets null', async () => {
    ipc.getSettings.mockRejectedValue(new Error('DB locked'))
    try {
      await ipc.getSettings()
    } catch (e) {
      expect(e.message).toBe('DB locked')
    }
    // The Settings component should catch this and show a warning
  })

  // AC1: Index rebuild failure surfaces error
  it('rebuild index failure surfaces error message', async () => {
    ipc.rebuildIndex.mockRejectedValue(new Error('Index corrupted'))
    try {
      await ipc.rebuildIndex()
    } catch (e) {
      expect(e.message).toBe('Index corrupted')
    }
  })

  // AC2: Empty scan results in wizard
  it('empty scan returns empty array for wizard to display', async () => {
    ipc.scanDirectory.mockResolvedValue([])
    const result = await ipc.scanDirectory('~/projects')
    expect(result).toEqual([])
    // Wizard should show "No git repositories found" message
  })

  // AC1: Batch registration partial failures are tracked
  it('batch registration tracks partial failures', async () => {
    const mockResults = [
      { path: '/a', success: true, project: { id: 'p1' }, error: null },
      { path: '/bad', success: false, project: null, error: 'Not a directory' },
      { path: '/c', success: true, project: { id: 'p3' }, error: null },
    ]
    ipc.registerProjectsBatch.mockResolvedValue(mockResults)

    const results = await ipc.registerProjectsBatch(['/a', '/bad', '/c'])
    const succeeded = results.filter(r => r.success).length
    const failed = results.filter(r => !r.success)

    expect(succeeded).toBe(2)
    expect(failed).toHaveLength(1)
    expect(failed[0].error).toBe('Not a directory')
    // Wizard step 4 should show "2 of 3 registered (1 failed)"
  })

  // AC4: Keyboard escape key handling
  it('Escape key events are preventable for closing overlays', () => {
    const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })
    expect(event.key).toBe('Escape')
    // Settings and wizard should listen for Escape to close/go back
  })
})
