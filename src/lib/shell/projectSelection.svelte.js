import { createAsyncGuard } from '../asyncGuard.js'
import {
  classifyProjectLoadResults,
  loadDeferredProjectSelectionData,
  prefetchProjectSelectionData,
} from '../projectSelection.js'
import {
  applyNavEntryState,
  buildCriticalProjectSelectionState,
  createProjectPosition,
  switchTabState,
} from './navigation.svelte.js'

function errorMessage(error) {
  if (error && typeof error === 'object' && typeof error.message === 'string' && error.message.trim()) {
    return error.message
  }
  if (typeof error === 'string' && error.trim()) {
    return error
  }
  return String(error)
}

function nowMs() {
  if (typeof performance !== 'undefined' && typeof performance.now === 'function') {
    return performance.now()
  }
  return Date.now()
}

function logProjectSelectionLifecycle(logger, event, payload = {}) {
  logger.info('[shell.project-selection] lifecycle', {
    event,
    ...payload,
  })
}

export function createShellProjectSelectionController({
  state,
  positions,
  nav,
  ipc,
  getDaemonRecoveryPending,
  getDaemonStatus,
  getSessionBridgeLive,
  logger = console,
  doc = document,
}) {
  const projectPositions = new Map()
  const sessionsLoadGuard = createAsyncGuard()
  const readmeLoadGuard = createAsyncGuard()
  const relationshipsLoadGuard = createAsyncGuard()
  const selectLoadGuard = createAsyncGuard()

  function saveProjectPosition() {
    if (!state.selectedProject) return
    projectPositions.set(state.selectedProject.id, createProjectPosition({
      activeTab: state.activeTab,
      visitedTabs: state.visitedTabs,
      filesPosition: positions.files,
      gitPosition: positions.git,
      taskPosition: positions.task,
    }))
  }

  function switchTab(tab, navEntry) {
    const nextState = switchTabState(state.visitedTabs, tab)
    state.visitedTabs = nextState.visitedTabs
    state.activeTab = nextState.activeTab
    nav.push(navEntry || { tab })
  }

  function applyNavEntry(entry) {
    nav.withSuppressed(() => {
      const nextState = applyNavEntryState(state.visitedTabs, entry)
      state.visitedTabs = nextState.visitedTabs
      state.activeTab = nextState.activeTab
      state.filesNavTarget = nextState.filesNavTarget
      state.gitNavTarget = nextState.gitNavTarget
    })
  }

  async function loadProjects() {
    state.sidebarLoading = true
    state.sidebarError = null
    try {
      state.projects = await ipc.listProjects()
      if (!state.selectedProject && state.projects.length > 0) {
        const firstProject = state.projects[0]
        void bootstrapInitialProject(firstProject)
      }
    } catch (error) {
      state.sidebarError = error.message || 'Failed to load projects'
      logger.error('[shell] failed to load projects', {
        error_message: errorMessage(error),
      })
    } finally {
      state.sidebarLoading = false
    }
  }

  async function handleProjectCreated(project) {
    await loadProjects()
    const created = state.projects.find((entry) => entry.id === project?.id) || project
    if (created?.id) {
      await selectProject(created)
    }
  }

  async function retryProjectLoad() {
    if (!state.selectedProject) return
    await selectProject(state.selectedProject)
  }

  async function bootstrapInitialProject(project) {
    await selectProject(project)
  }

  function prefetchProjectSelection(project) {
    if (!project?.id || project.id === state.selectedProject?.id) return
    void prefetchProjectSelectionData(project.id, ipc, {
      logger,
      projectPath: project.path ?? null,
      daemonStatus: getDaemonStatus(),
      batchKind: 'deferred',
    })
  }

  async function selectProject(project) {
    const projectId = project.id
    const startedAt = nowMs()

    saveProjectPosition()

    const savedPosition = projectPositions.get(projectId)
    const generation = selectLoadGuard.next()

    state.projectLoadIssues = []
    state.pendingProjectLoadRetry = false
    const nextState = buildCriticalProjectSelectionState({
      project,
      savedPosition,
    })

    state.selectedProject = nextState.selectedProject
    state.detailLoading = nextState.detailLoading
    state.showAllCommits = nextState.showAllCommits
    state.activeTab = nextState.activeTab
    state.visitedTabs = nextState.visitedTabs
    nav.reset()
    nav.push(nextState.navEntry)
    state.gitNavTarget = nextState.gitNavTarget
    state.taskNavTarget = nextState.taskNavTarget
    state.recentCommits = nextState.recentCommits
    state.commitsLoading = nextState.commitsLoading
    state.latestSession = nextState.latestSession
    state.sessionHistory = nextState.sessionHistory
    state.sessionLoading = nextState.sessionLoading
    state.readmeContent = nextState.readmeContent
    state.relationships = nextState.relationships
    state.relationshipsLoading = nextState.relationshipsLoading
    state.filesNavTarget = nextState.filesNavTarget

    logProjectSelectionLifecycle(logger, 'shell.project_selection.started', {
      project_id: projectId,
      project_path: project.path ?? null,
      daemon_status: getDaemonStatus() ?? null,
      session_bridge_live: getSessionBridgeLive(),
      visibility_state: doc.hidden ? 'hidden' : 'visible',
      selection_generation: generation,
      blocking: false,
      deferred: true,
    })

    const { detail, commits, latest, sessionList, readme, rels } = await loadDeferredProjectSelectionData(projectId, ipc, {
      logger,
      projectPath: project.path ?? null,
      daemonStatus: getDaemonStatus(),
      batchKind: 'deferred',
    })

    if (!selectLoadGuard.isCurrent(generation)) {
      logProjectSelectionLifecycle(logger, 'shell.project_selection.discarded', {
        project_id: projectId,
        elapsed_ms: Number((nowMs() - startedAt).toFixed(1)),
        daemon_status: getDaemonStatus() ?? null,
        selection_generation: generation,
        reason: 'stale_generation',
        blocking: false,
        deferred: true,
      })
      return
    }

    const classifiedLoadIssues = classifyProjectLoadResults(
      [detail, commits, latest, sessionList, readme, rels],
      { deferRetryableIssues: getDaemonRecoveryPending() }
    )
    state.pendingProjectLoadRetry = classifiedLoadIssues.pendingRetry
    state.projectLoadIssues = classifiedLoadIssues.visibleIssues
    if (state.projectLoadIssues.length > 0) {
      logger.warn(
        `[shell] project ${projectId} loaded with degraded data`,
        classifiedLoadIssues.issues
      )
    }

    state.selectedProject = detail.value ? { ...state.selectedProject, ...detail.value } : state.selectedProject
    state.detailLoading = false
    state.recentCommits = commits.value || []
    state.commitsLoading = false
    state.latestSession = latest.value
    state.sessionHistory = sessionList.value || []
    state.sessionLoading = false
    state.readmeContent = readme.value
    state.relationships = rels.value || []
    state.relationshipsLoading = false

    logProjectSelectionLifecycle(logger, 'shell.project_selection.applied', {
      project_id: projectId,
      elapsed_ms: Number((nowMs() - startedAt).toFixed(1)),
      daemon_status: getDaemonStatus() ?? null,
      issue_count: state.projectLoadIssues.length,
      pending_retry: state.pendingProjectLoadRetry,
      selection_generation: generation,
      blocking: false,
      deferred: true,
    })
  }

  async function loadSessions(projectId) {
    const sequence = sessionsLoadGuard.next()
    state.sessionLoading = true
    try {
      const [latest, history] = await Promise.all([
        ipc.getLatestSession(projectId),
        ipc.listSessions(projectId, 10),
      ])
      if (!sessionsLoadGuard.isCurrent(sequence) || state.selectedProject?.id !== projectId) return
      state.latestSession = latest
      state.sessionHistory = history || []
    } catch (error) {
      if (!sessionsLoadGuard.isCurrent(sequence) || state.selectedProject?.id !== projectId) return
      logger.warn('[sessions] failed to refresh session data; using empty fallback', {
        project_id: projectId,
        error_message: errorMessage(error),
      })
      state.latestSession = null
      state.sessionHistory = []
    } finally {
      if (sessionsLoadGuard.isCurrent(sequence) && state.selectedProject?.id === projectId) {
        state.sessionLoading = false
      }
    }
  }

  async function loadReadmeForOverview(projectId) {
    const sequence = readmeLoadGuard.next()
    try {
      const readme = await ipc.getReadme(projectId)
      if (!readmeLoadGuard.isCurrent(sequence) || state.selectedProject?.id !== projectId) return
      state.readmeContent = readme
    } catch (error) {
      if (!readmeLoadGuard.isCurrent(sequence) || state.selectedProject?.id !== projectId) return
      logger.warn('[overview] failed to load README; clearing README panel', {
        project_id: projectId,
        error_message: errorMessage(error),
      })
      state.readmeContent = null
    }
  }

  async function loadRelationships(projectId) {
    const sequence = relationshipsLoadGuard.next()
    state.relationshipsLoading = true
    try {
      const loadedRelationships = await ipc.getRelationships(projectId)
      if (!relationshipsLoadGuard.isCurrent(sequence) || state.selectedProject?.id !== projectId) return
      state.relationships = loadedRelationships
    } catch (error) {
      if (!relationshipsLoadGuard.isCurrent(sequence) || state.selectedProject?.id !== projectId) return
      logger.warn('[overview] failed to load relationships; using empty fallback', {
        project_id: projectId,
        error_message: errorMessage(error),
      })
      state.relationships = []
    } finally {
      if (relationshipsLoadGuard.isCurrent(sequence) && state.selectedProject?.id === projectId) {
        state.relationshipsLoading = false
      }
    }
  }

  async function handleDismissRelationship(relId, onError) {
    try {
      await ipc.dismissRelationship(relId)
      state.relationships = state.relationships.filter((relationship) => relationship.id !== relId)
    } catch (error) {
      logger.error(`[overview] failed to dismiss relationship (${relId}):`, error)
      onError?.('Failed to dismiss relationship. Please try again.')
    }
  }

  async function loadCommits(projectId, limit) {
    state.commitsLoading = true
    try {
      state.recentCommits = await (state.showAllCommits
        ? ipc.getAllCommits(projectId, 50)
        : ipc.getRecentCommits(projectId, limit))
    } catch (error) {
      logger.warn('[overview] failed to load commits; using empty fallback', {
        project_id: projectId,
        limit,
        show_all_commits: state.showAllCommits,
        error_message: errorMessage(error),
      })
      state.recentCommits = []
    } finally {
      state.commitsLoading = false
    }
  }

  async function viewAllCommits() {
    if (!state.selectedProject) return
    state.showAllCommits = true
    await loadCommits(state.selectedProject.id, 50)
  }

  function handleProjectRemoved(id) {
    state.projects = state.projects.filter((project) => project.id !== id)
    if (state.selectedProject?.id === id) {
      state.selectedProject = state.projects.length > 0 ? state.projects[0] : null
      if (state.selectedProject) {
        void selectProject(state.selectedProject)
      }
    }
  }

  function maybeRetryPendingProjectLoad() {
    if (!state.pendingProjectLoadRetry || !state.selectedProject || getDaemonRecoveryPending()) {
      return
    }

    state.pendingProjectLoadRetry = false
    void retryProjectLoad()
  }

  function clearTaskNavTarget() {
    state.taskNavTarget = null
  }

  function clearGitNavTarget() {
    state.gitNavTarget = null
  }

  function clearFilesNavTarget() {
    state.filesNavTarget = null
  }

  function clearProjectLoadIssues() {
    state.projectLoadIssues = []
  }

  return {
    switchTab,
    applyNavEntry,
    loadProjects,
    handleProjectCreated,
    retryProjectLoad,
    bootstrapInitialProject,
    prefetchProjectSelection,
    selectProject,
    loadSessions,
    loadReadmeForOverview,
    loadRelationships,
    handleDismissRelationship,
    loadCommits,
    viewAllCommits,
    handleProjectRemoved,
    maybeRetryPendingProjectLoad,
    clearTaskNavTarget,
    clearGitNavTarget,
    clearFilesNavTarget,
    clearProjectLoadIssues,
  }
}
