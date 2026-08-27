<script>
  import { onMount } from 'svelte'
  import { listProjects, getProject, getRecentCommits, getAllCommits, getReadme, getLatestSession, listSessions, getRelationships, dismissRelationship, isTauri, isFirstRun, getSettings, updateSettings, getDaemonStatus, checkDaemonInstallStatus, installDaemon, launchCliSession, navigateToSession, getForegroundProject, getRemoteUrl, checkPathType, openExternalUrl, getPlatform, listClaudeSessions, startDaemon } from './lib/ipc.js'
  import { getSessionForProject, getSessions, applyDaemonSessionUpdate, hydrateFromBackend as hydrateSessionsFromBackend, markSessionPresenceStale, DEFAULT_TAURI_POLL_INTERVAL_MS } from './lib/sessionStore.svelte.js'
  import * as assetCache from './lib/assetCache.js'
  import { anyPathMatches } from './lib/fileChange.js'
  import ShellMainPanel from './lib/components/shell/ShellMainPanel.svelte'
  import ShellTitlebar from './lib/components/shell/ShellTitlebar.svelte'
  import SearchOverlay from './lib/SearchOverlay.svelte'
  import AccountChooser from './lib/components/AccountChooser.svelte'
  import {
    accountState,
    pendingAccountChoice,
    refreshAccounts,
    requestLaunch,
    resolveChooserAccounts,
  } from './lib/accounts.svelte.js'
  import AddProjectModal from './lib/AddProjectModal.svelte'
  import FirstRunWizard from './lib/FirstRunWizard.svelte'
  import Sidebar from './lib/Sidebar.svelte'
  import { startPolling as startSessionPolling, stopPolling as stopSessionPolling } from './lib/sessionStore.svelte.js'
  import { push as pushNav, goBack as navGoBack, goForward as navGoForward, reset as resetNav, withSuppressed as navWithSuppressed } from './lib/navHistory.svelte.js'
  import { createShellDaemonStatusController } from './lib/shell/daemonStatus.svelte.js'
  import { setupShellEventListeners } from './lib/shell/events.svelte.js'
  import { createShellNavigationController } from './lib/shell/navigation.svelte.js'
  import { createShellProjectSelectionController } from './lib/shell/projectSelection.svelte.js'
  import { createShellSessionLifecycleController } from './lib/shell/sessionLifecycle.svelte.js'
  import { createStateBridge } from './lib/shell/stateBridge.js'
  import { setupHistoryNavigation, setupSearchShortcut } from './lib/shell/shortcuts.svelte.js'
  import { loadThemePreferences, persistDarkModePreference } from './lib/shell/themePreferences.js'
  import {
    closeShellWindow,
    minimizeShellWindow,
    syncWindowsStartupViewport as syncStartupViewportWindow,
    toggleShellMaximize,
  } from './lib/shell/window.js'
  import { setProjectContext } from './lib/context/ProjectContext.js'
  import { setSessionContext } from './lib/context/SessionContext.js'
  import { setModelCatalogContext } from './lib/context/ModelCatalogContext.js'
  import { EMPTY_MODEL_CATALOG } from './lib/modelCatalog.js'
  import { configureToolRegistry, tools as registryTools } from './lib/toolRegistry.js'
  import { DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME } from './lib/shikiThemes.js'

  let { initialDaemonStatus = undefined } = $props()

  let dark = $state(false)
  let codeThemeLight = $state(DEFAULT_LIGHT_THEME)
  let codeThemeDark = $state(DEFAULT_DARK_THEME)
  const codeTheme = $derived(dark ? codeThemeDark : codeThemeLight)
  const pendingAccount = $derived(pendingAccountChoice())
  const pendingAccountState = $derived(
    pendingAccount ? accountState(pendingAccount.tool) : null
  )

  $effect(() => {
    document.documentElement.classList.toggle('dark', dark)
  })

  let searchOpen = $state(false)
  let settingsOpen = $state(false)
  let showAddProject = $state(false)
  let showWizard = $state(false)
  let wizardChecked = $state(false)
  let startupViewportSyncAttempted = false

  let daemonStatus = $state(null)
  let daemonStatusInitialized = $state(false)
  let daemonUpdateAvailable = $state(null)  // { version, bundled_version } or null
  let daemonUpdateDismissed = $state(false)
  let daemonUpdating = $state(false)
  let daemonRestarting = $state(false)
  let daemonRecoveryStartedAt = $state(null)
  let daemonRecoveryEscalated = $state(false)
  let shellNotice = $state(null)

  let projects = $state([])
  let selectedProject = $state(null)
  let foregroundProjectId = $state(null)
  let sidebarLoading = $state(true)
  let sidebarError = $state(null)
  let detailLoading = $state(false)

  let activeTab = $state('overview')
  let visitedTabs = $state(new Set(['overview']))
  let recentCommits = $state([])
  let commitsLoading = $state(false)
  let showAllCommits = $state(false)
  let filesNavTarget = $state(null)
  let filesPosition = $state(null)
  let fileChangePaths = $state(null)
  let latestSession = $state(null)
  let sessionHistory = $state([])
  let sessionLoading = $state(false)
  let readmeContent = $state(null)
  let sessionBridgeLive = $state(false)
  let relationships = $state([])
  let relationshipsLoading = $state(false)
  let projectLoadIssues = $state([])
  let pendingProjectLoadRetry = $state(false)
  let gitNavTarget = $state(null)
  let gitPosition = $state(null)
  let taskPosition = $state(null)
  let taskNavTarget = $state(null)

  const daemonController = createShellDaemonStatusController({
    getInitialDaemonStatus: () => initialDaemonStatus,
    state: createStateBridge({
      daemonStatus: [() => daemonStatus, (value) => daemonStatus = value],
      daemonStatusInitialized: [() => daemonStatusInitialized, (value) => daemonStatusInitialized = value],
      daemonUpdateAvailable: [() => daemonUpdateAvailable, (value) => daemonUpdateAvailable = value],
      daemonUpdateDismissed: [() => daemonUpdateDismissed, (value) => daemonUpdateDismissed = value],
      daemonUpdating: [() => daemonUpdating, (value) => daemonUpdating = value],
      daemonRestarting: [() => daemonRestarting, (value) => daemonRestarting = value],
      daemonRecoveryStartedAt: [() => daemonRecoveryStartedAt, (value) => daemonRecoveryStartedAt = value],
      daemonRecoveryEscalated: [() => daemonRecoveryEscalated, (value) => daemonRecoveryEscalated = value],
    }),
    ipc: {
      getDaemonStatus,
      checkDaemonInstallStatus,
      installDaemon,
      startDaemon,
    },
    onNotice: (message) => {
      shellNotice = message
    },
    logger: console,
  })

  const projectController = createShellProjectSelectionController({
    state: createStateBridge({
      projects: [() => projects, (value) => projects = value],
      selectedProject: [() => selectedProject, (value) => selectedProject = value],
      sidebarLoading: [() => sidebarLoading, (value) => sidebarLoading = value],
      sidebarError: [() => sidebarError, (value) => sidebarError = value],
      detailLoading: [() => detailLoading, (value) => detailLoading = value],
      activeTab: [() => activeTab, (value) => activeTab = value],
      visitedTabs: [() => visitedTabs, (value) => visitedTabs = value],
      recentCommits: [() => recentCommits, (value) => recentCommits = value],
      commitsLoading: [() => commitsLoading, (value) => commitsLoading = value],
      showAllCommits: [() => showAllCommits, (value) => showAllCommits = value],
      filesNavTarget: [() => filesNavTarget, (value) => filesNavTarget = value],
      latestSession: [() => latestSession, (value) => latestSession = value],
      sessionHistory: [() => sessionHistory, (value) => sessionHistory = value],
      sessionLoading: [() => sessionLoading, (value) => sessionLoading = value],
      readmeContent: [() => readmeContent, (value) => readmeContent = value],
      relationships: [() => relationships, (value) => relationships = value],
      relationshipsLoading: [() => relationshipsLoading, (value) => relationshipsLoading = value],
      projectLoadIssues: [() => projectLoadIssues, (value) => projectLoadIssues = value],
      pendingProjectLoadRetry: [() => pendingProjectLoadRetry, (value) => pendingProjectLoadRetry = value],
      gitNavTarget: [() => gitNavTarget, (value) => gitNavTarget = value],
      taskNavTarget: [() => taskNavTarget, (value) => taskNavTarget = value],
    }),
    positions: createStateBridge({
      files: [() => filesPosition, () => {}],
      git: [() => gitPosition, () => {}],
      task: [() => taskPosition, () => {}],
    }),
    nav: {
      push: pushNav,
      reset: resetNav,
      withSuppressed: navWithSuppressed,
    },
    ipc: {
      listProjects,
      getProject,
      getRecentCommits,
      getAllCommits,
      getReadme,
      getLatestSession,
      listSessions,
      getRelationships,
      dismissRelationship,
    },
    getDaemonRecoveryPending: () => daemonController.recoveryPending(),
    getDaemonStatus: () => daemonStatus,
    getSessionBridgeLive: () => sessionBridgeLive,
    logger: console,
    doc: document,
  })

  const sessionController = createShellSessionLifecycleController({
    state: createStateBridge({
      foregroundProjectId: [() => foregroundProjectId, (value) => foregroundProjectId = value],
      sessionBridgeLive: [() => sessionBridgeLive, (value) => sessionBridgeLive = value],
    }),
    getProjects: () => projects,
    ipc: {
      getForegroundProject,
      listClaudeSessions,
      navigateToSession,
    },
    sessionStore: {
      getSessions,
      applyDaemonSessionUpdate,
      markSessionPresenceStale,
    },
    logger: console,
  })

  const navigationController = createShellNavigationController({
    state: createStateBridge({
      selectedProject: [() => selectedProject, () => {}],
      projects: [() => projects, () => {}],
      activeTab: [() => activeTab, () => {}],
      readmeContent: [() => readmeContent, () => {}],
      filesPosition: [() => filesPosition, () => {}],
      filesNavTarget: [() => filesNavTarget, (value) => filesNavTarget = value],
      gitNavTarget: [() => gitNavTarget, (value) => gitNavTarget = value],
    }),
    ipc: {
      getRemoteUrl,
      checkPathType,
      openExternalUrl,
    },
    selectProject: (project) => projectController.selectProject(project),
    switchTab: (tab, navEntry) => projectController.switchTab(tab, navEntry),
    logger: console,
  })

  let projectContextValue = $state({
    projects: [],
    selectedProject: null,
    selectProject: (project) => projectController.selectProject(project),
    navigateToCommit: (hash) => navigationController.navigateToCommit(hash),
    navigateToFile: (path, lineNumber) => navigationController.navigateToFile(path, lineNumber),
    navigateToCommitRange: (after, before) => navigationController.navigateToCommitRange(after, before),
    onProjectRemoved: (id) => projectController.handleProjectRemoved(id),
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
      projectController.loadProjects()
    },
  })
  const sessionContext = setSessionContext(sessionContextValue)

  // Model catalog is owned by the backend and reaches every roster/role editor
  // through this context instead of a hardcoded frontend list.
  let modelCatalogContextValue = $state({ catalog: EMPTY_MODEL_CATALOG })
  setModelCatalogContext(modelCatalogContextValue)

  $effect(() => {
    projectContext.projects = projects
    projectContext.selectedProject = selectedProject
  })

  $effect(() => {
    sessionContext.daemonStatus = daemonStatus
  })

  function errorMessage(error) {
    if (error && typeof error === 'object' && typeof error.message === 'string' && error.message.trim()) {
      return error.message
    }
    if (typeof error === 'string' && error.trim()) {
      return error
    }
    return String(error)
  }

  async function loadModelCatalogFromSettings() {
    try {
      const settings = await getSettings()
      const catalog = settings?.terminal_contract?.model_catalog
      if (catalog) modelCatalogContextValue.catalog = catalog
      configureToolRegistry(settings?.terminal_contract?.tools)
    } catch (error) {
      console.error('[settings] failed to load the model catalog:', error)
    }
  }

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

  $effect(() => {
    void checkFirstRun()
  })

  $effect(() => {
    void loadModelCatalogFromSettings()
  })

  // Account detection reads the WSL home through the daemon on Windows, so a
  // daemon that arrives late has to be asked again — until then the chooser
  // has nothing to offer.
  let lastAccountDetectionDaemonStatus = null
  $effect(() => {
    const status = daemonStatus
    if (status === lastAccountDetectionDaemonStatus) return
    const reconnected = status === 'connected' && lastAccountDetectionDaemonStatus !== null
    lastAccountDetectionDaemonStatus = status
    for (const tool of registryTools().filter((entry) => entry.capabilities.accountSelection)) {
      void refreshAccounts(tool.id, { force: reconnected })
    }
  })

  $effect(() => {
    void sessionController.loadForegroundProject()
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
      void projectController.loadProjects()
      void loadCodeThemeFromSettings()
      void daemonController.loadDaemonStatus()
      void syncWindowsStartupViewport()
    }
  }

  function handleWizardComplete() {
    showWizard = false
    void projectController.loadProjects()
    void loadCodeThemeFromSettings()
    void daemonController.loadDaemonStatus({ allowInitial: false })
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

  $effect(() => {
    daemonController.syncRecoveryEscalation()
  })

  $effect(() => {
    return () => {
      daemonController.cleanup()
    }
  })

  $effect(() => {
    return sessionController.setupPolling({
      isTauri,
      startPolling: () => startSessionPolling({
        intervalMs: isTauri() ? DEFAULT_TAURI_POLL_INTERVAL_MS : undefined,
        onSessionsReceived: () => {
          sessionController.markSessionBridgeLive()
        },
      }),
      stopPolling: () => stopSessionPolling({ flushActivity: false }),
      doc: document,
    })
  })

  onMount(() => {
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
          void projectController.loadCommits(project_id, showAllCommits ? 50 : 10)
        }
      },
      onSessionImported: ({ project_id }) => {
        if (selectedProject?.id === project_id) {
          void projectController.loadSessions(project_id)
        }
      },
      onProjectsReseedComplete: () => {
        void projectController.loadProjects()
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
          void projectController.loadReadmeForOverview(project_id)
        }
        fileChangePaths = paths
      },
      onDaemonStatus: ({ status }) => {
        daemonController.handleDaemonStatusEvent(status)
        if (status !== 'connected') {
          sessionController.handleDaemonDisconnected()
        }
        projectController.maybeRetryPendingProjectLoad()
      },
      onSessionsUpdated: (payload) => {
        sessionController.handleSessionsUpdated(payload)
      },
      onTmuxFocusChanged: (payload) => {
        sessionController.handleTmuxFocusChanged(payload)
      },
      onHydrateSessions: () => {
        hydrateSessionsFromBackend()
      },
      logger: console,
    })
  })

  async function handleDismissRelationship(relId) {
    await projectController.handleDismissRelationship(relId, (message) => {
      shellNotice = message
    })
  }

  function handleOverviewLaunchSession(tool) {
    if (!selectedProject) return
    requestLaunch({
      project: selectedProject,
      mode: 'fresh',
      tool,
      launch: (projectId, mode, launchTool, accountId) =>
        launchCliSession(projectId, mode, launchTool, accountId).then((r) =>
          console.log('[overview] launch OK:', r)
        ),
      onError: (error) => console.error('[overview] launch FAILED:', error),
    })
  }

  function handleOverviewOpenTerminal() {
    if (!selectedProject) return
    const session = getSessionForProject(selectedProject.path)
    if (session?.tmux_session && session?.tmux_window && session?.tmux_pane) {
      sessionController.setForegroundProject(selectedProject.id)
      navigateToSession(session.tmux_session, session.tmux_window, session.tmux_pane, true)
    }
  }

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
      if (entry) projectController.applyNavEntry(entry)
    },
    onGoForward: () => {
      const entry = navGoForward()
      if (entry) projectController.applyNavEntry(entry)
    },
  }))
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
      onSwitchTab={(tab) => projectController.switchTab(tab)}
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
          onForegroundProjectChange={(projectId) => sessionController.setForegroundProject(projectId)}
          daemonStatus={daemonStatus}
          {settingsOpen}
          {dark}
          actions={{
            onProjectHover: projectController.prefetchProjectSelection,
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
        onSettingsChanged={() => projectController.loadProjects()}
        onCodeThemeChanged={handleCodeThemeChanged}
        onViewAllCommits={() => projectController.viewAllCommits()}
        onDismissRelationship={handleDismissRelationship}
        onMarkdownNavigate={(relativePath) => navigationController.handleMarkdownNavigate(relativePath)}
        onRetryProjectLoad={() => projectController.retryProjectLoad()}
        onHandleDaemonUpdate={() => daemonController.handleDaemonUpdate()}
        onRestartDaemon={() => daemonController.handleRestartDaemon()}
        onDismissDaemonUpdate={() => {
          daemonController.dismissDaemonUpdate()
        }}
        onDismissProjectLoadIssues={() => {
          projectController.clearProjectLoadIssues()
        }}
        onDismissShellNotice={() => {
          shellNotice = null
        }}
        onNavigateToFile={(path, lineNumber) => navigationController.navigateToFile(path, lineNumber)}
        onMeshFocusPane={(paneId) => sessionController.handleMeshFocusPane(paneId)}
        onClearTaskNavTarget={() => {
          projectController.clearTaskNavTarget()
        }}
        onClearGitNavTarget={() => {
          projectController.clearGitNavTarget()
        }}
        onClearFilesNavTarget={() => {
          projectController.clearFilesNavTarget()
        }}
        onChangedPathsConsumed={() => {
          fileChangePaths = null
        }}
      />
    </div>
  </div>

  <SearchOverlay bind:open={searchOpen} {dark} onNavigate={(action) => navigationController.handleSearchNavigate(action)} />

  {#if pendingAccount && pendingAccountState}
    <!-- The chooser brings its own `data-shell-overlay` backdrop: a wrapper
         here would inherit the frame's `position: relative` and drop it out of
         the viewport. -->
    <AccountChooser
      tool={pendingAccount.tool}
      accounts={resolveChooserAccounts(pendingAccount.tool)}
      projectName={pendingAccount.projectName}
      defaultAccountId={pendingAccountState.defaultAccountId}
      degraded={pendingAccountState.degraded}
      {dark}
      onConfirm={(accountId, remember) => pendingAccount?.confirm(accountId, remember)}
      onCancel={() => pendingAccount?.cancel()}
      onRequestUsage={() => void refreshUsage(pendingAccount.tool)}
    />
  {/if}

  {#if showAddProject}
    <AddProjectModal
      {dark}
      onClose={() => showAddProject = false}
      onProjectsChanged={() => projectController.loadProjects()}
      onProjectCreated={(project) => projectController.handleProjectCreated(project)}
    />
  {/if}

</div>
{/if}
