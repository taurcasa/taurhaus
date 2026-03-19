<script>
  import { listProjects, getProject, getRecentCommits, getAllCommits, getReadme, getLatestSession, listSessions, getRelationships, dismissRelationship, isTauri, isFirstRun, getSettings, updateSettings, getDaemonStatus, checkDaemonInstallStatus, installDaemon, launchClaudeSession, navigateToSession, getForegroundProject, getRemoteUrl, checkPathType, openExternalUrl, getPlatform, listClaudeSessions, startDaemon } from './lib/ipc.js'
  import { getSessionForProject, getSessions, applyDaemonSessionUpdate, hydrateFromBackend as hydrateSessionsFromBackend, markSessionPresenceStale, DEFAULT_TAURI_POLL_INTERVAL_MS } from './lib/sessionStore.svelte.js'
  import * as assetCache from './lib/assetCache.js'
  import { anyPathMatches } from './lib/fileChange.js'
  import { describeDaemonSetupError } from './lib/errorCopy.js'
  import {
    applyShellDaemonStatusSnapshot,
    canCheckDaemonUpdate,
    consumeInitialShellDaemonStatus,
    isShellDaemonRecoveryPending,
  } from './lib/daemonStatus.js'
  import ShellMainPanel from './lib/components/shell/ShellMainPanel.svelte'
  import ShellTitlebar from './lib/components/shell/ShellTitlebar.svelte'
  import SearchOverlay from './lib/SearchOverlay.svelte'
  import AddProjectModal from './lib/AddProjectModal.svelte'
  import FirstRunWizard from './lib/FirstRunWizard.svelte'
  import Sidebar from './lib/Sidebar.svelte'
  import { startPolling as startSessionPolling, stopPolling as stopSessionPolling } from './lib/sessionStore.svelte.js'
  import { push as pushNav, goBack as navGoBack, goForward as navGoForward, reset as resetNav, withSuppressed as navWithSuppressed } from './lib/navHistory.svelte.js'
  import { createAsyncGuard } from './lib/asyncGuard.js'
  import {
    applyNavEntryState,
    buildCriticalProjectSelectionState,
    classifyMarkdownNavigateAction,
    createProjectPosition,
    normalizeMarkdownTarget,
    switchTabState,
  } from './lib/shell/navigation.svelte.js'
  import {
    setupSessionPollingLifecycle,
    setupShellEventListeners,
  } from './lib/shell/events.svelte.js'
  import { setupHistoryNavigation, setupSearchShortcut } from './lib/shell/shortcuts.svelte.js'
  import {
    hasAttachedTmuxFocus,
    resolveProjectIdFromSession,
    resolveProjectIdFromTmuxFocusPayload,
  } from './lib/shell/tmuxFocus.js'
  import { loadThemePreferences, persistDarkModePreference } from './lib/shell/themePreferences.js'
  import {
    closeShellWindow,
    minimizeShellWindow,
    syncWindowsStartupViewport as syncStartupViewportWindow,
    toggleShellMaximize,
  } from './lib/shell/window.js'
  import { setProjectContext } from './lib/context/ProjectContext.js'
  import { setSessionContext } from './lib/context/SessionContext.js'
  import {
    classifyProjectLoadResults,
    loadDeferredProjectSelectionData,
    prefetchProjectSelectionData,
  } from './lib/projectSelection.js'

  import { DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME } from './lib/shikiThemes.js'

  let { initialDaemonStatus = undefined } = $props()

  let dark = $state(false)

  // Code theme preferences (persisted in settings)
  let codeThemeLight = $state(DEFAULT_LIGHT_THEME)
  let codeThemeDark = $state(DEFAULT_DARK_THEME)
  const codeTheme = $derived(dark ? codeThemeDark : codeThemeLight)

  // Sync dark mode to <html> element so global CSS (scrollbar styling) can react
  $effect(() => {
    document.documentElement.classList.toggle('dark', dark)
  })
  $effect(() => {
    return () => {
      if (daemonStatusDismissTimer !== null) {
        clearTimeout(daemonStatusDismissTimer)
      }
      if (daemonStatusRefreshTimer !== null) {
        clearTimeout(daemonStatusRefreshTimer)
      }
      if (daemonRecoveryEscalationTimer !== null) {
        clearTimeout(daemonRecoveryEscalationTimer)
      }
    }
  })
  let searchOpen = $state(false)
  let settingsOpen = $state(false)
  let showAddProject = $state(false)
  let showWizard = $state(false)
  let wizardChecked = $state(false)
  let startupViewportSyncAttempted = false
  // Daemon status: 'connected' | 'busy' | 'disconnected' | 'reconnecting' | 'failed' | 'not_configured' | null
  let daemonStatus = $state(null)
  let daemonStatusInitialized = $state(false)
  let daemonStatusDismissTimer = $state(null)
  let consumedInitialDaemonStatus = false
  let daemonStatusRefreshTimer = null

  // Daemon update banner state
  let daemonUpdateAvailable = $state(null)  // { version, bundled_version } or null
  let daemonUpdateDismissed = $state(false)
  let daemonUpdating = $state(false)
  let daemonRestarting = $state(false)
  let daemonRecoveryStartedAt = $state(null)
  let daemonRecoveryEscalated = $state(false)
  let daemonRecoveryEscalationTimer = null
  let shellNotice = $state(null)

  /*
   * Layout dimensions
   * - Titlebar: 46px tall, holds logo + tab pill + controls
   * - Sidebar:  252px wide, matches logo area in titlebar
   * - Gap:      6px (gap-1.5) between sidebar and main panel
   * - Frame:    6px (p-1.5) padding around panels inside the dark frame
   */
  // --- Data state ---
  let projects = $state([])
  let selectedProject = $state(null)
  let foregroundProjectId = $state(null)
  let sidebarLoading = $state(true)
  let sidebarError = $state(null)
  let detailLoading = $state(false)

  // Tab state
  let activeTab = $state('overview')
  let visitedTabs = $state(new Set(['overview']))

  // Overview: commits
  let recentCommits = $state([])
  let commitsLoading = $state(false)
  let showAllCommits = $state(false)

  // Cross-tab navigation state for Files tab
  let filesNavTarget = $state(null) // { file: string, lineNumber?: number, anchor?: string|null } | { directory: string } | null
  let filesPosition = $state(null)

  // File change signal — set by the central project-files-changed listener,
  // consumed by FilesTab to refresh the tree and currently open file.
  let fileChangePaths = $state(null) // string[] | null

  // Session state
  let latestSession = $state(null)
  let sessionHistory = $state([])
  let sessionLoading = $state(false)
  let readmeContent = $state(null)
  let sessionBridgeLive = $state(false)
  let tmuxFocusRefreshTimer = null

  // Relationship state
  let relationships = $state([])
  let relationshipsLoading = $state(false)
  let projectLoadIssues = $state([]) // [{ section, message }]
  let pendingProjectLoadRetry = $state(false)

  // Cross-tab navigation state for Git tab
  let gitNavTarget = $state(null) // { type: 'commit', hash } | { type: 'range', after, before } | null

  // Per-project position memory — remembers where you were when you switch away
  const projectPositions = new Map() // projectId → { tab, visitedTabs, file?, gitPosition?, taskPosition? }

  // Bound positions from child components (synced via $bindable)
  let gitPosition = $state(null)
  let taskPosition = $state(null)
  let taskNavTarget = $state(null)
  const sessionsLoadGuard = createAsyncGuard()
  const readmeLoadGuard = createAsyncGuard()
  const relationshipsLoadGuard = createAsyncGuard()
  const selectLoadGuard = createAsyncGuard()

  let projectContextValue = $state({
    projects: [],
    selectedProject: null,
    selectProject: (project) => selectProject(project),
    navigateToCommit: (hash) => navigateToCommit(hash),
    navigateToFile: (path, lineNumber) => navigateToFile(path, lineNumber),
    navigateToCommitRange: (after, before) => navigateToCommitRange(after, before),
    onProjectRemoved: (id) => handleProjectRemoved(id),
  })
  const projectContext = setProjectContext(projectContextValue)

  let sessionContextValue = $state({
    daemonStatus: null,
    launchSession: (tool) => handleOverviewLaunchSession(tool),
    openTerminal: () => handleOverviewOpenTerminal(),
    openManageProjects: () => {
      showAddProject = true
    },
    toggleSettings: () => {
      settingsOpen = !settingsOpen
    },
    retryProjects: () => {
      loadProjects()
    },
  })
  const sessionContext = setSessionContext(sessionContextValue)

  $effect(() => {
    projectContext.projects = projects
    projectContext.selectedProject = selectedProject
  })

  $effect(() => {
    sessionContext.daemonStatus = daemonStatus
  })

  function saveProjectPosition() {
    if (!selectedProject) return
    projectPositions.set(selectedProject.id, createProjectPosition({
      activeTab,
      visitedTabs,
      filesPosition,
      gitPosition,
      taskPosition,
    }))
  }

  function navigateToCommit(hash) {
    gitNavTarget = { type: 'commit', hash }
    switchTab('git', { tab: 'git', commit: hash })
  }

  function navigateToCommitRange(after, before) {
    gitNavTarget = { type: 'range', after, before }
    switchTab('git', { tab: 'git', rangeFilter: { after, before } })
  }

  function navigateToFile(path, lineNumber) {
    filesNavTarget = { file: path, lineNumber }
    switchTab('files', { tab: 'files', file: path, lineNumber })
  }

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

  function logProjectSelectionLifecycle(event, payload = {}) {
    console.info('[shell.project-selection] lifecycle', {
      event,
      ...payload,
    })
  }

  function handleProjectRemoved(id) {
    projects = projects.filter((project) => project.id !== id)
    if (selectedProject?.id === id) {
      selectedProject = projects.length > 0 ? projects[0] : null
      if (selectedProject) {
        selectProject(selectedProject)
      }
    }
  }

  // Load code theme prefs + dark mode from settings
  async function loadCodeThemeFromSettings() {
    try {
      const preferences = await loadThemePreferences({
        getSettings,
        defaultLightTheme: DEFAULT_LIGHT_THEME,
        defaultDarkTheme: DEFAULT_DARK_THEME,
      })
      codeThemeLight = preferences.codeThemeLight
      codeThemeDark = preferences.codeThemeDark
      dark = preferences.darkMode
    } catch (error) {
      console.error('[settings] failed to load code theme preferences:', error)
      // Keep defaults on error
    }
  }

  function setDarkMode(value) {
    dark = value
    persistDarkModePreference({ getSettings, updateSettings, value })
      .catch((error) => {
        console.error('[settings] failed to persist dark mode preference:', error)
        shellNotice = 'Failed to save dark mode preference.'
      })
  }

  function handleCodeThemeChanged() {
    loadCodeThemeFromSettings()
  }

  // Check first-run + load projects on mount
  $effect(() => {
    checkFirstRun()
  })

  $effect(() => {
    loadForegroundProject()
  })

  async function checkFirstRun() {
    try {
      const first = await isFirstRun()
      showWizard = first
    } catch (e) {
      console.warn('[startup] first-run check failed; defaulting to non-wizard startup', {
        error_message: errorMessage(e),
      })
      showWizard = false
    } finally {
      wizardChecked = true
    }
    if (!showWizard) {
      loadProjects()
      loadCodeThemeFromSettings()
      loadDaemonStatus()
      void syncWindowsStartupViewport()
    }
  }

  function handleWizardComplete() {
    showWizard = false
    loadProjects()
    loadCodeThemeFromSettings()
    loadDaemonStatus({ allowInitial: false })
    void syncWindowsStartupViewport()
  }

  async function syncWindowsStartupViewport() {
    const attempted = startupViewportSyncAttempted
    startupViewportSyncAttempted = true
    await syncStartupViewportWindow({
      attempted,
      isTauriRuntime: isTauri(),
      getPlatform,
      logger: console,
    })
  }

  async function loadDaemonStatus({ allowInitial = true } = {}) {
    return loadDaemonStatusWithRefresh({
      allowInitial,
      confirmBusy: true,
      includeUpdateCheck: true,
    })
  }

  function clearDaemonStatusRefreshTimer() {
    if (daemonStatusRefreshTimer !== null) {
      clearTimeout(daemonStatusRefreshTimer)
      daemonStatusRefreshTimer = null
    }
  }

  function daemonRecoveryPending() {
    return isShellDaemonRecoveryPending(daemonStatus, { initialized: daemonStatusInitialized })
  }

  $effect(() => {
    const recovering = daemonStatus === 'busy' || daemonStatus === 'reconnecting' || daemonStatus === 'disconnected'
    if (!recovering) {
      daemonRecoveryStartedAt = null
      daemonRecoveryEscalated = false
      if (daemonRecoveryEscalationTimer !== null) {
        clearTimeout(daemonRecoveryEscalationTimer)
        daemonRecoveryEscalationTimer = null
      }
      return
    }

    const startedAt = daemonRecoveryStartedAt ?? Date.now()
    daemonRecoveryStartedAt = startedAt
    const elapsedMs = Date.now() - startedAt
    const shouldEscalate = elapsedMs >= 30_000
    daemonRecoveryEscalated = shouldEscalate

    if (daemonRecoveryEscalationTimer !== null) {
      clearTimeout(daemonRecoveryEscalationTimer)
      daemonRecoveryEscalationTimer = null
    }

    if (!shouldEscalate) {
      daemonRecoveryEscalationTimer = setTimeout(() => {
        daemonRecoveryEscalated = true
        daemonRecoveryEscalationTimer = null
      }, 30_000 - elapsedMs)
    }
  })

  function maybeRetryPendingProjectLoad() {
    if (!pendingProjectLoadRetry || !selectedProject || daemonRecoveryPending()) {
      return
    }

    pendingProjectLoadRetry = false
    void retryProjectLoad()
  }

  function scheduleDaemonStatusRefresh({ delayMs, confirmBusy }) {
    clearDaemonStatusRefreshTimer()
    daemonStatusRefreshTimer = setTimeout(() => {
      daemonStatusRefreshTimer = null
      void loadDaemonStatusWithRefresh({
        allowInitial: false,
        confirmBusy,
        includeUpdateCheck: false,
      })
    }, delayMs)
  }

  async function loadDaemonStatusWithRefresh({ allowInitial = true, confirmBusy = true, includeUpdateCheck = true } = {}) {
    if (allowInitial && !consumedInitialDaemonStatus && initialDaemonStatus !== undefined) {
      consumedInitialDaemonStatus = true
      const initial = consumeInitialShellDaemonStatus(initialDaemonStatus)
      daemonStatus = initial.daemonStatus
      daemonStatusInitialized = true
      if (initial.needsRefresh) {
        scheduleDaemonStatusRefresh({
          delayMs: initialDaemonStatus === 'busy' ? 450 : 1200,
          confirmBusy: initial.confirmBusyOnRefresh,
        })
      } else {
        clearDaemonStatusRefreshTimer()
      }

      maybeRetryPendingProjectLoad()

      if (includeUpdateCheck) {
        checkDaemonUpdate()
      }
      return
    }

    try {
      const status = await getDaemonStatus()
      const next = applyShellDaemonStatusSnapshot(daemonStatus, status.status, { confirmBusy })
      daemonStatus = next.daemonStatus
      daemonStatusInitialized = true
      if (next.needsRefresh) {
        scheduleDaemonStatusRefresh({
          delayMs: next.daemonStatus === 'busy' ? 1500 : 750,
          confirmBusy: next.confirmBusyOnRefresh,
        })
      } else {
        clearDaemonStatusRefreshTimer()
      }

      maybeRetryPendingProjectLoad()
    } catch (error) {
      console.warn('[daemon] status check failed; preserving current status', {
        error_message: errorMessage(error),
      })
    }

    if (includeUpdateCheck) {
      // Non-blocking: check if daemon binary needs updating
      checkDaemonUpdate()
    }
  }

  async function checkDaemonUpdate() {
    if (!canCheckDaemonUpdate(daemonStatus, { initialized: daemonStatusInitialized })) {
      daemonUpdateAvailable = null
      return
    }

    try {
      const status = await checkDaemonInstallStatus()
      const installed = Boolean(status?.installed)
      const needsUpdate = Boolean(status?.needsUpdate ?? status?.needs_update)
      const bundledVersion = String(status?.bundledVersion ?? status?.bundled_version ?? '').trim()
      const installedVersion = String(status?.version ?? '').trim()
      if (installed && needsUpdate && installedVersion && bundledVersion) {
        daemonUpdateAvailable = {
          version: installedVersion,
          bundled_version: bundledVersion,
        }
      } else {
        daemonUpdateAvailable = null
      }
    } catch (error) {
      console.warn('[daemon] install-status check failed; skipping update banner', {
        error_message: errorMessage(error),
      })
    }
  }

  function setForegroundProject(projectId) {
    foregroundProjectId = typeof projectId === 'string' && projectId.trim()
      ? projectId
      : null
  }

  function clearTmuxFocusRefreshTimer() {
    if (tmuxFocusRefreshTimer !== null) {
      clearTimeout(tmuxFocusRefreshTimer)
      tmuxFocusRefreshTimer = null
    }
  }

  function logTmuxFocus(stage, details = {}) {
    console.debug('[tmux-focus]', {
      stage,
      ...details,
    })
  }

  function scheduleForegroundProjectRefresh() {
    clearTmuxFocusRefreshTimer()
    tmuxFocusRefreshTimer = setTimeout(() => {
      tmuxFocusRefreshTimer = null
      void loadForegroundProject()
    }, 75)
  }

  async function loadForegroundProject() {
    try {
      const projectId = await getForegroundProject()
      logTmuxFocus('foreground-ipc-refresh', { projectId })
      setForegroundProject(projectId)
    } catch (error) {
      console.warn('[sessions] failed to load foreground project; clearing foreground marker', {
        error_message: errorMessage(error),
      })
      setForegroundProject(null)
    }
  }

  async function handleDaemonUpdate() {
    daemonUpdating = true
    try {
      await installDaemon()
      daemonUpdateAvailable = null
      daemonUpdateDismissed = false
    } catch (e) {
      console.error('Daemon update failed:', e)
    } finally {
      daemonUpdating = false
    }
  }

  async function handleRestartDaemon() {
    if (daemonRestarting) return
    daemonRestarting = true
    try {
      await startDaemon()
      await loadDaemonStatusWithRefresh({
        allowInitial: false,
        confirmBusy: false,
        includeUpdateCheck: true,
      })
    } catch (error) {
      console.error('[daemon] restart failed:', error)
      shellNotice = describeDaemonSetupError(error, { action: 'restart' })
    } finally {
      daemonRestarting = false
    }
  }

  // Session updates:
  // - Tauri runtime: event-driven via daemon bridge (`sessions-updated`)
  // - Fallback polling stays on until bridge events are observed
  $effect(() => {
    return setupSessionPollingLifecycle({
      isTauri: isTauri(),
      sessionBridgeLive,
      startPolling: () => startSessionPolling({
        intervalMs: isTauri() ? DEFAULT_TAURI_POLL_INTERVAL_MS : undefined,
        onSessionsReceived: () => { sessionBridgeLive = true },
      }),
      stopPolling: () => stopSessionPolling({ flushActivity: false }),
      doc: document,
      logger: console,
    })
  })

  // Tauri real-time event listeners (ADR-022)
  $effect(() => {
    return setupShellEventListeners({
      enabled: isTauri(),
      loadEventApi: () => import('@tauri-apps/api/event'),
      onProjectGitChanged: (payload) => {
        const { project_id } = payload
        const isDirty = payload?.isDirty ?? payload?.is_dirty
        const idx = projects.findIndex(p => p.id === project_id)
        if (idx !== -1 && payload.branch !== undefined) {
          projects[idx] = { ...projects[idx], branch: payload.branch, isDirty }
        }
        if (selectedProject?.id === project_id) {
          selectedProject = { ...selectedProject, branch: payload.branch ?? selectedProject.branch, isDirty: isDirty ?? selectedProject.isDirty }
        }
      },
      onSessionImported: ({ project_id }) => {
        if (selectedProject?.id === project_id) {
          loadSessions(project_id)
        }
      },
      onProjectsReseedComplete: () => {
        loadProjects()
      },
      onProjectFilesChanged: ({ project_id, paths }) => {
        if (paths?.length) {
          for (const p of paths) {
            if (/\.(png|jpg|jpeg|gif|svg|webp|ico|bmp)$/i.test(p)) {
              assetCache.invalidateProject(project_id)
              break
            }
          }
        }
        if (project_id !== selectedProject?.id) return
        if (anyPathMatches(paths, /readme\.md$/i)) {
          loadReadmeForOverview(project_id)
        }
        fileChangePaths = paths
      },
      onDaemonStatus: ({ status }) => {
        daemonStatus = status
        daemonStatusInitialized = true
        clearDaemonStatusRefreshTimer()
        if (status !== 'connected') {
          sessionBridgeLive = false
          markSessionPresenceStale()
        }
        maybeRetryPendingProjectLoad()
        clearTimeout(daemonStatusDismissTimer)
        if (status === 'connected') {
          void checkDaemonUpdate()
          daemonStatusDismissTimer = setTimeout(() => { daemonStatus = null }, 3000)
        }
      },
      onSessionsUpdated: (payload) => {
        sessionBridgeLive = true
        applyDaemonSessionUpdate(payload)
      },
      onTmuxFocusChanged: (payload) => {
        const projectId = resolveProjectIdFromTmuxFocusPayload(payload, {
          projects,
          liveSessions: Array.from(getSessions().values()).flat(),
        })
        if (projectId) {
          logTmuxFocus('event-resolved-from-session-store', { payload, projectId })
          clearTmuxFocusRefreshTimer()
          setForegroundProject(projectId)
          return
        }

        if (hasAttachedTmuxFocus(payload)) {
          logTmuxFocus('event-scheduling-ipc-refresh', { payload })
          scheduleForegroundProjectRefresh()
          return
        }

        logTmuxFocus('event-cleared', { payload })
        clearTmuxFocusRefreshTimer()
        setForegroundProject(null)
      },
      onHydrateSessions: () => {
        hydrateSessionsFromBackend()
      },
      logger: console,
    })
  })

  $effect(() => {
    return () => {
      clearTmuxFocusRefreshTimer()
    }
  })

  async function loadProjects() {
    sidebarLoading = true
    sidebarError = null
    try {
      projects = await listProjects()
      // Auto-select first project if none selected
      if (!selectedProject && projects.length > 0) {
        const firstProject = projects[0]
        void bootstrapInitialProject(firstProject)
      }
      // Git status now comes from cached columns in list_projects (no extra IPC calls).
      // The cache is refreshed by the file watcher and startup reseed.
    } catch (e) {
      sidebarError = e.message || 'Failed to load projects'
      console.error('[shell] failed to load projects', {
        error_message: errorMessage(e),
      })
    } finally {
      sidebarLoading = false
    }
  }

  async function handleProjectCreated(project) {
    await loadProjects()
    const created = projects.find((p) => p.id === project?.id) || project
    if (created?.id) {
      await selectProject(created)
    }
  }

  async function retryProjectLoad() {
    if (!selectedProject) return
    await selectProject(selectedProject)
  }

  async function bootstrapInitialProject(project) {
    await selectProject(project)
  }

  function prefetchProjectSelection(project) {
    if (!project?.id || project.id === selectedProject?.id) return
    void prefetchProjectSelectionData(project.id, {
      getProject,
      getRecentCommits,
      getLatestSession,
      listSessions,
      getReadme,
      getRelationships,
    }, {
      logger: console,
      projectPath: project.path ?? null,
      daemonStatus,
      batchKind: 'deferred',
    })
  }

  async function selectProject(project) {
    const projectId = project.id
    const startedAt = nowMs()

    // Save position in the current project before switching away
    saveProjectPosition()

    const savedPosition = projectPositions.get(projectId)
    const generation = selectLoadGuard.next()

    projectLoadIssues = []
    pendingProjectLoadRetry = false
    const nextState = buildCriticalProjectSelectionState({
      project,
      savedPosition,
    })

    selectedProject = nextState.selectedProject
    detailLoading = nextState.detailLoading
    showAllCommits = nextState.showAllCommits
    activeTab = nextState.activeTab
    visitedTabs = nextState.visitedTabs
    resetNav()
    pushNav(nextState.navEntry)
    gitNavTarget = nextState.gitNavTarget
    taskNavTarget = nextState.taskNavTarget
    recentCommits = nextState.recentCommits
    commitsLoading = nextState.commitsLoading
    latestSession = nextState.latestSession
    sessionHistory = nextState.sessionHistory
    sessionLoading = nextState.sessionLoading
    readmeContent = nextState.readmeContent
    relationships = nextState.relationships
    relationshipsLoading = nextState.relationshipsLoading
    filesNavTarget = nextState.filesNavTarget
    logProjectSelectionLifecycle('shell.project_selection.started', {
      project_id: projectId,
      project_path: project.path ?? null,
      daemon_status: daemonStatus ?? null,
      session_bridge_live: sessionBridgeLive,
      visibility_state: document.hidden ? 'hidden' : 'visible',
      selection_generation: generation,
      blocking: false,
      deferred: true,
    })
    const { detail, commits, latest, sessionList, readme, rels } = await loadDeferredProjectSelectionData(projectId, {
      getProject,
      getRecentCommits,
      getLatestSession,
      listSessions,
      getReadme,
      getRelationships,
    }, {
      logger: console,
      projectPath: project.path ?? null,
      daemonStatus,
      batchKind: 'deferred',
    })
    if (!selectLoadGuard.isCurrent(generation)) {
      logProjectSelectionLifecycle('shell.project_selection.discarded', {
        project_id: projectId,
        elapsed_ms: Number((nowMs() - startedAt).toFixed(1)),
        daemon_status: daemonStatus ?? null,
        selection_generation: generation,
        reason: 'stale_generation',
        blocking: false,
        deferred: true,
      })
      return
    }

    const classifiedLoadIssues = classifyProjectLoadResults(
      [detail, commits, latest, sessionList, readme, rels],
      { deferRetryableIssues: daemonRecoveryPending() }
    )
    pendingProjectLoadRetry = classifiedLoadIssues.pendingRetry
    projectLoadIssues = classifiedLoadIssues.visibleIssues
    if (projectLoadIssues.length > 0) {
      console.warn(
        `[shell] project ${projectId} loaded with degraded data`,
        classifiedLoadIssues.issues
      )
    }

    selectedProject = detail.value ? { ...selectedProject, ...detail.value } : selectedProject
    detailLoading = false
    recentCommits = commits.value || []
    commitsLoading = false
    latestSession = latest.value
    sessionHistory = sessionList.value || []
    sessionLoading = false
    readmeContent = readme.value
    relationships = rels.value || []
    relationshipsLoading = false
    logProjectSelectionLifecycle('shell.project_selection.applied', {
      project_id: projectId,
      elapsed_ms: Number((nowMs() - startedAt).toFixed(1)),
      daemon_status: daemonStatus ?? null,
      issue_count: projectLoadIssues.length,
      pending_retry: pendingProjectLoadRetry,
      selection_generation: generation,
      blocking: false,
      deferred: true,
    })
  }

  async function loadSessions(projectId) {
    const sequence = sessionsLoadGuard.next()
    sessionLoading = true
    try {
      const [latest, history] = await Promise.all([
        getLatestSession(projectId),
        listSessions(projectId, 10),
      ])
      if (!sessionsLoadGuard.isCurrent(sequence) || selectedProject?.id !== projectId) return
      latestSession = latest
      sessionHistory = history || []
    } catch (error) {
      if (!sessionsLoadGuard.isCurrent(sequence) || selectedProject?.id !== projectId) return
      console.warn('[sessions] failed to refresh session data; using empty fallback', {
        project_id: projectId,
        error_message: errorMessage(error),
      })
      latestSession = null
      sessionHistory = []
    } finally {
      if (sessionsLoadGuard.isCurrent(sequence) && selectedProject?.id === projectId) {
        sessionLoading = false
      }
    }
  }

  async function loadReadmeForOverview(projectId) {
    const sequence = readmeLoadGuard.next()
    try {
      const readme = await getReadme(projectId)
      if (!readmeLoadGuard.isCurrent(sequence) || selectedProject?.id !== projectId) return
      readmeContent = readme
    } catch (error) {
      if (!readmeLoadGuard.isCurrent(sequence) || selectedProject?.id !== projectId) return
      console.warn('[overview] failed to load README; clearing README panel', {
        project_id: projectId,
        error_message: errorMessage(error),
      })
      readmeContent = null
    }
  }

  async function loadRelationships(projectId) {
    const sequence = relationshipsLoadGuard.next()
    relationshipsLoading = true
    try {
      const loadedRelationships = await getRelationships(projectId)
      if (!relationshipsLoadGuard.isCurrent(sequence) || selectedProject?.id !== projectId) return
      relationships = loadedRelationships
    } catch (error) {
      if (!relationshipsLoadGuard.isCurrent(sequence) || selectedProject?.id !== projectId) return
      console.warn('[overview] failed to load relationships; using empty fallback', {
        project_id: projectId,
        error_message: errorMessage(error),
      })
      relationships = []
    } finally {
      if (relationshipsLoadGuard.isCurrent(sequence) && selectedProject?.id === projectId) {
        relationshipsLoading = false
      }
    }
  }

  async function handleDismissRelationship(relId) {
    try {
      await dismissRelationship(relId)
      relationships = relationships.filter(r => r.id !== relId)
    } catch (error) {
      console.error(`[overview] failed to dismiss relationship (${relId}):`, error)
      shellNotice = 'Failed to dismiss relationship. Please try again.'
    }
  }

  async function loadCommits(projectId, limit) {
    commitsLoading = true
    try {
      recentCommits = await (showAllCommits
        ? getAllCommits(projectId, 50)
        : getRecentCommits(projectId, limit))
    } catch (error) {
      console.warn('[overview] failed to load commits; using empty fallback', {
        project_id: projectId,
        limit,
        show_all_commits: showAllCommits,
        error_message: errorMessage(error),
      })
      recentCommits = []
    } finally {
      commitsLoading = false
    }
  }

  async function viewAllCommits() {
    if (!selectedProject) return
    showAllCommits = true
    await loadCommits(selectedProject.id, 50)
  }

  function handleOverviewLaunchSession(tool) {
    if (!selectedProject) return
    launchClaudeSession(selectedProject.id, 'fresh', tool)
      .then(r => console.log('[overview] launch OK:', r))
      .catch(e => console.error('[overview] launch FAILED:', e))
  }

  function handleOverviewOpenTerminal() {
    if (!selectedProject) return
    const session = getSessionForProject(selectedProject.path)
    if (session?.tmux_session && session?.tmux_window && session?.tmux_pane) {
      setForegroundProject(selectedProject.id)
      navigateToSession(session.tmux_session, session.tmux_window, session.tmux_pane, true)
    }
  }

  async function handleMeshFocusPane(paneId) {
    const normalizedPaneId = String(paneId || '').trim()
    if (!normalizedPaneId) return

    try {
      const sessions = await listClaudeSessions()
      const matchingSession = Array.isArray(sessions)
        ? sessions.find((session) => {
          const sessionPane = session?.tmux_pane ?? session?.tmuxPane ?? null
          return sessionPane === normalizedPaneId
        })
        : null

      const tmuxSession = matchingSession?.tmux_session ?? matchingSession?.tmuxSession ?? null
      const tmuxWindow = matchingSession?.tmux_window ?? matchingSession?.tmuxWindow ?? null
      const tmuxPane = matchingSession?.tmux_pane ?? matchingSession?.tmuxPane ?? null

      if (!tmuxSession || !tmuxWindow || !tmuxPane) {
        console.warn('[mesh] focus pane skipped: missing tmux coordinates', {
          pane_id: normalizedPaneId,
        })
        return
      }

      setForegroundProject(resolveProjectIdFromSession(matchingSession, projects))
      await navigateToSession(tmuxSession, tmuxWindow, tmuxPane, true)
    } catch (error) {
      console.error('[mesh] focus pane failed:', {
        pane_id: normalizedPaneId,
        error_message: errorMessage(error),
      })
    }
  }

  function switchTab(tab, navEntry) {
    const nextState = switchTabState(visitedTabs, tab)
    visitedTabs = nextState.visitedTabs
    activeTab = nextState.activeTab
    pushNav(navEntry || { tab })
  }

  /** Restore a navigation history entry (back/forward). */
  function applyNavEntry(entry) {
    navWithSuppressed(() => {
      const nextState = applyNavEntryState(visitedTabs, entry)
      visitedTabs = nextState.visitedTabs
      activeTab = nextState.activeTab
      filesNavTarget = nextState.filesNavTarget
      gitNavTarget = nextState.gitNavTarget
    })
  }

  // Window controls (Tauri custom titlebar — decorations: false)
  function minimizeWindow() {
    return minimizeShellWindow()
  }

  function toggleMaximize() {
    return toggleShellMaximize()
  }

  function closeWindow() {
    return closeShellWindow()
  }

  $effect(() => setupSearchShortcut({
    onToggleSearch: () => {
      searchOpen = !searchOpen
    },
  }))

  $effect(() => setupHistoryNavigation({
    onGoBack: () => {
      const entry = navGoBack()
      if (entry) applyNavEntry(entry)
    },
    onGoForward: () => {
      const entry = navGoForward()
      if (entry) applyNavEntry(entry)
    },
  }))

  async function handleMarkdownNavigate(relativePath) {
    if (!selectedProject) return

    if (!relativePath || relativePath.startsWith('#')) return

    const contextFile = activeTab === 'overview'
      ? readmeContent?.path
      : (filesPosition?.selectedFile || readmeContent?.path)

    const normalized = normalizeMarkdownTarget(relativePath, contextFile)
    if (!normalized) return

    if (normalized.escapedAboveRoot) {
      let remoteUrl = null
      try {
        remoteUrl = await getRemoteUrl(selectedProject.id)
      } catch (err) {
        console.warn('[markdown] failed to resolve remote URL for platform route', err)
        return
      }

      if (!remoteUrl) {
        console.warn(`[markdown] platform route detected but no remote available: "${relativePath}"`)
        return
      }

      const action = classifyMarkdownNavigateAction({
        relativePath,
        contextFile,
        remoteUrl,
        pathType: 'not_found',
      })
      if (!action?.url) return
      console.log(`[markdown] navigate platform route: "${relativePath}" → "${action.url}"`)
      openExternalUrl(action.url).catch((err) => {
        console.error(`[markdown] failed to open platform route URL: ${action.url}`, err)
      })
      return
    }

    let pathType = 'not_found'
    try {
      pathType = await checkPathType(selectedProject.id, normalized.resolvedPath)
    } catch (err) {
      console.warn(`[markdown] failed to classify path: "${normalized.resolvedPath}"`, err)
      return
    }

    const action = classifyMarkdownNavigateAction({
      relativePath,
      contextFile,
      remoteUrl: null,
      pathType,
    })

    if (action?.type === 'directory') {
      console.log(`[markdown] navigate directory: "${relativePath}" → "${action.directory}"`)
      filesNavTarget = { directory: action.directory }
      switchTab('files', { tab: 'files' })
      return
    }

    if (action?.type === 'file') {
      console.log(`[markdown] navigate: "${relativePath}" → "${action.file}"${action.anchor ? ` #${action.anchor}` : ''}`)
      filesNavTarget = { file: action.file, anchor: action.anchor }
      switchTab('files', { tab: 'files', file: action.file })
      return
    }

    console.warn(`[markdown] unresolved markdown path (not_found): "${relativePath}" → "${normalized.resolvedPath}"`)
  }

  async function handleSearchNavigate(action) {
    // Switch project if the result belongs to a different project
    if (action.projectId && action.projectId !== selectedProject?.id) {
      const targetProject = projects.find(p => p.id === action.projectId)
      if (targetProject) {
        await selectProject(targetProject)
      }
    }

    if (action.tab === 'files' && action.filePath) {
      filesNavTarget = { file: action.filePath }
      switchTab('files', { tab: 'files', file: action.filePath })
    } else if (action.tab === 'overview') {
      switchTab('overview')
    }
  }
</script>

{#if showWizard}
  <div class="shell-frame h-full font-sans antialiased" data-tauri-drag-region>
    <FirstRunWizard {dark} onComplete={handleWizardComplete} onDismiss={handleWizardComplete} />
  </div>
{:else}
<div class="shell-frame h-full flex flex-col font-sans antialiased">
  <div data-shell-app-root class="contents">
    <ShellTitlebar
      {dark}
      {activeTab}
      {settingsOpen}
      onSwitchTab={(tab) => switchTab(tab)}
      onToggleSearch={() => {
        searchOpen = !searchOpen
      }}
      onSetDarkMode={setDarkMode}
      onMinimizeWindow={minimizeWindow}
      onToggleMaximize={toggleMaximize}
      onCloseWindow={closeWindow}
    />

    <!-- ═══ BODY — panels floating inside the dark frame ═══ -->
    <div class="flex-1 flex gap-1.5 p-1.5 pt-0 min-h-0">

      <!-- ═══ SIDEBAR ═══ -->
        <Sidebar
          {projects}
          {sidebarLoading}
          {sidebarError}
          {selectedProject}
          {foregroundProjectId}
          onForegroundProjectChange={setForegroundProject}
          daemonStatus={daemonStatus}
          {settingsOpen}
          {dark}
          actions={{
            onProjectHover: prefetchProjectSelection,
          }}
      />

      <ShellMainPanel
        {dark}
        {codeTheme}
        {codeThemeLight}
        {codeThemeDark}
        {settingsOpen}
        {daemonStatus}
        {daemonRecoveryEscalated}
        {daemonUpdateAvailable}
        {daemonUpdateDismissed}
        {daemonUpdating}
        {daemonRestarting}
        {shellNotice}
        {projectLoadIssues}
        {selectedProject}
        {projects}
        {activeTab}
        {visitedTabs}
        {recentCommits}
        {commitsLoading}
        {latestSession}
        {sessionHistory}
        {sessionLoading}
        {readmeContent}
        {relationships}
        {relationshipsLoading}
        {gitNavTarget}
        {filesNavTarget}
        {taskNavTarget}
        {fileChangePaths}
        bind:filesPosition
        bind:gitPosition
        bind:taskPosition
        onCloseSettings={() => {
          settingsOpen = false
        }}
        onSettingsChanged={loadProjects}
        onCodeThemeChanged={handleCodeThemeChanged}
        onViewAllCommits={viewAllCommits}
        onDismissRelationship={handleDismissRelationship}
        onMarkdownNavigate={handleMarkdownNavigate}
        onRetryProjectLoad={retryProjectLoad}
        onHandleDaemonUpdate={handleDaemonUpdate}
        onRestartDaemon={handleRestartDaemon}
        onDismissDaemonUpdate={() => {
          daemonUpdateDismissed = true
        }}
        onDismissProjectLoadIssues={() => {
          projectLoadIssues = []
        }}
        onDismissShellNotice={() => {
          shellNotice = null
        }}
        onNavigateToFile={navigateToFile}
        onMeshFocusPane={handleMeshFocusPane}
        onClearTaskNavTarget={() => {
          taskNavTarget = null
        }}
        onClearGitNavTarget={() => {
          gitNavTarget = null
        }}
        onClearFilesNavTarget={() => {
          filesNavTarget = null
        }}
        onChangedPathsConsumed={() => {
          fileChangePaths = null
        }}
      />
    </div>
  </div>

  <SearchOverlay bind:open={searchOpen} {dark} onNavigate={handleSearchNavigate} />

  {#if showAddProject}
    <AddProjectModal
      {dark}
      onClose={() => showAddProject = false}
      onProjectsChanged={loadProjects}
      onProjectCreated={handleProjectCreated}
    />
  {/if}

</div>
{/if}
