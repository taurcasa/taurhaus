<script>
  import { navigateToSession, launchClaudeSession, stopClaudeSession, removeProject } from './ipc.js'
  import { getSessionForProject, getSessionsForProject } from './sessionStore.svelte.js'
  import { rowTintForSessions, toolIndicators } from './sessionIndicator.js'
  import ContextMenu from './ContextMenu.svelte'
  import HoverCard from './HoverCard.svelte'

  let {
    projects = [],
    selectedProject = null,
    sidebarLoading = false,
    sidebarError = null,
    daemonStatus = null,
    settingsOpen = false,
    onSelectProject = () => {},
    onAddProject = () => {},
    onToggleSettings = () => {},
    onRetry = () => {},
    onProjectRemoved = () => {},
  } = $props()

  // Activity state groups for sidebar ordering
  const groups = [
    { key: 'active', label: 'ACTIVE' },
    { key: 'recent', label: 'RECENT' },
    { key: 'stale', label: 'STALE' },
    { key: 'dormant', label: 'DORMANT' },
  ]

  // Sidebar filter
  let filterQuery = $state('')
  const filteredProjects = $derived(
    filterQuery.trim()
      ? projects.filter(p => p.name.toLowerCase().includes(filterQuery.trim().toLowerCase()))
      : projects
  )

  // --- Context menu state ---
  let ctxMenu = $state(null) // { x, y, project }
  let ctxConfirmRemove = $state(false)
  let ctxConfirmStop = $state(false)
  let ctxConfirmTimeout = $state(null)

  // --- Hover card state ---
  let hoverCard = $state(null) // { project, sessions, anchorEl }
  let hoverTimeout = $state(null)

  function showHoverCard(project, sessions, el) {
    clearTimeout(hoverTimeout)
    hoverTimeout = setTimeout(() => {
      if (!ctxMenu) hoverCard = { project, sessions, anchorEl: el }
    }, 80)
  }

  function hideHoverCard() {
    clearTimeout(hoverTimeout)
    hoverTimeout = setTimeout(() => { hoverCard = null }, 80)
  }

  /** Navigate to a project's CLI session in tmux. */
  function jumpToSession(e, session) {
    e.stopPropagation()
    if (session?.tmux_session && session?.tmux_window && session?.tmux_pane) {
      navigateToSession(session.tmux_session, session.tmux_window, session.tmux_pane)
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
      onProjectRemoved(project.id)
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
  const CTX_ICON_COPY = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 0 1-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 0 1 1.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 0 0-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 0 1-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 0 0-3.375-3.375h-1.5a1.125 1.125 0 0 1-1.125-1.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H9.75"/></svg>'
  const CTX_ICON_TRASH = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0"/></svg>'

  function ctxLaunchTool(mode, tool = 'claude') {
    if (!ctxMenu?.project) return
    console.log(`[cmd-center] ${mode} ${tool} session:`, ctxMenu.project.id, ctxMenu.project.name)
    launchClaudeSession(ctxMenu.project.id, mode, tool)
      .then(r => console.log('[cmd-center] launch OK:', r))
      .catch(e => console.error('[cmd-center] launch FAILED:', e))
  }

  function ctxOpenInTerminal() {
    if (!ctxMenu?.project) return
    const session = getSessionForProject(ctxMenu.project.path)
    console.log('[cmd-center] Open in Terminal:', ctxMenu.project.path, 'session:', session ? { tmux_session: session.tmux_session, tmux_window: session.tmux_window, tmux_pane: session.tmux_pane } : 'null')
    if (session?.tmux_session && session?.tmux_window && session?.tmux_pane) {
      navigateToSession(session.tmux_session, session.tmux_window, session.tmux_pane, true)
        .then(() => console.log('[cmd-center] navigate OK'))
        .catch(e => console.error('[cmd-center] navigate FAILED:', e))
    } else {
      console.warn('[cmd-center] Open in Terminal: missing tmux fields, cannot navigate')
    }
  }

  function ctxStopTool(session) {
    if (!session?.tmux_pane) return
    if (!ctxConfirmStop) {
      ctxConfirmStop = true
      ctxConfirmTimeout = setTimeout(() => {
        ctxConfirmStop = false
        ctxConfirmTimeout = null
      }, 3000)
      return
    }
    stopClaudeSession(session.tmux_pane, session.cli_tool).catch(e => console.error('Failed to stop session:', e))
    closeContextMenu()
  }

  function ctxRestartTool(session) {
    if (!ctxMenu?.project || !session?.tmux_pane) return
    const projectId = ctxMenu.project.id
    const tool = session.cli_tool
    stopClaudeSession(session.tmux_pane, tool)
      .then(() => launchClaudeSession(projectId, 'continue', tool))
      .catch(e => console.error('Failed to restart session:', e))
  }

  /** Tool display names for context menu labels. */
  const TOOL_DISPLAY = { claude: 'Claude', codex: 'Codex', gemini: 'Gemini' }

  /** Generate session-specific context menu items based on current session state. */
  function sessionCtxItems() {
    if (!ctxMenu?.project) return []
    const allSessions = getSessionsForProject(ctxMenu.project.path)
    const liveSessions = allSessions.filter(s => s.state === 'active' || s.state === 'idle')

    const items = []

    if (liveSessions.length > 0) {
      items.push({ label: 'Open in Terminal', action: ctxOpenInTerminal, icon: CTX_ICON_TERMINAL })
      items.push({ separator: true })
    }

    // Continue — consistent across all tools
    items.push({ label: 'Continue Claude', action: () => ctxLaunchTool('continue', 'claude'), icon: CTX_ICON_PLAY })
    items.push({ label: 'Continue Codex', action: () => ctxLaunchTool('continue', 'codex'), icon: CTX_ICON_PLAY })
    items.push({ label: 'Continue Gemini', action: () => ctxLaunchTool('continue', 'gemini'), icon: CTX_ICON_PLAY })

    // New session — consistent across all tools
    items.push({ separator: true })
    items.push({ label: 'New Claude Session', action: () => ctxLaunchTool('fresh', 'claude'), icon: CTX_ICON_PLUS })
    items.push({ label: 'New Codex Session', action: () => ctxLaunchTool('fresh', 'codex'), icon: CTX_ICON_PLUS })
    items.push({ label: 'New Gemini Session', action: () => ctxLaunchTool('fresh', 'gemini'), icon: CTX_ICON_PLUS })

    // Resume — consistent across all tools
    items.push({ separator: true })
    items.push({ label: 'Resume Claude', action: () => ctxLaunchTool('resume', 'claude'), icon: CTX_ICON_CLOCK })
    items.push({ label: 'Resume Codex', action: () => ctxLaunchTool('resume', 'codex'), icon: CTX_ICON_CLOCK })
    items.push({ label: 'Resume Gemini', action: () => ctxLaunchTool('resume', 'gemini'), icon: CTX_ICON_CLOCK })

    // Per-tool stop/restart for each running session
    if (liveSessions.length > 0) {
      items.push({ separator: true })
      for (const s of liveSessions) {
        const name = TOOL_DISPLAY[s.cli_tool] || 'Session'
        items.push({ label: `Restart ${name}`, action: () => ctxRestartTool(s), icon: CTX_ICON_RESTART })
        items.push({
          label: ctxConfirmStop ? `Confirm stop ${name}?` : `Stop ${name}`,
          action: () => ctxStopTool(s),
          danger: true,
          keepOpen: !ctxConfirmStop,
          icon: CTX_ICON_STOP,
        })
      }
    }

    return items
  }

  const ctxMenuItems = $derived(ctxMenu ? [
    { label: 'Copy Path', action: ctxCopyPath, icon: CTX_ICON_COPY },
    { separator: true },
    ...sessionCtxItems(),
    { separator: true },
    { label: ctxConfirmRemove ? 'Confirm remove?' : 'Remove from taurhaus', action: ctxRemoveProject, danger: true, keepOpen: !ctxConfirmRemove, icon: CTX_ICON_TRASH },
  ] : [])
</script>

<aside class="w-[252px] bg-brand-950 rounded-lg flex flex-col shrink-0 border border-white/[0.06] overflow-hidden">

  <!-- Filter -->
  <div class="px-3 pt-3 pb-1">
    <div class="flex items-center gap-2 px-3 h-[32px] rounded-md bg-white/[0.05] border border-white/[0.07] text-[13px] text-white/25 transition-colors focus-within:border-brand-500/40 focus-within:bg-white/[0.07]">
      <svg class="w-[13px] h-[13px] shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607Z"/></svg>
      <input
        type="text"
        bind:value={filterQuery}
        placeholder="Filter..."
        class="flex-1 bg-transparent text-[13px] text-white/75 outline-none placeholder:text-white/25"
        spellcheck="false"
        autocomplete="off"
        data-testid="sidebar-filter"
      />
      {#if filterQuery}
        <button
          class="text-white/30 hover:text-white/60 transition-colors"
          onclick={() => { filterQuery = '' }}
          aria-label="Clear filter"
          data-testid="sidebar-filter-clear"
        >
          <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke-width="2.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      {/if}
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
          onclick={onRetry}
        >Retry</button>
      </div>
    {:else if projects.length === 0}
      <!-- Empty state -->
      <div class="px-4 pt-8 text-center" data-testid="sidebar-empty">
        <svg class="w-10 h-10 text-white/10 mx-auto" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z"/></svg>
        <p class="mt-2 text-[12px] text-white/40">No projects yet</p>
        <button class="mt-2 text-[12px] text-brand-400 hover:text-brand-300 transition-colors">Scan for projects</button>
      </div>
    {:else if filteredProjects.length === 0 && filterQuery}
      <!-- No filter matches -->
      <div class="px-4 pt-6 text-center" data-testid="sidebar-no-matches">
        <p class="text-[12px] text-white/30">No matching projects</p>
      </div>
    {:else}
      {#each groups as group}
        {@const items = filteredProjects.filter(p => p.activity_state === group.key)}
        {#if items.length > 0}
          <div class="px-3.5 pt-8 pb-1.5">
            <span class="text-[10px] font-semibold uppercase tracking-[0.06em] text-white/35">{group.label}</span>
          </div>
          {#each items as project}
            {@const selected = selectedProject && project.id === selectedProject.id}
            {@const projectSessions = getSessionsForProject(project.path)}
            {@const session = projectSessions[0] ?? null}
            {@const indicators = toolIndicators(projectSessions)}
            <button
              data-testid="project-item"
              class="w-full flex items-center gap-2 px-3 h-[36px] rounded-md text-left transition-all duration-75 cursor-pointer
                {selected ? 'bg-white/[0.08]' : ctxMenu?.project?.id === project.id ? 'bg-white/[0.08]' : `hover:bg-white/[0.04] ${rowTintForSessions(projectSessions)}`}"
              onclick={() => onSelectProject(project)}
              oncontextmenu={(e) => { hoverCard = null; clearTimeout(hoverTimeout); openContextMenu(e, project) }}
              onmouseenter={(e) => showHoverCard(project, projectSessions, e.currentTarget)}
              onmouseleave={hideHoverCard}
            >
              {#if selected}
                <span class="w-[3px] h-3.5 bg-brand-400 rounded-full shrink-0 -ml-1 mr-0.5"></span>
              {/if}
              <span class="text-[14px] truncate flex-1 {selected ? 'font-medium text-white' : 'text-white/75'}">{project.name}</span>
              {#if indicators.length > 0}
                <span class="flex items-center gap-1 shrink-0">
                  {#each indicators as ind}
                    {#if ind.interactive}
                      <span
                        class="w-[14px] h-[14px] shrink-0 inline-flex items-center justify-center cursor-pointer {ind.colorClass} {ind.isActive ? 'session-pill-active' : 'session-pill-idle'}"
                        role="button"
                        tabindex="0"
                        aria-label={ind.ariaLabel}
                        onclick={(e) => jumpToSession(e, ind.session)}
                        onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); jumpToSession(e, ind.session) } }}
                      >
                        <svg class="w-[12px] h-[12px]" viewBox={ind.icon.viewBox} fill="currentColor" aria-hidden="true">
                          <path d={ind.icon.path}/>
                        </svg>
                      </span>
                    {:else}
                      <span
                        class="w-[14px] h-[14px] shrink-0 inline-flex items-center justify-center {ind.colorClass} {ind.isActive ? 'session-pill-active' : 'session-pill-idle'}"
                        aria-label={ind.ariaLabel}
                      >
                        <svg class="w-[12px] h-[12px]" viewBox={ind.icon.viewBox} fill="currentColor" aria-hidden="true">
                          <path d={ind.icon.path}/>
                        </svg>
                      </span>
                    {/if}
                  {/each}
                </span>
              {/if}
              {#if project.branch}
                <span class="text-[10px] font-mono shrink-0 px-1.5 py-0.5 rounded {selected ? 'text-white/50 bg-white/10' : 'text-white/30 bg-white/[0.07]'}">{project.branch}</span>
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
    <button class="w-7 h-7 flex items-center justify-center rounded-md text-white/20 hover:text-white/40 hover:bg-white/[0.06] transition-colors" aria-label="Manage projects" data-testid="manage-projects-btn" onclick={onAddProject}>
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/></svg>
    </button>
    {#if daemonStatus && daemonStatus !== 'not_configured'}
      <span class="flex items-center gap-1.5 text-[11px] font-medium" data-testid="daemon-status">
        {#if daemonStatus === 'connected'}
          <span class="w-1.5 h-1.5 rounded-full bg-success-400"></span>
          <span class="text-success-400/80">Connected</span>
        {:else if daemonStatus === 'reconnecting'}
          <span class="w-1.5 h-1.5 rounded-full bg-warning-400 animate-pulse"></span>
          <span class="text-warning-400/80">Reconnecting</span>
        {:else if daemonStatus === 'disconnected'}
          <span class="w-1.5 h-1.5 rounded-full bg-warning-400"></span>
          <span class="text-warning-400/80">Daemon offline</span>
        {:else if daemonStatus === 'failed'}
          <span class="w-1.5 h-1.5 rounded-full bg-danger-400"></span>
          <span class="text-danger-400/80">Daemon failed</span>
        {/if}
      </span>
    {/if}
    <button
      class="w-7 h-7 flex items-center justify-center rounded-md transition-colors {settingsOpen ? 'text-white/60 bg-white/[0.08]' : 'text-white/20 hover:text-white/40 hover:bg-white/[0.06]'}"
      aria-label="Settings"
      onclick={onToggleSettings}
      data-testid="settings-toggle"
    >
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 0 1 0 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 0 1 0-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z"/><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z"/></svg>
    </button>
  </div>
</aside>

{#if ctxMenu}
  <ContextMenu items={ctxMenuItems} x={ctxMenu.x} y={ctxMenu.y} dark={true} onClose={closeContextMenu} />
{/if}

{#if hoverCard}
  <HoverCard project={hoverCard.project} sessions={hoverCard.sessions} anchorEl={hoverCard.anchorEl} />
{/if}
