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

  describe('listAccounts()', () => {
    it('carries the accounts and the state of detection', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        accounts: [
          {
            tool: 'claude',
            id: 'account-1',
            dir: '/home/user/.claude',
            identity: {
              id: 'account-1',
              label: 'a@example.com',
              displayName: 'A',
              loggedIn: true,
              usageCapable: false,
            },
            is_default: true,
          },
        ],
        source: 'daemon',
        degraded: false,
        error: null,
      })

      const result = await ipc.listAccounts('claude')

      expect(tauriCore.invoke).toHaveBeenCalledWith('list_accounts', { tool: 'claude' })
      expect(result.accounts).toHaveLength(1)
      expect(result.accounts[0].usage_capable).toBe(false)
      expect(result.source).toBe('daemon')
      expect(result.degraded).toBe(false)
      delete window.__TAURI_INTERNALS__
    })

    // Regression: 518aace let a daemon failure arrive as a plain empty list,
    // indistinguishable from a host with no subscriptions signed in.
    it('reports a degraded detection rather than an empty answer', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        accounts: [],
        source: 'daemon',
        degraded: true,
        error: 'The WSL daemon is not reachable',
      })

      const result = await ipc.listAccounts('claude')

      expect(result.accounts).toEqual([])
      expect(result.degraded).toBe(true)
      expect(result.error).toBe('The WSL daemon is not reachable')
      delete window.__TAURI_INTERNALS__
    })

    // Regression: d6839a3 normalized accounts without usage, so the camelCase
    // the backend serializes (`fiveHour.usedPercentage`) would have reached the
    // meter as `undefined` and every subscription would have looked unreported.
    it('carries the usage the status line reported for each account', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        accounts: [
          {
            tool: 'claude',
            id: 'account-1',
            dir: '/home/user/.claude',
            identity: { id: 'account-1', label: 'a@example.com', loggedIn: true },
            isDefault: true,
            usage: {
              status: 'ok',
              windows: [
                { key: 'session', title: 'Current session', usedPercentage: 26, resetsAt: 1787784600, severity: 'normal', isActive: true },
                { key: 'weekly_all', title: 'Current week (all models)', usedPercentage: 17, resetsAt: 1788300000, severity: 'normal', isActive: true },
              ],
              observedAt: '2026-08-27T00:29:00Z',
            },
          },
          { tool: 'claude', id: 'account-2', dir: '/home/user/.claude-work', identity: { id: 'account-2', label: 'b@example.com' } },
        ],
        source: 'native',
        degraded: false,
        error: null,
      })

      const result = await ipc.listAccounts('claude')

      expect(result.accounts[0].usage).toEqual({
        observed_at: '2026-08-27T00:29:00Z',
        status: 'ok',
        windows: [
          { key: 'session', title: 'Current session', used_percentage: 26, resets_at: 1787784600, severity: 'normal', is_active: true },
          { key: 'weekly_all', title: 'Current week (all models)', used_percentage: 17, resets_at: 1788300000, severity: 'normal', is_active: true },
        ],
        note: null,
      })
      // An account nothing has reported for stays without usage; it is not 0 %.
      expect(result.accounts[1].usage).toBeNull()
      delete window.__TAURI_INTERNALS__
    })
  })

  describe('generic account commands', () => {
    it('refreshes usage for one tool', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(true)

      await ipc.refreshAccountsUsage('claude')

      expect(tauriCore.invoke).toHaveBeenCalledWith('refresh_accounts_usage', { tool: 'claude' })
      delete window.__TAURI_INTERNALS__
    })

    it('pins and resolves accounts with the tool on the wire', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue(undefined)

      await ipc.setProjectAccount('p1', 'claude', 'account-2')
      expect(tauriCore.invoke).toHaveBeenCalledWith('set_project_account', {
        projectId: 'p1',
        tool: 'claude',
        accountId: 'account-2',
      })

      tauriCore.invoke.mockResolvedValue({ accountId: 'account-2' })
      await ipc.resolveLaunchAccount('p1', 'claude', 'resume', 's1')
      expect(tauriCore.invoke).toHaveBeenCalledWith('resolve_launch_account', {
        projectId: 'p1',
        tool: 'claude',
        mode: 'resume',
        sessionId: 's1',
      })
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
        projectDialogLastPath: '/projects/remembered',
        codeTheme: { light: 'solarized-light', dark: 'one-dark-pro' },
        daemon: { port: 17233, path: '/tmp/daemon', autoStart: false },
        terminal: {
          emulator: 'windows_terminal',
          customCommand: '',
          tmuxLayout: 'new_window',
          cliCommands: {
            claude: { continueCmd: 'claude --continue', fresh: 'claude', resume: 'claude --resume' },
            codex: { continueCmd: 'codex resume --last --yolo', fresh: 'codex --yolo', resume: 'codex resume --yolo' },
            agy: {
              continueCmd: 'agy --dangerously-skip-permissions --continue',
              fresh: 'agy --dangerously-skip-permissions',
              resume: 'agy --dangerously-skip-permissions --conversation {session_id}',
            },
          },
        },
        terminalContract: {
          platform: 'windows',
          defaultEmulator: 'windows_terminal',
          supportedEmulators: ['windows_terminal', 'custom'],
          cliCommandDefaults: {
            claude: {
              continueCmd: 'claude --dangerously-skip-permissions --continue',
              fresh: 'claude --dangerously-skip-permissions',
              resume: 'claude --dangerously-skip-permissions --resume',
            },
            codex: {
              continueCmd: 'codex --yolo',
              fresh: 'codex --yolo',
              resume: 'codex resume --last --yolo',
            },
            agy: {
              continueCmd: 'agy --dangerously-skip-permissions --continue',
              fresh: 'agy --dangerously-skip-permissions',
              resume: 'agy --dangerously-skip-permissions --conversation {session_id}',
            },
          },
          modelCatalog: {
            claude: [{ id: 'opus', label: 'Opus 5', efforts: ['high'], defaultEffort: null, deprecated: false, replacement: null }],
            codex: [{ id: 'gpt-5.6-sol', label: 'GPT-5.6-Sol', efforts: ['low', 'high'], defaultEffort: 'low', deprecated: false, replacement: null }],
            agy: [{ id: 'gemini-3.7-flash-high', label: 'Gemini 3.7 Flash (High)', efforts: [], defaultEffort: null, deprecated: false, replacement: null }],
          },
          cliVersions: {
            codex: '0.149.0',
            claude: '2.1.246',
            agy: '1.1.22',
            codexCompactionHooksSupported: true,
            codexNotifySupported: true,
            codexQueueWakeSupported: true,
            agyHooksSupported: true,
          },
        },
      })

      const result = await ipc.getSettings()

      expect(result.scan_directories).toEqual(['~/work'])
      expect(result.thresholds).toEqual({ active_days: 5, recent_days: 14, stale_days: 60 })
      expect(result.ignore_patterns).toEqual(['node_modules'])
      expect(result.dark_mode).toBe(true)
      expect(result.project_dialog_last_path).toBe('/projects/remembered')
      expect(result.code_theme).toEqual({ light: 'solarized-light', dark: 'one-dark-pro' })
      expect(result.daemon.auto_start).toBe(false)
      expect(result.terminal.emulator).toBe('windows_terminal')
      expect(result.terminal.custom_command).toBe('')
      expect(result.terminal.tmux_layout).toBe('new_window')
      expect(result.terminal.harness).not.toHaveProperty('codex_compaction')
      expect(result.terminal.cli_commands.claude.continue_cmd).toBe('claude --continue')
      expect(result.terminal_contract.default_emulator).toBe('windows_terminal')
      expect(result.terminal_contract.supported_emulators).toEqual(['windows_terminal', 'custom'])
      expect(result.terminal_contract.model_catalog.codex[0]).toEqual({
        id: 'gpt-5.6-sol',
        label: 'GPT-5.6-Sol',
        efforts: ['low', 'high'],
        defaultEffort: 'low',
        deprecated: false,
        replacement: null,
      })
      // Regression: 2cf41db exposed no per-tool version gate through the
      // platform contract, so native capability support could not be audited.
      // Regression: 4e9e2c5 shipped the Antigravity hook sink without a
      // version gate, so the contract could not report why it stays off.
      expect(result.terminal_contract.cli_versions).toEqual({
        codex: '0.149.0',
        claude: '2.1.246',
        agy: '1.1.22',
        codex_compaction_hooks_supported: true,
        codex_notify_supported: true,
        codex_queue_wake_supported: true,
        agy_hooks_supported: true,
      })
      // Regression: a574720 exposed the retired status-line usage capability
      // on the frontend terminal contract after the bridge itself was removed.
      expect(result.terminal_contract.cli_versions).not.toHaveProperty(
        'claude_statusline_usage_supported',
      )
      delete window.__TAURI_INTERNALS__
    })

    it('falls back to the backend contract default when the saved emulator is invalid for the platform', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        terminal: {
          emulator: 'windows_terminal',
          customCommand: '',
          tmuxLayout: 'new_window',
          cliCommands: {},
        },
        terminalContract: {
          platform: 'linux',
          defaultEmulator: 'manual',
          supportedEmulators: ['manual'],
          cliCommandDefaults: {
            claude: {
              continueCmd: 'claude --dangerously-skip-permissions --continue',
              fresh: 'claude --dangerously-skip-permissions',
              resume: 'claude --dangerously-skip-permissions --resume',
            },
            codex: {
              continueCmd: 'codex --yolo',
              fresh: 'codex --yolo',
              resume: 'codex resume --last --yolo',
            },
            agy: {
              continueCmd: 'agy --dangerously-skip-permissions --continue',
              fresh: 'agy --dangerously-skip-permissions',
              resume: 'agy --dangerously-skip-permissions --conversation {session_id}',
            },
          },
        },
      })

      const result = await ipc.getSettings()

      expect(result.terminal.emulator).toBe('manual')
      expect(result.terminal.cli_commands.claude.continue_cmd)
        .toBe('claude --dangerously-skip-permissions --continue')
      delete window.__TAURI_INTERNALS__
    })

    it('returns mock settings when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.getSettings()

      expect(result).toHaveProperty('scan_directories')
      expect(result).toHaveProperty('thresholds')
      expect(result.thresholds).toHaveProperty('active_days')
      expect(result).toHaveProperty('project_dialog_last_path')
      expect(result.terminal.harness).not.toHaveProperty('codex_compaction')
      // Regression: 4e9e2c5 defaulted the Antigravity activity hooks off while
      // their trust-gated loading was unverified; settings that predate the
      // harness block must now normalize to on, like grok's.
      expect(result.terminal.harness.agy_hooks).toBe(true)
      expect(result.terminal.harness.grok_hooks).toBe(true)
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

  describe('openExternalUrl()', () => {
    it('opens https URLs in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce(undefined)

      await expect(ipc.openExternalUrl('https://example.com/docs')).resolves.toBeUndefined()

      expect(tauriCore.invoke).toHaveBeenCalledWith('plugin:opener|open_url', {
        url: 'https://example.com/docs',
      })
      delete window.__TAURI_INTERNALS__
    })

    it('opens mailto URLs in mock mode', async () => {
      const openSpy = vi.spyOn(window, 'open').mockImplementation(() => null)

      await expect(ipc.openExternalUrl('mailto:test@example.com')).resolves.toBeUndefined()

      expect(openSpy).toHaveBeenCalledWith('mailto:test@example.com', '_blank')
      openSpy.mockRestore()
    })

    it('rejects insecure http URLs before invoking the opener plugin', async () => {
      window.__TAURI_INTERNALS__ = {}

      await expect(ipc.openExternalUrl('http://example.com')).rejects.toThrow(
        'Only HTTPS and mailto links can be opened externally.'
      )

      expect(tauriCore.invoke).not.toHaveBeenCalled()
      delete window.__TAURI_INTERNALS__
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

  describe('launchCliSession()', () => {
    it('calls invoke with project ID and mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      const mockResult = { tmux_window: 'proj', tmux_pane: '%5' }
      tauriCore.invoke.mockResolvedValue(mockResult)

      const result = await ipc.launchCliSession('p1', 'continue')

      expect(tauriCore.invoke).toHaveBeenCalledWith('launch_cli_session', {
        projectId: 'p1',
        mode: 'continue',
        cliTool: null,
        accountId: null,
      })
      expect(result).toEqual(mockResult)
      delete window.__TAURI_INTERNALS__
    })

    it('returns mock result when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__
      const result = await ipc.launchCliSession('p1', 'fresh')

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

  describe('getForegroundProject()', () => {
    it('calls invoke with the foreground project command', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue('proj-2')

      const result = await ipc.getForegroundProject()

      expect(tauriCore.invoke).toHaveBeenCalledWith('get_foreground_project')
      expect(result).toBe('proj-2')
      delete window.__TAURI_INTERNALS__
    })

    it('returns null when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__

      const result = await ipc.getForegroundProject()

      expect(result).toBeNull()
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

    // Regression: 179a767 rendered `session.account_label`, but the archived
    // session IPC boundary did not normalize a camelCase accountLabel field.
    it('normalizes archived-session account labels from Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        sessions: [{ session_id: 'session-1', accountLabel: 'Work subscription' }],
        errors: [],
      })

      const result = await ipc.getArchivedSessions('/tmp/project')

      expect(result.sessions[0].account_label).toBe('Work subscription')
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

    it('preserves busy daemon status from Tauri', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        status: 'busy',
        protocolVersion: 4,
        expectedProtocolVersion: 4,
        port: 17233,
      })

      const result = await ipc.getDaemonStatus()

      expect(result.status).toBe('busy')
      expect(result.protocol_version).toBe(4)
      expect(result.expected_protocol_version).toBe(4)
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
        bundled_contract: {
          version: '0.1.0',
          protocol_version: 1,
          schema_version: 1,
          git_commit: 'mock-mesh-commit',
        },
        installed_contract: {
          version: '0.1.0',
          protocol_version: 1,
          schema_version: 1,
          git_commit: 'mock-mesh-commit',
        },
        compatibility_issues: [],
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
        bundled_contract: {
          version: '0.1.1',
          protocol_version: 1,
          schema_version: 1,
          git_commit: 'cutover-commit',
        },
        installed_contract: null,
        compatibility_issues: [],
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
        bundledContract: {
          version: '0.1.1',
          protocolVersion: 1,
          schemaVersion: 1,
          gitCommit: 'expected-commit',
        },
        installedContract: {
          version: '0.1.0',
          protocolVersion: 0,
          schemaVersion: 1,
          gitCommit: 'actual-commit',
        },
        compatibilityIssues: [
          {
            code: 'protocol_version_mismatch',
            message:
              'Installed Mesh CLI protocol version 0 does not match taurhaus required protocol version 1. Install bundled Mesh to continue.',
            expected: '1',
            actual: '0',
          },
        ],
        environmentAvailable: true,
        error: null,
      })

      const result = await ipc.checkMeshInstallStatus()

      expect(result).toEqual({
        installed: true,
        version: '0.1.0',
        bundled_version: '0.1.1',
        needs_update: true,
        bundled_contract: {
          version: '0.1.1',
          protocol_version: 1,
          schema_version: 1,
          git_commit: 'expected-commit',
        },
        installed_contract: {
          version: '0.1.0',
          protocol_version: 0,
          schema_version: 1,
          git_commit: 'actual-commit',
        },
        compatibility_issues: [
          {
            code: 'protocol_version_mismatch',
            message:
              'Installed Mesh CLI protocol version 0 does not match taurhaus required protocol version 1. Install bundled Mesh to continue.',
            expected: '1',
            actual: '0',
          },
        ],
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
          focus_area: 'Scoped implementation',
          context_summary: 'Owns code changes, tests, and debugging within assigned scope.',
          behavior_summary: 'Implements narrowly and escalates blockers instead of broadening scope.',
        },
      ])

      const result = await ipc.listRoleTemplates()

      expect(tauriCore.invoke).toHaveBeenCalledTimes(1)
      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_list_roles_full')
      expect(result).toEqual([
        // Regression: ff40911 and 5d2ce27 stored the effort inside the model
        // string; the response normalizer now splits it into the canonical pair.
        expect.objectContaining({
          roleId: 'codex-developer',
          cliTool: 'codex',
          model: 'gpt-5.4',
          reasoningEffort: 'high',
          focusArea: 'Scoped implementation',
          contextSummary: 'Owns code changes, tests, and debugging within assigned scope.',
          behaviorSummary: 'Implements narrowly and escalates blockers instead of broadening scope.',
          builtIn: true,
          readOnly: true,
        }),
      ])
      delete window.__TAURI_INTERNALS__
    })

    it('preserves builtIn roles in non-Tauri mock mode when source is absent', async () => {
      delete window.__TAURI_INTERNALS__

      const result = await ipc.listRoleTemplates()
      const orchestrator = result.find((role) => role.roleId === 'claude-orchestrator')

      expect(orchestrator).toEqual(expect.objectContaining({
        roleId: 'claude-orchestrator',
        builtIn: true,
      }))
    })

    it('normalizes snake_case role metadata into canonical Mesh fields', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce([
        {
          role_id: 'codex-developer',
          name: 'Codex Developer',
          kind: 'agent',
          cli_tool: 'codex',
          focus_area: 'Scoped implementation',
          context_summary: 'Owns code changes and tests within its scope.',
          behavior_summary: 'Implements narrowly and escalates blockers quickly.',
          behavioral_contract: {
            communication: ['Report concise progress.'],
            execution: ['Ship the assigned slice end-to-end.'],
            escalation: ['Escalate blockers immediately.'],
          },
          capabilities: ['frontend'],
        },
      ])

      const result = await ipc.listRoleTemplates()

      expect(result).toEqual([
        expect.objectContaining({
          roleId: 'codex-developer',
          cliTool: 'codex',
          focusArea: 'Scoped implementation',
          contextSummary: 'Owns code changes and tests within its scope.',
          behaviorSummary: 'Implements narrowly and escalates blockers quickly.',
          behavioralContract: {
            communication: ['Report concise progress.'],
            execution: ['Ship the assigned slice end-to-end.'],
            escalation: ['Escalate blockers immediately.'],
          },
          capabilities: ['frontend'],
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
          presetId: 'pair',
          name: 'Pair',
          leadRoleId: 'v3-lead-claude',
          source: 'built_in',
          readOnly: true,
          agentSlots: [{ roleId: 'quick-dev-codex', count: 1 }],
        },
      ])

      const result = await ipc.listTeamPresets()

      expect(tauriCore.invoke).toHaveBeenCalledTimes(1)
      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_list_presets_full')
      expect(result).toEqual([
        expect.objectContaining({
          presetId: 'pair',
          leadRoleId: 'v3-lead-claude',
          roleCount: 1,
          agentCount: 1,
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
          cliTool: 'agy',
          defaults: { model: 'gemini-3.7-flash-high' },
          focus_area: 'Documentation systems',
          context_summary: 'Maintains operational docs and architecture-facing explanations.',
          behavior_summary: 'Clarifies shipped behavior without assuming implementation ownership.',
          capabilities: 'not-an-array',
        },
      ])

      const result = await ipc.listRoleTemplates()

      expect(result).toEqual([
        expect.objectContaining({
          roleId: 'role-a',
          cliTool: 'agy',
          model: 'gemini-3.7-flash-high',
          focusArea: 'Documentation systems',
          contextSummary: 'Maintains operational docs and architecture-facing explanations.',
          behaviorSummary: 'Clarifies shipped behavior without assuming implementation ownership.',
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

    it('getTeamPreset normalizes snake_case preset detail fields', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce({
        preset_id: 'pair',
        name: 'Pair',
        lead_role_id: 'v3-lead-claude',
        agent_slots: [
          {
            role_id: 'quick-dev-codex',
            count: 1,
            project_binding: 'lead_project',
            project_id: null,
            overrides: {
              name_pattern: 'quick-dev',
            },
          },
        ],
        defaults: {
          team_name_pattern: '{project}-team',
          tmux_layout: 'tiled',
        },
      })

      const result = await ipc.getTeamPreset('pair')

      expect(result).toEqual(expect.objectContaining({
        presetId: 'pair',
        name: 'Pair',
        description: '',
        leadRoleId: 'v3-lead-claude',
        agentSlots: expect.arrayContaining([
          expect.objectContaining({
            roleId: 'quick-dev-codex',
            count: 1,
            projectBinding: 'lead_project',
            projectId: null,
            overrides: expect.objectContaining({
              namePattern: 'quick-dev',
            }),
          }),
        ]),
        defaults: {
          teamNamePattern: '{project}-team',
          tmuxLayout: 'tiled',
        },
      }))

      delete window.__TAURI_INTERNALS__
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
        focusArea: 'Frontend implementation',
        contextSummary: 'Owns UI components, interaction flows, and regression-safe changes.',
        behaviorSummary: 'Ships UI work directly and escalates architecture or product-direction calls.',
        instructions: 'Ship UI updates.',
        behavioralContract: [{ rule: 'Report progress', enabled: true }],
      })

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_upsert_role', {
        request: {
          template: expect.objectContaining({
            roleId: 'frontend-dev',
            defaults: expect.objectContaining({ cliTool: 'codex' }),
            focusArea: 'Frontend implementation',
            contextSummary: 'Owns UI components, interaction flows, and regression-safe changes.',
            behaviorSummary: 'Ships UI work directly and escalates architecture or product-direction calls.',
            behavioralContract: expect.objectContaining({
              execution: ['Report progress'],
            }),
          }),
        },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('upsertRoleTemplate does not backfill Claude defaults for lead roles and keeps fallback behavioral contract', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ ok: true })

      await ipc.upsertRoleTemplate({
        roleId: 'lead-alpha',
        name: 'Lead Alpha',
        kind: 'LEAD',
        behavioralContract: [],
        constraints: { minInstances: -5, maxInstances: 0 },
      })

      expect(tauriCore.invoke).toHaveBeenCalledWith('templates_upsert_role', {
        request: {
          template: expect.objectContaining({
            roleId: 'lead-alpha',
            kind: 'lead',
            defaults: expect.objectContaining({
              cliTool: '',
              model: '',
            }),
            focusArea: null,
            contextSummary: null,
            behaviorSummary: null,
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

    it('importRoleFromFile calls import_role_from_file in Tauri mode', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        success: true,
        role: {
          roleId: 'imported-role',
          name: 'Imported Role',
        },
        conflict: null,
      })

      const result = await ipc.importRoleFromFile('/tmp/imported-role.md')

      expect(tauriCore.invoke).toHaveBeenCalledWith('import_role_from_file', {
        request: {
          filePath: '/tmp/imported-role.md',
        },
      })
      expect(result).toEqual({
        success: true,
        role: {
          roleId: 'imported-role',
          name: 'Imported Role',
        },
        conflict: null,
      })
      delete window.__TAURI_INTERNALS__
    })

    it('importRoleFromFile returns deterministic mock content outside Tauri', async () => {
      delete window.__TAURI_INTERNALS__

      const result = await ipc.importRoleFromFile('/tmp/imported-role.md')

      expect(result).toEqual(
        expect.objectContaining({
          success: true,
          conflict: null,
          role: expect.objectContaining({
            roleId: 'imported-role',
            provenance: expect.objectContaining({
              sourcePath: '/tmp/imported-role.md',
            }),
          }),
        })
      )
    })

    it('exportRoleToFile calls export_role_to_file in Tauri mode and normalizes the response', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        target_format: 'agents_md',
        file_content: '# Custom Role\n',
        lossy_fields: ['constraints'],
      })

      const result = await ipc.exportRoleToFile('custom-role', 'agents_md')

      expect(tauriCore.invoke).toHaveBeenCalledWith('export_role_to_file', {
        request: {
          roleId: 'custom-role',
          targetFormat: 'agents_md',
        },
      })
      expect(result).toEqual({
        targetFormat: 'agents_md',
        fileContent: '# Custom Role\n',
        lossyFields: ['constraints'],
      })
      delete window.__TAURI_INTERNALS__
    })

    it('exportRoleToFile returns deterministic mock content when not in Tauri', async () => {
      delete window.__TAURI_INTERNALS__

      const result = await ipc.exportRoleToFile('custom-doc-writer', 'gemini_md')

      expect(result.targetFormat).toBe('gemini_md')
      expect(result.fileContent).toContain('# Documentation Writer')
      expect(result.lossyFields).toContain('capabilities')
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
      const result = await ipc.composeTeam({ leadRoleId: 'claude-orchestrator' })
      expect(result.warnings).toContain('No agent slots selected; roster includes lead only.')
      expect(result.roster[0]).toEqual(expect.objectContaining({
        focusArea: 'Team orchestration',
        contextSummary: 'Keeps the team aligned on sequencing, blockers, and delivery quality.',
        behaviorSummary: 'Coordinates specialists and avoids taking over implementation lanes.',
      }))
    })

    it('normalizes composeTeam backend failures to Error messages', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockRejectedValueOnce({ message: 'compose failed' })
      await expect(ipc.composeTeam({ leadRoleId: 'lead-a' })).rejects.toThrow('compose failed')
      delete window.__TAURI_INTERNALS__
    })

    it('normalizes snake_case composition roster fields from the backend', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce({
        roster: [
          {
            name: 'team-lead',
            role_id: 'claude-orchestrator',
            role_kind: 'lead',
            cli_tool: 'claude',
            focus_area: 'Team orchestration',
            context_summary: 'Carries the team plan.',
            behavior_summary: 'Delegates and escalates blockers.',
            behavioral_contract: {
              communication: ['Report progress clearly.'],
              execution: ['Keep work scoped.'],
              escalation: ['Escalate blockers immediately.'],
            },
            project_binding: 'lead_project',
            project_id: '/projects/taurhaus',
          },
        ],
        warnings: [],
        validation_errors: [],
      })

      const result = await ipc.composeTeam({ leadRoleId: 'lead-a' })

      expect(result).toEqual({
        roster: [
          expect.objectContaining({
            name: 'team-lead',
            roleId: 'claude-orchestrator',
            roleKind: 'lead',
            cliTool: 'claude',
            focusArea: 'Team orchestration',
            contextSummary: 'Carries the team plan.',
            behaviorSummary: 'Delegates and escalates blockers.',
            behavioralContract: {
              communication: ['Report progress clearly.'],
              execution: ['Keep work scoped.'],
              escalation: ['Escalate blockers immediately.'],
            },
            projectBinding: 'lead_project',
            projectId: '/projects/taurhaus',
          }),
        ],
        warnings: [],
        validationErrors: [],
      })

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
      await ipc.coordinationResumeMember('arch', 'bob')
      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_resume_member', {
        request: { teamName: 'arch', memberName: 'bob' },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationResumeTeam calls invoke with request and returns deterministic mock shape', async () => {
      const mockModeResult = await ipc.coordinationResumeTeam('arch')
      expect(mockModeResult).toEqual({
        teamName: 'arch',
        resumed: true,
        totalMembers: 2,
        resumedMembers: ['team-lead', 'frontend-dev'],
        failedMembers: [],
        warnings: [],
        startedTeamDaemon: false,
        teamDaemonWarning: null,
      })

      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({ ok: true })
      await ipc.coordinationResumeTeam('arch')
      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_resume_team', {
        request: { teamName: 'arch' },
      })
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationResumeTeam normalizes snake_case reports from the backend', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce({
        team_name: 'arch',
        resumed: true,
        total_members: 2,
        resumed_members: ['team-lead'],
        failed_members: [{ member_name: 'frontend-dev', message: 'tmux missing' }],
        warnings: ['partial'],
        started_team_daemon: true,
        team_daemon_warning: 'daemon restarted',
      })

      const result = await ipc.coordinationResumeTeam('arch')

      expect(result).toEqual({
        teamName: 'arch',
        resumed: true,
        totalMembers: 2,
        resumedMembers: ['team-lead'],
        failedMembers: [{ memberName: 'frontend-dev', message: 'tmux missing' }],
        warnings: ['partial'],
        startedTeamDaemon: true,
        teamDaemonWarning: 'daemon restarted',
      })

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
      expect(mockModeResult.runtimeSnapshotFreshness).toBe('fresh')
      expect(Array.isArray(mockModeResult.members)).toBe(true)
      expect(mockModeResult.members[0]).toHaveProperty('sessionStatus')

      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValue({
        teamName: 'arch',
        leadName: 'lead',
        runtimeSnapshotFreshness: 'cached',
        members: [],
      })
      await ipc.coordinationGetLiveTeamStatus('arch')
      expect(tauriCore.invoke).toHaveBeenCalledWith('coordination_get_live_team_status', { teamName: 'arch' })
      delete window.__TAURI_INTERNALS__
    })

    it('coordinationGetLiveTeamStatus normalizes snake_case runtime freshness', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce({
        team_name: 'arch-team',
        lead_name: 'team-lead',
        runtime_snapshot_freshness: 'attachments_only',
        members: [
          {
            name: 'frontend-dev',
            role: 'member',
            cli_tool: 'codex',
            session_status: 'idle',
            pane_id: '%2',
          },
        ],
      })

      const result = await ipc.coordinationGetLiveTeamStatus('arch-team')

      expect(result).toEqual({
        teamName: 'arch-team',
        leadName: 'team-lead',
        runtimeSnapshotFreshness: 'attachments_only',
        members: [
          expect.objectContaining({
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            sessionStatus: 'idle',
            paneId: '%2',
          }),
        ],
      })

      delete window.__TAURI_INTERNALS__
    })

    it('coordinationGetProjectMeshSnapshot calls invoke and returns deterministic mock shape', async () => {
      const mockModeResult = await ipc.coordinationGetProjectMeshSnapshot('/projects/arch')
      expect(mockModeResult).toEqual({
        meshAvailable: true,
        tmuxAvailable: true,
        teamName: 'mock-team',
        teamRuntimeState: 'active',
        teamStatus: {
          leadName: 'team-lead',
          runtimeSnapshotFreshness: null,
          members: [
            {
              name: 'team-lead',
              role: 'lead',
              cliTool: 'claude',
              roleId: 'claude-orchestrator',
              roleName: 'Claude Orchestrator',
              focusArea: 'Team sequencing and escalation',
              contextSummary: 'Keeps the full delivery plan and blocker state in view.',
              behaviorSummary: 'Coordinates specialists and escalates blockers.',
              projectId: '/projects/arch',
              isCrossProject: false,
              projectLabel: '',
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

    it('coordinationGetProjectMeshSnapshot normalizes snake_case Mesh snapshot fields', async () => {
      window.__TAURI_INTERNALS__ = {}
      tauriCore.invoke.mockResolvedValueOnce({
        mesh_available: false,
        tmux_available: true,
        team_name: 'arch-team',
        team_runtime_state: 'cold_resume',
        team_status: {
          lead_name: 'team-lead',
          members: [
            {
              name: 'frontend-dev',
              role: 'member',
              cli_tool: 'codex',
              model: 'gpt-5.4 high',
              role_id: 'codex-developer',
              role_name: 'Codex Developer',
              focus_area: 'Scoped implementation',
              context_summary: 'Owns code changes.',
              behavior_summary: 'Implements narrowly.',
              project_id: '/projects/ui',
              is_cross_project: true,
              project_label: 'ui',
              description: 'Owns the UI.',
              session_status: 'idle',
              pane_id: '%2',
            },
          ],
        },
        warnings: ['mesh missing'],
      })

      const result = await ipc.coordinationGetProjectMeshSnapshot('/projects/arch')

      expect(result).toEqual({
        meshAvailable: false,
        tmuxAvailable: true,
        teamName: 'arch-team',
        teamRuntimeState: 'coldResume',
        teamStatus: {
          leadName: 'team-lead',
          runtimeSnapshotFreshness: null,
          members: [
            expect.objectContaining({
              name: 'frontend-dev',
              role: 'member',
              cliTool: 'codex',
              model: 'gpt-5.4 high',
              roleId: 'codex-developer',
              roleName: 'Codex Developer',
              focusArea: 'Scoped implementation',
              contextSummary: 'Owns code changes.',
              behaviorSummary: 'Implements narrowly.',
              projectId: '/projects/ui',
              isCrossProject: true,
              projectLabel: 'ui',
              description: 'Owns the UI.',
              sessionStatus: 'idle',
              paneId: '%2',
            }),
          ],
        },
        warnings: ['mesh missing'],
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

      expect(tauriEvent.listen).toHaveBeenCalledTimes(1)
      const listenCall = tauriEvent.listen.mock.calls.at(-1)
      expect(listenCall?.[0]).toBe('coordination-step-progress')
      expect(typeof listenCall?.[1]).toBe('function')

      const handler = listenCall?.[1]
      handler?.({
        payload: {
          teamName: 'arch-team',
          operation: 'initialize_team',
          progress: {
            step: 'create_panes',
            status: 'running',
            message: null,
          },
        },
      })

      expect(callback).toHaveBeenCalledWith({
        payload: {
          teamName: 'arch-team',
          operation: 'initialize_team',
          progress: {
            step: 'create_panes',
            status: 'running',
            message: null,
            canonicalStages: ['acquire_pane', 'launch_session'],
          },
        },
      })
      expect(returned).toBe(unlisten)
    })

    it('onCoordinationResumeTeamProgress listens to coordination-resume-team-progress', async () => {
      const callback = vi.fn()
      const unlisten = vi.fn()
      tauriEvent.listen.mockResolvedValue(unlisten)

      const returned = await ipc.onCoordinationResumeTeamProgress(callback)

      expect(tauriEvent.listen).toHaveBeenCalledWith(
        'coordination-resume-team-progress',
        expect.any(Function)
      )

      const handler = tauriEvent.listen.mock.calls.at(-1)?.[1]
      handler?.({
        payload: {
          operation: 'resume_team',
          teamName: 'architecture-final',
          memberName: 'frontend-dev',
          memberIndex: 2,
          memberCount: 3,
          stage: 'launch_session',
          status: 'running',
          message: 'launching',
        },
      })

      expect(callback).toHaveBeenCalledWith({
        payload: {
          operation: 'resume_team',
          teamName: 'architecture-final',
          memberName: 'frontend-dev',
          memberIndex: 2,
          memberCount: 3,
          stage: 'launch_session',
          status: 'running',
          message: 'launching',
        },
      })
      expect(returned).toBe(unlisten)
    })

    it('normalizes legacy resume stage aliases for streamed team progress', async () => {
      const callback = vi.fn()
      const unlisten = vi.fn()
      tauriEvent.listen.mockResolvedValue(unlisten)

      await ipc.onCoordinationResumeTeamProgress(callback)

      const handler = tauriEvent.listen.mock.calls.at(-1)?.[1]
      handler?.({
        payload: {
          operation: 'resume_team',
          teamName: 'architecture-final',
          memberName: 'frontend-dev',
          memberIndex: 2,
          memberCount: 3,
          stage: 'commit_member_runtime',
          status: 'running',
          message: 'launching',
        },
      })

      expect(callback).toHaveBeenCalledWith({
        payload: {
          operation: 'resume_team',
          teamName: 'architecture-final',
          memberName: 'frontend-dev',
          memberIndex: 2,
          memberCount: 3,
          stage: 'commit_runtime',
          status: 'running',
          message: 'launching',
        },
      })
    })
  })
})
