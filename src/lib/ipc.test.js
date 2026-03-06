import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock @tauri-apps/api/core — must be before importing ipc module
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

describe('ipc module', () => {
  let ipc
  let tauriCore
  let tauriEvent

  beforeEach(async () => {
    vi.resetModules()
    vi.clearAllMocks()
    delete window.__TAURI_INTERNALS__
    // Re-import after reset so each test gets fresh state
    tauriCore = await import('@tauri-apps/api/core')
    tauriEvent = await import('@tauri-apps/api/event')
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
        { id: 'p1', name: 'test', path: '/test', activityState: 'active', lastActivityAt: null, branch: null, isDirty: null },
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
      expect(result[0]).toHaveProperty('activityState')
    })

    it('preserves camelCase project fields from Tauri payloads', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue([
        {
          id: 'p1',
          name: 'test',
          path: '/test',
          activityState: 'active',
          lastActivityAt: '2026-03-05T12:00:00Z',
          isDirty: true,
        },
      ])

      const result = await ipc.listProjects()

      expect(result).toHaveLength(1)
      expect(result[0].activityState).toBe('active')
      expect(result[0].lastActivityAt).toBe('2026-03-05T12:00:00Z')
      expect(result[0].isDirty).toBe(true)
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('getProject()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockDetail = {
        id: 'p1', name: 'test', path: '/test',
        description: null, activityState: 'active',
        lastActivityAt: null, heroPreference: null,
        createdAt: '', updatedAt: '',
        branch: null, isDirty: null,
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
      expect(result).toHaveProperty('activityState')
    })

    it('preserves camelCase project detail fields from Tauri payloads', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        id: 'p1',
        name: 'test',
        path: '/test',
        activityState: 'recent',
        lastActivityAt: '2026-03-05T12:00:00Z',
        heroPreference: 'builder',
        isDirty: false,
      })

      const result = await ipc.getProject('p1')

      expect(result.activityState).toBe('recent')
      expect(result.lastActivityAt).toBe('2026-03-05T12:00:00Z')
      expect(result.heroPreference).toBe('builder')
      expect(result.isDirty).toBe(false)
      delete window.__TAURI_INTERNALS__
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

  describe('createProject()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockDetail = { id: 'p2', name: 'new-app', path: '/projects/new-app' }
      tauriCore.invoke.mockResolvedValue(mockDetail)

      const result = await ipc.createProject('new-app', '/projects')

      expect(tauriCore.invoke).toHaveBeenCalledWith('create_project', {
        name: 'new-app',
        parentDir: '/projects',
      })
      expect(result).toEqual(mockDetail)
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__

      const result = await ipc.createProject('new-app', '/projects')

      expect(result).toMatchObject({
        name: 'new-app',
        path: '/projects/new-app',
      })
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
      const mockStatus = { branch: 'main', isDirty: false, ahead: 0, behind: 0 }
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
      expect(result).toHaveProperty('isDirty')
    })
  })

  describe('getRemoteUrl()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue('https://github.com/org/repo')

      const result = await ipc.getRemoteUrl('p1')

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_remote_url', { projectId: 'p1' })
      expect(result).toBe('https://github.com/org/repo')
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to null when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getRemoteUrl('p1')

      expect(result).toBeNull()
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

  describe('checkPathType()', () => {
    it('calls invoke with correct command and args', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue('directory')

      const result = await ipc.checkPathType('p1', 'docs')

      expect(tauriCore.invoke).toHaveBeenCalledWith('check_path_type', {
        projectId: 'p1',
        relativePath: 'docs',
      })
      expect(result).toBe('directory')
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to not_found when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__

      const result = await ipc.checkPathType('p1', 'docs')

      expect(tauriCore.invoke).not.toHaveBeenCalled()
      expect(result).toBe('not_found')
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

    it('returns null for missing project in mock mode', async () => {
      delete window.__TAURI_INTERNALS__
      await expect(ipc.getReadme('missing-project')).resolves.toBeNull()
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
      expect(result).toMatchObject(mockSession)
      expect(result.next_steps).toEqual([])
      expect(result.open_questions).toEqual([])
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to mock data when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getLatestSession('p1')

      expect(result).toHaveProperty('id')
      expect(result).toHaveProperty('summary')
      expect(result).toHaveProperty('next_steps')
    })

    it('returns null for missing project in mock mode', async () => {
      delete window.__TAURI_INTERNALS__
      await expect(ipc.getLatestSession('missing-project')).resolves.toBeNull()
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
      expect(result).toMatchObject(mockSession)
      expect(result.next_steps).toEqual([])
      expect(result.open_questions).toEqual([])
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
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject(mockRels[0])
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
      expect(result).toMatchObject(mockRel)
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
      expect(result).toMatchObject(mockSettings)
      delete window.__TAURI_INTERNALS__
    })

    it('normalizes camelCase settings from Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        scanDirectories: ['~/work'],
        thresholds: { activeDays: 5, recentDays: 14, staleDays: 60 },
        ignorePatterns: ['node_modules'],
        darkMode: true,
        codeTheme: { light: 'solarized-light', dark: 'one-dark-pro' },
        daemon: { port: 17233, path: '/tmp/daemon', autoStart: false },
        terminal: {
          emulator: 'default',
          customCommand: '',
          tmuxLayout: 'new_window',
          cliCommands: {
            claude: { continueCmd: 'claude --continue', fresh: 'claude', resume: 'claude --resume' },
            codex: { continueCmd: 'codex resume --last --yolo', fresh: 'codex --yolo', resume: 'codex resume --yolo' },
            gemini: { continueCmd: 'gemini --resume', fresh: 'gemini', resume: 'gemini --resume' },
          },
        },
      })

      const result = await ipc.getSettings()

      expect(result.scan_directories).toEqual(['~/work'])
      expect(result.thresholds).toEqual({ active_days: 5, recent_days: 14, stale_days: 60 })
      expect(result.ignore_patterns).toEqual(['node_modules'])
      expect(result.dark_mode).toBe(true)
      expect(result.code_theme).toEqual({ light: 'solarized-light', dark: 'one-dark-pro' })
      expect(result.daemon.auto_start).toBe(false)
      expect(result.terminal.custom_command).toBe('')
      expect(result.terminal.tmux_layout).toBe('new_window')
      expect(result.terminal.cli_commands.claude.continue_cmd).toBe('claude --continue')
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
      expect(result).toMatchObject(newSettings)
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

  // -----------------------------------------------------------------------
  // Command Center — Claude Code session management
  // -----------------------------------------------------------------------

  describe('listClaudeSessions()', () => {
    it('calls invoke with correct command in Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockSessions = [{ pid: 123, project_path: '/test', state: 'active' }]
      tauriCore.invoke.mockResolvedValue(mockSessions)

      const result = await ipc.listClaudeSessions()

      expect(tauriCore.invoke).toHaveBeenCalledWith('list_cli_sessions')
      expect(result).toEqual(mockSessions)
      delete window.__TAURI_INTERNALS__
    })

    it('returns mock sessions when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.listClaudeSessions()

      expect(Array.isArray(result)).toBe(true)
      expect(result.length).toBeGreaterThan(0)
      expect(result[0]).toHaveProperty('pid')
      expect(result[0]).toHaveProperty('state')
      expect(result[0]).toHaveProperty('project_path')
    })
  })

  describe('launchClaudeSession()', () => {
    it('calls invoke with project ID and mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockResult = { tmux_window: 'proj', tmux_pane: '%5' }
      tauriCore.invoke.mockResolvedValue(mockResult)

      const result = await ipc.launchClaudeSession('p1', 'continue')

      expect(tauriCore.invoke).toHaveBeenCalledWith('launch_cli_session', {
        projectId: 'p1',
        mode: 'continue',
        cliTool: null,
      })
      expect(result).toEqual(mockResult)
      delete window.__TAURI_INTERNALS__
    })

    it('returns mock result when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.launchClaudeSession('p1', 'fresh')

      expect(result).toHaveProperty('tmux_window')
      expect(result).toHaveProperty('tmux_pane')
    })
  })

  describe('stopClaudeSession()', () => {
    it('calls invoke with tmux pane ID', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(undefined)

      await ipc.stopClaudeSession('%3')

      expect(tauriCore.invoke).toHaveBeenCalledWith('stop_cli_session', { tmuxPane: '%3', cliTool: null })
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('navigateToSession()', () => {
    it('calls invoke with tmux coordinates and openTerminal=false by default', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(undefined)

      await ipc.navigateToSession('taurhaus', '1', '%3')

      expect(tauriCore.invoke).toHaveBeenCalledWith('navigate_to_session', {
        tmuxSession: 'taurhaus',
        tmuxWindow: '1',
        tmuxPane: '%3',
        openTerminal: false,
      })
      delete window.__TAURI_INTERNALS__
    })

    it('passes openTerminal=true when requested', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(undefined)

      await ipc.navigateToSession('taurhaus', '1', '%3', true)

      expect(tauriCore.invoke).toHaveBeenCalledWith('navigate_to_session', {
        tmuxSession: 'taurhaus',
        tmuxWindow: '1',
        tmuxPane: '%3',
        openTerminal: true,
      })
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('task IPC contract fields', () => {
    it('normalizes camelCase commit diff fields from Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue([
        {
          oldStart: 1,
          oldLines: 2,
          newStart: 1,
          newLines: 3,
          lines: [{ origin: '+', content: 'line', oldLineno: null, newLineno: 1 }],
        },
      ])

      const result = await ipc.getCommitDiff('/tmp/project', 'abc', 'src/main.rs')

      expect(result[0].old_start).toBe(1)
      expect(result[0].new_start).toBe(1)
      expect(result[0].lines[0].old_lineno).toBeNull()
      expect(result[0].lines[0].new_lineno).toBe(1)
      delete window.__TAURI_INTERNALS__
    })

    it('normalizes camelCase commits-in-range fields from Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        commits: [],
        files: [],
        truncated: true,
        totalCount: 7,
      })

      const result = await ipc.getCommitsInRange('/tmp/project', 'abc', 'def')

      expect(result.truncated).toBe(true)
      expect(result.total_count).toBe(7)
      delete window.__TAURI_INTERNALS__
    })

    it('getProjectTasks mock includes source_outcomes', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getProjectTasks('/tmp/project')

      expect(result).toHaveProperty('tasks')
      expect(result).toHaveProperty('source_outcomes')
      expect(Array.isArray(result.source_outcomes)).toBe(true)
    })

    it('getCommitsInRange mock includes truncated and total_count', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getCommitsInRange('/tmp/project', 'abc', 'def')

      expect(result).toHaveProperty('commits')
      expect(result).toHaveProperty('files')
      expect(result).toHaveProperty('truncated')
      expect(result).toHaveProperty('total_count')
    })

    it('surfaces structured IPC error metadata for task commands', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockRejectedValueOnce({
        code: 'NOT_FOUND',
        message: 'Task not found',
        command: 'get_task_detail',
        retryable: false,
      })

      try {
        await ipc.getTaskDetail('/tmp/project', 'missing', 'claude', 'session-x')
        throw new Error('expected getTaskDetail to reject')
      } catch (error) {
        expect(error.message).toBe('Task not found')
        expect(error.code).toBe('NOT_FOUND')
        expect(error.command).toBe('get_task_detail')
        expect(error.retryable).toBe(false)
      }
      delete window.__TAURI_INTERNALS__
    })
  })

  // ── Platform + Daemon install commands ──────────────────────────────────

  describe('getDaemonStatus()', () => {
    it('returns normalized status shape in non-Tauri mode', async () => {
      const result = await ipc.getDaemonStatus()
      expect(result).toEqual({
        status: 'connected',
        version: null,
        protocol_version: 0,
        expected_protocol_version: 0,
        uptime_secs: null,
        port: 17233,
        wsl_distro: null,
      })
    })

    it('normalizes camelCase daemon status fields from Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        status: 'connected',
        version: '0.3.2',
        protocolVersion: 4,
        expectedProtocolVersion: 4,
        uptimeSecs: 15,
        port: 17233,
        wslDistro: 'Ubuntu',
      })

      const result = await ipc.getDaemonStatus()

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_daemon_status')
      expect(result).toEqual({
        status: 'connected',
        version: '0.3.2',
        protocol_version: 4,
        expected_protocol_version: 4,
        uptime_secs: 15,
        port: 17233,
        wsl_distro: 'Ubuntu',
      })
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('getPlatform()', () => {
    it('returns mock linux in non-Tauri mode', async () => {
      const result = await ipc.getPlatform()
      expect(result).toBe('linux')
    })

    it('calls get_platform in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue('macos')

      const result = await ipc.getPlatform()

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_platform')
      expect(result).toBe('macos')
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('startDaemon()', () => {
    it('returns mock success in non-Tauri mode', async () => {
      const result = await ipc.startDaemon()
      expect(result).toContain('Daemon started')
    })

    it('calls start_daemon in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue('Daemon started')

      const result = await ipc.startDaemon()

      expect(tauriCore.invoke).toHaveBeenCalledWith('start_daemon')
      expect(result).toContain('Daemon started')
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('checkDaemonInstallStatus()', () => {
    it('returns mock data in non-Tauri mode', async () => {
      const result = await ipc.checkDaemonInstallStatus()
      expect(result).toEqual({
        installed: true,
        version: '0.3.1',
        bundled_version: '0.3.1',
        needs_update: false,
        wsl_available: true,
        error: null,
      })
    })

    it('calls check_daemon_install_status in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        installed: false,
        version: null,
        bundled_version: '0.3.2',
        needs_update: false,
        wsl_available: true,
        error: null,
      })

      const result = await ipc.checkDaemonInstallStatus()

      expect(tauriCore.invoke).toHaveBeenCalledWith('check_daemon_install_status')
      expect(result.installed).toBe(false)
      delete window.__TAURI_INTERNALS__
    })

    it('normalizes camelCase daemon install fields from Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        installed: true,
        version: '0.3.2',
        bundledVersion: '0.3.3',
        needsUpdate: true,
        wslAvailable: true,
        error: null,
      })

      const result = await ipc.checkDaemonInstallStatus()

      expect(result).toEqual({
        installed: true,
        version: '0.3.2',
        bundled_version: '0.3.3',
        needs_update: true,
        wsl_available: true,
        error: null,
      })
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('installDaemon()', () => {
    it('returns mock success in non-Tauri mode', async () => {
      const result = await ipc.installDaemon()
      expect(result).toEqual({
        success: true,
        message: 'Daemon installed successfully: taurhaus-daemon 0.3.1',
      })
    })

    it('calls install_daemon in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        success: true,
        message: 'Daemon installed successfully: taurhaus-daemon 0.3.2',
      })

      const result = await ipc.installDaemon()

      expect(tauriCore.invoke).toHaveBeenCalledWith('install_daemon')
      expect(result.message).toContain('0.3.2')
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('checkMeshInstallStatus()', () => {
    it('returns mock data in non-Tauri mode', async () => {
      const result = await ipc.checkMeshInstallStatus()
      expect(result).toEqual({
        installed: true,
        version: '0.1.0',
        bundled_version: '0.1.0',
        needs_update: false,
        environment_available: true,
        error: null,
      })
    })

    it('calls check_mesh_install_status in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        installed: false,
        version: null,
        bundled_version: '0.1.1',
        needs_update: false,
        environment_available: true,
        error: null,
      })

      const result = await ipc.checkMeshInstallStatus()

      expect(tauriCore.invoke).toHaveBeenCalledWith('check_mesh_install_status')
      expect(result.installed).toBe(false)
      delete window.__TAURI_INTERNALS__
    })

    it('normalizes camelCase mesh install fields from Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        installed: true,
        version: '0.1.0',
        bundledVersion: '0.1.1',
        needsUpdate: true,
        environmentAvailable: true,
        error: null,
      })

      const result = await ipc.checkMeshInstallStatus()

      expect(result).toEqual({
        installed: true,
        version: '0.1.0',
        bundled_version: '0.1.1',
        needs_update: true,
        environment_available: true,
        error: null,
      })
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('installMesh()', () => {
    it('returns mock success in non-Tauri mode', async () => {
      const result = await ipc.installMesh()
      expect(result).toEqual({
        success: true,
        message: 'Mesh installed successfully: mesh 0.1.0',
      })
    })

    it('calls install_mesh in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        success: true,
        message: 'Mesh installed successfully: mesh 0.1.1',
      })

      const result = await ipc.installMesh()

      expect(tauriCore.invoke).toHaveBeenCalledWith('install_mesh')
      expect(result.message).toContain('0.1.1')
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('listRoleTemplates()', () => {
    it('maps role defaults.cli_tool/defaults.model into top-level fields in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce([
        {
          roleId: 'codex-developer',
          name: 'Codex Developer',
          version: '1.0.0',
          kind: 'agent',
          source: 'built_in',
          readOnly: true,
          defaults: {
            cliTool: 'codex',
            model: 'gpt-5.4 high',
          },
          capabilities: ['implementation'],
        },
      ])

      const result = await ipc.listRoleTemplates()

      expect(tauriCore.invoke).toHaveBeenCalledTimes(1)
      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_list_roles_full')
      expect(result).toEqual([
        expect.objectContaining({
          roleId: 'codex-developer',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          capabilities: ['implementation'],
          builtIn: true,
          readOnly: true,
        }),
      ])
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('listTeamPresets()', () => {
    it('normalizes aliased preset fields in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce([
        {
          presetId: 'review-team',
          name: 'Review Team',
          leadRoleId: 'claude-reviewer',
          source: 'built_in',
          readOnly: true,
          agentSlots: [{ roleId: 'codex-developer', count: 2 }],
        },
      ])

      const result = await ipc.listTeamPresets()

      expect(tauriCore.invoke).toHaveBeenCalledTimes(1)
      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_list_presets_full')
      expect(result).toEqual([
        expect.objectContaining({
          presetId: 'review-team',
          leadRoleId: 'claude-reviewer',
          roleCount: 1,
          agentCount: 2,
          builtIn: true,
          readOnly: true,
        }),
      ])

      delete window.__TAURI_INTERNALS__
    })
  })

  describe('template mapping edge cases', () => {
    it('listRoleTemplates handles nullable fields and array guards', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce([
        {
          roleId: 'role-a',
          name: 'Role A',
          source: 'CUSTOM',
          readOnly: true,
          cliTool: 'gemini',
          defaults: { model: 'gemini-3.1-pro' },
          capabilities: 'not-an-array',
        },
      ])

      const result = await ipc.listRoleTemplates()

      expect(result).toEqual([
        expect.objectContaining({
          roleId: 'role-a',
          cliTool: 'gemini',
          model: 'gemini-3.1-pro',
          capabilities: [],
          builtIn: false,
          readOnly: true,
        }),
      ])
      delete window.__TAURI_INTERNALS__
    })

    it('listRoleTemplates returns empty list when backend returns null', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce(null)

      const result = await ipc.listRoleTemplates()
      expect(result).toEqual([])

      delete window.__TAURI_INTERNALS__
    })

    it('listTeamPresets normalizes role counts and array guards', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce([
        {
          presetId: 'preset-a',
          leadRoleId: 'lead-1',
          source: 'built_in',
          readOnly: true,
          agentSlots: [{ roleId: 'r1', count: 3 }],
          tools: 'oops',
          capabilities: null,
        },
      ])

      const result = await ipc.listTeamPresets()

      expect(result).toEqual([
        expect.objectContaining({
          presetId: 'preset-a',
          leadRoleId: 'lead-1',
          roleCount: 1,
          agentCount: 3,
          tools: [],
          capabilities: [],
          builtIn: true,
          readOnly: true,
        }),
      ])
      delete window.__TAURI_INTERNALS__
    })

    it('getRoleTemplate and getTeamPreset return null for unknown IDs in mock mode', async () => {
      delete window.__TAURI_INTERNALS__

      await expect(ipc.getRoleTemplate('missing-role')).resolves.toBeNull()
      await expect(ipc.getTeamPreset('missing-preset')).resolves.toBeNull()
    })
  })

  describe('role/preset CRUD template wrappers', () => {
    it('upsertRoleTemplate returns sensible mock defaults when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__

      const result = await ipc.upsertRoleTemplate({
        roleId: 'custom-role',
        name: 'Custom Role',
        kind: 'agent',
      })

      expect(result).toEqual({
        roleId: 'custom-role',
        name: 'Custom Role',
        kind: 'agent',
        builtIn: false,
        readOnly: false,
      })
    })

    it('upsertRoleTemplate calls templates_upsert_role in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      const roleData = { roleId: 'custom-role', name: 'Custom Role' }
      tauriCore.invoke.mockResolvedValue({ ok: true })

      await ipc.upsertRoleTemplate(roleData)

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_upsert_role', {
        request: {
          template: expect.objectContaining({
            roleId: 'custom-role',
            name: 'Custom Role',
            kind: 'agent',
            schema: { kind: 'role_template', version: 1 },
          }),
        },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('upsertRoleTemplate normalizes role-editor payload shape', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ ok: true })

      await ipc.upsertRoleTemplate({
        roleId: 'frontend-dev',
        name: 'Frontend Dev',
        tool: 'codex',
        model: 'gpt-5.4 high',
        instructions: 'Ship UI updates.',
        behavioralContract: [{ rule: 'Report progress', enabled: true }],
        capabilities: ['frontend'],
      })

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_upsert_role', {
        request: {
          template: expect.objectContaining({
            roleId: 'frontend-dev',
            defaults: expect.objectContaining({ cliTool: 'codex' }),
            behavioralContract: expect.objectContaining({
              execution: ['Report progress'],
            }),
          }),
        },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('upsertRoleTemplate enforces lead defaults and fallback behavioral contract', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ ok: true })

      await ipc.upsertRoleTemplate({
        roleId: 'lead-alpha',
        name: 'Lead Alpha',
        kind: 'LEAD',
        capabilities: [],
        behavioralContract: [],
        constraints: { minInstances: -5, maxInstances: 0 },
      })

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_upsert_role', {
        request: {
          template: expect.objectContaining({
            roleId: 'lead-alpha',
            kind: 'lead',
            defaults: expect.objectContaining({
              cliTool: 'claude',
              model: 'claude-opus-4-6',
            }),
            capabilities: ['orchestration'],
            constraints: expect.objectContaining({
              minInstances: 1,
              maxInstances: 1,
            }),
            behavioralContract: expect.objectContaining({
              execution: ['Execute assigned tasks and report status clearly.'],
            }),
          }),
        },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('deleteRoleTemplate returns deterministic mock result when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__

      const result = await ipc.deleteRoleTemplate('custom-role')

      expect(result).toEqual({
        roleId: 'custom-role',
        deleted: true,
      })
    })

    it('deleteRoleTemplate calls templates_delete_role in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(undefined)

      await ipc.deleteRoleTemplate('custom-role')

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_delete_role', {
        roleId: 'custom-role',
      })
      delete window.__TAURI_INTERNALS__
    })

    it('upsertTeamPreset returns sensible mock defaults when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__

      const result = await ipc.upsertTeamPreset({
        presetId: 'custom-preset',
        name: 'Custom Preset',
        leadRoleId: 'claude-orchestrator',
        agentSlots: [],
      })

      expect(result).toEqual({
        presetId: 'custom-preset',
        name: 'Custom Preset',
        leadRoleId: 'claude-orchestrator',
        agentSlots: [],
        builtIn: false,
        readOnly: false,
      })
    })

    it('upsertTeamPreset calls templates_upsert_preset in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      const presetData = { presetId: 'custom-preset', name: 'Custom Preset' }
      tauriCore.invoke.mockResolvedValue({ ok: true })

      await ipc.upsertTeamPreset(presetData)

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_upsert_preset', {
        request: {
          preset: expect.objectContaining({
            presetId: 'custom-preset',
            name: 'Custom Preset',
            schema: { kind: 'team_preset', version: 1 },
          }),
        },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('upsertTeamPreset accepts nested preset payloads', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ ok: true })

      await ipc.upsertTeamPreset({
        preset: {
          presetId: 'nested-preset',
          name: 'Nested Preset',
          leadRoleId: 'claude-orchestrator',
          agentSlots: [],
        },
      })

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_upsert_preset', {
        request: {
          preset: expect.objectContaining({
            presetId: 'nested-preset',
            name: 'Nested Preset',
          }),
        },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('upsertTeamPreset normalizes slot counts and defaults', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ ok: true })

      await ipc.upsertTeamPreset({
        presetId: 'preset-a',
        name: 'Preset A',
        leadRoleId: 'lead-alpha',
        agentSlots: [
          {
            roleId: 'impl-1',
            count: 0,
            projectBinding: 'slot_project',
            projectId: 'proj-2',
          },
        ],
        defaults: {
          teamNamePattern: '{project}-snake',
          tmuxLayout: 'even-horizontal',
        },
      })

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_upsert_preset', {
        request: {
          preset: expect.objectContaining({
            presetId: 'preset-a',
            leadRoleId: 'lead-alpha',
            agentSlots: [
              expect.objectContaining({
                roleId: 'impl-1',
                count: 1,
                projectBinding: 'slot_project',
                projectId: 'proj-2',
              }),
            ],
            defaults: expect.objectContaining({
              teamNamePattern: '{project}-snake',
              tmuxLayout: 'even-horizontal',
            }),
          }),
        },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('deleteTeamPreset returns deterministic mock result when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__

      const result = await ipc.deleteTeamPreset('custom-preset')

      expect(result).toEqual({
        presetId: 'custom-preset',
        deleted: true,
      })
    })

    it('deleteTeamPreset calls templates_delete_preset in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(undefined)

      await ipc.deleteTeamPreset('custom-preset')

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_delete_preset', {
        presetId: 'custom-preset',
      })
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('composeTeam()', () => {
    it('normalizes compose request before invoking backend', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ roster: [], warnings: [], validationErrors: [] })

      await ipc.composeTeam({
        leadRoleId: 'lead-a',
        projectName: 'atlas',
        agentSlots: [
          { roleId: 'dev-1', count: '2', projectBinding: 'slot_project', projectId: 'p2' },
        ],
        overrides: { mode: 'strict' },
      })

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_compose_team', {
        request: {
          leadRoleId: 'lead-a',
          agentSlots: [
            {
              roleId: 'dev-1',
              count: 2,
              projectBinding: 'slot_project',
              projectId: 'p2',
              overrides: null,
            },
          ],
          overrides: {
            mode: 'strict',
            projectName: 'atlas',
          },
        },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('returns lead-only warning in mock mode when no agent slots are provided', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.composeTeam({ leadRoleId: 'lead-a' })
      expect(result.warnings).toContain('No agent slots selected; roster includes lead only.')
    })

    it('normalizes composeTeam backend failures to Error messages', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockRejectedValueOnce({ message: 'compose failed' })
      await expect(ipc.composeTeam({ leadRoleId: 'lead-a' })).rejects.toThrow('compose failed')
      delete window.__TAURI_INTERNALS__
    })
  })

  // -----------------------------------------------------------------------
  // Coordination IPC wrappers (frontend-only task surface)
  // -----------------------------------------------------------------------

  describe('coordination wrappers', () => {
    it('coordinationInitializeTeam calls invoke with request in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      const request = { teamName: 'arch', lead: { name: 'team-lead' }, agents: [] }
      const report = { teamName: 'arch', succeededSteps: [], failedStep: null, retryable: false, message: 'ok', steps: [] }
      tauriCore.invoke.mockResolvedValue(report)

      const result = await ipc.coordinationInitializeTeam(request)

      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_initialize_team', {
        request: {
          teamName: 'arch',
          lead: {
            name: 'team-lead',
            roleId: null,
            instructions: null,
            behavioralContract: null,
            capabilities: null,
          },
          agents: [],
        },
      })
      expect(result).toEqual(report)
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationInitializeTeam returns deterministic mock shape', async () => {
      const result = await ipc.coordinationInitializeTeam({
        teamName: 'arch',
        lead: { name: 'team-lead' },
        agents: [],
      })
      expect(result.teamName).toBe('arch')
      expect(Array.isArray(result.succeededSteps)).toBe(true)
      expect(Array.isArray(result.steps)).toBe(true)
      expect(result).toHaveProperty('retryable')
    })

    it('coordinationAddAgent calls invoke with request and returns mock report shape', async () => {
      const request = { teamName: 'arch', agent: { name: 'bob' } }
      const mockModeResult = await ipc.coordinationAddAgent(request)
      expect(mockModeResult.teamName).toBe('arch')
      expect(mockModeResult.memberName).toBe('bob')
      expect(Array.isArray(mockModeResult.succeededSteps)).toBe(true)

      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ ok: true })
      await ipc.coordinationAddAgent(request)
      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_add_agent', { request })
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationReonboard calls invoke with named args and returns delivery mock', async () => {
      const mockModeResult = await ipc.coordinationReonboard('arch', 'bob')
      expect(mockModeResult).toEqual({ delivered: true, method: 'tmux_injection' })

      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ delivered: true, method: 'tmux_injection' })
      await ipc.coordinationReonboard('arch', 'bob')
      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_reonboard', {
        request: { teamName: 'arch', memberName: 'bob' },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationResumeMember calls invoke with request and returns deterministic mock shape', async () => {
      const mockModeResult = await ipc.coordinationResumeMember('arch', 'bob', 'continue')
      expect(mockModeResult).toEqual({
        teamName: 'arch',
        memberName: 'bob',
        resumed: true,
        succeededSteps: ['validate', 'resolve_pane', 'launch_session', 'update_runtime'],
        failedStep: null,
        retryable: false,
        message: 'member resumed',
        steps: [
          { step: 'validate', status: 'succeeded', message: 'request validated' },
          { step: 'update_runtime', status: 'succeeded', message: 'runtime updated' },
        ],
        warnings: [],
        paneId: '%2',
        reusedPane: false,
      })

      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ ok: true })
      await ipc.coordinationResumeMember('arch', 'bob', 'fresh')
      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_resume_member', {
        request: { teamName: 'arch', memberName: 'bob', contextMode: 'fresh' },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationGetFeatureAvailability calls invoke and returns deterministic mock shape', async () => {
      const mockModeResult = await ipc.coordinationGetFeatureAvailability()
      expect(mockModeResult).toEqual({
        canInitialize: true,
        meshAvailable: true,
        tmuxAvailable: true,
        blockingErrors: [],
      })

      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        canInitialize: false,
        meshAvailable: false,
        tmuxAvailable: true,
        blockingErrors: ['Mesh CLI not found'],
      })
      await ipc.coordinationGetFeatureAvailability()
      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_get_feature_availability')
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationPreflightCheck calls invoke and returns deterministic mock shape', async () => {
      const request = { teamName: 'arch', lead: { name: 'lead' }, agents: [] }
      const mockModeResult = await ipc.coordinationPreflightCheck(request)
      expect(mockModeResult).toEqual({
        canInitialize: true,
        blockingErrors: [],
        agentWarnings: [],
      })

      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ canInitialize: false, blockingErrors: ['x'], agentWarnings: [] })
      await ipc.coordinationPreflightCheck(request)
      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_preflight_check', { request })
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationGetLiveTeamStatus calls invoke and returns realistic mock shape', async () => {
      const mockModeResult = await ipc.coordinationGetLiveTeamStatus('arch')
      expect(mockModeResult.teamName).toBe('arch')
      expect(mockModeResult).toHaveProperty('leadName')
      expect(Array.isArray(mockModeResult.members)).toBe(true)
      expect(mockModeResult.members[0]).toHaveProperty('sessionStatus')

      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ teamName: 'arch', leadName: 'lead', members: [] })
      await ipc.coordinationGetLiveTeamStatus('arch')
      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_get_live_team_status', { teamName: 'arch' })
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationGetProjectMeshSnapshot calls invoke and returns deterministic mock shape', async () => {
      const mockModeResult = await ipc.coordinationGetProjectMeshSnapshot('/projects/arch')
      expect(mockModeResult).toEqual({
        meshAvailable: true,
        tmuxAvailable: true,
        teamName: 'mock-team',
        teamStatus: {
          leadName: 'team-lead',
          members: [
            {
              name: 'team-lead',
              role: 'lead',
              cliTool: 'claude',
              projectId: '/projects/arch',
              description: 'Own orchestration',
              sessionStatus: 'active',
              paneId: '%1',
            },
          ],
        },
        warnings: [],
      })

      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        meshAvailable: false,
        tmuxAvailable: true,
        teamName: null,
        teamStatus: null,
        warnings: ['mesh missing'],
      })
      await ipc.coordinationGetProjectMeshSnapshot('/projects/arch')
      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_get_project_mesh_snapshot', {
        projectPath: '/projects/arch',
      })
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationDisbandTeam returns structured mock response', async () => {
      const result = await ipc.coordinationDisbandTeam('arch')
      expect(result).toEqual({
        teamName: 'arch',
        disbanded: true,
        alreadyDisbanded: false,
        message: 'team disbanded',
      })
    })

    it('onCoordinationStepProgress listens to coordination-step-progress', async () => {
      const callback = vi.fn()
      const unlisten = vi.fn()
      tauriEvent.listen.mockResolvedValue(unlisten)

      const returned = await ipc.onCoordinationStepProgress(callback)

      expect(tauriEvent.listen).toHaveBeenCalledWith('coordination-step-progress', callback)
      expect(returned).toBe(unlisten)
    })
  })
})
