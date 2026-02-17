import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock @tauri-apps/api/core — must be before importing ipc module
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

describe('ipc module', () => {
  let ipc
  let tauriCore

  beforeEach(async () => {
    vi.resetModules()
    vi.clearAllMocks()
    delete window.__TAURI_INTERNALS__
    // Re-import after reset so each test gets fresh state
    tauriCore = await import('@tauri-apps/api/core')
    ipc = await import('./ipc.js')
  })

  describe('isTauri()', () => {
    it('returns true when __TAURI_INTERNALS__ exists', () => {
      window.__TAURI_INTERNALS__ = {}
      expect(ipc.isTauri()).toBe(true)
      delete window.__TAURI_INTERNALS__
    })

    it('returns false when __TAURI_INTERNALS__ is absent', () => {
      delete window.__TAURI_INTERNALS__
      expect(ipc.isTauri()).toBe(false)
    })
  })

  describe('listProjects()', () => {
    it('calls invoke with correct command name', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockProjects = [
        { id: 'p1', name: 'test', path: '/test', activity_state: 'active', last_activity_at: null, branch: null, is_dirty: null },
      ]
      tauriCore.invoke.mockResolvedValue(mockProjects)

      const result = await ipc.listProjects()

      expect(tauriCore.invoke).toHaveBeenCalledWith('list_projects')
      expect(result).toEqual(mockProjects)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.listProjects()

      expect(tauriCore.invoke).not.toHaveBeenCalled()
      expect(Array.isArray(result)).toBe(true)
      expect(result.length).toBeGreaterThan(0)
      // Mock data should have the expected shape
      expect(result[0]).toHaveProperty('id')
      expect(result[0]).toHaveProperty('name')
      expect(result[0]).toHaveProperty('activity_state')
    })
  })

  describe('getProject()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockDetail = {
        id: 'p1', name: 'test', path: '/test',
        description: null, activity_state: 'active',
        last_activity_at: null, hero_preference: null,
        created_at: '', updated_at: '',
        branch: null, is_dirty: null,
      }
      tauriCore.invoke.mockResolvedValue(mockDetail)

      const result = await ipc.getProject('p1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_project', { projectId: 'p1' })
      expect(result).toEqual(mockDetail)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getProject('mock-1')

      expect(tauriCore.invoke).not.toHaveBeenCalled()
      expect(result).toHaveProperty('id')
      expect(result).toHaveProperty('name')
      expect(result).toHaveProperty('description')
      expect(result).toHaveProperty('activity_state')
    })
  })

  describe('registerProject()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockDetail = { id: 'p1', name: 'new', path: '/new' }
      tauriCore.invoke.mockResolvedValue(mockDetail)

      const result = await ipc.registerProject('/new', 'new')

      expect(tauriCore.invoke).toHaveBeenCalledWith('register_project', { path: '/new', name: 'new' })
      expect(result).toEqual(mockDetail)
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('updateProject()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockDetail = { id: 'p1', name: 'renamed' }
      tauriCore.invoke.mockResolvedValue(mockDetail)

      const result = await ipc.updateProject('p1', { name: 'renamed' })

      expect(tauriCore.invoke).toHaveBeenCalledWith('update_project', {
        projectId: 'p1',
        fields: { name: 'renamed' },
      })
      expect(result).toEqual(mockDetail)
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('removeProject()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(undefined)

      await ipc.removeProject('p1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('remove_project', { projectId: 'p1' })
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('scanDirectory()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockDiscovered = [{ path: '/a', name: 'a', hasGit: true }]
      tauriCore.invoke.mockResolvedValue(mockDiscovered)

      const result = await ipc.scanDirectory('/projects')

      expect(tauriCore.invoke).toHaveBeenCalledWith('scan_directory', { path: '/projects' })
      expect(result).toEqual(mockDiscovered)
      delete window.__TAURI_INTERNALS__
    })
  })
})
