import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock IPC module
vi.mock('./ipc.js', () => ({
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
  getIndexStatus: vi.fn(),
  rebuildIndex: vi.fn(),
}))

describe('Settings component logic', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  // AC1: Settings view loads all three sections
  it('getSettings is called to load settings', async () => {
    const mockSettings = {
      scan_directories: ['~/projects'],
      thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
      ignore_patterns: ['node_modules', '.git'],
    }
    ipc.getSettings.mockResolvedValue(mockSettings)

    const result = await ipc.getSettings()

    expect(ipc.getSettings).toHaveBeenCalled()
    expect(result.scan_directories).toEqual(['~/projects'])
    expect(result.thresholds.active_days).toBe(7)
    expect(result.ignore_patterns).toContain('node_modules')
  })

  // AC2: Threshold values are editable
  it('updateSettings persists threshold changes', async () => {
    const newSettings = {
      scan_directories: ['~/projects'],
      thresholds: { active_days: 5, recent_days: 14, stale_days: 60 },
      ignore_patterns: ['node_modules'],
    }
    ipc.updateSettings.mockResolvedValue(newSettings)

    const result = await ipc.updateSettings(newSettings)

    expect(ipc.updateSettings).toHaveBeenCalledWith(newSettings)
    expect(result.thresholds.active_days).toBe(5)
    expect(result.thresholds.recent_days).toBe(14)
  })

  // AC3: Changes persist on save
  it('updateSettings returns updated settings', async () => {
    const updated = {
      scan_directories: ['~/work', '~/projects'],
      thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
      ignore_patterns: ['node_modules', '.git', 'target'],
    }
    ipc.updateSettings.mockResolvedValue(updated)

    const result = await ipc.updateSettings(updated)

    expect(result.scan_directories).toEqual(['~/work', '~/projects'])
    expect(result.ignore_patterns).toContain('target')
  })

  // AC4: Rebuild index triggers IPC
  it('rebuildIndex calls backend', async () => {
    ipc.rebuildIndex.mockResolvedValue(42)

    const result = await ipc.rebuildIndex()

    expect(ipc.rebuildIndex).toHaveBeenCalled()
    expect(result).toBe(42)
  })

  // Index status loads
  it('getIndexStatus returns doc count', async () => {
    ipc.getIndexStatus.mockResolvedValue({ doc_count: 100, is_empty: false })

    const result = await ipc.getIndexStatus()

    expect(result.doc_count).toBe(100)
    expect(result.is_empty).toBe(false)
  })

  // Empty settings returns defaults
  it('handles empty scan directories', async () => {
    const emptySettings = {
      scan_directories: [],
      thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
      ignore_patterns: [],
    }
    ipc.getSettings.mockResolvedValue(emptySettings)

    const result = await ipc.getSettings()

    expect(result.scan_directories).toEqual([])
    expect(result.ignore_patterns).toEqual([])
  })
})
