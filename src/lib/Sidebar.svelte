<script>
  import { getContextMenuPoint } from './a11y.js'
  import { navigateToSession, launchCliSession, stopClaudeSession, removeProject } from './ipc.js'
  import { getProjectContext } from './context/ProjectContext.js'
  import { getSessionContext } from './context/SessionContext.js'
  import { normalizeProjectPath } from './pathUtils.js'
  import { getSessionForProject, getSessionsForProject } from './sessionStore.svelte.js'
  import { hasLiveSession, rowTintForSessions, toolIndicators } from './sessionIndicator.js'
  import { buildSidebarProjection } from './sidebar.js'
  import { describeSessionActionError } from './errorCopy.js'
  import {
    activeAccountId,
    accountState,
    launchFollowsHistory,
    refreshAccounts,
    refreshUsage,
    resolveChooserAccounts,
    rememberChoice,
    requestLaunch,
  } from './accounts.svelte.js'
  import {
    accountSubmenuApplies,
    buildAccountMenuChildren,
    launchDelegatesToTeam,
    TEAM_ACCOUNT_NOTE,
  } from './accountMenu.js'
  import { toolLabel, tools } from './toolRegistry.js'
  import SidebarProjectList from './SidebarProjectList.svelte'
  import ContextMenu from './ContextMenu.svelte'
  import HoverCard from './HoverCard.svelte'

  let {
    projects = [],
    sidebarLoading = false,
    sidebarError = null,
    selectedProject: selectedProjectProp = null,
    foregroundProjectId = null,
    onForegroundProjectChange = () => {},
    daemonStatus: daemonStatusProp = null,
    settingsOpen = false,
    dark = false,
    actions = {},
  } = $props()
  const projectContext = getProjectContext()
  const sessionContext = getSessionContext()
  const selectedProject = $derived.by(() => selectedProjectProp ?? projectContext?.selectedProject ?? null)
  const daemonStatus = $derived.by(() => daemonStatusProp ?? sessionContext?.daemonStatus ?? null)

  const SIDEBAR_PROJECT_ROW_HEIGHT = 36
  const SIDEBAR_HEADER_ROW_HEIGHT = 42
  const SIDEBAR_OVERSCAN_PX = 220
  const SIDEBAR_VIRTUALIZE_THRESHOLD = 50

  // Sidebar filter
  let filterQuery = $state('')
  let projectListEl = $state(null)
  let projectListScrollTop = $state(0)
  let projectListViewportHeight = $state(480)
  const sidebarProjection = $derived.by(() => buildSidebarProjection(projects, filterQuery))
  const filteredProjects = $derived(sidebarProjection.filtered)
  const groupedProjects = $derived(sidebarProjection.grouped)

  // --- Context menu state ---
  let ctxMenu = $state(null) // { x, y, project }
  let ctxConfirmRemove = $state(false)
  let ctxConfirmStop = $state(false)
  let ctxConfirmTimeout = $state(null)

  // --- Hover card state ---
  let hoverCard = $state(null) // { project, sessions, anchorEl }
  let hoverCardVisible = $state(false)
  let hoverTimeout = $state(null)
  let sessionJumpInFlight = $state(false)
  let sidebarNotice = $state(null)
  let sidebarNoticeTimeout = $state(null)

  function showHoverCard(project, sessions, el) {
    clearTimeout(hoverTimeout)
    if (hoverCard) {
      hoverCard = { project, sessions, anchorEl: el }
      hoverCardVisible = true
      return
    }
    hoverTimeout = setTimeout(() => {
      if (!ctxMenu) {
        hoverCard = { project, sessions, anchorEl: el }
        hoverCardVisible = true
      }
    }, 100)
  }

  function hideHoverCard() {
    clearTimeout(hoverTimeout)
    hoverCardVisible = false
    hoverTimeout = setTimeout(() => { hoverCard = null }, 70)
  }

  $effect(() => {
    if (!projectListEl) return

    const updateViewport = () => {
      projectListViewportHeight = projectListEl?.clientHeight || 480
      projectListScrollTop = projectListEl?.scrollTop || 0
    }
    updateViewport()

    if (typeof ResizeObserver !== 'undefined') {
      const observer = new ResizeObserver(updateViewport)
      observer.observe(projectListEl)
      return () => observer.disconnect()
    }

    window.addEventListener('resize', updateViewport)
    return () => window.removeEventListener('resize', updateViewport)
  })

  $effect(() => {
    return () => {
      if (hoverTimeout) {
        clearTimeout(hoverTimeout)
        hoverTimeout = null
      }
      if (ctxConfirmTimeout) {
        clearTimeout(ctxConfirmTimeout)
        ctxConfirmTimeout = null
      }
      if (sidebarNoticeTimeout) {
        clearTimeout(sidebarNoticeTimeout)
        sidebarNoticeTimeout = null
      }
    }
  })

  function showSidebarNotice(message) {
    sidebarNotice = message
    if (sidebarNoticeTimeout) {
      clearTimeout(sidebarNoticeTimeout)
    }
    sidebarNoticeTimeout = setTimeout(() => {
      sidebarNotice = null
      sidebarNoticeTimeout = null
    }, 6000)
  }

  function handleProjectListScroll(event) {
    hoverCardVisible = false
    hoverCard = null
    if (hoverTimeout) clearTimeout(hoverTimeout)
    projectListScrollTop = event.currentTarget?.scrollTop || 0
  }

  function resolveSessionProjectId(session, fallbackProject = null) {
    const directProjectId = session?.project_id ?? session?.projectId ?? null
    if (typeof directProjectId === 'string' && directProjectId.trim()) {
      return directProjectId
    }

    const sessionProjectPath = session?.project_path ?? session?.projectPath ?? null
    if (typeof sessionProjectPath === 'string' && sessionProjectPath.trim()) {
      const normalizedSessionPath = normalizeProjectPath(sessionProjectPath)
      const matchingProject = projects.find((project) =>
        normalizeProjectPath(project?.path) === normalizedSessionPath
      )
      if (matchingProject?.id) {
        return matchingProject.id
      }
    }

    return fallbackProject?.id ?? null
  }

  async function navigateToSidebarSession(session, project = null, openTerminal = false) {
    if (
      sessionJumpInFlight
      || !session?.tmux_session
      || !session?.tmux_window
      || !session?.tmux_pane
    ) {
      return
    }

    sessionJumpInFlight = true
    onForegroundProjectChange(resolveSessionProjectId(session, project))

    try {
      if (openTerminal) {
        await navigateToSession(
          session.tmux_session,
          session.tmux_window,
          session.tmux_pane,
          true,
        )
      } else {
        await navigateToSession(
          session.tmux_session,
          session.tmux_window,
          session.tmux_pane,
        )
      }
      return true
    } catch (error) {
      console.error('[sidebar] navigate failed:', error)
      showSidebarNotice(describeSessionActionError('navigate', {}, error))
      return false
    } finally {
      sessionJumpInFlight = false
    }
  }

  /** Navigate to a project's CLI session in tmux. */
  function jumpToSession(e, session, project = null) {
    e.stopPropagation()
    void navigateToSidebarSession(session, project)
  }

  // --- Context menu ---
  function openContextMenu(e, project) {
    e.preventDefault()
    ctxConfirmRemove = false
    ctxConfirmStop = false
    if (ctxConfirmTimeout) { clearTimeout(ctxConfirmTimeout); ctxConfirmTimeout = null }
    ctxMenu = { x: e.clientX, y: e.clientY, project }
  }

  function openContextMenuFromKeyboard(event, project, _projectSessions, target) {
    event.preventDefault()
    event.stopPropagation()
    const point = getContextMenuPoint(target)
    ctxConfirmRemove = false
    ctxConfirmStop = false
    if (ctxConfirmTimeout) { clearTimeout(ctxConfirmTimeout); ctxConfirmTimeout = null }
    ctxMenu = { x: point.x, y: point.y, project }
  }

  function closeContextMenu() {
    ctxMenu = null
    ctxConfirmRemove = false
    ctxConfirmStop = false
    if (ctxConfirmTimeout) { clearTimeout(ctxConfirmTimeout); ctxConfirmTimeout = null }
  }

  function ctxCopyPath() {
    if (ctxMenu?.project?.path) {
      navigator.clipboard.writeText(ctxMenu.project.path).catch((error) => {
        console.warn('[sidebar] failed to copy project path to clipboard:', error)
      })
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
      actions?.onProjectRemoved?.(project.id)
      projectContext?.onProjectRemoved?.(project.id)
    }).catch(e => {
      console.error('Failed to remove project:', e)
    })
  }

  function handleSelectProject(project) {
    actions?.onSelectProject?.(project)
    projectContext?.selectProject?.(project)
  }

  function handleProjectMouseEnter(project, sessions, el) {
    actions?.onProjectHover?.(project)
    showHoverCard(project, sessions, el)
  }

  function handleOpenManageProjects() {
    actions?.onAddProject?.()
    sessionContext?.openManageProjects?.()
  }

  function handleToggleSettings() {
    actions?.onToggleSettings?.()
    sessionContext?.toggleSettings?.()
  }

  function handleRetry() {
    actions?.onRetry?.()
    sessionContext?.retryProjects?.()
  }

  // --- Session context menu actions ---
  const CTX_ICON_TERMINAL = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m6.75 7.5 3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0 0 21 18V6a2.25 2.25 0 0 0-2.25-2.25H5.25A2.25 2.25 0 0 0 3 6v12a2.25 2.25 0 0 0 2.25 2.25Z"/></svg>'
  const CTX_ICON_PLAY = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.347a1.125 1.125 0 0 1 0 1.972l-11.54 6.347a1.125 1.125 0 0 1-1.667-.986V5.653Z"/></svg>'
  const CTX_ICON_PLUS = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/></svg>'
  const CTX_ICON_CLOCK = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"/></svg>'
  const CTX_ICON_RESTART = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182"/></svg>'
  const CTX_ICON_STOP = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M5.25 7.5A2.25 2.25 0 0 1 7.5 5.25h9a2.25 2.25 0 0 1 2.25 2.25v9a2.25 2.25 0 0 1-2.25 2.25h-9a2.25 2.25 0 0 1-2.25-2.25v-9Z"/></svg>'
  const CTX_ICON_USER = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 6a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0ZM4.501 20.118a7.5 7.5 0 0 1 14.998 0A17.933 17.933 0 0 1 12 21.75c-2.676 0-5.216-.584-7.499-1.632Z"/></svg>'
  const CTX_ICON_COPY = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 0 1-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 0 1 1.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 0 0-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 0 1-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 0 0-3.375-3.375h-1.5a1.125 1.125 0 0 1-1.125-1.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H9.75"/></svg>'
  const CTX_ICON_TRASH = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0"/></svg>'

  /**
   * Say when the launch could not run on the account it was given.
   *
   * A team member's project has no live session to mark until the team is
   * running, so the menu cannot always see the delegation coming. The backend
   * always can, and reports it on the launch itself.
   */
  function noteAccountNotApplied(project, result) {
    if (result?.account_applied !== false) return
    showSidebarNotice(`${project?.name ?? 'This project'} continued on the team's default account`)
  }

  function ctxLaunchTool(mode, tool = tools()[0]?.id, accountId = null) {
    if (!ctxMenu?.project) return
    const project = ctxMenu.project
    console.log(`[cmd-center] ${mode} ${tool} session:`, project.id, project.name)
    // A launch without a named account may first ask which subscription to run
    // on; the store opens the chooser and takes over from there.
    requestLaunch({
      project,
      mode,
      tool,
      accountId,
      launch: (projectId, launchMode, launchTool, launchAccountId) =>
        launchCliSession(projectId, launchMode, launchTool, launchAccountId).then((r) => {
          console.log('[cmd-center] launch OK:', r)
          noteAccountNotApplied(project, r)
        }),
      onError: (error) => {
        console.error('[cmd-center] launch FAILED:', error)
        showSidebarNotice(describeSessionActionError('launch', { tool }, error))
      },
    })
  }

  function hasNavigableTmuxTarget(session) {
    return Boolean(session?.tmux_session && session?.tmux_window && session?.tmux_pane)
  }

  function getNavigableSessionForProject(project) {
    if (!project?.path) return null
    const focusedSession = getSessionForProject(project.path)
    return hasNavigableTmuxTarget(focusedSession) ? focusedSession : null
  }

  function ctxOpenInTerminal() {
    if (!ctxMenu?.project) return
    const session = getNavigableSessionForProject(ctxMenu.project)
    console.log('[cmd-center] Open in Terminal:', ctxMenu.project.path, 'session:', session ? { tmux_session: session.tmux_session, tmux_window: session.tmux_window, tmux_pane: session.tmux_pane } : 'null')
    if (!session) {
      console.warn('[cmd-center] Open in Terminal: missing tmux fields, cannot navigate')
      showSidebarNotice('No active terminal is available for this project yet.')
      return
    }

    navigateToSidebarSession(session, ctxMenu.project, true)
      .then((didNavigate) => {
        if (didNavigate) {
          console.log('[cmd-center] navigate OK')
        }
      })
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
    stopClaudeSession(session.tmux_pane, session.cli_tool).catch((error) => {
      console.error('Failed to stop session:', error)
      showSidebarNotice(describeSessionActionError('stop', { tool: session.cli_tool }, error))
    })
    closeContextMenu()
  }

  function ctxRestartTool(session, accountId = null) {
    if (!ctxMenu?.project || !session?.tmux_pane) return
    const project = ctxMenu.project
    const tool = session.cli_tool
    const pane = session.tmux_pane
    // The subscription is settled before anything is torn down: the chooser can
    // open while the pane is still alive, so cancelling costs the user nothing.
    requestLaunch({
      project,
      mode: 'fresh',
      tool,
      accountId,
      launch: (projectId, launchMode, launchTool, launchAccountId) =>
        stopClaudeSession(pane, launchTool).then(() =>
          launchCliSession(projectId, launchMode, launchTool, launchAccountId)
        ),
      onError: (error) => {
        console.error('Failed to restart session:', error)
        showSidebarNotice(describeSessionActionError('restart', { tool }, error))
      },
    })
  }

  /**
   * Detected accounts for the open menu.
   *
   * The submenus are built from the detected list, and detection is a cached
   * IPC round trip — asking when the menu opens means the first right-click of
   * a session fills the submenus in rather than showing none.
   */
  $effect(() => {
    if (!ctxMenu) return
    for (const tool of tools().filter((entry) => entry.capabilities.accountSelection)) {
      void refreshAccounts(tool.id).then(() => refreshUsage(tool.id))
    }
  })

  /**
   * Turn a launch item into an account submenu parent, when its tool has
   * accounts to choose between.
   *
   * The capability comes from the registry, never from the tool's name: the
   * next tool to gain account selection grows the same submenu on the same day
   * its descriptor says so. `onPick` receives the account the user named.
   *
   * The tick says "this is the one you get if you just click the row", so a
   * mode whose account the backend reads off the transcript gets none: the
   * project's pin is not what a resume would use.
   *
   * A launch the team runtime would take over gets rows that say so and cannot
   * be picked: the team resumes in its own config dir, and offering a choice
   * that goes nowhere is worse than offering none. Per-team accounts are a
   * follow-up.
   */
  function withAccountSubmenu(item, tool, onPick, mode = 'fresh', sessions = []) {
    const accounts = resolveChooserAccounts(tool)
    if (!accountSubmenuApplies(tool, accounts)) return item
    const delegated = launchDelegatesToTeam(mode, tool, sessions)
    return {
      ...item,
      children: buildAccountMenuChildren({
        accounts,
        activeAccountId: launchFollowsHistory(mode)
          ? null
          : activeAccountId(ctxMenu?.project, tool),
        onSelect: onPick,
        disabledNote: delegated ? TEAM_ACCOUNT_NOTE : null,
      }),
    }
  }

  /** Pin the project to an account (or clear it) without launching anything. */
  function ctxPinAccount(tool, accountId) {
    const projectId = ctxMenu?.project?.id
    if (!projectId) return
    void rememberChoice(projectId, tool, accountId)
  }

  /**
   * A `<Tool> account` submenu per tool that has accounts to choose between:
   * the same rows, but a pin instead of a launch, plus the way back to
   * inheriting the default.
   */
  function accountPinItems() {
    return tools()
      .filter((descriptor) =>
        accountSubmenuApplies(descriptor.id, resolveChooserAccounts(descriptor.id))
      )
      .map((descriptor) => {
        const tool = descriptor.id
        const detected = resolveChooserAccounts(tool)
        const pinnedId = effectiveProjectAccountId(tool)
        return {
          label: `${descriptor.label} account`,
          icon: CTX_ICON_USER,
          children: [
            ...buildAccountMenuChildren({
              accounts: detected,
              activeAccountId: pinnedId,
              onSelect: (accountId) => ctxPinAccount(tool, accountId),
            }),
            {
              label: 'Use default',
              check: !pinnedId,
              action: () => ctxPinAccount(tool, null),
            },
          ],
        }
      })
  }

  /** What this project has pinned for itself, as opposed to what it inherits. */
  function effectiveProjectAccountId(tool) {
    const project = ctxMenu?.project
    if (!project) return null
    const projectId = project.id
    const state = accountState(tool)
    if (projectId && projectId in state.projectChoices) {
      return state.projectChoices[projectId]
    }
    const memory = project.accountMemory?.[tool] ?? project.account_memory?.[tool]
    return memory?.origin === 'pinned' ? (memory.accountId ?? memory.account_id ?? null) : null
  }

  /** Generate session-specific context menu items based on current session state. */
  function sessionCtxItems() {
    if (!ctxMenu?.project) return []
    const allSessions = getSessionsForProject(ctxMenu.project.path)
    const liveSessions = allSessions.filter(hasLiveSession)
    const navigableSession = getNavigableSessionForProject(ctxMenu.project)

    const items = []

    if (navigableSession) {
      items.push({ label: 'Open in Terminal', action: ctxOpenInTerminal, icon: CTX_ICON_TERMINAL })
      items.push({ separator: true })
    }

    const launch = (label, mode, tool, icon) => withAccountSubmenu(
      { label, action: () => ctxLaunchTool(mode, tool), icon },
      tool,
      (accountId) => ctxLaunchTool(mode, tool, accountId),
      mode,
      allSessions,
    )

    // Continue is listed for Claude and Grok, which reopen the project's last
    // conversation. Antigravity's continue command also differs from its fresh
    // one; it is deliberately absent here until that lane is exercised.
    items.push(launch('Continue Claude', 'continue', 'claude', CTX_ICON_PLAY))
    items.push(launch('Continue Grok', 'continue', 'grok', CTX_ICON_PLAY))

    // New session remains distinct for all tools.
    items.push({ separator: true })
    items.push(launch('New Claude Session', 'fresh', 'claude', CTX_ICON_PLUS))
    items.push(launch('New Codex Session', 'fresh', 'codex', CTX_ICON_PLUS))
    items.push(launch('New Antigravity Session', 'fresh', 'agy', CTX_ICON_PLUS))
    items.push(launch('New Grok Session', 'fresh', 'grok', CTX_ICON_PLUS))

    // Resume stays distinct for every harness that can name a session.
    items.push({ separator: true })
    items.push(launch('Resume Claude', 'resume', 'claude', CTX_ICON_CLOCK))
    items.push(launch('Resume Codex', 'resume', 'codex', CTX_ICON_CLOCK))
    items.push(launch('Resume Antigravity', 'resume', 'agy', CTX_ICON_CLOCK))
    items.push(launch('Resume Grok', 'resume', 'grok', CTX_ICON_CLOCK))

    // Per-tool stop/restart for each running session
    if (liveSessions.length > 0) {
      items.push({ separator: true })
      for (const s of liveSessions) {
        const name = toolLabel(s.cli_tool, 'Session')
        items.push(withAccountSubmenu(
          { label: `Restart ${name}`, action: () => ctxRestartTool(s), icon: CTX_ICON_RESTART },
          s.cli_tool,
          (accountId) => ctxRestartTool(s, accountId),
        ))
        items.push({
          label: ctxConfirmStop ? `Confirm stop ${name}?` : `Stop ${name}`,
          action: () => ctxStopTool(s),
          danger: true,
          keepOpen: !ctxConfirmStop,
          icon: CTX_ICON_STOP,
        })
      }
    }

    // The pin sits after the launch group: it is about this project's default,
    // not about starting anything now.
    const pinItems = accountPinItems()
    if (pinItems.length) {
      items.push({ separator: true })
      items.push(...pinItems)
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

  function sidebarRowHeight(row) {
    return row.type === 'header'
      ? SIDEBAR_HEADER_ROW_HEIGHT
      : SIDEBAR_PROJECT_ROW_HEIGHT
  }

  const sidebarRows = $derived.by(() => {
    const rows = []
    for (const group of groupedProjects) {
      if (!group.items.length) continue
      rows.push({ type: 'header', key: `header-${group.key}`, group })
      for (const project of group.items) {
        rows.push({ type: 'project', key: `project-${project.id}`, project })
      }
    }
    return rows
  })

  const sidebarProjectCount = $derived.by(
    () => sidebarRows.filter((row) => row.type === 'project').length
  )
  const useVirtualizedSidebar = $derived.by(
    () => sidebarProjectCount > SIDEBAR_VIRTUALIZE_THRESHOLD
  )

  const sidebarLayout = $derived.by(() => {
    const offsets = []
    let totalHeight = 0
    for (const row of sidebarRows) {
      offsets.push(totalHeight)
      totalHeight += sidebarRowHeight(row)
    }
    return { offsets, totalHeight }
  })

  const sidebarWindow = $derived.by(() => {
    const rows = sidebarRows
    const { offsets, totalHeight } = sidebarLayout
    if (!useVirtualizedSidebar) {
      return { start: 0, end: rows.length, paddingTop: 0, paddingBottom: 0 }
    }

    const minOffset = Math.max(0, projectListScrollTop - SIDEBAR_OVERSCAN_PX)
    const maxOffset = projectListScrollTop + projectListViewportHeight + SIDEBAR_OVERSCAN_PX

    let start = 0
    while (
      start < rows.length
      && offsets[start] + sidebarRowHeight(rows[start]) <= minOffset
    ) {
      start += 1
    }

    let end = start
    while (end < rows.length && offsets[end] < maxOffset) {
      end += 1
    }

    const startOffset = offsets[start] ?? totalHeight
    const endOffset = offsets[end] ?? totalHeight

    return {
      start,
      end,
      paddingTop: startOffset,
      paddingBottom: Math.max(0, totalHeight - endOffset),
    }
  })
</script>

<aside class="w-[252px] bg-brand-950 rounded-lg flex flex-col shrink-0 border border-white/[0.06] overflow-hidden">

  <!-- Filter -->
  <div class="px-3 pt-3 pb-1">
    <div class="flex items-center gap-2 px-3 h-[32px] rounded-md bg-white/[0.05] border border-white/[0.07] text-[13px] text-white/25 transition-colors focus-within:border-brand-500/40 focus-within:bg-white/[0.07] focus-within:ring-1 focus-within:ring-brand-500/70">
      <svg class="w-[13px] h-[13px] shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607Z"/></svg>
      <input
        type="text"
        bind:value={filterQuery}
        placeholder="Filter..."
        class="flex-1 rounded-sm bg-transparent text-[13px] text-white/75 outline-none placeholder:text-white/25 focus-visible:ring-1 focus-visible:ring-brand-500/70"
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

  {#if sidebarNotice}
    <div class="mx-3 mb-1 rounded-md border border-warning-400/30 bg-warning-500/10 px-3 py-2 text-[11px] text-warning-100" role="status" aria-live="polite" data-testid="sidebar-notice">
      <div class="flex items-start gap-2">
        <svg class="mt-0.5 h-3.5 w-3.5 shrink-0 text-warning-300" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.007M4.93 19.5h14.14c1.54 0 2.502-1.667 1.732-3L13.732 4.25c-.77-1.333-2.694-1.333-3.464 0L3.198 16.5c-.77 1.333.192 3 1.732 3Z"/></svg>
        <span class="flex-1" data-testid="sidebar-notice-message">{sidebarNotice}</span>
        <button
          class="text-warning-200/70 transition-colors hover:text-warning-100"
          onclick={() => { sidebarNotice = null }}
          aria-label="Dismiss sidebar notice"
          data-testid="sidebar-notice-dismiss"
        >
          <svg class="h-3 w-3" fill="none" viewBox="0 0 24 24" stroke-width="2.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12"/></svg>
        </button>
      </div>
    </div>
  {/if}

  <!-- Project list -->
  <div
    bind:this={projectListEl}
    class="flex-1 overflow-y-auto px-1.5 pt-1"
    onscroll={handleProjectListScroll}
    data-testid="sidebar-project-scroll"
  >
    <SidebarProjectList
      {sidebarLoading}
      {sidebarError}
      {projects}
      {filteredProjects}
      {filterQuery}
      {useVirtualizedSidebar}
      {sidebarRows}
      {sidebarWindow}
      {selectedProject}
      {foregroundProjectId}
      {dark}
      ctxMenuProjectId={ctxMenu?.project?.id ?? null}
      {getSessionsForProject}
      {toolIndicators}
      {rowTintForSessions}
      onProjectClick={handleSelectProject}
      onProjectContextMenu={(event, project) => {
        hoverCardVisible = false
        hoverCard = null
        clearTimeout(hoverTimeout)
        openContextMenu(event, project)
      }}
      onProjectContextMenuKey={openContextMenuFromKeyboard}
      onProjectMouseEnter={handleProjectMouseEnter}
      onProjectMouseLeave={hideHoverCard}
      onSessionJump={jumpToSession}
      onRetry={handleRetry}
      onOpenManageProjects={handleOpenManageProjects}
    />
  </div>

  <!-- Footer -->
  <div class="h-[44px] flex items-center justify-between px-4 border-t border-white/[0.06]">
    <button class="w-7 h-7 flex items-center justify-center rounded-md text-white/20 hover:text-white/40 hover:bg-white/[0.06] transition-colors" aria-label="Manage projects" data-testid="manage-projects-btn" onclick={handleOpenManageProjects}>
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/></svg>
    </button>
    {#if daemonStatus && daemonStatus !== 'not_configured'}
      <span class="flex items-center gap-1.5 text-[11px] font-medium" data-testid="daemon-status">
        {#if daemonStatus === 'connected'}
          <span class="w-1.5 h-1.5 rounded-full bg-success-400"></span>
          <span class="text-success-400/80">Connected</span>
        {:else if daemonStatus === 'busy'}
          <span class="w-1.5 h-1.5 rounded-full bg-brand-400 animate-pulse"></span>
          <span class="text-brand-400/80">Daemon busy</span>
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
        onclick={handleToggleSettings}
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
  <HoverCard
    project={hoverCard.project}
    sessions={hoverCard.sessions}
    anchorEl={hoverCard.anchorEl}
    {dark}
    visible={hoverCardVisible}
  />
{/if}
