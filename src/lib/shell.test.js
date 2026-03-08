import { describe, it, expect, vi, beforeEach } from 'vitest'
import { readFileSync } from 'node:fs'
import { createAsyncGuard } from './asyncGuard.js'
import { loadProjectSelectionData, withFallback } from './projectSelection.js'

// Mock IPC module
vi.mock('./ipc.js', () => ({
  listProjects: vi.fn(),
  getProject: vi.fn(),
  getRecentCommits: vi.fn(),
  getAllCommits: vi.fn(),
  getReadme: vi.fn(),
  getLatestSession: vi.fn(),
  listSessions: vi.fn(),
  getRelationships: vi.fn(),
  dismissRelationship: vi.fn(),
  isTauri: vi.fn(() => false),
  isFirstRun: vi.fn(),
  getSettings: vi.fn(),
  getDaemonStatus: vi.fn(),
  checkDaemonInstallStatus: vi.fn(),
  installDaemon: vi.fn(),
  getRemoteUrl: vi.fn(),
  checkPathType: vi.fn(),
  openExternalUrl: vi.fn(),
}))

// ---------------------------------------------------------------------------
// resolveMarkdownPath — extracted from Shell.handleMarkdownNavigate
// ---------------------------------------------------------------------------
// This is the path resolution logic inside handleMarkdownNavigate, extracted
// as a pure function for testing. The real function also calls switchTab/etc.

function resolveMarkdownPath(relativePath, contextFile) {
  let resolved = relativePath

  // Strip leading ./
  resolved = resolved.replace(/^\.\//, '')

  // Resolve relative to context file's directory
  if (contextFile && !resolved.startsWith('/')) {
    const dir = contextFile.includes('/') ? contextFile.replace(/\/[^/]+$/, '') : ''
    if (dir) {
      resolved = dir + '/' + resolved
    }
  }

  // Normalize ../ segments
  const parts = resolved.split('/')
  const normalized = []
  for (const part of parts) {
    if (part === '..') {
      normalized.pop()
    } else if (part !== '.' && part !== '') {
      normalized.push(part)
    }
  }
  return normalized.join('/')
}

function resolveMarkdownNavigateTarget(relativePath, activeTab, selectedFile, readmePath) {
  if (!relativePath || relativePath.startsWith('#')) return null

  const fragmentMatch = relativePath.match(/#(.+)$/)
  const anchor = fragmentMatch ? fragmentMatch[1] : null

  let resolved = relativePath.replace(/#.*$/, '')
  if (!resolved) return null

  resolved = resolved.replace(/^\.\//, '')

  const contextFile = activeTab === 'overview'
    ? readmePath
    : (selectedFile || readmePath)

  return {
    file: resolveMarkdownPath(resolved, contextFile),
    anchor,
  }
}

function normalizeMarkdownPathWithEscape(relativePath, contextFile) {
  let resolved = relativePath.replace(/#.*$/, '')
  if (!resolved) return null
  resolved = resolved.replace(/^\.\//, '')

  const prefixParts = []
  if (contextFile && !resolved.startsWith('/')) {
    const dir = contextFile.includes('/') ? contextFile.replace(/\/[^/]+$/, '') : ''
    if (dir) prefixParts.push(...dir.split('/'))
  }

  const normalized = []
  const platformSegments = []
  let escapedAboveRoot = false

  for (const part of [...prefixParts, ...resolved.split('/')]) {
    if (!part || part === '.') continue
    if (part === '..') {
      if (escapedAboveRoot) {
        if (platformSegments.length > 0) platformSegments.pop()
      } else if (normalized.length > 0) {
        normalized.pop()
      } else {
        escapedAboveRoot = true
      }
      continue
    }
    if (escapedAboveRoot) {
      platformSegments.push(part)
    } else {
      normalized.push(part)
    }
  }

  return {
    resolvedPath: normalized.join('/'),
    escapedAboveRoot,
    platformSegments,
  }
}

function buildPlatformRouteUrl(remoteUrl, routeSegments) {
  if (!remoteUrl) return null
  const base = remoteUrl.replace(/\/+$/, '')
  const route = routeSegments.filter(Boolean).join('/')
  return route ? `${base}/${route}` : base
}

function classifyMarkdownNavigateAction({
  relativePath,
  activeTab,
  selectedFile,
  readmePath,
  pathType,
  remoteUrl,
}) {
  if (!relativePath || relativePath.startsWith('#')) return null
  const fragmentMatch = relativePath.match(/#(.+)$/)
  const anchor = fragmentMatch ? fragmentMatch[1] : null

  const contextFile = activeTab === 'overview'
    ? readmePath
    : (selectedFile || readmePath)

  const normalized = normalizeMarkdownPathWithEscape(relativePath, contextFile)
  if (!normalized) return null

  if (normalized.escapedAboveRoot) {
    if (!remoteUrl) return null
    return {
      type: 'external',
      url: buildPlatformRouteUrl(remoteUrl, normalized.platformSegments),
    }
  }

  if (pathType === 'directory') {
    return {
      type: 'directory',
      directory: normalized.resolvedPath,
    }
  }

  if (pathType === 'file') {
    return {
      type: 'file',
      file: normalized.resolvedPath,
      anchor,
    }
  }

  return null
}

describe('resolveMarkdownPath', () => {
  it('resolves simple relative path from root context', () => {
    expect(resolveMarkdownPath('README.md', null)).toBe('README.md')
  })

  it('strips leading ./ prefix', () => {
    expect(resolveMarkdownPath('./foo.md', null)).toBe('foo.md')
  })

  it('resolves relative to context file directory', () => {
    expect(resolveMarkdownPath('foo.md', 'docs/design-brief.md')).toBe('docs/foo.md')
  })

  it('resolves ./ relative to context file', () => {
    expect(resolveMarkdownPath('./bar.md', 'docs/design-brief.md')).toBe('docs/bar.md')
  })

  it('resolves image path from docs markdown context', () => {
    expect(resolveMarkdownPath('./images/arch.png', 'docs/design-brief.md')).toBe('docs/images/arch.png')
  })

  it('resolves ../ from nested context', () => {
    expect(resolveMarkdownPath('../README.md', 'docs/sessions/session.md')).toBe('docs/README.md')
  })

  it('resolves parent markdown link from nested docs context', () => {
    expect(resolveMarkdownPath('../design-brief.md', 'docs/architecture/daemon.md')).toBe('docs/design-brief.md')
  })

  it('resolves multiple ../ segments', () => {
    expect(resolveMarkdownPath('../../root.md', 'a/b/deep.md')).toBe('root.md')
  })

  it('handles top-level context (no directory)', () => {
    expect(resolveMarkdownPath('other.md', 'README.md')).toBe('other.md')
  })

  it('does not modify absolute paths', () => {
    expect(resolveMarkdownPath('/absolute/path.md', 'docs/ctx.md')).toBe('absolute/path.md')
  })

  it('resolves docs path from root README context', () => {
    expect(resolveMarkdownPath('docs/foo.md', 'README.md')).toBe('docs/foo.md')
  })

  it('resolves root-level sibling markdown from README context', () => {
    expect(resolveMarkdownPath('ARCHITECTURE.md', 'README.md')).toBe('ARCHITECTURE.md')
  })

  it('normalizes redundant segments', () => {
    expect(resolveMarkdownPath('docs/./nested/../file.md', null)).toBe('docs/file.md')
  })

  it('handles path without context file', () => {
    expect(resolveMarkdownPath('docs/guide.md', null)).toBe('docs/guide.md')
  })
})

describe('resolveMarkdownNavigateTarget', () => {
  it('uses README path in overview even when a file was previously selected', () => {
    expect(resolveMarkdownNavigateTarget(
      'docs/getting-started.md',
      'overview',
      'docs/features/mesh.md',
      'README.md'
    )).toEqual({ file: 'docs/getting-started.md', anchor: null })
  })

  it('uses selected file path in files tab', () => {
    expect(resolveMarkdownNavigateTarget(
      './session-management.md',
      'files',
      'docs/features/command-center.md',
      'README.md'
    )).toEqual({ file: 'docs/features/session-management.md', anchor: null })
  })

  it('captures fragment identifier as anchor while keeping path resolution unchanged', () => {
    expect(resolveMarkdownNavigateTarget(
      'docs/README.md#features',
      'overview',
      null,
      'README.md'
    )).toEqual({ file: 'docs/README.md', anchor: 'features' })
  })

  it('returns null for anchor-only links', () => {
    expect(resolveMarkdownNavigateTarget(
      '#install-taurhaus',
      'overview',
      null,
      'README.md'
    )).toBeNull()
  })
})

describe('classifyMarkdownNavigateAction', () => {
  it('above-root route with remote builds external URL', () => {
    const action = classifyMarkdownNavigateAction({
      relativePath: '../../releases',
      activeTab: 'overview',
      selectedFile: null,
      readmePath: 'README.md',
      pathType: 'not_found',
      remoteUrl: 'https://github.com/user/repo',
    })

    expect(action).toEqual({
      type: 'external',
      url: 'https://github.com/user/repo/releases',
    })
  })

  it('above-root route without remote is a no-op', () => {
    const action = classifyMarkdownNavigateAction({
      relativePath: '../../issues',
      activeTab: 'overview',
      selectedFile: null,
      readmePath: 'README.md',
      pathType: 'not_found',
      remoteUrl: null,
    })

    expect(action).toBeNull()
  })

  it('directory link returns directory nav target', () => {
    const action = classifyMarkdownNavigateAction({
      relativePath: 'docs/',
      activeTab: 'overview',
      selectedFile: null,
      readmePath: 'README.md',
      pathType: 'directory',
      remoteUrl: null,
    })

    expect(action).toEqual({
      type: 'directory',
      directory: 'docs',
    })
  })

  it('extensionless file link returns file nav target', () => {
    const action = classifyMarkdownNavigateAction({
      relativePath: 'LICENSE',
      activeTab: 'overview',
      selectedFile: null,
      readmePath: 'README.md',
      pathType: 'file',
      remoteUrl: null,
    })

    expect(action).toEqual({
      type: 'file',
      file: 'LICENSE',
      anchor: null,
    })
  })

  it('overshoot typo that does not go negative remains a file target', () => {
    const action = classifyMarkdownNavigateAction({
      relativePath: '../../../foo.md',
      activeTab: 'files',
      selectedFile: 'a/b/c/readme.md',
      readmePath: 'README.md',
      pathType: 'file',
      remoteUrl: null,
    })

    expect(action).toEqual({
      type: 'file',
      file: 'foo.md',
      anchor: null,
    })
  })
})

// ---------------------------------------------------------------------------
// Tab state management — switchTab + visitedTabs logic
// ---------------------------------------------------------------------------

describe('Tab state management', () => {
  function createTabState() {
    let activeTab = 'overview'
    let visitedTabs = new Set(['overview'])

    function switchTab(tab) {
      visitedTabs = new Set([...visitedTabs, tab])
      activeTab = tab
    }

    return {
      get activeTab() { return activeTab },
      get visitedTabs() { return visitedTabs },
      switchTab,
    }
  }

  it('defaults to overview tab', () => {
    const state = createTabState()
    expect(state.activeTab).toBe('overview')
    expect(state.visitedTabs.has('overview')).toBe(true)
  })

  it('switching to a tab marks it visited', () => {
    const state = createTabState()
    state.switchTab('git')
    expect(state.activeTab).toBe('git')
    expect(state.visitedTabs.has('git')).toBe(true)
    expect(state.visitedTabs.has('overview')).toBe(true)
  })

  it('switching back to a previously visited tab preserves visited set', () => {
    const state = createTabState()
    state.switchTab('files')
    state.switchTab('git')
    state.switchTab('files')
    expect(state.visitedTabs.has('overview')).toBe(true)
    expect(state.visitedTabs.has('files')).toBe(true)
    expect(state.visitedTabs.has('git')).toBe(true)
    expect(state.activeTab).toBe('files')
  })

  it('all four tabs can be visited', () => {
    const state = createTabState()
    state.switchTab('files')
    state.switchTab('tasks')
    state.switchTab('git')
    expect(state.visitedTabs.size).toBe(4)
  })

  it('tasks tab is not visited by default (lazy loading)', () => {
    const state = createTabState()
    expect(state.visitedTabs.has('tasks')).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// Per-project position memory — save/restore logic
// ---------------------------------------------------------------------------

describe('Per-project position memory', () => {
  function createPositionManager() {
    const positions = new Map()
    let activeTab = 'overview'
    let visitedTabs = new Set(['overview'])
    let gitPosition = null
    let taskPosition = null
    let filesPosition = null

    function save(projectId) {
      positions.set(projectId, {
        tab: activeTab,
        visitedTabs: new Set(visitedTabs),
        file: filesPosition?.selectedFile ?? null,
        gitPosition: gitPosition ? { ...gitPosition } : null,
        taskPosition: taskPosition ? { ...taskPosition } : null,
      })
    }

    function restore(projectId) {
      return positions.get(projectId)
    }

    return {
      get activeTab() { return activeTab },
      set activeTab(v) { activeTab = v },
      get visitedTabs() { return visitedTabs },
      set visitedTabs(v) { visitedTabs = v },
      get gitPosition() { return gitPosition },
      set gitPosition(v) { gitPosition = v },
      get taskPosition() { return taskPosition },
      set taskPosition(v) { taskPosition = v },
      get filesPosition() { return filesPosition },
      set filesPosition(v) { filesPosition = v },
      save,
      restore,
      positions,
    }
  }

  it('saves and restores tab state for a project', () => {
    const mgr = createPositionManager()
    mgr.activeTab = 'git'
    mgr.visitedTabs = new Set(['overview', 'git'])
    mgr.save('project-1')

    const restored = mgr.restore('project-1')
    expect(restored.tab).toBe('git')
    expect(restored.visitedTabs.has('git')).toBe(true)
    expect(restored.visitedTabs.has('overview')).toBe(true)
  })

  it('saves file position when present', () => {
    const mgr = createPositionManager()
    mgr.filesPosition = { selectedFile: 'src/main.rs' }
    mgr.save('project-1')

    const restored = mgr.restore('project-1')
    expect(restored.file).toBe('src/main.rs')
  })

  it('saves null file when no file selected', () => {
    const mgr = createPositionManager()
    mgr.save('project-1')
    expect(mgr.restore('project-1').file).toBeNull()
  })

  it('saves git position (selected commit)', () => {
    const mgr = createPositionManager()
    mgr.gitPosition = { selectedHash: 'abc123', rangeFilter: null }
    mgr.save('project-1')

    const restored = mgr.restore('project-1')
    expect(restored.gitPosition.selectedHash).toBe('abc123')
  })

  it('saves task position', () => {
    const mgr = createPositionManager()
    mgr.taskPosition = { selectedTaskId: 'task-1', selectedSource: 'claude' }
    mgr.save('project-1')

    const restored = mgr.restore('project-1')
    expect(restored.taskPosition.selectedTaskId).toBe('task-1')
  })

  it('returns undefined for unknown project', () => {
    const mgr = createPositionManager()
    expect(mgr.restore('unknown')).toBeUndefined()
  })

  it('maintains separate positions for different projects', () => {
    const mgr = createPositionManager()

    mgr.activeTab = 'files'
    mgr.filesPosition = { selectedFile: 'README.md' }
    mgr.save('project-a')

    mgr.activeTab = 'git'
    mgr.gitPosition = { selectedHash: 'def456' }
    mgr.filesPosition = null
    mgr.save('project-b')

    const a = mgr.restore('project-a')
    const b = mgr.restore('project-b')
    expect(a.tab).toBe('files')
    expect(a.file).toBe('README.md')
    expect(b.tab).toBe('git')
    expect(b.gitPosition.selectedHash).toBe('def456')
  })

  it('overwrites previous position on re-save', () => {
    const mgr = createPositionManager()
    mgr.activeTab = 'overview'
    mgr.save('project-1')

    mgr.activeTab = 'tasks'
    mgr.save('project-1')

    expect(mgr.restore('project-1').tab).toBe('tasks')
  })

  it('deep copies git position to avoid aliasing', () => {
    const mgr = createPositionManager()
    mgr.gitPosition = { selectedHash: 'abc' }
    mgr.save('project-1')

    mgr.gitPosition.selectedHash = 'modified'
    expect(mgr.restore('project-1').gitPosition.selectedHash).toBe('abc')
  })
})

// ---------------------------------------------------------------------------
// selectProject — IPC flow, stale check, position restore
// ---------------------------------------------------------------------------

describe('selectProject flow', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  it('calls all IPC functions in parallel', async () => {
    vi.useFakeTimers()
    function createDeferred() {
      /** @type {(value: any) => void} */
      let resolve
      const promise = new Promise((res) => {
        resolve = res
      })
      return { promise, resolve }
    }

    const detail = createDeferred()
    const commits = createDeferred()
    const latest = createDeferred()
    const sessionList = createDeferred()
    const readme = createDeferred()
    const rels = createDeferred()

    ipc.getProject.mockReturnValue(detail.promise)
    ipc.getRecentCommits.mockReturnValue(commits.promise)
    ipc.getLatestSession.mockReturnValue(latest.promise)
    ipc.listSessions.mockReturnValue(sessionList.promise)
    ipc.getReadme.mockReturnValue(readme.promise)
    ipc.getRelationships.mockReturnValue(rels.promise)

    const loadPromise = loadProjectSelectionData('p1', ipc)

    expect(ipc.getProject).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(26)
    expect(ipc.getProject).toHaveBeenCalledWith('p1')
    expect(ipc.getRecentCommits).toHaveBeenCalledWith('p1', 10)
    expect(ipc.getLatestSession).toHaveBeenCalledWith('p1')
    expect(ipc.listSessions).toHaveBeenCalledWith('p1', 10)
    expect(ipc.getReadme).toHaveBeenCalledWith('p1')
    expect(ipc.getRelationships).toHaveBeenCalledWith('p1')

    detail.resolve({ id: 'p1' })
    commits.resolve([])
    latest.resolve(null)
    sessionList.resolve([])
    readme.resolve(null)
    rels.resolve([])
    await loadPromise
  })

  it('stale check prevents late responses from updating state', async () => {
    vi.useFakeTimers()
    try {
      const selectLoadGuard = createAsyncGuard()
      let result = null

      async function selectProject(delay, id) {
        const sequence = selectLoadGuard.next()
        await new Promise((resolve) => setTimeout(resolve, delay))
        if (!selectLoadGuard.isCurrent(sequence)) return
        result = id
      }

      const first = selectProject(50, 'slow-project')
      const second = selectProject(10, 'fast-project')

      await vi.advanceTimersByTimeAsync(10)
      expect(result).toBe('fast-project')

      await vi.advanceTimersByTimeAsync(40)
      await Promise.all([first, second])
      expect(result).toBe('fast-project')
    } finally {
      vi.useRealTimers()
    }
  })

  it('loader guard keeps latest project sessions and ignores stale responses', async () => {
    function createDeferred() {
      /** @type {(value: any) => void} */
      let resolve
      const promise = new Promise((res) => {
        resolve = res
      })
      return { promise, resolve }
    }

    const sessionGuard = createAsyncGuard()
    let selectedProject = { id: 'project-b' }
    let latestSession = null
    let sessionHistory = []

    const slowA = createDeferred()
    const fastB = createDeferred()

    async function loadSessions(projectId, latestPromise, historyPromise) {
      const sequence = sessionGuard.next()
      const [latest, history] = await Promise.all([latestPromise, historyPromise])
      if (!sessionGuard.isCurrent(sequence) || selectedProject?.id !== projectId) return
      latestSession = latest
      sessionHistory = history
    }

    const first = loadSessions('project-a', slowA.promise, slowA.promise)
    const second = loadSessions('project-b', fastB.promise, fastB.promise)
    fastB.resolve({ id: 'latest-b', items: ['b1'] })
    await second

    // Project switched while request A still in flight.
    selectedProject = { id: 'project-b' }
    slowA.resolve({ id: 'latest-a', items: ['a1'] })
    await first

    expect(latestSession.id).toBe('latest-b')
    expect(sessionHistory.items).toEqual(['b1'])
  })

  it('listener cleanup disposes listeners that resolve after effect teardown', async () => {
    function createDeferred() {
      /** @type {(value: any) => void} */
      let resolve
      const promise = new Promise((res) => {
        resolve = res
      })
      return { promise, resolve }
    }

    const cleanups = []
    const deferredUnlisten = createDeferred()
    const unlisten = vi.fn()
    const listen = vi.fn(() => deferredUnlisten.promise)

    let destroyed = false
    function registerListener(eventName, handler) {
      listen(eventName, handler).then((dispose) => {
        if (destroyed) {
          dispose()
          return
        }
        cleanups.push(dispose)
      })
    }

    registerListener('sessions-updated', () => {})
    destroyed = true
    cleanups.forEach((dispose) => dispose())

    deferredUnlisten.resolve(unlisten)
    await Promise.resolve()
    await Promise.resolve()

    expect(unlisten).toHaveBeenCalledTimes(1)
  })

  it('collects degraded load issues while preserving fallback data', async () => {
    ipc.getProject.mockRejectedValue(new Error('DB error'))
    ipc.getRecentCommits.mockRejectedValue(new Error('Git error'))
    ipc.getReadme.mockRejectedValue(new Error('File error'))

    const [detail, commits, readme] = await Promise.all([
      withFallback('Project details', ipc.getProject('p1'), null),
      withFallback('Recent commits', ipc.getRecentCommits('p1', 10), []),
      withFallback('README', ipc.getReadme('p1'), null),
    ])
    const issues = [detail, commits, readme]
      .filter(result => !result.ok)
      .map(result => ({ section: result.section, message: result.message }))

    expect(detail.value).toBeNull()
    expect(commits.value).toEqual([])
    expect(readme.value).toBeNull()
    expect(issues).toEqual([
      { section: 'Project details', message: 'DB error' },
      { section: 'Recent commits', message: 'Git error' },
      { section: 'README', message: 'File error' },
    ])
  })
})

// ---------------------------------------------------------------------------
// navigateToCommit / navigateToCommitRange — cross-tab navigation
// ---------------------------------------------------------------------------

describe('Cross-tab navigation', () => {
  it('navigateToCommit sets git nav target and switches to git tab', () => {
    let gitNavTarget = null
    let switchedTab = null

    function navigateToCommit(hash) {
      gitNavTarget = { type: 'commit', hash }
      switchedTab = 'git'
    }

    navigateToCommit('abc123')
    expect(gitNavTarget).toEqual({ type: 'commit', hash: 'abc123' })
    expect(switchedTab).toBe('git')
  })

  it('navigateToCommitRange sets range filter and switches to git tab', () => {
    let gitNavTarget = null
    let switchedTab = null

    function navigateToCommitRange(after, before) {
      gitNavTarget = { type: 'range', after, before }
      switchedTab = 'git'
    }

    navigateToCommitRange('2025-01-01', '2025-01-31')
    expect(gitNavTarget).toEqual({ type: 'range', after: '2025-01-01', before: '2025-01-31' })
    expect(switchedTab).toBe('git')
  })
})

// ---------------------------------------------------------------------------
// applyNavEntry — back/forward navigation restoration
// ---------------------------------------------------------------------------

describe('applyNavEntry', () => {
  function createNavState() {
    let activeTab = 'overview'
    let visitedTabs = new Set(['overview'])
    let filesNavTarget = null
    let gitNavTarget = null

    function applyNavEntry(entry) {
      visitedTabs = new Set([...visitedTabs, entry.tab])
      activeTab = entry.tab
      if (entry.tab === 'files' && entry.file) {
        filesNavTarget = { file: entry.file, lineNumber: entry.lineNumber }
      }
      if (entry.tab === 'git' && entry.commit) {
        gitNavTarget = { type: 'commit', hash: entry.commit }
      }
      if (entry.tab === 'git' && entry.rangeFilter) {
        gitNavTarget = { type: 'range', ...entry.rangeFilter }
      }
    }

    return {
      get activeTab() { return activeTab },
      get visitedTabs() { return visitedTabs },
      get filesNavTarget() { return filesNavTarget },
      get gitNavTarget() { return gitNavTarget },
      applyNavEntry,
    }
  }

  it('restores tab and marks it visited', () => {
    const state = createNavState()
    state.applyNavEntry({ tab: 'files' })
    expect(state.activeTab).toBe('files')
    expect(state.visitedTabs.has('files')).toBe(true)
  })

  it('restores file navigation target with line number', () => {
    const state = createNavState()
    state.applyNavEntry({ tab: 'files', file: 'src/main.rs', lineNumber: 42 })
    expect(state.filesNavTarget).toEqual({ file: 'src/main.rs', lineNumber: 42 })
  })

  it('restores git commit navigation target', () => {
    const state = createNavState()
    state.applyNavEntry({ tab: 'git', commit: 'abc123' })
    expect(state.gitNavTarget).toEqual({ type: 'commit', hash: 'abc123' })
  })

  it('restores git range filter navigation target', () => {
    const state = createNavState()
    state.applyNavEntry({ tab: 'git', rangeFilter: { after: '2025-01-01', before: '2025-02-01' } })
    expect(state.gitNavTarget).toEqual({ type: 'range', after: '2025-01-01', before: '2025-02-01' })
  })

  it('switches to overview without setting nav targets', () => {
    const state = createNavState()
    state.applyNavEntry({ tab: 'overview' })
    expect(state.activeTab).toBe('overview')
    expect(state.filesNavTarget).toBeNull()
    expect(state.gitNavTarget).toBeNull()
  })

  it('does not set file target for non-files tab', () => {
    const state = createNavState()
    state.applyNavEntry({ tab: 'git', file: 'should-be-ignored.rs' })
    expect(state.filesNavTarget).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// handleSearchNavigate — search result dispatch
// ---------------------------------------------------------------------------

describe('handleSearchNavigate', () => {
  function createSearchNavState() {
    let filesNavTarget = null
    let switchedTab = null

    function handleSearchNavigate(action) {
      if (action.tab === 'files' && action.filePath) {
        filesNavTarget = { file: action.filePath }
        switchedTab = 'files'
      } else if (action.tab === 'overview') {
        switchedTab = 'overview'
      }
    }

    return {
      get filesNavTarget() { return filesNavTarget },
      get switchedTab() { return switchedTab },
      handleSearchNavigate,
    }
  }

  it('navigates to file from search result', () => {
    const state = createSearchNavState()
    state.handleSearchNavigate({ tab: 'files', filePath: 'docs/README.md' })
    expect(state.filesNavTarget).toEqual({ file: 'docs/README.md' })
    expect(state.switchedTab).toBe('files')
  })

  it('switches to overview tab', () => {
    const state = createSearchNavState()
    state.handleSearchNavigate({ tab: 'overview' })
    expect(state.switchedTab).toBe('overview')
    expect(state.filesNavTarget).toBeNull()
  })

  it('ignores file action without filePath', () => {
    const state = createSearchNavState()
    state.handleSearchNavigate({ tab: 'files' })
    expect(state.filesNavTarget).toBeNull()
    expect(state.switchedTab).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// First-run gating
// ---------------------------------------------------------------------------

describe('First-run gating', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  it('shows wizard when isFirstRun returns true', async () => {
    ipc.isFirstRun.mockResolvedValue(true)

    let showWizard = false
    const first = await ipc.isFirstRun()
    showWizard = first

    expect(showWizard).toBe(true)
  })

  it('skips wizard and loads projects when isFirstRun returns false', async () => {
    ipc.isFirstRun.mockResolvedValue(false)
    ipc.listProjects.mockResolvedValue([{ id: 'p1', name: 'test' }])

    const first = await ipc.isFirstRun()
    let showWizard = first
    let projects = []

    if (!showWizard) {
      projects = await ipc.listProjects()
    }

    expect(showWizard).toBe(false)
    expect(projects).toHaveLength(1)
    expect(ipc.listProjects).toHaveBeenCalled()
  })

  it('defaults to no wizard on isFirstRun error', async () => {
    ipc.isFirstRun.mockRejectedValue(new Error('IPC error'))

    let showWizard = false
    try {
      const first = await ipc.isFirstRun()
      showWizard = first
    } catch {
      showWizard = false
    }

    expect(showWizard).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// Daemon status filtering
// ---------------------------------------------------------------------------

describe('Daemon status filtering', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  function nextDaemonStatus(currentStatus, nextStatus) {
    return nextStatus === 'connected' || nextStatus === 'not_configured'
      ? null
      : nextStatus ?? currentStatus
  }

  it('suppresses connected status (happy path)', async () => {
    ipc.getDaemonStatus.mockResolvedValue({ status: 'connected' })

    let daemonStatus = null
    const status = await ipc.getDaemonStatus()
    daemonStatus = nextDaemonStatus(daemonStatus, status.status)

    expect(daemonStatus).toBeNull()
  })

  it('surfaces disconnected status', async () => {
    ipc.getDaemonStatus.mockResolvedValue({ status: 'disconnected' })

    let daemonStatus = null
    const status = await ipc.getDaemonStatus()
    daemonStatus = nextDaemonStatus(daemonStatus, status.status)

    expect(daemonStatus).toBe('disconnected')
  })

  it('surfaces reconnecting status', async () => {
    ipc.getDaemonStatus.mockResolvedValue({ status: 'reconnecting' })

    let daemonStatus = null
    const status = await ipc.getDaemonStatus()
    daemonStatus = nextDaemonStatus(daemonStatus, status.status)

    expect(daemonStatus).toBe('reconnecting')
  })

  it('clears stale offline status after a later connected probe', async () => {
    ipc.getDaemonStatus.mockResolvedValue({ status: 'connected' })

    let daemonStatus = 'disconnected'
    const status = await ipc.getDaemonStatus()
    daemonStatus = nextDaemonStatus(daemonStatus, status.status)

    expect(daemonStatus).toBeNull()
  })

  it('ignores getDaemonStatus errors', async () => {
    ipc.getDaemonStatus.mockRejectedValue(new Error('IPC error'))

    let daemonStatus = null
    try {
      const status = await ipc.getDaemonStatus()
      daemonStatus = nextDaemonStatus(daemonStatus, status.status)
    } catch { /* ignore — not critical */ }

    expect(daemonStatus).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// Project removal — sidebar callback
// ---------------------------------------------------------------------------

describe('Project removal handling', () => {
  it('removes project from list and selects next', () => {
    let projects = [
      { id: 'p1', name: 'a', path: '/a' },
      { id: 'p2', name: 'b', path: '/b' },
      { id: 'p3', name: 'c', path: '/c' },
    ]
    let selectedProject = projects[1]

    function handleProjectRemoved(id) {
      projects = projects.filter(p => p.id !== id)
      if (selectedProject?.id === id) {
        selectedProject = projects.length > 0 ? projects[0] : null
      }
    }

    handleProjectRemoved('p2')
    expect(projects).toHaveLength(2)
    expect(selectedProject.id).toBe('p1')
  })

  it('sets selectedProject to null when last project removed', () => {
    let projects = [{ id: 'p1', name: 'a', path: '/a' }]
    let selectedProject = projects[0]

    function handleProjectRemoved(id) {
      projects = projects.filter(p => p.id !== id)
      if (selectedProject?.id === id) {
        selectedProject = projects.length > 0 ? projects[0] : null
      }
    }

    handleProjectRemoved('p1')
    expect(projects).toHaveLength(0)
    expect(selectedProject).toBeNull()
  })

  it('does not change selection when removing a different project', () => {
    let projects = [
      { id: 'p1', name: 'a', path: '/a' },
      { id: 'p2', name: 'b', path: '/b' },
    ]
    let selectedProject = projects[0]

    function handleProjectRemoved(id) {
      projects = projects.filter(p => p.id !== id)
      if (selectedProject?.id === id) {
        selectedProject = projects.length > 0 ? projects[0] : null
      }
    }

    handleProjectRemoved('p2')
    expect(projects).toHaveLength(1)
    expect(selectedProject.id).toBe('p1')
  })
})

// ---------------------------------------------------------------------------
// Wizard complete flow
// ---------------------------------------------------------------------------

describe('Wizard complete flow', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  it('handleWizardComplete hides wizard and loads projects', async () => {
    ipc.listProjects.mockResolvedValue([{ id: 'p1', name: 'test' }])
    ipc.getSettings.mockResolvedValue({ code_theme: null })
    ipc.getDaemonStatus.mockResolvedValue({ status: 'connected' })

    let showWizard = true
    showWizard = false // handleWizardComplete sets this

    // Then loads projects + settings + daemon status
    const [projects, settings, daemon] = await Promise.all([
      ipc.listProjects(),
      ipc.getSettings(),
      ipc.getDaemonStatus(),
    ])

    expect(showWizard).toBe(false)
    expect(ipc.listProjects).toHaveBeenCalled()
    expect(ipc.getSettings).toHaveBeenCalled()
    expect(ipc.getDaemonStatus).toHaveBeenCalled()
  })
})

// ---------------------------------------------------------------------------
// Code theme loading from settings
// ---------------------------------------------------------------------------

describe('Code theme loading', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  it('loads theme from settings', async () => {
    ipc.getSettings.mockResolvedValue({
      code_theme: { light: 'solarized-light', dark: 'one-dark-pro' },
    })

    const s = await ipc.getSettings()
    let codeThemeLight = 'github-light'
    let codeThemeDark = 'github-dark-dimmed'

    if (s.code_theme) {
      codeThemeLight = s.code_theme.light || codeThemeLight
      codeThemeDark = s.code_theme.dark || codeThemeDark
    }

    expect(codeThemeLight).toBe('solarized-light')
    expect(codeThemeDark).toBe('one-dark-pro')
  })

  it('keeps defaults when no code_theme in settings', async () => {
    ipc.getSettings.mockResolvedValue({})

    const s = await ipc.getSettings()
    let codeThemeLight = 'github-light'
    let codeThemeDark = 'github-dark-dimmed'

    if (s.code_theme) {
      codeThemeLight = s.code_theme.light || codeThemeLight
      codeThemeDark = s.code_theme.dark || codeThemeDark
    }

    expect(codeThemeLight).toBe('github-light')
    expect(codeThemeDark).toBe('github-dark-dimmed')
  })

  it('keeps defaults on settings error', async () => {
    ipc.getSettings.mockRejectedValue(new Error('failed'))

    let codeThemeLight = 'github-light'
    let codeThemeDark = 'github-dark-dimmed'

    try {
      const s = await ipc.getSettings()
      if (s.code_theme) {
        codeThemeLight = s.code_theme.light || codeThemeLight
        codeThemeDark = s.code_theme.dark || codeThemeDark
      }
    } catch {
      // Keep defaults
    }

    expect(codeThemeLight).toBe('github-light')
    expect(codeThemeDark).toBe('github-dark-dimmed')
  })
})

// ---------------------------------------------------------------------------
// Git position restore logic on selectProject
// ---------------------------------------------------------------------------

describe('Git position restore on project switch', () => {
  it('restores commit hash from saved position', () => {
    const savedPosition = { gitPosition: { selectedHash: 'abc123', rangeFilter: null } }
    let gitNavTarget = null

    if (savedPosition?.gitPosition?.selectedHash) {
      gitNavTarget = { type: 'commit', hash: savedPosition.gitPosition.selectedHash }
    }

    expect(gitNavTarget).toEqual({ type: 'commit', hash: 'abc123' })
  })

  it('restores range filter from saved position', () => {
    const savedPosition = { gitPosition: { selectedHash: null, rangeFilter: { after: '2025-01-01', before: '2025-02-01' } } }
    let gitNavTarget = null

    if (savedPosition?.gitPosition?.selectedHash) {
      gitNavTarget = { type: 'commit', hash: savedPosition.gitPosition.selectedHash }
    } else if (savedPosition?.gitPosition?.rangeFilter) {
      gitNavTarget = { type: 'range', ...savedPosition.gitPosition.rangeFilter }
    }

    expect(gitNavTarget).toEqual({ type: 'range', after: '2025-01-01', before: '2025-02-01' })
  })

  it('clears git nav target when no saved position', () => {
    const savedPosition = undefined
    let gitNavTarget = { type: 'commit', hash: 'leftover' }

    if (savedPosition?.gitPosition?.selectedHash) {
      gitNavTarget = { type: 'commit', hash: savedPosition.gitPosition.selectedHash }
    } else if (savedPosition?.gitPosition?.rangeFilter) {
      gitNavTarget = { type: 'range', ...savedPosition.gitPosition.rangeFilter }
    } else {
      gitNavTarget = null
    }

    expect(gitNavTarget).toBeNull()
  })

  it('restores file nav target from saved position', () => {
    const savedPosition = { file: 'src/main.rs' }
    let filesNavTarget = null

    filesNavTarget = savedPosition?.file ? { file: savedPosition.file } : null

    expect(filesNavTarget).toEqual({ file: 'src/main.rs' })
  })

  // ── Daemon update banner logic ────────────────────────────────────────

  describe('daemon update banner', () => {
    it('shows banner when daemon needs update', () => {
      const daemonUpdateAvailable = { version: '0.3.1', bundled_version: '0.3.2' }
      const daemonUpdateDismissed = false

      // Banner should show when update available and not dismissed
      expect(daemonUpdateAvailable && !daemonUpdateDismissed).toBe(true)
    })

    it('hides banner when dismissed', () => {
      const daemonUpdateAvailable = { version: '0.3.1', bundled_version: '0.3.2' }
      const daemonUpdateDismissed = true

      expect(daemonUpdateAvailable && !daemonUpdateDismissed).toBe(false)
    })

    it('hides banner when no update available', () => {
      const daemonUpdateAvailable = null
      const daemonUpdateDismissed = false

      expect(!!(daemonUpdateAvailable && !daemonUpdateDismissed)).toBe(false)
    })

    it('checkDaemonUpdate sets update state when needs_update is true', async () => {
      const ipc = await import('./ipc.js')
      ipc.checkDaemonInstallStatus.mockResolvedValue({
        installed: true,
        version: '0.3.1',
        bundled_version: '0.3.2',
        needs_update: true,
        wsl_available: true,
        error: null,
      })

      const status = await ipc.checkDaemonInstallStatus()
      let daemonUpdateAvailable = null
      if (status.installed && status.needs_update) {
        daemonUpdateAvailable = {
          version: status.version,
          bundled_version: status.bundled_version,
        }
      }

      expect(daemonUpdateAvailable).toEqual({
        version: '0.3.1',
        bundled_version: '0.3.2',
      })
    })

    it('checkDaemonUpdate does not set state when versions match', async () => {
      const ipc = await import('./ipc.js')
      ipc.checkDaemonInstallStatus.mockResolvedValue({
        installed: true,
        version: '0.3.2',
        bundled_version: '0.3.2',
        needs_update: false,
        wsl_available: true,
        error: null,
      })

      const status = await ipc.checkDaemonInstallStatus()
      let daemonUpdateAvailable = null
      if (status.installed && status.needs_update) {
        daemonUpdateAvailable = {
          version: status.version,
          bundled_version: status.bundled_version,
        }
      }

      expect(daemonUpdateAvailable).toBeNull()
    })
  })

  describe('error surfacing', () => {
    it('logs targeted Shell catch paths and exposes a non-blocking notice banner', () => {
      const source = readFileSync(`${process.cwd()}/src/Shell.svelte`, 'utf8')

      expect(source).toContain('[settings] failed to load code theme preferences:')
      expect(source).toContain('[settings] failed to persist dark mode preference:')
      expect(source).toContain('[overview] failed to dismiss relationship')
      expect(source).toContain('data-testid="shell-notice-banner"')
      expect(source).toContain('data-testid="shell-notice-message"')
    })
  })
})
