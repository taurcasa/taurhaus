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
  const restoredTab = savedPosition?.tab || 'overview'

  return {
    selectedProject: detail ? { ...project, ...detail } : project,
    detailLoading: false,
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
    recentCommits: commits || [],
    commitsLoading: false,
    latestSession: latest,
    sessionHistory: sessionList || [],
    sessionLoading: false,
    readmeContent: readme,
    relationships: relationships || [],
    relationshipsLoading: false,
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
