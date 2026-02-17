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

  // -----------------------------------------------------------------------
  // Git IPC functions
  // -----------------------------------------------------------------------

  describe('getRecentCommits()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockCommits = [{ hash: 'abc', message: 'test', author: 'dev', date: '2h' }]
      tauriCore.invoke.mockResolvedValue(mockCommits)

      const result = await ipc.getRecentCommits('p1', 5)

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_recent_commits', { projectId: 'p1', limit: 5 })
      expect(result).toEqual(mockCommits)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getRecentCommits('p1')

      expect(tauriCore.invoke).not.toHaveBeenCalled()
      expect(Array.isArray(result)).toBe(true)
      expect(result[0]).toHaveProperty('hash')
      expect(result[0]).toHaveProperty('message')
    })
  })

  describe('getAllCommits()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue([])

      await ipc.getAllCommits('p1', 20, 10)

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_all_commits', { projectId: 'p1', limit: 20, offset: 10 })
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('getGitStatus()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockStatus = { branch: 'main', is_dirty: false, ahead: 0, behind: 0 }
      tauriCore.invoke.mockResolvedValue(mockStatus)

      const result = await ipc.getGitStatus('p1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_git_status', { projectId: 'p1' })
      expect(result).toEqual(mockStatus)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getGitStatus('p1')

      expect(result).toHaveProperty('branch')
      expect(result).toHaveProperty('is_dirty')
    })
  })

  // -----------------------------------------------------------------------
  // File IPC functions
  // -----------------------------------------------------------------------

  describe('getFileTree()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockTree = [{ name: 'src', path: 'src', is_dir: true, children: [] }]
      tauriCore.invoke.mockResolvedValue(mockTree)

      const result = await ipc.getFileTree('p1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_file_tree', { projectId: 'p1' })
      expect(result).toEqual(mockTree)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getFileTree('p1')

      expect(Array.isArray(result)).toBe(true)
      expect(result[0]).toHaveProperty('name')
      expect(result[0]).toHaveProperty('is_dir')
    })
  })

  describe('readFile()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockContent = { path: 'src/main.rs', content: 'fn main() {}', language: 'rust' }
      tauriCore.invoke.mockResolvedValue(mockContent)

      const result = await ipc.readFile('p1', 'src/main.rs')

      expect(tauriCore.invoke).toHaveBeenCalledWith('read_file', { projectId: 'p1', relativePath: 'src/main.rs' })
      expect(result).toEqual(mockContent)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.readFile('p1', 'src/main.rs')

      expect(result).toHaveProperty('path')
      expect(result).toHaveProperty('content')
      expect(result).toHaveProperty('language')
    })
  })

  describe('getReadme()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockReadme = { path: 'README.md', content: '# Hello', language: 'markdown' }
      tauriCore.invoke.mockResolvedValue(mockReadme)

      const result = await ipc.getReadme('p1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_readme', { projectId: 'p1' })
      expect(result).toEqual(mockReadme)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getReadme('p1')

      expect(result).toHaveProperty('path')
      expect(result).toHaveProperty('content')
    })
  })
})
