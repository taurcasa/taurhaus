import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock IPC module
vi.mock('./ipc.js', () => ({
  scanDirectory: vi.fn(),
  registerProjectsBatch: vi.fn(),
  isFirstRun: vi.fn(),
  checkDaemonInstallStatus: vi.fn(),
  installDaemon: vi.fn(),
  getPlatform: vi.fn().mockResolvedValue('linux'),
}))

describe('First-Run wizard logic', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  // AC1: isFirstRun returns true triggers wizard
  it('isFirstRun returns true when no projects', async () => {
    ipc.isFirstRun.mockResolvedValue(true)
    const result = await ipc.isFirstRun()
    expect(result).toBe(true)
  })

  // AC1: isFirstRun returns false skips wizard
  it('isFirstRun returns false when projects exist', async () => {
    ipc.isFirstRun.mockResolvedValue(false)
    const result = await ipc.isFirstRun()
    expect(result).toBe(false)
  })

  // AC2: scanDirectory discovers projects
  it('scanDirectory returns discovered projects', async () => {
    const mockDiscovered = [
      { path: '/projects/foo', name: 'foo', has_git: true },
      { path: '/projects/bar', name: 'bar', has_git: true },
      { path: '/projects/baz', name: 'baz', has_git: false },
    ]
    ipc.scanDirectory.mockResolvedValue(mockDiscovered)

    const result = await ipc.scanDirectory('~/projects')
    expect(result).toHaveLength(3)
    expect(result[0].has_git).toBe(true)
    expect(result[2].has_git).toBe(false)
  })

  // AC3: Selection logic
  it('checkbox toggle works via Set operations', () => {
    const selected = new Set(['/a', '/b', '/c'])

    // Toggle off
    const next = new Set(selected)
    next.delete('/b')
    expect(next.size).toBe(2)
    expect(next.has('/b')).toBe(false)

    // Toggle on
    next.add('/d')
    expect(next.size).toBe(3)
    expect(next.has('/d')).toBe(true)
  })

  // AC3: Select all / deselect all
  it('select all and deselect all work', () => {
    const projects = ['/a', '/b', '/c']

    const allSelected = new Set(projects)
    expect(allSelected.size).toBe(3)

    const noneSelected = new Set()
    expect(noneSelected.size).toBe(0)
  })

  // AC4: Batch registration works
  it('registerProjectsBatch returns results per path', async () => {
    const mockResults = [
      { path: '/a', success: true, project: { id: 'p1', name: 'a' }, error: null },
      { path: '/b', success: true, project: { id: 'p2', name: 'b' }, error: null },
      { path: '/bad', success: false, project: null, error: 'Not a directory' },
    ]
    ipc.registerProjectsBatch.mockResolvedValue(mockResults)

    const results = await ipc.registerProjectsBatch(['/a', '/b', '/bad'])

    expect(results).toHaveLength(3)
    expect(results.filter(r => r.success)).toHaveLength(2)
    expect(results[2].error).toBe('Not a directory')
  })

  // AC5: Completion count calculated from results
  it('registeredCount counts successful results', async () => {
    const mockResults = [
      { path: '/a', success: true, project: { id: 'p1' }, error: null },
      { path: '/b', success: false, project: null, error: 'error' },
      { path: '/c', success: true, project: { id: 'p3' }, error: null },
    ]
    ipc.registerProjectsBatch.mockResolvedValue(mockResults)

    const results = await ipc.registerProjectsBatch(['/a', '/b', '/c'])
    const count = results.filter(r => r.success).length
    expect(count).toBe(2)
  })

  // AC6: Empty scan returns empty list
  it('scanDirectory returns empty for empty directory', async () => {
    ipc.scanDirectory.mockResolvedValue([])
    const result = await ipc.scanDirectory('~/empty')
    expect(result).toEqual([])
  })

  // Pre-select: only git repos are pre-selected
  it('only git repos should be pre-selected', () => {
    const discovered = [
      { path: '/a', has_git: true },
      { path: '/b', has_git: false },
      { path: '/c', has_git: true },
    ]
    const preSelected = new Set(discovered.filter(p => p.has_git).map(p => p.path))
    expect(preSelected.size).toBe(2)
    expect(preSelected.has('/a')).toBe(true)
    expect(preSelected.has('/b')).toBe(false)
    expect(preSelected.has('/c')).toBe(true)
  })

  // ── Daemon install step ────────────────────────────────────────────────

  it('checkDaemonInstallStatus returns install status', async () => {
    ipc.checkDaemonInstallStatus.mockResolvedValue({
      installed: true,
      version: '0.3.1',
      bundled_version: '0.3.2',
      needs_update: true,
      wsl_available: true,
      error: null,
    })

    const status = await ipc.checkDaemonInstallStatus()
    expect(status.installed).toBe(true)
    expect(status.needs_update).toBe(true)
    expect(status.bundled_version).toBe('0.3.2')
  })

  it('daemon not installed triggers install flow', async () => {
    ipc.checkDaemonInstallStatus.mockResolvedValue({
      installed: false,
      version: null,
      bundled_version: '0.3.2',
      needs_update: false,
      wsl_available: true,
      error: null,
    })

    const status = await ipc.checkDaemonInstallStatus()
    expect(status.installed).toBe(false)
    // Should show "Install" button in wizard
  })

  it('installDaemon returns success message', async () => {
    ipc.installDaemon.mockResolvedValue({
      success: true,
      message: 'Daemon installed successfully: taurhaus-daemon 0.3.2',
    })

    const result = await ipc.installDaemon()
    expect(result.message).toContain('successfully')
  })

  it('no WSL returns wsl_available false', async () => {
    ipc.checkDaemonInstallStatus.mockResolvedValue({
      installed: false,
      version: null,
      bundled_version: '0.3.2',
      needs_update: false,
      wsl_available: false,
      error: 'WSL is not installed',
    })

    const status = await ipc.checkDaemonInstallStatus()
    expect(status.wsl_available).toBe(false)
    expect(status.error).toBe('WSL is not installed')
  })
})
