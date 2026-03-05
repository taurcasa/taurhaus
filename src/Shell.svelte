<script>
  import { listProjects, getProject, getRecentCommits, getAllCommits, getReadme, getLatestSession, listSessions, getRelationships, dismissRelationship, isTauri, isFirstRun, getSettings, updateSettings, getDaemonStatus, checkDaemonInstallStatus, installDaemon, launchClaudeSession, navigateToSession, getRemoteUrl, checkPathType, openExternalUrl, getPlatform } from './lib/ipc.js'
  import { getSessionForProject, applyDaemonSessionUpdate, hydrateFromBackend as hydrateSessionsFromBackend } from './lib/sessionStore.svelte.js'
  import * as assetCache from './lib/assetCache.js'
  import { anyPathMatches } from './lib/fileChange.js'
  import TaskBoard from './lib/TaskBoard.svelte'
  import GitTab from './lib/GitTab.svelte'
  import MeshTab from './lib/components/MeshTab.svelte'
  import SearchOverlay from './lib/SearchOverlay.svelte'
  import Settings from './lib/Settings.svelte'
  import AddProjectModal from './lib/AddProjectModal.svelte'
  import FirstRunWizard from './lib/FirstRunWizard.svelte'
  import OverviewTab from './lib/OverviewTab.svelte'
  import FilesTab from './lib/FilesTab.svelte'
  import Sidebar from './lib/Sidebar.svelte'
  import { startPolling as startSessionPolling, stopPolling as stopSessionPolling } from './lib/sessionStore.svelte.js'
  import { push as pushNav, goBack as navGoBack, goForward as navGoForward, reset as resetNav, withSuppressed as navWithSuppressed } from './lib/navHistory.svelte.js'
  import { createAsyncGuard } from './lib/asyncGuard.js'
  import { createProjectSelectionRequests } from './lib/projectSelection.js'
  import { loadThemePreferences, persistDarkModePreference } from './lib/shell/themePreferences.js'
  import { setProjectContext } from './lib/context/ProjectContext.js'
  import { setSessionContext } from './lib/context/SessionContext.js'

  import { DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME } from './lib/shikiThemes.js'
  import { themeTokens } from './lib/themeTokens.js'

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
  let searchOpen = $state(false)
  let settingsOpen = $state(false)
  let showAddProject = $state(false)
  let showWizard = $state(false)
  let wizardChecked = $state(false)
  let startupViewportSyncAttempted = false

  // Daemon status: 'connected' | 'disconnected' | 'reconnecting' | 'failed' | 'not_configured' | null
  let daemonStatus = $state(null)
  let daemonStatusDismissTimer = $state(null)
  let consumedInitialDaemonStatus = false

  // Daemon update banner state
  let daemonUpdateAvailable = $state(null)  // { version, bundled_version } or null
  let daemonUpdateDismissed = $state(false)
  let daemonUpdating = $state(false)
  let shellNotice = $state(null)

  /*
   * Layout dimensions
   * - Titlebar: 46px tall, holds logo + tab pill + controls
   * - Sidebar:  252px wide, matches logo area in titlebar
   * - Gap:      6px (gap-1.5) between sidebar and main panel
   * - Frame:    6px (p-1.5) padding around panels inside the dark frame
   */


  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens
  const tabSeparator   = $derived(dark ? 'bg-zinc-700' : 'bg-zinc-200')
  const panelBorder    = $derived(dark ? 'border border-zinc-800' : '')

  // --- Data state ---
  let projects = $state([])
  let selectedProject = $state(null)
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

  // Relationship state
  let relationships = $state([])
  let relationshipsLoading = $state(false)
  let projectLoadIssues = $state([]) // [{ section, message }]

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
    projectPositions.set(selectedProject.id, {
      tab: activeTab,
      visitedTabs: new Set(visitedTabs),
      file: filesPosition?.selectedFile ?? null,
      gitPosition: gitPosition ? { ...gitPosition } : null,
      taskPosition: taskPosition ? { ...taskPosition } : null,
    })
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

  async function checkFirstRun() {
    try {
      const first = await isFirstRun()
      showWizard = first
    } catch (e) {
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
    if (startupViewportSyncAttempted || !isTauri()) return
    startupViewportSyncAttempted = true
    try {
      const platform = await getPlatform()
      if (platform !== 'windows') return

      const { getCurrentWindow, PhysicalSize } = await import('@tauri-apps/api/window')
      const appWindow = getCurrentWindow()
      const [maximized, fullscreen] = await Promise.all([
        appWindow.isMaximized(),
        appWindow.isFullscreen(),
      ])

      // Keep maximized/fullscreen startup untouched.
      if (maximized || fullscreen) {
        window.dispatchEvent(new Event('resize'))
        return
      }

      await new Promise((resolve) => requestAnimationFrame(resolve))
      const size = await appWindow.innerSize()
      if (!size?.width || !size?.height) return

      // Force one native resize cycle: this mirrors the manual resize workaround.
      await appWindow.setSize(new PhysicalSize(size.width + 1, size.height))
      await appWindow.setSize(new PhysicalSize(size.width, size.height))
      window.dispatchEvent(new Event('resize'))
    } catch (error) {
      console.warn('[window] startup viewport sync failed:', error)
    }
  }

  async function loadDaemonStatus({ allowInitial = true } = {}) {
    if (allowInitial && !consumedInitialDaemonStatus && initialDaemonStatus !== undefined) {
      consumedInitialDaemonStatus = true
      if (initialDaemonStatus === 'connected' || initialDaemonStatus === 'not_configured') {
        daemonStatus = null
      } else {
        daemonStatus = initialDaemonStatus
      }

      checkDaemonUpdate()
      return
    }

    try {
      const status = await getDaemonStatus()
      // Only show non-connected states (connected is the happy path, don't clutter)
      if (status.status !== 'connected') {
        daemonStatus = status.status
      }
    } catch { /* ignore — not critical */ }

    // Non-blocking: check if daemon binary needs updating
    checkDaemonUpdate()
  }

  async function checkDaemonUpdate() {
    try {
      const status = await checkDaemonInstallStatus()
      const installed = Boolean(status?.installed)
      const needsUpdate = Boolean(status?.needsUpdate ?? status?.needs_update)
      const bundledVersion = status?.bundledVersion ?? status?.bundled_version ?? null
      if (installed && needsUpdate) {
        daemonUpdateAvailable = {
          version: status.version,
          bundled_version: bundledVersion,
        }
      }
    } catch { /* ignore — not critical */ }
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

  // Session updates:
  // - Tauri runtime: event-driven via daemon bridge (`sessions-updated`)
  // - Frontend-only mock mode: fallback polling
  $effect(() => {
    if (isTauri()) {
      return
    }

    startSessionPolling()

    // Pause polling when document is hidden
    function onVisibilityChange() {
      if (document.hidden) {
        stopSessionPolling()
      } else {
        startSessionPolling()
      }
    }
    document.addEventListener('visibilitychange', onVisibilityChange)

    return () => {
      stopSessionPolling()
      document.removeEventListener('visibilitychange', onVisibilityChange)
    }
  })

  // Tauri real-time event listeners (ADR-022)
  $effect(() => {
    if (!isTauri()) return
    let destroyed = false
    let cleanups = []

    function runListenerHandler(eventName, handler, event) {
      try {
        const maybePromise = handler(event)
        if (maybePromise && typeof maybePromise.then === 'function') {
          maybePromise.catch((error) => {
            console.error(`[events] listener '${eventName}' handler failed:`, error)
          })
        }
      } catch (error) {
        console.error(`[events] listener '${eventName}' handler failed:`, error)
      }
    }

    function registerListener(listen, eventName, handler) {
      listen(eventName, (event) => runListenerHandler(eventName, handler, event))
        .then((unlisten) => {
          if (destroyed) {
            unlisten()
            return
          }
          cleanups.push(unlisten)
        })
        .catch((error) => {
          console.error(`[events] failed to register listener '${eventName}':`, error)
        })
    }

    import('@tauri-apps/api/event').then(({ listen }) => {
      if (destroyed) return
      // Git status changed — refresh sidebar project status
      registerListener(listen, 'project-git-changed', (event) => {
        const { project_id } = event.payload
        const isDirty = event.payload?.isDirty ?? event.payload?.is_dirty
        const idx = projects.findIndex(p => p.id === project_id)
        if (idx !== -1 && event.payload.branch !== undefined) {
          projects[idx] = { ...projects[idx], branch: event.payload.branch, isDirty }
        }
        if (selectedProject?.id === project_id) {
          selectedProject = { ...selectedProject, branch: event.payload.branch ?? selectedProject.branch, isDirty: isDirty ?? selectedProject.isDirty }
        }
      })

      // Session imported — refresh session display
      registerListener(listen, 'session-imported', (event) => {
        const { project_id } = event.payload
        if (selectedProject?.id === project_id) {
          loadSessions(project_id)
        }
      })

      // Startup reseed complete — reload project list to pick up cached git status
      registerListener(listen, 'projects-reseed-complete', () => {
        loadProjects()
      })

      // File changes — central handler for all file-change responses.
      // Invalidates caches, refreshes Overview README, and signals
      // FilesTab to refresh via the fileChangePaths reactive prop.
      registerListener(listen, 'project-files-changed', (event) => {
        const { project_id, paths } = event.payload
        // Invalidate asset cache for changed images
        if (paths?.length) {
          for (const p of paths) {
            if (/\.(png|jpg|jpeg|gif|svg|webp|ico|bmp)$/i.test(p)) {
              assetCache.invalidateProject(project_id)
              break // one invalidation is enough per event batch
            }
          }
        }
        if (project_id !== selectedProject?.id) return
        // Refresh README in Overview tab
        if (anyPathMatches(paths, /readme\.md$/i)) {
          loadReadmeForOverview(project_id)
        }
        // Signal FilesTab to refresh (it reads this reactively)
        fileChangePaths = paths
      })

      // Daemon status changes (bootstrap chain + health check)
      registerListener(listen, 'daemon-status', (event) => {
        const { status } = event.payload
        daemonStatus = status
        clearTimeout(daemonStatusDismissTimer)
        // Auto-dismiss "connected" after 3 seconds
        if (status === 'connected') {
          daemonStatusDismissTimer = setTimeout(() => { daemonStatus = null }, 3000)
        }
      })

      // Session activity updates from daemon bridge (event-driven above daemon).
      registerListener(listen, 'sessions-updated', (event) => {
        applyDaemonSessionUpdate(event.payload)
      })

      // Prime the session store once on startup so indicators do not stay empty
      // until the first pushed delta arrives.
      if (!destroyed) {
        hydrateSessionsFromBackend()
      }
    }).catch((error) => {
      console.error('[events] failed to initialize Tauri listeners:', error)
    })

    return () => {
      destroyed = true
      cleanups.forEach(u => u())
    }
  })

  async function loadProjects() {
    sidebarLoading = true
    sidebarError = null
    try {
      projects = await listProjects()
      // Auto-select first project if none selected
      if (!selectedProject && projects.length > 0) {
        void selectProject(projects[0])
      }
      // Git status now comes from cached columns in list_projects (no extra IPC calls).
      // The cache is refreshed by the file watcher and startup reseed.
    } catch (e) {
      sidebarError = e.message || 'Failed to load projects'
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

  async function selectProject(project) {
    const projectId = project.id

    // Save position in the current project before switching away
    saveProjectPosition()

    const savedPosition = projectPositions.get(projectId)
    const restoredTab = savedPosition?.tab || 'overview'
    const generation = selectLoadGuard.next()

    // Commit fast initial state so sidebar and tab shell are interactive immediately.
    projectLoadIssues = []
    selectedProject = project
    detailLoading = true
    showAllCommits = false
    activeTab = restoredTab
    visitedTabs = savedPosition?.visitedTabs || new Set([restoredTab])
    resetNav()
    pushNav({ tab: restoredTab, file: savedPosition?.file })
    // Restore Git position via existing gitNavTarget mechanism
    if (savedPosition?.gitPosition?.selectedHash) {
      gitNavTarget = { type: 'commit', hash: savedPosition.gitPosition.selectedHash }
    } else if (savedPosition?.gitPosition?.rangeFilter) {
      gitNavTarget = { type: 'range', ...savedPosition.gitPosition.rangeFilter }
    } else {
      gitNavTarget = null
    }
    // Restore Task position via separate restoreTarget prop
    taskNavTarget = savedPosition?.taskPosition ?? null
    // Reset detail panels so they reveal progressively as each section loads.
    recentCommits = []
    commitsLoading = true
    latestSession = null
    sessionHistory = []
    sessionLoading = true
    readmeContent = null
    relationships = []
    relationshipsLoading = true
    // Restore file position via navigateTarget — FilesTab loads its own tree
    filesNavTarget = savedPosition?.file ? { file: savedPosition.file } : null

    const requests = createProjectSelectionRequests(projectId, {
      getProject,
      getRecentCommits,
      getLatestSession,
      listSessions,
      getReadme,
      getRelationships,
    })

    function updateProjectLoadIssue(result) {
      if (!result?.section) return
      if (result.ok) {
        projectLoadIssues = projectLoadIssues.filter((issue) => issue.section !== result.section)
        return
      }
      projectLoadIssues = [
        ...projectLoadIssues.filter((issue) => issue.section !== result.section),
        { section: result.section, message: result.message },
      ]
    }

    const detailTask = requests.detail.then((detail) => {
      if (!selectLoadGuard.isCurrent(generation)) return
      updateProjectLoadIssue(detail)
      selectedProject = detail.value ? { ...project, ...detail.value } : project
      detailLoading = false
    })

    const commitsTask = requests.commits.then((commits) => {
      if (!selectLoadGuard.isCurrent(generation)) return
      updateProjectLoadIssue(commits)
      recentCommits = commits.value
      commitsLoading = false
    })

    const sessionsTask = Promise.all([requests.latest, requests.sessionList]).then(
      ([latest, sessionList]) => {
        if (!selectLoadGuard.isCurrent(generation)) return
        updateProjectLoadIssue(latest)
        updateProjectLoadIssue(sessionList)
        latestSession = latest.value
        sessionHistory = sessionList.value || []
        sessionLoading = false
      }
    )

    const readmeTask = requests.readme.then((readme) => {
      if (!selectLoadGuard.isCurrent(generation)) return
      updateProjectLoadIssue(readme)
      readmeContent = readme.value
    })

    const relationshipsTask = requests.rels.then((rels) => {
      if (!selectLoadGuard.isCurrent(generation)) return
      updateProjectLoadIssue(rels)
      relationships = rels.value
      relationshipsLoading = false
    })

    await Promise.allSettled([
      detailTask,
      commitsTask,
      sessionsTask,
      readmeTask,
      relationshipsTask,
    ])

    if (!selectLoadGuard.isCurrent(generation)) return

    if (projectLoadIssues.length > 0) {
      console.warn(`[shell] project ${projectId} loaded with degraded data`, projectLoadIssues)
    }
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
    } catch {
      if (!sessionsLoadGuard.isCurrent(sequence) || selectedProject?.id !== projectId) return
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
    } catch {
      if (!readmeLoadGuard.isCurrent(sequence) || selectedProject?.id !== projectId) return
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
    } catch {
      if (!relationshipsLoadGuard.isCurrent(sequence) || selectedProject?.id !== projectId) return
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
    } catch {
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
      navigateToSession(session.tmux_session, session.tmux_window, session.tmux_pane, true)
    }
  }

  function switchTab(tab, navEntry) {
    visitedTabs = new Set([...visitedTabs, tab])
    activeTab = tab
    pushNav(navEntry || { tab })
  }

  /** Restore a navigation history entry (back/forward). */
  function applyNavEntry(entry) {
    navWithSuppressed(() => {
      visitedTabs = new Set([...visitedTabs, entry.tab])
      activeTab = entry.tab
      if (entry.tab === 'files' && entry.file) {
        filesNavTarget = { file: entry.file, lineNumber: entry.lineNumber }
      }
      if (entry.tab === 'git' && entry.commit) gitNavTarget = { type: 'commit', hash: entry.commit }
      if (entry.tab === 'git' && entry.rangeFilter) gitNavTarget = { type: 'range', ...entry.rangeFilter }
    })
  }

  // Window controls (Tauri custom titlebar — decorations: false)
  async function minimizeWindow() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().minimize()
    } catch { /* dev mode — no Tauri runtime */ }
  }

  async function toggleMaximize() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().toggleMaximize()
    } catch { /* dev mode — no Tauri runtime */ }
  }

  async function closeWindow() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().close()
    } catch { /* dev mode — no Tauri runtime */ }
  }

  // Cmd+K / Ctrl+K global search shortcut
  $effect(() => {
    const handler = (e) => {
      if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        searchOpen = !searchOpen
      }
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  })

  // Back/Forward navigation — mouse buttons + Alt+Arrow keys
  // Use mousedown (not mouseup) to intercept before WebView2 handles back/forward.
  $effect(() => {
    function onMouseDown(e) {
      if (e.button === 3) {
        e.preventDefault()
        const entry = navGoBack()
        if (entry) applyNavEntry(entry)
      } else if (e.button === 4) {
        e.preventDefault()
        const entry = navGoForward()
        if (entry) applyNavEntry(entry)
      }
    }
    function onKeyDown(e) {
      if (e.altKey && e.key === 'ArrowLeft') {
        e.preventDefault()
        const entry = navGoBack()
        if (entry) applyNavEntry(entry)
      } else if (e.altKey && e.key === 'ArrowRight') {
        e.preventDefault()
        const entry = navGoForward()
        if (entry) applyNavEntry(entry)
      }
    }
    document.addEventListener('mousedown', onMouseDown)
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('mousedown', onMouseDown)
      document.removeEventListener('keydown', onKeyDown)
    }
  })

  function normalizeMarkdownTarget(relativePath, contextFile) {
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

  async function handleMarkdownNavigate(relativePath) {
    if (!selectedProject) return

    // Ignore in-document anchors.
    if (!relativePath || relativePath.startsWith('#')) return

    const fragmentMatch = relativePath.match(/#(.+)$/)
    const anchor = fragmentMatch ? fragmentMatch[1] : null

    // In Overview, resolve links against README path.
    // In Files, resolve against the selected file.
    const contextFile = activeTab === 'overview'
      ? readmeContent?.path
      : (filesPosition?.selectedFile || readmeContent?.path)

    const normalized = normalizeMarkdownTarget(relativePath, contextFile)
    if (!normalized) return

    const { resolvedPath, escapedAboveRoot, platformSegments } = normalized

    if (escapedAboveRoot) {
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

      const externalUrl = buildPlatformRouteUrl(remoteUrl, platformSegments)
      if (!externalUrl) return
      console.log(`[markdown] navigate platform route: "${relativePath}" → "${externalUrl}"`)
      openExternalUrl(externalUrl).catch((err) => {
        console.error(`[markdown] failed to open platform route URL: ${externalUrl}`, err)
      })
      return
    }

    let pathType = 'not_found'
    try {
      pathType = await checkPathType(selectedProject.id, resolvedPath)
    } catch (err) {
      console.warn(`[markdown] failed to classify path: "${resolvedPath}"`, err)
      return
    }

    if (pathType === 'directory') {
      console.log(`[markdown] navigate directory: "${relativePath}" → "${resolvedPath}"`)
      filesNavTarget = { directory: resolvedPath }
      switchTab('files', { tab: 'files' })
      return
    }

    if (pathType === 'file') {
      console.log(`[markdown] navigate: "${relativePath}" → "${resolvedPath}"${anchor ? ` #${anchor}` : ''}`)
      filesNavTarget = { file: resolvedPath, anchor }
      switchTab('files', { tab: 'files', file: resolvedPath })
      return
    }

    console.warn(`[markdown] unresolved markdown path (not_found): "${relativePath}" → "${resolvedPath}"`)
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
  <div class="h-full bg-brand-950 font-sans antialiased" data-tauri-drag-region>
    <FirstRunWizard {dark} onComplete={handleWizardComplete} />
  </div>
{:else}
<div class="h-full bg-brand-950 flex flex-col font-sans antialiased">

  <!-- ═══ TITLEBAR ═══ -->
  <div class="h-[46px] flex items-end shrink-0 pl-1.5" data-tauri-drag-region>

    <!-- Logo area (width matches sidebar panel below) -->
    <div class="w-[252px] flex items-center px-4 pb-2 shrink-0" data-tauri-drag-region>
      <div class="flex items-center gap-2.5">
        <img src="/logo-22.png" alt="taurhaus" width="22" height="22" class="block" />
        <span class="text-[13px] font-semibold text-white/90 tracking-[-0.01em]">taurhaus</span>
      </div>
    </div>

    <!-- Tab pill + drag space + controls -->
    <div class="flex-1 flex items-end min-w-0" data-tauri-drag-region>

      <!-- Tab pill — shares bg with main panel (Manila Folder pattern) -->
      <div class="flex items-center px-4 h-[36px] {t.mainBg} rounded-t-lg ml-1.5">
        {#if settingsOpen}
          <span class="px-3 py-1 text-[13px] font-medium {t.textPrimary}">Settings</span>
        {:else}
          <button
            data-testid="tab-overview"
            class="px-3 py-1 text-[13px] transition-colors border-b-2
              {activeTab === 'overview' ? `font-medium ${t.textPrimary} border-brand-500` : `${t.textTertiary} hover:text-zinc-500 border-transparent`}"
            onclick={() => switchTab('overview')}
          >Overview</button>
          <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
          <button
            data-testid="tab-files"
            class="px-3 py-1 text-[13px] transition-colors border-b-2
              {activeTab === 'files' ? `font-medium ${t.textPrimary} border-brand-500` : `${t.textTertiary} hover:text-zinc-500 border-transparent`}"
            onclick={() => switchTab('files')}
          >Files</button>
          <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
          <button
            data-testid="tab-tasks"
            class="px-3 py-1 text-[13px] transition-colors border-b-2
              {activeTab === 'tasks' ? `font-medium ${t.textPrimary} border-brand-500` : `${t.textTertiary} hover:text-zinc-500 border-transparent`}"
            onclick={() => switchTab('tasks')}
          >Tasks</button>
          <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
          <button
            data-testid="tab-mesh"
            class="px-3 py-1 text-[13px] transition-colors border-b-2
              {activeTab === 'mesh' ? `font-medium ${t.textPrimary} border-brand-500` : `${t.textTertiary} hover:text-zinc-500 border-transparent`}"
            onclick={() => switchTab('mesh')}
          >Mesh</button>
          <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
          <button
            data-testid="tab-git"
            class="px-3 py-1 text-[13px] transition-colors border-b-2
              {activeTab === 'git' ? `font-medium ${t.textPrimary} border-brand-500` : `${t.textTertiary} hover:text-zinc-500 border-transparent`}"
            onclick={() => switchTab('git')}
          >Git</button>
        {/if}
      </div>

      <!-- Right scoop: inverse radius where tab pill meets dark frame -->
      <div class="w-2.5 h-2.5 {t.mainBg} self-end overflow-hidden shrink-0">
        <div class="w-full h-full bg-brand-950 rounded-bl-full"></div>
      </div>

      <!-- Drag region (data-tauri-drag-region in production) -->
      <div class="flex-1 h-full" data-tauri-drag-region></div>

      <!-- Titlebar controls -->
      <div class="flex items-center gap-0.5 pb-2 pr-1 shrink-0">
        <button
          data-testid="search-btn"
          class="w-7 h-7 flex items-center justify-center rounded text-white/30 hover:text-white/60 hover:bg-white/10 transition-colors mr-1"
          onclick={() => searchOpen = !searchOpen}
          title={navigator.platform?.includes('Mac') ? 'Search (⌘K)' : 'Search (Ctrl+K)'}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
          </svg>
        </button>
        <button
          data-testid="theme-light"
          class="px-2 py-0.5 rounded text-[11px] font-medium transition-colors
            {!dark ? 'bg-white/10 text-white/90' : 'text-white/30 hover:text-white/60'}"
          onclick={() => setDarkMode(false)}
        >Light</button>
        <button
          data-testid="theme-dark"
          class="px-2 py-0.5 rounded text-[11px] font-medium transition-colors
            {dark ? 'bg-white/10 text-white/90' : 'text-white/30 hover:text-white/60'}"
          onclick={() => setDarkMode(true)}
        >Dark</button>

        <!-- Window controls -->
        <div class="flex items-center ml-2">
          <button
            class="w-7 h-7 flex items-center justify-center rounded text-white/40 hover:text-white/80 hover:bg-white/10 transition-colors"
            onclick={minimizeWindow}
            title="Minimize"
          >
            <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor"/></svg>
          </button>
          <button
            class="w-7 h-7 flex items-center justify-center rounded text-white/40 hover:text-white/80 hover:bg-white/10 transition-colors"
            onclick={toggleMaximize}
            title="Maximize"
          >
            <svg width="9" height="9" viewBox="0 0 9 9" fill="none"><rect x="0.5" y="0.5" width="8" height="8" rx="1" stroke="currentColor"/></svg>
          </button>
          <button
            class="w-7 h-7 flex items-center justify-center rounded text-white/40 hover:text-white/80 hover:bg-red-500/80 transition-colors"
            onclick={closeWindow}
            title="Close"
          >
            <svg width="10" height="10" viewBox="0 0 10 10"><path d="M1 1L9 9M9 1L1 9" stroke="currentColor" stroke-width="1.2"/></svg>
          </button>
        </div>
      </div>
    </div>
  </div>

  <!-- ═══ BODY — panels floating inside the dark frame ═══ -->
  <div class="flex-1 flex gap-1.5 p-1.5 pt-0 min-h-0">

    <!-- ═══ SIDEBAR ═══ -->
    <Sidebar
      {projects}
      {sidebarLoading}
      {sidebarError}
      {selectedProject}
      daemonStatus={daemonStatus}
      {settingsOpen}
    />

    <!-- ═══ MAIN PANEL ═══ -->
    <main class="flex-1 {t.mainBg} {t.textBody} rounded-b-lg rounded-tr-lg flex flex-col min-w-0 overflow-hidden {panelBorder}">

      <!-- Non-blocking daemon reconnect notice -->
      {#if (daemonStatus === 'reconnecting' || daemonStatus === 'disconnected') && !settingsOpen}
        <div
          class="flex items-center gap-3 px-4 py-2 {dark ? 'bg-brand-500/10 border-b border-brand-500/20' : 'bg-brand-50 border-b border-brand-200'}"
          data-testid="daemon-connecting-banner"
        >
          <svg class="h-4 w-4 shrink-0 text-brand-500 animate-pulse" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
            <path fill-rule="evenodd" d="M10 18a8 8 0 1 0-5.657-2.343l1.414-1.414A6 6 0 1 1 10 16v2Zm1-11V4H9v5h5V7h-3Z" clip-rule="evenodd" />
          </svg>
          <span class="flex-1 text-[12px] {t.textSecondary}">
            Connecting to daemon. The shell is available; session updates may be delayed.
          </span>
        </div>
      {/if}

      <!-- Daemon update banner -->
      {#if daemonUpdateAvailable && !daemonUpdateDismissed && !settingsOpen}
        <div class="flex items-center gap-3 px-4 py-2 {dark ? 'bg-warning-500/10 border-b border-warning-500/20' : 'bg-warning-50 border-b border-warning-200'}" data-testid="daemon-update-banner">
          <svg class="w-4 h-4 text-warning-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z"/></svg>
          <span class="text-[12px] {t.textSecondary} flex-1">
            Daemon update available: v{daemonUpdateAvailable.version} → v{daemonUpdateAvailable.bundled_version}
          </span>
          <button
            class="text-[12px] font-medium text-brand-500 hover:text-brand-400 transition-colors disabled:opacity-50"
            onclick={handleDaemonUpdate}
            disabled={daemonUpdating}
            data-testid="daemon-update-button"
          >{daemonUpdating ? 'Updating...' : 'Update now'}</button>
          <button
            class="text-[12px] {t.textTertiary} hover:text-white/60 transition-colors"
            onclick={() => daemonUpdateDismissed = true}
            data-testid="daemon-update-dismiss"
          >Dismiss</button>
        </div>
      {/if}

      {#if projectLoadIssues.length > 0 && !settingsOpen}
        <div class="flex items-center gap-3 px-4 py-2 {dark ? 'bg-red-500/10 border-b border-red-500/20' : 'bg-red-50 border-b border-red-200'}" data-testid="project-load-degraded-banner">
          <svg class="w-4 h-4 text-red-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.007M4.93 19.5h14.14c1.54 0 2.502-1.667 1.732-3L13.732 4.25c-.77-1.333-2.694-1.333-3.464 0L3.198 16.5c-.77 1.333.192 3 1.732 3Z"/></svg>
          <span class="text-[12px] {t.textSecondary} flex-1" data-testid="project-load-degraded-message">
            Partial project load: {projectLoadIssues.map(i => i.section).join(', ')} failed.
          </span>
          <button
            class="text-[12px] font-medium text-brand-500 hover:text-brand-400 transition-colors"
            onclick={retryProjectLoad}
            data-testid="project-load-retry"
          >Retry</button>
          <button
            class="text-[12px] {t.textTertiary} hover:text-white/60 transition-colors"
            onclick={() => projectLoadIssues = []}
            data-testid="project-load-dismiss"
          >Dismiss</button>
        </div>
      {/if}

      {#if shellNotice && !settingsOpen}
        <div class="flex items-center gap-3 px-4 py-2 {dark ? 'bg-warning-500/10 border-b border-warning-500/20' : 'bg-warning-50 border-b border-warning-200'}" data-testid="shell-notice-banner">
          <svg class="w-4 h-4 text-warning-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.007M4.93 19.5h14.14c1.54 0 2.502-1.667 1.732-3L13.732 4.25c-.77-1.333-2.694-1.333-3.464 0L3.198 16.5c-.77 1.333.192 3 1.732 3Z"/></svg>
          <span class="text-[12px] {t.textSecondary} flex-1" data-testid="shell-notice-message">{shellNotice}</span>
          <button
            class="text-[12px] {t.textTertiary} hover:text-white/60 transition-colors"
            onclick={() => shellNotice = null}
            data-testid="shell-notice-dismiss"
          >Dismiss</button>
        </div>
      {/if}

      {#if settingsOpen}
        <Settings {dark} onClose={() => settingsOpen = false} onSettingsChanged={loadProjects} {codeThemeLight} {codeThemeDark} onCodeThemeChanged={handleCodeThemeChanged} />
      {:else if !selectedProject}
        <!-- No project selected -->
        <div class="flex-1 flex items-center justify-center">
          <p class="text-[13px] {t.textTertiary}">Select a project</p>
        </div>
      {:else}
      {#key selectedProject.id}
      <div class="flex-1 flex flex-col min-w-0 overflow-hidden content-enter" data-testid="content-wrapper">
      <!-- ═══ OVERVIEW TAB ═══ -->
      <div class="flex-1 flex flex-col min-h-0 overflow-hidden" class:hidden={activeTab !== 'overview'}>
        <OverviewTab
          {dark}
          {codeTheme}
          data={{
            selectedProject,
            projects,
            recentCommits,
            commitsLoading,
            latestSession,
            sessionHistory,
            sessionLoading,
            readmeContent,
            relationships,
            relationshipsLoading,
          }}
          onViewAllCommits={viewAllCommits}
          onDismissRelationship={handleDismissRelationship}
          onMarkdownNavigate={handleMarkdownNavigate}
        />
      </div>

      <!-- ═══ TASKS TAB ═══ -->
      <div class="flex-1 flex min-h-0 overflow-hidden" class:hidden={activeTab !== 'tasks'}>
        {#if visitedTabs.has('tasks')}
          <TaskBoard
            projectId={selectedProject.id}
            projectPath={selectedProject.path}
            isActive={activeTab === 'tasks'}
            {dark}
            {codeTheme}
            bind:position={taskPosition}
            navTarget={taskNavTarget}
            onClearNavTarget={() => { taskNavTarget = null }}
          />
        {/if}
      </div>

      <!-- ═══ MESH TAB ═══ -->
      <div class="flex-1 flex min-h-0 overflow-hidden" class:hidden={activeTab !== 'mesh'}>
        {#if visitedTabs.has('mesh')}
          <MeshTab
            {dark}
            projectPath={selectedProject.path}
            availableProjects={projects}
          />
        {/if}
      </div>

      <!-- ═══ GIT TAB ═══ -->
      <div class="flex-1 flex min-h-0 overflow-hidden" class:hidden={activeTab !== 'git'}>
        {#if visitedTabs.has('git')}
          <GitTab
            projectPath={selectedProject.path}
            projectId={selectedProject.id}
            {dark}
            navTarget={gitNavTarget}
            bind:position={gitPosition}
            onNavigateToFile={navigateToFile}
            onClearNavTarget={() => { gitNavTarget = null }}
          />
        {/if}
      </div>

      <!-- ═══ FILES TAB ═══ -->
      <div class="flex-1 flex min-h-0 overflow-hidden" class:hidden={activeTab !== 'files'}>
        {#if visitedTabs.has('files')}
          <FilesTab
            {dark}
            {codeTheme}
            {selectedProject}
            isActive={activeTab === 'files'}
            navTarget={filesNavTarget}
            onClearNavTarget={() => { filesNavTarget = null }}
            bind:position={filesPosition}
            onMarkdownNavigate={handleMarkdownNavigate}
            changedPaths={fileChangePaths}
            onChangedPathsConsumed={() => { fileChangePaths = null }}
          />
        {/if}
      </div>
      </div>
      {/key}
      {/if}
    </main>
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
