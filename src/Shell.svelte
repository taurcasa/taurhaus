<script>
  import { listProjects, getProject, getRecentCommits, getAllCommits, getFileTree, readFile, readProjectAsset, getReadme, getLatestSession, listSessions, getRelationships, dismissRelationship, removeProject, isTauri, isFirstRun, navigateToSession, launchClaudeSession, stopClaudeSession } from './lib/ipc.js'
  import SearchOverlay from './lib/SearchOverlay.svelte'
  import Settings from './lib/Settings.svelte'
  import AddProjectModal from './lib/AddProjectModal.svelte'
  import FirstRunWizard from './lib/FirstRunWizard.svelte'
  import MarkdownRenderer from './lib/MarkdownRenderer.svelte'
  import CodeViewer from './lib/CodeViewer.svelte'
  import ContextMenu from './lib/ContextMenu.svelte'
  import { classifyFile } from './lib/fileClassifier.js'
  import * as assetCache from './lib/assetCache.js'
  import { startPolling as startSessionPolling, stopPolling as stopSessionPolling, getSessionForProject } from './lib/sessionStore.svelte.js'
  import { rowTintClass, sessionBadge } from './lib/sessionIndicator.js'
  import HoverCard from './lib/HoverCard.svelte'

  let dark = $state(false)
  let preview = $state(false)
  let searchOpen = $state(false)
  let settingsOpen = $state(false)
  let showAddProject = $state(false)
  let showWizard = $state(false)
  let wizardChecked = $state(false)

  // Context menu state
  let ctxMenu = $state(null) // { x, y, project }
  let ctxConfirmRemove = $state(false)
  let ctxConfirmStop = $state(false)
  let ctxConfirmTimeout = $state(null)

  // Hover card state
  let hoverCard = $state(null) // { project, session, anchorEl }
  let hoverTimeout = $state(null)

  function showHoverCard(project, session, el) {
    clearTimeout(hoverTimeout)
    hoverTimeout = setTimeout(() => {
      if (!ctxMenu) hoverCard = { project, session, anchorEl: el }
    }, 80)
  }

  function hideHoverCard() {
    clearTimeout(hoverTimeout)
    hoverTimeout = setTimeout(() => { hoverCard = null }, 80)
  }

  /*
   * Layout dimensions
   * - Titlebar: 46px tall, holds logo + tab pill + controls
   * - Sidebar:  252px wide, matches logo area in titlebar
   * - Gap:      6px (gap-1.5) between sidebar and main panel
   * - Frame:    6px (p-1.5) padding around panels inside the dark frame
   */

  // Sidebar status dots — color = git activity state only (never changes for session)
  const dotColor     = { active: 'bg-success-300', recent: 'bg-info-300', stale: 'bg-warning-300', dormant: 'bg-zinc-400' }
  const dotColorDark = { active: 'bg-success-300', recent: 'bg-info-300', stale: 'bg-warning-300', dormant: 'bg-zinc-500' }

  // Main content panel — all dark-mode switching via $derived tokens
  const mainBg         = $derived(dark ? 'bg-zinc-950' : 'bg-white')
  const textPrimary    = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary  = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const textTertiary   = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const textMuted      = $derived(dark ? 'text-zinc-600' : 'text-zinc-500')
  const textBody       = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const keyline        = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const statusColor    = $derived(dark ? 'text-success-400' : 'text-success-600')
  const linkColor      = $derived(dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700')
  const dangerColor    = $derived(dark ? 'text-danger-400/70 hover:text-danger-400' : 'text-danger-600/60 hover:text-danger-600')
  const hoverRow       = $derived(dark ? 'hover:bg-zinc-900' : 'hover:bg-zinc-50')
  const hashColor      = $derived(dark ? 'text-zinc-600' : 'text-zinc-400')
  const timeColor      = $derived(dark ? 'text-zinc-700' : 'text-zinc-300')
  const dashBorder     = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const tabSeparator   = $derived(dark ? 'bg-zinc-700' : 'bg-zinc-200')
  const sessionTint    = $derived(dark ? 'bg-brand-500/[0.03]' : 'bg-brand-50/40')
  const sessionBorder  = $derived(dark ? 'border-brand-400' : 'border-brand-500')
  const tagBg          = $derived(dark ? 'bg-zinc-800 text-zinc-400' : 'bg-zinc-100 text-zinc-600')
  const dots           = $derived(dark ? dotColorDark : dotColor)

  /** Dot class = git activity color + ambient shadow. No session logic (Option B). */
  function dotClassFor(project) {
    return dots[project.activity_state] + ' shadow-[0_0_4px_rgba(255,255,255,0.15)]'
  }

  /** Navigate to a project's Claude Code session in tmux. */
  function jumpToSession(e, session) {
    e.stopPropagation()
    if (session?.tmux_session && session?.tmux_window && session?.tmux_pane) {
      navigateToSession(session.tmux_session, session.tmux_window, session.tmux_pane)
    }
  }

  const panelBorder    = $derived(dark ? 'border border-zinc-800' : '')
  const treeBg         = $derived(dark ? 'bg-zinc-900' : 'bg-zinc-50')
  const treeHover      = $derived(dark ? 'hover:bg-zinc-800' : 'hover:bg-zinc-100')
  const treeSelected   = $derived(dark ? 'bg-brand-900/40 text-brand-300' : 'bg-brand-100/80 text-brand-700')
  const treeIcon       = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const lineNumColor   = $derived(dark ? 'text-zinc-700' : 'text-zinc-300')

  // Activity state groups for sidebar ordering
  const groups = [
    { key: 'active', label: 'ACTIVE' },
    { key: 'recent', label: 'RECENT' },
    { key: 'stale', label: 'STALE' },
    { key: 'dormant', label: 'DORMANT' },
  ]

  // --- Data state ---
  let projects = $state([])
  let selectedProject = $state(null)
  let sidebarLoading = $state(true)
  let sidebarError = $state(null)
  let detailLoading = $state(false)

  // Tab state
  let activeTab = $state('overview')

  // Overview: commits
  let recentCommits = $state([])
  let commitsLoading = $state(false)
  let showAllCommits = $state(false)

  // Files tab state
  let fileTree = $state([])
  let fileTreeLoading = $state(false)
  let selectedFile = $state(null)
  let fileContent = $state(null)
  let fileContentLoading = $state(false)
  let expandedDirs = $state(new Set())

  // Session state
  let latestSession = $state(null)
  let sessionHistory = $state([])
  let sessionLoading = $state(false)
  let readmeContent = $state(null)
  let heroMode = $state('auto') // 'auto' | 'session' | 'readme'

  // Relationship state
  let relationships = $state([])
  let relationshipsLoading = $state(false)

  // Computed hero display — session if fresh (<7 days), README otherwise
  const showSession = $derived(
    heroMode === 'session' ||
    (heroMode === 'auto' && latestSession && isSessionFresh(latestSession.date))
  )
  const showReadme = $derived(!showSession)
  const hasToggle = $derived(latestSession && readmeContent)

  // Strip the first H1 from README for Overview — our header already shows the title
  const readmeForOverview = $derived.by(() => {
    if (!readmeContent?.content) return ''
    return readmeContent.content.replace(/^#\s+[^\n]*\n?/, '')
  })

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
    }
  }

  function handleWizardComplete() {
    showWizard = false
    loadProjects()
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

      // Files changed — refresh file tree if on Files tab (debounced)
      let fileTreeRefreshTimer = null
      listen('project-files-changed', (event) => {
        const { project_id } = event.payload
        if (selectedProject?.id === project_id && activeTab === 'files') {
          clearTimeout(fileTreeRefreshTimer)
          fileTreeRefreshTimer = setTimeout(() => loadFileTree(project_id), 2000)
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


  // --- Context menu ---
  function openContextMenu(e, project) {
    e.preventDefault()
    ctxConfirmRemove = false
    ctxConfirmStop = false
    if (ctxConfirmTimeout) { clearTimeout(ctxConfirmTimeout); ctxConfirmTimeout = null }
    ctxMenu = { x: e.clientX, y: e.clientY, project }
  }

  function closeContextMenu() {
    ctxMenu = null
    ctxConfirmRemove = false
    ctxConfirmStop = false
    if (ctxConfirmTimeout) { clearTimeout(ctxConfirmTimeout); ctxConfirmTimeout = null }
  }

  function ctxCopyPath() {
    if (ctxMenu?.project?.path) {
      navigator.clipboard.writeText(ctxMenu.project.path).catch(() => {})
    }
  }

  function ctxRemoveProject() {
    if (!ctxConfirmRemove) {
      // First click — show confirmation (menu stays open via keepOpen flag)
      ctxConfirmRemove = true
      ctxConfirmTimeout = setTimeout(() => {
        ctxConfirmRemove = false
        ctxConfirmTimeout = null
      }, 3000)
      return
    }
    // Second click — actually remove
    const project = ctxMenu.project
    closeContextMenu()
    removeProject(project.id).then(() => {
      projects = projects.filter(p => p.id !== project.id)
      if (selectedProject?.id === project.id) {
        selectedProject = projects.length > 0 ? projects[0] : null
        if (selectedProject) selectProject(selectedProject)
      }
    }).catch(e => {
      console.error('Failed to remove project:', e)
    })
  }

  // --- Session context menu actions ---
  const CTX_ICON_TERMINAL = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m6.75 7.5 3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0 0 21 18V6a2.25 2.25 0 0 0-2.25-2.25H5.25A2.25 2.25 0 0 0 3 6v12a2.25 2.25 0 0 0 2.25 2.25Z"/></svg>'
  const CTX_ICON_PLAY = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.347a1.125 1.125 0 0 1 0 1.972l-11.54 6.347a1.125 1.125 0 0 1-1.667-.986V5.653Z"/></svg>'
  const CTX_ICON_PLUS = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/></svg>'
  const CTX_ICON_CLOCK = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"/></svg>'
  const CTX_ICON_RESTART = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182"/></svg>'
  const CTX_ICON_STOP = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M5.25 7.5A2.25 2.25 0 0 1 7.5 5.25h9a2.25 2.25 0 0 1 2.25 2.25v9a2.25 2.25 0 0 1-2.25 2.25h-9a2.25 2.25 0 0 1-2.25-2.25v-9Z"/></svg>'

  function ctxContinueSession() {
    if (!ctxMenu?.project) return
    console.log('[cmd-center] Continue Session:', ctxMenu.project.id, ctxMenu.project.name)
    launchClaudeSession(ctxMenu.project.id, 'continue')
      .then(r => console.log('[cmd-center] launch OK:', r))
      .catch(e => console.error('[cmd-center] launch FAILED:', e))
  }

  function ctxNewSession() {
    if (!ctxMenu?.project) return
    console.log('[cmd-center] New Session:', ctxMenu.project.id, ctxMenu.project.name)
    launchClaudeSession(ctxMenu.project.id, 'fresh')
      .then(r => console.log('[cmd-center] launch OK:', r))
      .catch(e => console.error('[cmd-center] launch FAILED:', e))
  }

  function ctxResumeSession() {
    if (!ctxMenu?.project) return
    console.log('[cmd-center] Resume Session:', ctxMenu.project.id, ctxMenu.project.name)
    launchClaudeSession(ctxMenu.project.id, 'resume')
      .then(r => console.log('[cmd-center] launch OK:', r))
      .catch(e => console.error('[cmd-center] launch FAILED:', e))
  }

  function ctxOpenInTerminal() {
    if (!ctxMenu?.project) return
    const session = getSessionForProject(ctxMenu.project.path)
    console.log('[cmd-center] Open in Terminal:', ctxMenu.project.path, 'session:', session ? { tmux_session: session.tmux_session, tmux_window: session.tmux_window, tmux_pane: session.tmux_pane } : 'null')
    if (session?.tmux_session && session?.tmux_window && session?.tmux_pane) {
      navigateToSession(session.tmux_session, session.tmux_window, session.tmux_pane)
        .then(() => console.log('[cmd-center] navigate OK'))
        .catch(e => console.error('[cmd-center] navigate FAILED:', e))
    } else {
      console.warn('[cmd-center] Open in Terminal: missing tmux fields, cannot navigate')
    }
  }

  function ctxStopSession() {
    if (!ctxMenu?.project) return
    if (!ctxConfirmStop) {
      ctxConfirmStop = true
      ctxConfirmTimeout = setTimeout(() => {
        ctxConfirmStop = false
        ctxConfirmTimeout = null
      }, 3000)
      return
    }
    const session = getSessionForProject(ctxMenu.project.path)
    if (session?.tmux_pane) {
      stopClaudeSession(session.tmux_pane).catch(e => console.error('Failed to stop session:', e))
    }
    closeContextMenu()
  }

  function ctxRestartSession() {
    if (!ctxMenu?.project) return
    const session = getSessionForProject(ctxMenu.project.path)
    const projectId = ctxMenu.project.id
    if (session?.tmux_pane) {
      stopClaudeSession(session.tmux_pane)
        .then(() => launchClaudeSession(projectId, 'continue'))
        .catch(e => console.error('Failed to restart session:', e))
    }
  }

  /** Generate session-specific context menu items based on current session state. */
  function sessionCtxItems() {
    if (!ctxMenu?.project) return []
    const session = getSessionForProject(ctxMenu.project.path)

    if (session?.state === 'active' || session?.state === 'idle') {
      return [
        { label: 'Open in Terminal', action: ctxOpenInTerminal, icon: CTX_ICON_TERMINAL },
        { separator: true },
        { label: 'Continue Session', disabled: true, icon: CTX_ICON_PLAY },
        { label: 'New Session', disabled: true, icon: CTX_ICON_PLUS },
        { label: 'Resume (pick)...', disabled: true, icon: CTX_ICON_CLOCK },
        { separator: true },
        { label: 'Restart Session', action: ctxRestartSession, icon: CTX_ICON_RESTART },
        { label: ctxConfirmStop ? 'Confirm stop?' : 'Stop Session', action: ctxStopSession, danger: true, keepOpen: !ctxConfirmStop, icon: CTX_ICON_STOP },
      ]
    }

    return [
      { label: 'Continue Session', action: ctxContinueSession, icon: CTX_ICON_PLAY },
      { label: 'New Session', action: ctxNewSession, icon: CTX_ICON_PLUS },
      { label: 'Resume (pick)...', action: ctxResumeSession, icon: CTX_ICON_CLOCK },
    ]
  }

  const CTX_ICON_COPY = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 0 1-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 0 1 1.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 0 0-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 0 1-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 0 0-3.375-3.375h-1.5a1.125 1.125 0 0 1-1.125-1.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H9.75"/></svg>'
  const CTX_ICON_TRASH = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0"/></svg>'

  const ctxMenuItems = $derived(ctxMenu ? [
    { label: 'Copy Path', action: ctxCopyPath, icon: CTX_ICON_COPY },
    { separator: true },
    ...sessionCtxItems(),
    { separator: true },
    { label: ctxConfirmRemove ? 'Confirm remove?' : 'Remove from taurhaus', action: ctxRemoveProject, danger: true, keepOpen: !ctxConfirmRemove, icon: CTX_ICON_TRASH },
  ] : [])

  function isSessionFresh(dateStr) {
    if (!dateStr) return false
    const sessionDate = new Date(dateStr)
    const now = new Date()
    const diffDays = (now - sessionDate) / (1000 * 60 * 60 * 24)
    return diffDays < 7
  }

  function formatSessionDate(dateStr) {
    if (!dateStr) return ''
    const d = new Date(dateStr)
    const now = new Date()
    const diffMs = now - d
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))
    if (diffDays === 0) return 'Today'
    if (diffDays === 1) return 'Yesterday'
    if (diffDays < 7) return `${diffDays} days ago`
    return d.toLocaleDateString()
  }

  let _selectGeneration = 0
  async function selectProject(project) {
    const projectId = project.id
    const wantFiles = activeTab === 'files'
    const generation = ++_selectGeneration

    // Fire all IPC calls in parallel — don't touch state yet
    const [detail, commits, sessions, readme, rels, tree] = await Promise.all([
      getProject(projectId).catch(() => null),
      getRecentCommits(projectId, 10).catch(() => []),
      Promise.all([
        getLatestSession(projectId).catch(() => null),
        listSessions(projectId, 10).catch(() => []),
      ]),
      getReadme(projectId).catch(() => null),
      getRelationships(projectId).catch(() => []),
      wantFiles ? getFileTree(projectId).catch(() => []) : Promise.resolve(null),
    ])

    // Stale check — user clicked a different project while we were loading
    if (generation !== _selectGeneration) return

    // Commit everything in one synchronous block → single DOM repaint
    selectedProject = detail ? { ...project, ...detail } : project
    detailLoading = false
    showAllCommits = false
    heroMode = 'auto'
    recentCommits = commits
    commitsLoading = false
    latestSession = sessions[0]
    sessionHistory = sessions[1] || []
    sessionLoading = false
    readmeContent = readme
    relationships = rels
    relationshipsLoading = false
    // Reset file viewer state
    selectedFile = null
    fileContent = null
    fileError = null
    fileType = null
    imageDataUri = null
    expandedDirs = new Set()
    if (tree !== null) {
      fileTree = tree
      fileTreeLoading = false
      // Auto-select README if on files tab
      if (!selectedFile) {
        const readmeNode = findReadmeInTree(fileTree)
        if (readmeNode) openFile(readmeNode.path)
      }
    } else {
      fileTree = []
    }
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

  function getRelatedProjectName(rel) {
    const otherId = rel.source_project_id === selectedProject?.id
      ? rel.target_project_id
      : rel.source_project_id
    const p = projects.find(p => p.id === otherId)
    return p?.name || otherId
  }

  function getRelationshipDirection(rel) {
    return rel.source_project_id === selectedProject?.id ? 'outgoing' : 'incoming'
  }

  const DETECTION_SOURCE_LABELS = {
    cargo_toml: 'via Cargo.toml',
    package_json: 'via package.json',
    claude_md: 'via CLAUDE.md',
    session_mention: 'via session',
    gitmodules: 'via .gitmodules',
    manual: 'manual',
  }

  const RELATIONSHIP_TYPE_LABELS = {
    depends_on: 'depends on',
    references: 'references',
    mentioned_in_session: 'mentioned in',
    includes: 'includes',
    workspace_sibling: 'sibling of',
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

  function switchTab(tab) {
    activeTab = tab
    if (tab === 'files' && selectedProject && fileTree.length === 0) {
      loadFileTree(selectedProject.id)
    }
  }

  async function loadFileTree(projectId) {
    // Only show skeleton on initial load — refreshes update silently
    const isInitialLoad = fileTree.length === 0
    if (isInitialLoad) fileTreeLoading = true
    try {
      fileTree = await getFileTree(projectId)
      // Auto-select README if no file selected
      if (!selectedFile) {
        const readme = findReadmeInTree(fileTree)
        if (readme) {
          await openFile(readme.path)
        }
      }
    } catch (e) {
      fileTree = []
    } finally {
      fileTreeLoading = false
    }
  }

  function findReadmeInTree(nodes) {
    for (const node of nodes) {
      if (!node.is_dir && /^readme/i.test(node.name)) return node
      if (node.is_dir && node.children) {
        const found = findReadmeInTree(node.children)
        if (found) return found
      }
    }
    return null
  }

  function toggleDir(path) {
    const next = new Set(expandedDirs)
    if (next.has(path)) {
      next.delete(path)
    } else {
      next.add(path)
    }
    expandedDirs = next
  }

  let fileError = $state(null)
  let fileType = $state(null)
  let imageDataUri = $state(null)

  async function openFile(relativePath) {
    if (!selectedProject) return
    selectedFile = relativePath
    fileContentLoading = true
    fileContent = null
    fileError = null
    imageDataUri = null
    fileType = classifyFile(relativePath)
    console.log(`[file] open: "${relativePath}" → classified as "${fileType}"`)

    try {
      if (fileType === 'image') {
        // Check asset cache first, then IPC
        const cached = assetCache.get(selectedProject.id, relativePath)
        if (cached) {
          imageDataUri = cached
        } else {
          const dataUri = await readProjectAsset(selectedProject.id, relativePath)
          if (dataUri) {
            assetCache.set(selectedProject.id, relativePath, dataUri)
            imageDataUri = dataUri
          } else {
            fileError = 'error'
          }
        }
      } else if (fileType === 'binary' || fileType === 'pdf') {
        // Known binary — no IPC call
        fileError = fileType
      } else {
        // text or markdown — read as text
        fileContent = await readFile(selectedProject.id, relativePath)
      }
    } catch (e) {
      const msg = String(e?.message || e || '')
      console.error(`[file] error loading "${relativePath}": ${msg}`)
      if (msg.includes('Binary file') || msg.includes('cannot be read as text')) {
        fileError = 'binary'
      } else if (msg.includes('too large')) {
        fileError = 'too-large'
      } else {
        fileError = 'error'
      }
    } finally {
      fileContentLoading = false
    }
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

  function handleMarkdownNavigate(relativePath) {
    if (!selectedProject) return

    // Resolve relative path against the currently viewed file's directory.
    // If viewing a file like "docs/design-brief.md" and clicking "./foo.md",
    // resolve to "docs/foo.md".
    let resolved = relativePath

    // Strip leading ./ for normalization
    resolved = resolved.replace(/^\.\//, '')

    // If we have a current file context, resolve relative to its directory
    const contextFile = selectedFile || (readmeContent?.path)
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

    // Switch to files tab and open the file
    switchTab('files')
    openFile(resolved)
  }

  function handleSearchNavigate(action) {
    if (action.tab === 'files' && action.filePath) {
      switchTab('files')
      openFile(action.filePath)
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
        <div class="w-[22px] h-[22px] rounded-[5px] bg-brand-500 flex items-center justify-center">
          <span class="text-[10px] font-bold text-white leading-none">t</span>
        </div>
        <span class="text-[13px] font-semibold text-white/90 tracking-[-0.01em]">taurhaus</span>
      </div>
    </div>

    <!-- Tab pill + drag space + controls -->
    <div class="flex-1 flex items-end min-w-0" data-tauri-drag-region>

      <!-- Tab pill — shares bg with main panel (Manila Folder pattern) -->
      <div class="flex items-center px-4 h-[36px] {mainBg} rounded-t-lg ml-1.5">
        {#if settingsOpen}
          <span class="px-3 py-1 text-[13px] font-medium {textPrimary}">Settings</span>
        {:else}
          <button
            class="px-3 py-1 text-[13px] transition-colors border-b-2
              {activeTab === 'overview' ? `font-medium ${textPrimary} border-brand-500` : `${textTertiary} hover:text-zinc-500 border-transparent`}"
            onclick={() => switchTab('overview')}
          >Overview</button>
          <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
          <button
            class="px-3 py-1 text-[13px] transition-colors border-b-2
              {activeTab === 'files' ? `font-medium ${textPrimary} border-brand-500` : `${textTertiary} hover:text-zinc-500 border-transparent`}"
            onclick={() => switchTab('files')}
          >Files</button>
        {/if}
      </div>

      <!-- Right scoop: inverse radius where tab pill meets dark frame -->
      <div class="w-2.5 h-2.5 {mainBg} self-end overflow-hidden shrink-0">
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
    <aside class="w-[252px] bg-brand-950 rounded-lg flex flex-col shrink-0 border border-white/[0.06] overflow-hidden">

      <!-- Filter -->
      <div class="px-3 pt-3 pb-1">
        <div class="flex items-center gap-2 px-3 h-[32px] rounded-md bg-white/[0.05] border border-white/[0.07] text-[13px] text-white/25 transition-colors hover:bg-white/[0.07]">
          <svg class="w-[13px] h-[13px]" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607Z"/></svg>
          Filter...
        </div>
      </div>

      <!-- Project list -->
      <div class="flex-1 overflow-y-auto px-1.5 pt-1" onscroll={() => { hoverCard = null; clearTimeout(hoverTimeout) }}>
        {#if sidebarLoading}
          <!-- Loading skeleton -->
          <div class="px-3 pt-3 space-y-1" data-testid="sidebar-skeleton">
            {#each Array(5) as _}
              <div class="flex items-center gap-2 h-[34px] px-3">
                <div class="w-[7px] h-[7px] rounded-full bg-white/[0.06] animate-pulse"></div>
                <div class="h-3 rounded bg-white/[0.06] animate-pulse flex-1"></div>
              </div>
            {/each}
          </div>
        {:else if sidebarError}
          <!-- Error state -->
          <div class="px-4 pt-6 text-center" data-testid="sidebar-error">
            <p class="text-[12px] text-white/40">{sidebarError}</p>
            <button
              class="mt-2 text-[12px] text-brand-400 hover:text-brand-300 transition-colors"
              onclick={loadProjects}
            >Retry</button>
          </div>
        {:else if projects.length === 0}
          <!-- Empty state -->
          <div class="px-4 pt-8 text-center" data-testid="sidebar-empty">
            <svg class="w-10 h-10 text-white/10 mx-auto" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z"/></svg>
            <p class="mt-2 text-[12px] text-white/40">No projects yet</p>
            <button class="mt-2 text-[12px] text-brand-400 hover:text-brand-300 transition-colors">Scan for projects</button>
          </div>
        {:else}
          {#each groups as group}
            {@const items = projects.filter(p => p.activity_state === group.key)}
            {#if items.length > 0}
              <div class="px-3.5 pt-3 pb-1">
                <span class="text-[10px] font-medium uppercase tracking-[0.06em] text-white/20">{group.label}</span>
              </div>
              {#each items as project}
                {@const selected = selectedProject && project.id === selectedProject.id}
                {@const session = getSessionForProject(project.path)}
                {@const badge = sessionBadge(session)}
                <button
                  class="w-full flex items-center gap-2 px-3 h-[34px] rounded-md text-left transition-all duration-75
                    {selected ? 'bg-white/[0.08]' : ctxMenu?.project?.id === project.id ? 'bg-white/[0.08]' : `hover:bg-white/[0.04] ${rowTintClass(session)}`}"
                  onclick={() => selectProject(project)}
                  oncontextmenu={(e) => { hoverCard = null; clearTimeout(hoverTimeout); openContextMenu(e, project) }}
                  onmouseenter={(e) => showHoverCard(project, session, e.currentTarget)}
                  onmouseleave={hideHoverCard}
                >
                  {#if selected}
                    <span class="w-[2px] h-3.5 bg-brand-400 rounded-full shrink-0 -ml-1 mr-0.5"></span>
                  {/if}
                  <span class="w-[7px] h-[7px] rounded-full shrink-0 {dotClassFor(project)}"></span>
                  <span class="text-[13px] truncate flex-1 {selected ? 'font-medium text-white' : 'text-white/60'}">{project.name}</span>
                  {#if badge.interactive}
                    <span
                      class="session-pill w-[33px] h-[16px] shrink-0 inline-flex items-center justify-center text-[9px] font-semibold tracking-[0.08em] transition-opacity duration-150 opacity-100 {badge.badgeClass}"
                      role="button"
                      tabindex="0"
                      aria-label={badge.ariaLabel}
                      onclick={(e) => jumpToSession(e, session)}
                      onkeydown={(e) => { if (e.key === 'Enter') jumpToSession(e, session) }}
                    >{badge.label}</span>
                  {:else if badge.visible}
                    <span
                      class="session-pill w-[33px] h-[16px] shrink-0 inline-flex items-center justify-center text-[9px] font-semibold tracking-[0.08em] transition-opacity duration-150 opacity-100 {badge.badgeClass}"
                      aria-label={badge.ariaLabel}
                    >{badge.label}</span>
                  {/if}
                  <span class="text-[10px] font-mono shrink-0 {selected ? 'text-white/30' : 'text-white/15'}">{project.branch || ''}</span>
                  {#if badge.visible}
                    {#if badge.interactive}
                      <span
                        class="w-3.5 h-3.5 shrink-0 inline-flex items-center justify-center text-white/40 hover:text-white/70 transition-colors"
                        role="button"
                        tabindex="0"
                        aria-label="Open in Terminal session"
                        onclick={(e) => jumpToSession(e, session)}
                        onkeydown={(e) => { if (e.key === 'Enter') jumpToSession(e, session) }}
                      >
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6.75 7.5l3 2.25-3 2.25m4.5 0h3"/></svg>
                      </span>
                    {:else}
                      <span class="w-3.5 h-3.5 shrink-0 inline-flex items-center justify-center text-white/20" aria-hidden="true">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6.75 7.5l3 2.25-3 2.25m4.5 0h3"/></svg>
                      </span>
                    {/if}
                  {:else}
                    <span class="w-3.5 h-3.5 shrink-0 opacity-0 pointer-events-none"></span>
                  {/if}
                  {#if project.is_dirty}
                    <span class="w-[5px] h-[5px] rounded-full bg-warning-400 shrink-0"></span>
                  {/if}
                </button>
              {/each}
            {/if}
          {/each}
        {/if}
      </div>

      <!-- Footer -->
      <div class="h-[44px] flex items-center justify-between px-4 border-t border-white/[0.06]">
        <button class="w-7 h-7 flex items-center justify-center rounded-md text-white/20 hover:text-white/40 hover:bg-white/[0.06] transition-colors" aria-label="Manage projects" onclick={() => showAddProject = true}>
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/></svg>
        </button>
        <button
          class="w-7 h-7 flex items-center justify-center rounded-md transition-colors {settingsOpen ? 'text-white/60 bg-white/[0.08]' : 'text-white/20 hover:text-white/40 hover:bg-white/[0.06]'}"
          aria-label="Settings"
          onclick={() => settingsOpen = !settingsOpen}
          data-testid="settings-toggle"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 0 1 0 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 0 1 0-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z"/><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z"/></svg>
        </button>
      </div>
    </aside>

    <!-- ═══ MAIN PANEL ═══ -->
    <main class="flex-1 {mainBg} {textBody} rounded-b-lg rounded-tr-lg flex flex-col min-w-0 overflow-hidden {panelBorder}">
      {#if settingsOpen}
        <Settings {dark} onClose={() => settingsOpen = false} onSettingsChanged={loadProjects} />
      {:else if !selectedProject}
        <!-- No project selected -->
        <div class="flex-1 flex items-center justify-center">
          <p class="text-[13px] {textTertiary}">Select a project</p>
        </div>
      {:else}
      {#key selectedProject.id}
      <div class="flex-1 flex flex-col min-w-0 overflow-hidden">
      {#if activeTab === 'overview'}
        <!-- ═══ OVERVIEW TAB ═══ -->
        <!-- Project header -->
        <div class="px-7 pt-5 pb-4 shrink-0 content-enter">
          <div class="flex items-baseline gap-3">
            <h1 class="text-[18px] font-semibold {textPrimary} tracking-[-0.02em]">{selectedProject.name}</h1>
            <span class="text-[11px] font-mono {textTertiary}">{selectedProject.branch || ''}</span>
            {#if selectedProject.activity_state}
              <span class="text-[11px] {statusColor} font-medium capitalize">{selectedProject.activity_state}</span>
            {/if}
          </div>
          {#if selectedProject.description}
            <p class="mt-0.5 text-[13px] {textTertiary}">{selectedProject.description}</p>
          {/if}
        </div>

        <!-- Scrollable content -->
        <div class="flex-1 overflow-y-auto content-enter">
          <div class="max-w-[700px] px-7 pb-8">

            <!-- Hero area: Session / README toggle (ADR-006) -->
            <section class="pb-6 border-b {keyline}">
              <div class="flex items-center justify-between mb-3">
                {#if hasToggle}
                  <!-- Segmented control -->
                  <div class="flex items-center gap-0.5 rounded-md p-0.5 {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'}">
                    <button
                      class="px-2.5 py-0.5 text-[11px] rounded transition-colors
                        {showSession ? `font-medium ${dark ? 'bg-zinc-700 text-zinc-200' : 'bg-white text-zinc-700 shadow-sm'}` : `${textTertiary} hover:${textSecondary}`}"
                      onclick={() => heroMode = 'session'}
                    >Session</button>
                    <button
                      class="px-2.5 py-0.5 text-[11px] rounded transition-colors
                        {showReadme ? `font-medium ${dark ? 'bg-zinc-700 text-zinc-200' : 'bg-white text-zinc-700 shadow-sm'}` : `${textTertiary} hover:${textSecondary}`}"
                      onclick={() => heroMode = 'readme'}
                    >README</button>
                  </div>
                {:else}
                  <span class="text-[11px] {textTertiary}">{latestSession ? 'Latest session' : readmeContent ? 'README' : 'Latest session'}</span>
                {/if}
                {#if latestSession}
                  <span class="text-[11px] {textTertiary}">{formatSessionDate(latestSession.date)}</span>
                {/if}
              </div>

              {#if sessionLoading}
                <div class="border-l-[3px] {sessionBorder} pl-5 py-3 -ml-0.5 rounded-r-sm {sessionTint}">
                  <div class="space-y-2 animate-pulse">
                    <div class="h-3 w-3/4 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'}"></div>
                    <div class="h-3 w-1/2 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'}"></div>
                  </div>
                </div>
              {:else if showSession && latestSession}
                <!-- Session card -->
                <div class="border-l-[3px] {sessionBorder} pl-5 py-3 -ml-0.5 rounded-r-sm {sessionTint}">
                  <p class="text-[13px] {textBody}">{latestSession.summary}</p>
                  {#if latestSession.next_steps && latestSession.next_steps.length > 0}
                    <div class="mt-3">
                      <span class="text-[11px] {textTertiary}">Next steps</span>
                      <ul class="mt-1 space-y-0.5">
                        {#each latestSession.next_steps as step}
                          <li class="text-[13px] {textBody} flex items-start gap-2">
                            <span class="text-[10px] {textTertiary} mt-1 shrink-0">▸</span>
                            <span>{step}</span>
                          </li>
                        {/each}
                      </ul>
                    </div>
                  {/if}
                  {#if latestSession.open_questions && latestSession.open_questions.length > 0}
                    <div class="mt-3">
                      <span class="text-[11px] {textTertiary}">Open questions</span>
                      <ul class="mt-1 space-y-0.5">
                        {#each latestSession.open_questions as question}
                          <li class="text-[13px] {textBody} flex items-start gap-2">
                            <span class="text-[10px] text-amber-500 mt-1 shrink-0">?</span>
                            <span>{question}</span>
                          </li>
                        {/each}
                      </ul>
                    </div>
                  {/if}
                </div>
              {:else if showReadme && readmeContent}
                <!-- README display (first H1 stripped — title is in the header above) -->
                <MarkdownRenderer source={readmeForOverview} {dark} projectId={selectedProject?.id} onNavigate={handleMarkdownNavigate} />
              {:else}
                <!-- Empty state -->
                <div class="border-l-[3px] {dashBorder} pl-5 py-3 -ml-0.5 rounded-r-sm">
                  <p class="text-[13px] {textMuted}">No sessions or README found for this project.</p>
                </div>
              {/if}
            </section>

            <!-- Recent Activity (commits) -->
            <section class="py-6 border-b {keyline}">
              <div class="flex items-center justify-between mb-3">
                <span class="text-[11px] {textTertiary}">Recent activity</span>
                {#if recentCommits.length > 0}
                  <span class="text-[11px] {textTertiary}">{recentCommits.length} commit{recentCommits.length !== 1 ? 's' : ''}</span>
                {/if}
              </div>
              {#if commitsLoading}
                <div class="space-y-1" data-testid="commits-loading">
                  {#each Array(3) as _}
                    <div class="flex items-center h-[30px]">
                      <div class="h-2.5 w-12 rounded bg-zinc-200 dark:bg-zinc-800 animate-pulse"></div>
                      <div class="h-2.5 flex-1 rounded bg-zinc-100 dark:bg-zinc-800/50 animate-pulse ml-3"></div>
                    </div>
                  {/each}
                </div>
              {:else if recentCommits.length === 0}
                <p class="text-[13px] {textMuted}">No commits found.</p>
              {:else}
                <div>
                  {#each recentCommits as commit}
                    <div class="flex items-center h-[30px] text-[13px] {hoverRow} -mx-2 px-2 rounded">
                      <span class="font-mono text-[11px] {hashColor} w-[58px] shrink-0">{commit.hash}</span>
                      <span class="{textBody} truncate flex-1">{commit.message}</span>
                      <span class="text-[11px] {timeColor} shrink-0 ml-3">{commit.date}</span>
                    </div>
                  {/each}
                </div>
                {#if !showAllCommits}
                  <button
                    class="mt-1 text-[11px] {textTertiary} hover:underline"
                    onclick={viewAllCommits}
                  >View all &rarr;</button>
                {/if}
              {/if}
            </section>

            <!-- Relationships -->
            <section class="py-6 border-b {keyline}">
              <div class="flex items-center justify-between mb-3">
                <span class="text-[11px] {textTertiary}">Relationships</span>
                {#if relationships.length > 0}
                  <span class="text-[11px] {textTertiary}">{relationships.length} connection{relationships.length !== 1 ? 's' : ''}</span>
                {/if}
              </div>
              {#if relationshipsLoading}
                <div class="space-y-1" data-testid="relationships-loading">
                  {#each Array(2) as _}
                    <div class="flex items-center h-[30px]">
                      <div class="h-2.5 w-4 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
                      <div class="h-2.5 w-24 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse ml-3"></div>
                      <div class="h-2.5 w-16 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse ml-3"></div>
                    </div>
                  {/each}
                </div>
              {:else if relationships.length === 0}
                <p class="text-[13px] {textMuted}">No connections detected yet.</p>
              {:else}
                <div>
                  {#each relationships as rel}
                    {@const direction = getRelationshipDirection(rel)}
                    {@const projectName = getRelatedProjectName(rel)}
                    {@const typeLabel = RELATIONSHIP_TYPE_LABELS[rel.relationship_type] || rel.relationship_type}
                    {@const sourceLabel = DETECTION_SOURCE_LABELS[rel.detection_source] || rel.detection_source}
                    <div class="flex items-center h-[30px] text-[13px] {hoverRow} -mx-2 px-2 rounded group" data-testid="relationship-row">
                      <!-- Direction arrow -->
                      <span class="w-5 text-center shrink-0 {textTertiary}" title={direction === 'outgoing' ? 'outgoing' : 'incoming'}>{direction === 'outgoing' ? '\u2192' : '\u2190'}</span>

                      <!-- Project name (clickable) -->
                      <button
                        class="text-[13px] {linkColor} truncate transition-colors"
                        onclick={() => {
                          const otherId = direction === 'outgoing' ? rel.target_project_id : rel.source_project_id
                          const p = projects.find(pr => pr.id === otherId)
                          if (p) selectProject(p)
                        }}
                      >{projectName}</button>

                      <!-- Type badge -->
                      <span class="ml-2 px-1.5 py-0.5 text-[10px] rounded {tagBg} shrink-0">{typeLabel}</span>

                      <!-- Detection source -->
                      <span class="ml-2 text-[10px] {textTertiary} shrink-0">{sourceLabel}</span>

                      <!-- Dismiss button (only for auto-detected) -->
                      {#if rel.detection_source !== 'manual'}
                        <button
                          class="ml-auto opacity-0 group-hover:opacity-100 w-5 h-5 flex items-center justify-center rounded {textMuted} hover:{textSecondary} transition-all shrink-0"
                          onclick={() => handleDismissRelationship(rel.id)}
                          aria-label="Dismiss relationship"
                          data-testid="dismiss-relationship"
                        >
                          <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12"/>
                          </svg>
                        </button>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            </section>

            <!-- Session History -->
            <section class="py-6 border-b {keyline}">
              <div class="flex items-center justify-between mb-3">
                <span class="text-[11px] {textTertiary}">Session history</span>
                {#if sessionHistory.length > 0}
                  <span class="text-[11px] {textTertiary}">{sessionHistory.length} session{sessionHistory.length !== 1 ? 's' : ''}</span>
                {/if}
              </div>
              {#if sessionLoading}
                <div class="space-y-1" data-testid="sessions-loading">
                  {#each Array(3) as _}
                    <div class="flex items-center h-[30px]">
                      <div class="h-2.5 w-16 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
                      <div class="h-2.5 flex-1 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse ml-3"></div>
                    </div>
                  {/each}
                </div>
              {:else if sessionHistory.length === 0}
                <p class="text-[13px] {textMuted}">No sessions imported yet.</p>
              {:else}
                <div>
                  {#each sessionHistory as session}
                    <div class="flex items-start gap-3 py-1.5 {hoverRow} -mx-2 px-2 rounded">
                      <span class="text-[11px] {textTertiary} shrink-0 w-[72px] pt-0.5">{formatSessionDate(session.date)}</span>
                      <span class="text-[13px] {textBody} flex-1">{session.summary}</span>
                    </div>
                  {/each}
                </div>
              {/if}
            </section>

            <!-- Project Info -->
            <section class="py-6 pb-10">
              <span class="text-[11px] {textTertiary}">Project info</span>
              <div class="mt-2 space-y-1 text-[13px]">
                <div class="flex items-center gap-3">
                  <span class="{textTertiary} w-14">Path</span>
                  <span class="font-mono text-[12px] {textMuted}">{selectedProject.path}</span>
                </div>
                {#if selectedProject.created_at}
                  <div class="flex items-center gap-3">
                    <span class="{textTertiary} w-14">Created</span>
                    <span class="text-[12px] {textMuted}">{new Date(selectedProject.created_at).toLocaleDateString()}</span>
                  </div>
                {/if}
              </div>
              <div class="mt-3 flex gap-3">
                <button class="text-[11px] {textTertiary}">Edit</button>
                <button class="text-[11px] {dangerColor}">Remove</button>
              </div>
            </section>

          </div>
        </div>
      {:else}
        <!-- ═══ FILES TAB ═══ -->
        <div class="flex-1 flex min-h-0">

          <!-- File tree (200px fixed) -->
          <div class="w-[200px] shrink-0 {treeBg} border-r {keyline} flex flex-col overflow-hidden" role="tree">
            <div class="flex-1 overflow-y-auto pt-2">
              {#if fileTreeLoading}
                <div class="px-3 space-y-1" data-testid="filetree-loading">
                  {#each Array(6) as _}
                    <div class="flex items-center h-[32px] gap-2 px-2">
                      <div class="w-3 h-3 rounded bg-zinc-300/30 animate-pulse"></div>
                      <div class="h-2.5 flex-1 rounded bg-zinc-300/20 animate-pulse"></div>
                    </div>
                  {/each}
                </div>
              {:else if fileTree.length === 0}
                <div class="px-4 pt-6 text-center">
                  <p class="text-[12px] {textMuted}">No viewable files</p>
                  <p class="text-[11px] {textTertiary} mt-1">Check ignore patterns in Settings</p>
                </div>
              {:else}
                {#snippet treeNodes(nodes, depth)}
                  {#each nodes as node}
                    {#if node.is_dir}
                      <button
                        class="w-full flex items-center gap-1.5 h-[32px] text-left text-[13px] {textSecondary} {treeHover} rounded transition-colors"
                        style="padding-left: {8 + depth * 16}px"
                        onclick={() => toggleDir(node.path)}
                        role="treeitem"
                        aria-selected={false}
                        aria-expanded={expandedDirs.has(node.path)}
                      >
                        <svg class="w-3 h-3 {treeIcon} shrink-0 transition-transform {expandedDirs.has(node.path) ? 'rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5"/></svg>
                        <svg class="w-3.5 h-3.5 shrink-0 {dark ? 'text-zinc-500' : 'text-zinc-400'}" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z"/></svg>
                        <span class="truncate">{node.name}</span>
                      </button>
                      {#if expandedDirs.has(node.path) && node.children}
                        {@render treeNodes(node.children, depth + 1)}
                      {/if}
                    {:else}
                      {@const isSelected = selectedFile === node.path}
                      <button
                        class="w-full flex items-center gap-1.5 h-[32px] text-left text-[13px] rounded transition-colors
                          {isSelected ? treeSelected : `${dark ? 'text-zinc-400' : 'text-zinc-600'} ${treeHover}`}"
                        style="padding-left: {22 + depth * 16}px"
                        onclick={() => openFile(node.path)}
                        role="treeitem"
                        aria-selected={isSelected}
                      >
                        <svg class="w-3.5 h-3.5 shrink-0 {isSelected ? '' : treeIcon}" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/></svg>
                        <span class="truncate">{node.name}</span>
                      </button>
                    {/if}
                  {/each}
                {/snippet}
                {@render treeNodes(fileTree, 0)}
              {/if}
            </div>
          </div>

          <!-- File content viewer -->
          <div class="flex-1 flex flex-col min-w-0 content-enter">
            {#if !selectedFile}
              <div class="flex-1 flex items-center justify-center">
                <p class="text-[13px] {textMuted}">Select a file from the tree</p>
              </div>
            {:else}
              <!-- File header -->
              <div class="h-[44px] flex items-center px-6 border-b {keyline} shrink-0">
                <span class="text-[14px] font-medium {textPrimary} truncate">{selectedFile}</span>
                {#if fileType === 'image'}
                  <span class="ml-3 text-[11px] {textTertiary}">image</span>
                {:else if fileContent?.language}
                  <span class="ml-3 text-[11px] {textTertiary}">{fileContent.language}</span>
                {/if}
              </div>

              <!-- File content -->
              <div class="flex-1 overflow-auto">
                {#if fileContentLoading}
                  <div class="p-6 space-y-2" data-testid="filecontent-loading">
                    {#each Array(8) as _}
                      <div class="h-3 rounded bg-zinc-200/50 animate-pulse" style="width: {40 + Math.random() * 50}%"></div>
                    {/each}
                  </div>
                {:else if fileError}
                  <div class="flex flex-col items-center justify-center h-full gap-2 {textTertiary}">
                    {#if fileError === 'binary'}
                      <svg class="w-8 h-8 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/></svg>
                      <span class="text-[13px]">Binary file — cannot display as text</span>
                    {:else if fileError === 'pdf'}
                      <svg class="w-8 h-8 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/></svg>
                      <span class="text-[13px]">PDF viewer coming soon</span>
                    {:else if fileError === 'too-large'}
                      <svg class="w-8 h-8 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/></svg>
                      <span class="text-[13px]">File too large to display (&gt;5 MB)</span>
                    {:else}
                      <svg class="w-8 h-8 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z"/></svg>
                      <span class="text-[13px]">Error loading file</span>
                    {/if}
                  </div>
                {:else if imageDataUri}
                  <div class="flex items-center justify-center p-6 h-full">
                    <img src={imageDataUri} alt={selectedFile} class="max-w-full max-h-full object-contain rounded-lg" />
                  </div>
                {:else if fileContent}
                  {#if fileType === 'markdown'}
                    <div class="p-6 overflow-auto">
                      <MarkdownRenderer source={fileContent.content} {dark} projectId={selectedProject?.id} onNavigate={handleMarkdownNavigate} />
                    </div>
                  {:else}
                    <CodeViewer code={fileContent.content} language={fileContent.language || ''} {dark} />
                  {/if}
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {/if}
      </div>
      {/key}
      {/if}
    </main>
  </div>

  <SearchOverlay bind:open={searchOpen} {dark} onNavigate={handleSearchNavigate} />

  {#if showAddProject}
    <AddProjectModal {dark} onClose={() => showAddProject = false} onProjectsChanged={loadProjects} />
  {/if}

  {#if ctxMenu}
    <ContextMenu items={ctxMenuItems} x={ctxMenu.x} y={ctxMenu.y} dark={true} onClose={closeContextMenu} />
  {/if}

  {#if hoverCard}
    <HoverCard project={hoverCard.project} session={hoverCard.session} anchorEl={hoverCard.anchorEl} />
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
