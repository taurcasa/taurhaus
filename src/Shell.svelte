<script>
  import { listProjects, getProject, getRecentCommits, getAllCommits, getReadme, getLatestSession, listSessions, getRelationships, dismissRelationship, isTauri, isFirstRun, getSettings, getDaemonStatus, launchClaudeSession, navigateToSession } from './lib/ipc.js'
  import { getSessionForProject } from './lib/sessionStore.svelte.js'
  import * as assetCache from './lib/assetCache.js'
  import { anyPathMatches } from './lib/fileChange.js'
  import TaskBoard from './lib/TaskBoard.svelte'
  import GitTab from './lib/GitTab.svelte'
  import SearchOverlay from './lib/SearchOverlay.svelte'
  import Settings from './lib/Settings.svelte'
  import AddProjectModal from './lib/AddProjectModal.svelte'
  import FirstRunWizard from './lib/FirstRunWizard.svelte'
  import OverviewTab from './lib/OverviewTab.svelte'
  import FilesTab from './lib/FilesTab.svelte'
  import Sidebar from './lib/Sidebar.svelte'
  import { startPolling as startSessionPolling, stopPolling as stopSessionPolling } from './lib/sessionStore.svelte.js'
  import { push as pushNav, goBack as navGoBack, goForward as navGoForward, reset as resetNav, withSuppressed as navWithSuppressed } from './lib/navHistory.svelte.js'

  import { DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME } from './lib/shikiThemes.js'
  import { themeTokens } from './lib/themeTokens.js'

  let dark = $state(false)
  let preview = $state(false)

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

  // Daemon status: 'connected' | 'disconnected' | 'reconnecting' | 'failed' | 'not_configured' | null
  let daemonStatus = $state(null)
  let daemonStatusDismissTimer = $state(null)

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
  let filesNavTarget = $state(null) // { file: string, lineNumber?: number } | null
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

  // Cross-tab navigation state for Git tab
  let gitNavTarget = $state(null) // { type: 'commit', hash } | { type: 'range', after, before } | null

  // Per-project position memory — remembers where you were when you switch away
  const projectPositions = new Map() // projectId → { tab, visitedTabs, file?, gitPosition?, taskPosition? }

  // Bound positions from child components (synced via $bindable)
  let gitPosition = $state(null)
  let taskPosition = $state(null)
  let taskNavTarget = $state(null)

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

  // Load code theme prefs from settings
  async function loadCodeThemeFromSettings() {
    try {
      const s = await getSettings()
      if (s.code_theme) {
        codeThemeLight = s.code_theme.light || DEFAULT_LIGHT_THEME
        codeThemeDark = s.code_theme.dark || DEFAULT_DARK_THEME
      }
    } catch {
      // Keep defaults on error
    }
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
    }
  }

  function handleWizardComplete() {
    showWizard = false
    loadProjects()
    loadCodeThemeFromSettings()
    loadDaemonStatus()
  }

  async function loadDaemonStatus() {
    try {
      const status = await getDaemonStatus()
      // Only show non-connected states (connected is the happy path, don't clutter)
      if (status.status !== 'connected') {
        daemonStatus = status.status
      }
    } catch { /* ignore — not critical */ }
  }

  // Command Center — poll for Claude Code sessions
  $effect(() => {
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
    let cleanups = []

    import('@tauri-apps/api/event').then(({ listen }) => {
      // Git status changed — refresh sidebar project status
      listen('project-git-changed', (event) => {
        const { project_id } = event.payload
        const idx = projects.findIndex(p => p.id === project_id)
        if (idx !== -1 && event.payload.branch !== undefined) {
          projects[idx] = { ...projects[idx], branch: event.payload.branch, is_dirty: event.payload.is_dirty }
        }
        if (selectedProject?.id === project_id) {
          selectedProject = { ...selectedProject, branch: event.payload.branch ?? selectedProject.branch, is_dirty: event.payload.is_dirty ?? selectedProject.is_dirty }
        }
      }).then(u => cleanups.push(u))

      // Session imported — refresh session display
      listen('session-imported', (event) => {
        const { project_id } = event.payload
        if (selectedProject?.id === project_id) {
          loadSessions(project_id)
        }
      }).then(u => cleanups.push(u))

      // Startup reseed complete — reload project list to pick up cached git status
      listen('projects-reseed-complete', () => {
        loadProjects()
      }).then(u => cleanups.push(u))

      // File changes — central handler for all file-change responses.
      // Invalidates caches, refreshes Overview README, and signals
      // FilesTab to refresh via the fileChangePaths reactive prop.
      listen('project-files-changed', (event) => {
        const { project_id, paths } = event.payload
        console.log(`[filewatch] Shell: project-files-changed for ${project_id}, ${paths?.length ?? 0} path(s)`, paths)
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
      }).then(u => cleanups.push(u))

      // Daemon status changes (bootstrap chain + health check)
      listen('daemon-status', (event) => {
        const { status } = event.payload
        daemonStatus = status
        clearTimeout(daemonStatusDismissTimer)
        // Auto-dismiss "connected" after 3 seconds
        if (status === 'connected') {
          daemonStatusDismissTimer = setTimeout(() => { daemonStatus = null }, 3000)
        }
      }).then(u => cleanups.push(u))
    })

    return () => {
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
        await selectProject(projects[0])
      }
      // Git status now comes from cached columns in list_projects (no extra IPC calls).
      // The cache is refreshed by the file watcher and startup reseed.
    } catch (e) {
      sidebarError = e.message || 'Failed to load projects'
    } finally {
      sidebarLoading = false
    }
  }


  let _selectGeneration = 0
  async function selectProject(project) {
    const projectId = project.id

    // Save position in the current project before switching away
    saveProjectPosition()

    const savedPosition = projectPositions.get(projectId)
    const restoredTab = savedPosition?.tab || 'overview'
    const generation = ++_selectGeneration

    // Fire all IPC calls in parallel — don't touch state yet
    const [detail, commits, sessions, readme, rels] = await Promise.all([
      getProject(projectId).catch(() => null),
      getRecentCommits(projectId, 10).catch(() => []),
      Promise.all([
        getLatestSession(projectId).catch(() => null),
        listSessions(projectId, 10).catch(() => []),
      ]),
      getReadme(projectId).catch(() => null),
      getRelationships(projectId).catch(() => []),
    ])

    // Stale check — user clicked a different project while we were loading
    if (generation !== _selectGeneration) return

    // Commit everything in one synchronous block → single DOM repaint
    selectedProject = detail ? { ...project, ...detail } : project
    detailLoading = false
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
    recentCommits = commits
    commitsLoading = false
    latestSession = sessions[0]
    sessionHistory = sessions[1] || []
    sessionLoading = false
    readmeContent = readme
    relationships = rels
    relationshipsLoading = false
    // Restore file position via navigateTarget — FilesTab loads its own tree
    filesNavTarget = savedPosition?.file ? { file: savedPosition.file } : null
  }

  async function loadSessions(projectId) {
    sessionLoading = true
    try {
      const [latest, history] = await Promise.all([
        getLatestSession(projectId),
        listSessions(projectId, 10),
      ])
      latestSession = latest
      sessionHistory = history || []
    } catch {
      latestSession = null
      sessionHistory = []
    } finally {
      sessionLoading = false
    }
  }

  async function loadReadmeForOverview(projectId) {
    try {
      readmeContent = await getReadme(projectId)
    } catch {
      readmeContent = null
    }
  }

  async function loadRelationships(projectId) {
    relationshipsLoading = true
    try {
      relationships = await getRelationships(projectId)
    } catch {
      relationships = []
    } finally {
      relationshipsLoading = false
    }
  }

  async function handleDismissRelationship(relId) {
    try {
      await dismissRelationship(relId)
      relationships = relationships.filter(r => r.id !== relId)
    } catch {
      // Silent fail — relationship may still show
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

  // Dev-only: fullscreen preview simulates Tauri desktop experience
  function togglePreview() {
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen()
      preview = true
    } else {
      document.exitFullscreen()
      preview = false
    }
  }

  $effect(() => {
    const handler = () => {
      if (!document.fullscreenElement) preview = false
    }
    document.addEventListener('fullscreenchange', handler)
    return () => document.removeEventListener('fullscreenchange', handler)
  })

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

  function handleMarkdownNavigate(relativePath) {
    if (!selectedProject) return

    // Resolve relative path against the currently viewed file's directory.
    // If viewing a file like "docs/design-brief.md" and clicking "./foo.md",
    // resolve to "docs/foo.md".
    let resolved = relativePath

    // Strip leading ./ for normalization
    resolved = resolved.replace(/^\.\//, '')

    // If we have a current file context, resolve relative to its directory
    const contextFile = filesPosition?.selectedFile || (readmeContent?.path)
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
    resolved = normalized.join('/')

    console.log(`[markdown] navigate: "${relativePath}" → "${resolved}"`)

    // Switch to files tab and navigate via FilesTab
    filesNavTarget = { file: resolved }
    switchTab('files', { tab: 'files', file: resolved })
  }

  function handleSearchNavigate(action) {
    if (action.tab === 'files' && action.filePath) {
      filesNavTarget = { file: action.filePath }
      switchTab('files', { tab: 'files', file: action.filePath })
    } else if (action.tab === 'overview') {
      switchTab('overview')
      // Scroll to section if specified (commits section)
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
            class="px-3 py-1 text-[13px] transition-colors border-b-2
              {activeTab === 'overview' ? `font-medium ${t.textPrimary} border-brand-500` : `${t.textTertiary} hover:text-zinc-500 border-transparent`}"
            onclick={() => switchTab('overview')}
          >Overview</button>
          <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
          <button
            class="px-3 py-1 text-[13px] transition-colors border-b-2
              {activeTab === 'files' ? `font-medium ${t.textPrimary} border-brand-500` : `${t.textTertiary} hover:text-zinc-500 border-transparent`}"
            onclick={() => switchTab('files')}
          >Files</button>
          <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
          <button
            class="px-3 py-1 text-[13px] transition-colors border-b-2
              {activeTab === 'tasks' ? `font-medium ${t.textPrimary} border-brand-500` : `${t.textTertiary} hover:text-zinc-500 border-transparent`}"
            onclick={() => switchTab('tasks')}
          >Tasks</button>
          <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
          <button
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
      <div class="flex items-center gap-0.5 pb-2 pr-3 shrink-0">
        <button
          class="px-2 py-0.5 rounded text-[11px] font-medium transition-colors
            {!dark ? 'bg-white/10 text-white/90' : 'text-white/30 hover:text-white/60'}"
          onclick={() => dark = false}
        >Light</button>
        <button
          class="px-2 py-0.5 rounded text-[11px] font-medium transition-colors
            {dark ? 'bg-white/10 text-white/90' : 'text-white/30 hover:text-white/60'}"
          onclick={() => dark = true}
        >Dark</button>
        <button
          class="ml-1.5 px-2 py-0.5 rounded text-[11px] font-medium text-brand-400/60 hover:text-brand-400 transition-colors"
          onclick={togglePreview}
        >{preview ? 'Exit' : 'Preview'}</button>
      </div>
    </div>
  </div>

  <!-- ═══ BODY — panels floating inside the dark frame ═══ -->
  <div class="flex-1 flex gap-1.5 p-1.5 pt-0 min-h-0">

    <!-- ═══ SIDEBAR ═══ -->
    <Sidebar
      {projects}
      {selectedProject}
      {sidebarLoading}
      {sidebarError}
      {daemonStatus}
      {settingsOpen}
      onSelectProject={selectProject}
      onAddProject={() => showAddProject = true}
      onToggleSettings={() => settingsOpen = !settingsOpen}
      onRetry={loadProjects}
      onProjectRemoved={(id) => {
        projects = projects.filter(p => p.id !== id)
        if (selectedProject?.id === id) {
          selectedProject = projects.length > 0 ? projects[0] : null
          if (selectedProject) selectProject(selectedProject)
        }
      }}
    />

    <!-- ═══ MAIN PANEL ═══ -->
    <main class="flex-1 {t.mainBg} {t.textBody} rounded-b-lg rounded-tr-lg flex flex-col min-w-0 overflow-hidden {panelBorder}">
      {#if settingsOpen}
        <Settings {dark} onClose={() => settingsOpen = false} onSettingsChanged={loadProjects} {codeThemeLight} {codeThemeDark} onCodeThemeChanged={handleCodeThemeChanged} />
      {:else if !selectedProject}
        <!-- No project selected -->
        <div class="flex-1 flex items-center justify-center">
          <p class="text-[13px] {t.textTertiary}">Select a project</p>
        </div>
      {:else}
      {#key selectedProject.id}
      <div class="flex-1 flex flex-col min-w-0 overflow-hidden">
      <!-- ═══ OVERVIEW TAB ═══ -->
      <div class="flex-1 flex flex-col min-h-0 overflow-hidden" class:hidden={activeTab !== 'overview'}>
        <OverviewTab
          {dark}
          {codeTheme}
          {selectedProject}
          {projects}
          {recentCommits}
          {commitsLoading}
          {latestSession}
          {sessionHistory}
          {sessionLoading}
          {readmeContent}
          {relationships}
          {relationshipsLoading}
          onNavigateToCommit={navigateToCommit}
          onViewAllCommits={viewAllCommits}
          onDismissRelationship={handleDismissRelationship}
          onSelectProject={selectProject}
          onMarkdownNavigate={handleMarkdownNavigate}
          onLaunchSession={handleOverviewLaunchSession}
          onOpenTerminal={handleOverviewOpenTerminal}
        />
      </div>

      <!-- ═══ TASKS TAB ═══ -->
      <div class="flex-1 flex min-h-0 overflow-hidden" class:hidden={activeTab !== 'tasks'}>
        {#if visitedTabs.has('tasks')}
          <TaskBoard
            projectPath={selectedProject.path}
            {dark}
            {codeTheme}
            bind:position={taskPosition}
            navTarget={taskNavTarget}
            onClearNavTarget={() => { taskNavTarget = null }}
            onNavigateToCommit={navigateToCommit}
            onNavigateToFile={navigateToFile}
            onNavigateToCommitRange={navigateToCommitRange}
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
    <AddProjectModal {dark} onClose={() => showAddProject = false} onProjectsChanged={loadProjects} />
  {/if}

</div>
{/if}

<style>
  /* Subtle fade-up on project switch — signals "new content" without
     feeling like a loading transition. Triggered by {#key} recreating
     the wrapper div. */
  .content-enter {
    animation: content-enter 120ms ease-out;
  }
  @keyframes content-enter {
    from { opacity: 0.6; transform: translateY(4px); }
    to   { opacity: 1;   transform: translateY(0); }
  }
</style>
