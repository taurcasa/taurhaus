export function createProjectPosition({
  activeTab,
  visitedTabs,
  filesPosition,
  gitPosition,
  taskPosition,
}) {
  return {
    tab: activeTab,
    visitedTabs: new Set(visitedTabs),
    file: filesPosition?.selectedFile ?? null,
    gitPosition: gitPosition ? { ...gitPosition } : null,
    taskPosition: taskPosition ? { ...taskPosition } : null,
  }
}

export function switchTabState(visitedTabs, tab) {
  return {
    activeTab: tab,
    visitedTabs: new Set([...visitedTabs, tab]),
  }
}

export function applyNavEntryState(visitedTabs, entry) {
  const nextState = {
    activeTab: entry.tab,
    visitedTabs: new Set([...visitedTabs, entry.tab]),
    filesNavTarget: null,
    gitNavTarget: null,
  }

  if (entry.tab === 'files' && entry.file) {
    nextState.filesNavTarget = { file: entry.file, lineNumber: entry.lineNumber }
  }

  if (entry.tab === 'git' && entry.commit) {
    nextState.gitNavTarget = { type: 'commit', hash: entry.commit }
  }

  if (entry.tab === 'git' && entry.rangeFilter) {
    nextState.gitNavTarget = { type: 'range', ...entry.rangeFilter }
  }

  return nextState
}

export function buildProjectSelectionState({
  project,
  detail,
  commits,
  latest,
  sessionList,
  readme,
  relationships,
  savedPosition,
}) {
  const baseState = buildCriticalProjectSelectionState({ project, savedPosition })

  return {
    ...baseState,
    selectedProject: detail ? { ...project, ...detail } : project,
    detailLoading: false,
    recentCommits: commits || [],
    commitsLoading: false,
    latestSession: latest,
    sessionHistory: sessionList || [],
    sessionLoading: false,
    readmeContent: readme,
    relationships: relationships || [],
    relationshipsLoading: false,
  }
}

export function buildCriticalProjectSelectionState({
  project,
  savedPosition,
}) {
  const restoredTab = savedPosition?.tab || 'overview'

  return {
    selectedProject: project,
    detailLoading: true,
    showAllCommits: false,
    activeTab: restoredTab,
    visitedTabs: savedPosition?.visitedTabs ? new Set(savedPosition.visitedTabs) : new Set([restoredTab]),
    navEntry: { tab: restoredTab, file: savedPosition?.file },
    gitNavTarget: savedPosition?.gitPosition?.selectedHash
      ? { type: 'commit', hash: savedPosition.gitPosition.selectedHash }
      : savedPosition?.gitPosition?.rangeFilter
      ? { type: 'range', ...savedPosition.gitPosition.rangeFilter }
        : null,
    taskNavTarget: savedPosition?.taskPosition ?? null,
    recentCommits: [],
    commitsLoading: true,
    latestSession: null,
    sessionHistory: [],
    sessionLoading: true,
    readmeContent: null,
    relationships: [],
    relationshipsLoading: true,
    filesNavTarget: savedPosition?.file ? { file: savedPosition.file } : null,
  }
}

export function normalizeMarkdownTarget(relativePath, contextFile) {
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

export function buildPlatformRouteUrl(remoteUrl, routeSegments) {
  if (!remoteUrl) return null
  const base = remoteUrl.replace(/\/+$/, '')
  const route = routeSegments.filter(Boolean).join('/')
  return route ? `${base}/${route}` : base
}

export function classifyMarkdownNavigateAction({
  relativePath,
  contextFile,
  pathType,
  remoteUrl,
}) {
  if (!relativePath || relativePath.startsWith('#')) return null

  const fragmentMatch = relativePath.match(/#(.+)$/)
  const anchor = fragmentMatch ? fragmentMatch[1] : null
  const normalized = normalizeMarkdownTarget(relativePath, contextFile)

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

export function createShellNavigationController({
  state,
  ipc,
  selectProject,
  switchTab,
  logger = console,
}) {
  function navigateToCommit(hash) {
    state.gitNavTarget = { type: 'commit', hash }
    switchTab('git', { tab: 'git', commit: hash })
  }

  function navigateToCommitRange(after, before) {
    state.gitNavTarget = { type: 'range', after, before }
    switchTab('git', { tab: 'git', rangeFilter: { after, before } })
  }

  function navigateToFile(path, lineNumber) {
    state.filesNavTarget = { file: path, lineNumber }
    switchTab('files', { tab: 'files', file: path, lineNumber })
  }

  async function handleMarkdownNavigate(relativePath) {
    if (!state.selectedProject) return
    if (!relativePath || relativePath.startsWith('#')) return

    const contextFile = state.activeTab === 'overview'
      ? state.readmeContent?.path
      : (state.filesPosition?.selectedFile || state.readmeContent?.path)

    const normalized = normalizeMarkdownTarget(relativePath, contextFile)
    if (!normalized) return

    if (normalized.escapedAboveRoot) {
      let remoteUrl = null
      try {
        remoteUrl = await ipc.getRemoteUrl(state.selectedProject.id)
      } catch (error) {
        logger.warn('[markdown] failed to resolve remote URL for platform route', error)
        return
      }

      if (!remoteUrl) {
        logger.warn(`[markdown] platform route detected but no remote available: "${relativePath}"`)
        return
      }

      const action = classifyMarkdownNavigateAction({
        relativePath,
        contextFile,
        remoteUrl,
        pathType: 'not_found',
      })
      if (!action?.url) return
      logger.log(`[markdown] navigate platform route: "${relativePath}" → "${action.url}"`)
      ipc.openExternalUrl(action.url).catch((error) => {
        logger.error(`[markdown] failed to open platform route URL: ${action.url}`, error)
      })
      return
    }

    let pathType = 'not_found'
    try {
      pathType = await ipc.checkPathType(state.selectedProject.id, normalized.resolvedPath)
    } catch (error) {
      logger.warn(`[markdown] failed to classify path: "${normalized.resolvedPath}"`, error)
      return
    }

    const action = classifyMarkdownNavigateAction({
      relativePath,
      contextFile,
      remoteUrl: null,
      pathType,
    })

    if (action?.type === 'directory') {
      logger.log(`[markdown] navigate directory: "${relativePath}" → "${action.directory}"`)
      state.filesNavTarget = { directory: action.directory }
      switchTab('files', { tab: 'files' })
      return
    }

    if (action?.type === 'file') {
      logger.log(`[markdown] navigate: "${relativePath}" → "${action.file}"${action.anchor ? ` #${action.anchor}` : ''}`)
      state.filesNavTarget = { file: action.file, anchor: action.anchor }
      switchTab('files', { tab: 'files', file: action.file })
      return
    }

    logger.warn(`[markdown] unresolved markdown path (not_found): "${relativePath}" → "${normalized.resolvedPath}"`)
  }

  async function handleSearchNavigate(action) {
    if (action.projectId && action.projectId !== state.selectedProject?.id) {
      const targetProject = state.projects.find((project) => project.id === action.projectId)
      if (targetProject) {
        await selectProject(targetProject)
      }
    }

    if (action.tab === 'files' && action.filePath) {
      state.filesNavTarget = { file: action.filePath }
      switchTab('files', { tab: 'files', file: action.filePath })
    } else if (action.tab === 'overview') {
      switchTab('overview')
    }
  }

  return {
    navigateToCommit,
    navigateToCommitRange,
    navigateToFile,
    handleMarkdownNavigate,
    handleSearchNavigate,
  }
}
