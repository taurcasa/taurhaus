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

  describe('listDirectory()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockEntries = [{ name: 'src', path: '/p/src', isExpandable: true }]
      tauriCore.invoke.mockResolvedValue(mockEntries)

      const result = await ipc.listDirectory('/p')

      expect(tauriCore.invoke).toHaveBeenCalledWith('list_directory', { path: '/p' })
      expect(result).toEqual(mockEntries)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.listDirectory('~/projects')

      expect(Array.isArray(result)).toBe(true)
      expect(result[0]).toHaveProperty('name')
      expect(result[0]).toHaveProperty('isExpandable')
    })
  })

  describe('validateProjectPath()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockValidation = { exists: true, isGitRepo: true, isRegistered: false }
      tauriCore.invoke.mockResolvedValue(mockValidation)

      const result = await ipc.validateProjectPath('/some/path')

      expect(tauriCore.invoke).toHaveBeenCalledWith('validate_project_path', { path: '/some/path' })
      expect(result).toEqual(mockValidation)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.validateProjectPath('/some/path')

      expect(result).toHaveProperty('exists')
      expect(result).toHaveProperty('isGitRepo')
      expect(result).toHaveProperty('isRegistered')
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

  // -----------------------------------------------------------------------
  // Session IPC functions
  // -----------------------------------------------------------------------

  describe('getLatestSession()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockSession = { id: 's1', project_id: 'p1', date: '2026-02-17', summary: 'Test' }
      tauriCore.invoke.mockResolvedValue(mockSession)

      const result = await ipc.getLatestSession('p1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_latest_session', { projectId: 'p1' })
      expect(result).toEqual(mockSession)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getLatestSession('p1')

      expect(result).toHaveProperty('id')
      expect(result).toHaveProperty('summary')
      expect(result).toHaveProperty('next_steps')
    })
  })

  describe('listSessions()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockSessions = [{ id: 's1', project_id: 'p1', date: '2026-02-17', summary: 'Test' }]
      tauriCore.invoke.mockResolvedValue(mockSessions)

      const result = await ipc.listSessions('p1', 10, 5)

      expect(tauriCore.invoke).toHaveBeenCalledWith('list_sessions', { projectId: 'p1', limit: 10, offset: 5 })
      expect(result).toEqual(mockSessions)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.listSessions('p1')

      expect(Array.isArray(result)).toBe(true)
      expect(result.length).toBeGreaterThan(0)
      expect(result[0]).toHaveProperty('summary')
    })
  })

  describe('getSession()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockSession = { id: 's1', project_id: 'p1', date: '2026-02-17', summary: 'Test' }
      tauriCore.invoke.mockResolvedValue(mockSession)

      const result = await ipc.getSession('s1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_session', { sessionId: 's1' })
      expect(result).toEqual(mockSession)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getSession('s1')

      expect(result).toHaveProperty('id')
      expect(result).toHaveProperty('summary')
    })
  })

  // -----------------------------------------------------------------------
  // Search IPC functions
  // -----------------------------------------------------------------------

  describe('search()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockResults = [{ project_id: 'p1', entity_type: 'document', file_path: 'README.md', title: 'README', snippet: 'test', relevance_score: 1.0 }]
      tauriCore.invoke.mockResolvedValue(mockResults)

      const result = await ipc.search('test query', 10)

      expect(tauriCore.invoke).toHaveBeenCalledWith('search', { query: 'test query', limit: 10 })
      expect(result).toEqual(mockResults)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.search('README')

      expect(Array.isArray(result)).toBe(true)
      expect(result.length).toBeGreaterThan(0)
      expect(result[0]).toHaveProperty('entity_type')
      expect(result[0]).toHaveProperty('snippet')
    })

    it('returns empty for empty query in mock mode', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.search('')

      expect(result).toEqual([])
    })
  })

  describe('getIndexStatus()', () => {
    it('calls invoke with correct command', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockStatus = { doc_count: 100, is_empty: false }
      tauriCore.invoke.mockResolvedValue(mockStatus)

      const result = await ipc.getIndexStatus()

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_index_status')
      expect(result).toEqual(mockStatus)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getIndexStatus()

      expect(result).toHaveProperty('doc_count')
      expect(result).toHaveProperty('is_empty')
    })
  })

  describe('rebuildIndex()', () => {
    it('calls invoke with correct command', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(42)

      const result = await ipc.rebuildIndex()

      expect(tauriCore.invoke).toHaveBeenCalledWith('rebuild_index')
      expect(result).toBe(42)
      delete window.__TAURI_INTERNALS__
    })
  })

  // -----------------------------------------------------------------------
  // Relationship IPC
  // -----------------------------------------------------------------------

  describe('getRelationships()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockRels = [{ id: 'r1', source_project_id: 'p1', target_project_id: 'p2' }]
      tauriCore.invoke.mockResolvedValue(mockRels)

      const result = await ipc.getRelationships('p1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_relationships', { projectId: 'p1' })
      expect(result).toEqual(mockRels)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getRelationships('p1')

      expect(Array.isArray(result)).toBe(true)
      expect(result.length).toBeGreaterThan(0)
      expect(result[0]).toHaveProperty('relationship_type')
    })
  })

  describe('dismissRelationship()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(undefined)

      await ipc.dismissRelationship('r1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('dismiss_relationship', { relationshipId: 'r1' })
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('createRelationship()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockRel = { id: 'r-new', source_project_id: 'p1', target_project_id: 'p2' }
      tauriCore.invoke.mockResolvedValue(mockRel)

      const result = await ipc.createRelationship('p1', 'p2', 'depends_on')

      expect(tauriCore.invoke).toHaveBeenCalledWith('create_relationship', {
        sourceId: 'p1',
        targetId: 'p2',
        relationshipType: 'depends_on',
      })
      expect(result).toEqual(mockRel)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.createRelationship('p1', 'p2', 'depends_on')

      expect(result).toHaveProperty('id')
      expect(result.detection_source).toBe('manual')
      expect(result.relationship_type).toBe('depends_on')
    })
  })

  describe('removeRelationship()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(undefined)

      await ipc.removeRelationship('r1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('remove_relationship', { relationshipId: 'r1' })
      delete window.__TAURI_INTERNALS__
    })
  })

  // -----------------------------------------------------------------------
  // Settings IPC functions
  // -----------------------------------------------------------------------

  describe('getSettings()', () => {
    it('calls invoke with correct command in Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockSettings = { scan_directories: ['~/projects'], thresholds: { active_days: 7, recent_days: 30, stale_days: 90 }, ignore_patterns: [] }
      tauriCore.invoke.mockResolvedValue(mockSettings)

      const result = await ipc.getSettings()

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_settings')
      expect(result).toEqual(mockSettings)
      delete window.__TAURI_INTERNALS__
    })

    it('returns mock settings when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getSettings()

      expect(result).toHaveProperty('scan_directories')
      expect(result).toHaveProperty('thresholds')
      expect(result.thresholds).toHaveProperty('active_days')
    })
  })

  describe('updateSettings()', () => {
    it('calls invoke with settings in Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      const newSettings = { scan_directories: ['~/work'], thresholds: { active_days: 5, recent_days: 14, stale_days: 60 }, ignore_patterns: [] }
      tauriCore.invoke.mockResolvedValue(newSettings)

      const result = await ipc.updateSettings(newSettings)

      expect(tauriCore.invoke).toHaveBeenCalledWith('update_settings', { settings: newSettings })
      expect(result).toEqual(newSettings)
      delete window.__TAURI_INTERNALS__
    })

    it('returns merged mock settings when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.updateSettings({ scan_directories: ['~/work'] })

      expect(result.scan_directories).toEqual(['~/work'])
      expect(result).toHaveProperty('thresholds')
    })
  })

  describe('isFirstRun()', () => {
    it('calls is_first_run command in Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(true)

      const result = await ipc.isFirstRun()

      expect(tauriCore.invoke).toHaveBeenCalledWith('is_first_run')
      expect(result).toBe(true)
      delete window.__TAURI_INTERNALS__
    })

    it('returns false in mock mode (mock projects exist)', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.isFirstRun()

      // MOCK_PROJECTS has 10 items, so isFirstRun returns false
      expect(result).toBe(false)
    })
  })

  // -----------------------------------------------------------------------
  // Batch Registration IPC functions
  // -----------------------------------------------------------------------

  describe('registerProjectsBatch()', () => {
    it('calls invoke with paths in Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockResults = [
        { path: '/a', success: true, project: { id: 'p1', name: 'a' }, error: null },
        { path: '/b', success: true, project: { id: 'p2', name: 'b' }, error: null },
      ]
      tauriCore.invoke.mockResolvedValue(mockResults)

      const result = await ipc.registerProjectsBatch(['/a', '/b'])

      expect(tauriCore.invoke).toHaveBeenCalledWith('register_projects_batch', { paths: ['/a', '/b'] })
      expect(result).toEqual(mockResults)
      delete window.__TAURI_INTERNALS__
    })

    it('returns mock results when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.registerProjectsBatch(['/a', '/b', '/c'])

      expect(result).toHaveLength(3)
      expect(result[0].success).toBe(true)
      expect(result[0].path).toBe('/a')
      expect(result[0].project).toHaveProperty('id')
      expect(result[1].path).toBe('/b')
      expect(result[2].path).toBe('/c')
    })
  })
})
