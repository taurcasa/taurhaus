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
    launchAccountNotice,
    launchFollowsHistory,
    refreshAccounts,
    refreshAccountRelationships,
    refreshUsage,
    resolveChooserAccounts,
    rememberChoice,
    requestLaunch,
  } from './accounts.svelte.js'
  import { ambientAccountSignal, ambientSignalDescription } from './accountPresentation.js'
  import {
    accountSubmenuApplies,
    buildAccountMenuChildren,
    launchDelegatesToTeam,
    TEAM_ACCOUNT_NOTE,
  } from './accountMenu.js'
  import { toolLabel, tools } from './toolRegistry.js'
  import { bridgeFrame, supportsScrollDrivenTracking } from './sidebarBridge.js'
  import { RAIL_ICONS } from './railIcons.js'
  import SidebarProjectList from './SidebarProjectList.svelte'
  import ContextMenu from './ContextMenu.svelte'
  import HoverCard from './HoverCard.svelte'
  import AccountUsageBoard from './components/AccountUsageBoard.svelte'

  let {
    projects = [],
    sidebarLoading = false,
    sidebarError = null,
    selectedProject: selectedProjectProp = null,
    foregroundProjectId = null,
    onForegroundProjectChange = () => {},
    daemonStatus: daemonStatusProp = null,
    settingsOpen = false,
    accountsOpen = false,
    projectsOpen = false,
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

  // --- Pulled-row bridge ---
  // The elements that continue the pulled row's material across the frame
  // gutter into the main panel. They are driven imperatively (measured
  // rects written straight to style) rather than through template state:
  // the geometry is a paint concern that must update in the same frame as
  // a scroll, and no other component state depends on it.
  let asideEl = $state(null)
  let bridgeClipEl = $state(null)
  let bridgeStripEl = $state(null)
  let laneClipEl = $state(null)
  let laneStripEl = $state(null)
  const bridgeUsesScrollTimeline = supportsScrollDrivenTracking()
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
  let accountsBoard = $state(null)
  let accountsBoardLeaveTimeout = null
  let accountsBoardRefreshTimeout = null
  const ACCOUNTS_BOARD_REFRESH_TTL_MS = 60_000

  const accountSignal = $derived(
    ambientAccountSignal(
      tools().map((tool) => ({ tool: tool.id, ...accountState(tool.id) }))
    )
  )

  // Rail tone ramp: idle .30 → signaled idle .55 → hover .60 over a .05 fill
  // → open = pulled material. Keyboard focus is the filter's ring, rail-wide.
  const railKeyFocus = 'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand-500/70'
  const railKeyIdle = 'text-rail-idle hover:text-rail-hover hover:bg-rail-hit-hover'
  const railKeySignal = 'text-rail-signal hover:text-rail-hover hover:bg-rail-hit-hover'
  const projectsKeyTone = $derived(projectsOpen ? 'rail-key-pulled' : railKeyIdle)
  const settingsKeyTone = $derived(settingsOpen ? 'rail-key-pulled' : railKeyIdle)
  const accountsKeyTone = $derived(
    accountsOpen
      ? 'rail-key-pulled'
      : accountSignal.visible
        ? railKeySignal
        : railKeyIdle
  )
  const accountBadgeTone = $derived(
    accountSignal.tone === 'danger'
      ? 'bg-danger-500 text-white'
      : 'bg-warning-400 text-brand-950'
  )
  // The pill is a bare number for the eye; AT gets the sentence.
  const accountSignalDescription = $derived(ambientSignalDescription(accountSignal))
  const accountsKeyLabel = $derived(
    accountSignalDescription ? `Accounts — ${accountSignalDescription}` : 'Accounts'
  )
  const accountsKeyTitle = $derived(
    accountSignalDescription
      ? `Accounts (Ctrl+Shift+A) — ${accountSignalDescription}`
      : 'Accounts (Ctrl+Shift+A)'
  )

  // While a utility surface occupies the main panel, its footer key wears the
  // pulled material instead — and the selected row demotes to "held".
  const utilityOpen = $derived(settingsOpen || accountsOpen || projectsOpen)

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

  /**
   * The main panel's left edge, measured from the Shell body (the aside and
   * the `<main>` panel are siblings there). The bridge host mirrors that
   * structure; where no panel exists the geometry falls back to the frame
   * gap token.
   */
  function measureBridgePanelLeft() {
    const panel = asideEl?.parentElement?.querySelector(':scope > main')
    return panel ? panel.getBoundingClientRect().left : null
  }

  function positionBridgeStrip(el, strip, scrollRange) {
    el.style.top = `${strip.top}px`
    el.style.height = `${strip.height}px`
    el.style.setProperty('--rail-scroll-range', `${scrollRange}px`)
    if (!bridgeUsesScrollTimeline) {
      el.style.transform = `translateY(${-(projectListEl?.scrollTop ?? 0)}px)`
    }
  }

  function applyBridgeFrame(frame) {
    if (!bridgeClipEl) return
    if (!frame.active) {
      delete bridgeClipEl.dataset.bridgeActive
      if (laneClipEl) delete laneClipEl.dataset.bridgeActive
      return
    }
    const { wrapper, strip, lane, scrollRange } = frame
    bridgeClipEl.dataset.bridgeActive = 'true'
    bridgeClipEl.style.left = `${wrapper.left}px`
    bridgeClipEl.style.top = `${wrapper.top}px`
    bridgeClipEl.style.width = `${wrapper.width}px`
    bridgeClipEl.style.height = `${wrapper.height}px`
    positionBridgeStrip(bridgeStripEl, strip, scrollRange)
    if (!laneClipEl || !laneStripEl) return
    if (lane) {
      laneClipEl.dataset.bridgeActive = 'true'
      laneClipEl.style.left = `${lane.left}px`
      laneClipEl.style.top = `${lane.top}px`
      laneClipEl.style.width = `${lane.width}px`
      laneClipEl.style.height = `${lane.height}px`
      positionBridgeStrip(laneStripEl, lane.strip, scrollRange)
    } else {
      delete laneClipEl.dataset.bridgeActive
    }
  }

  /**
   * One measurement pass: find the pulled row in the DOM (`held` renders no
   * `.sidebar-row-pulled`, so a utility surface hides the bridge by
   * construction) and lay the bridge over the gutter. The row's rect is the
   * source of truth — the virtualizer's offsets assume 36px rows while a
   * branch-line row renders 50px.
   */
  function updateBridge() {
    if (!bridgeClipEl || !bridgeStripEl) return
    applyBridgeFrame(bridgeFrame({
      rowRect: projectListEl?.querySelector('.sidebar-row-pulled')?.getBoundingClientRect() ?? null,
      railRect: asideEl?.getBoundingClientRect() ?? null,
      listRect: projectListEl?.getBoundingClientRect() ?? null,
      panelLeft: measureBridgePanelLeft(),
      scrollTop: projectListEl?.scrollTop ?? 0,
      scrollHeight: projectListEl?.scrollHeight ?? 0,
      clientHeight: projectListEl?.clientHeight ?? 0,
    }))
  }

  // Remeasure after every flush that can move or replace the pulled row:
  // selection, the utility surfaces (held state), list content and grouping,
  // the virtual window, the measured viewport, and scroll (the strip base is
  // scroll-invariant, but a scroll can mount/unmount the row through the
  // virtual window). The aside/list element refs are tracked so the first
  // pass runs on mount.
  $effect(() => {
    void sidebarRows
    void sidebarWindow
    void selectedProject
    void utilityOpen
    void projectListScrollTop
    void projectListViewportHeight
    void asideEl
    void projectListEl
    updateBridge()
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
      clearTimeout(accountsBoardLeaveTimeout)
      clearTimeout(accountsBoardRefreshTimeout)
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
    // Without scroll-driven animations the bridge strip's -scrollTop
    // translation is written here, synchronously with the scroll event, so
    // it lands in the same frame as the row's own movement.
    if (!bridgeUsesScrollTimeline) updateBridge()
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

  /** Open-only path — the empty-state scan action always lands on the surface. */
  function handleOpenManageProjects() {
    actions?.onAddProject?.()
    sessionContext?.openManageProjects?.()
  }

  /** The footer key toggles, like its Accounts and Settings siblings. */
  function handleToggleProjects() {
    actions?.onToggleProjects?.()
    sessionContext?.toggleProjects?.()
  }

  function handleToggleSettings() {
    actions?.onToggleSettings?.()
    sessionContext?.toggleSettings?.()
  }

  function handleToggleAccounts() {
    closeAccountsBoard()
    actions?.onToggleAccounts?.()
    sessionContext?.toggleAccounts?.()
  }

  function handleOpenAccounts() {
    closeAccountsBoard()
    if (actions?.onOpenAccounts) {
      actions.onOpenAccounts()
    } else if (sessionContext?.openAccounts) {
      sessionContext.openAccounts()
    } else if (!accountsOpen) {
      handleToggleAccounts()
    }
  }

  function showAccountsBoard(event) {
    clearTimeout(accountsBoardLeaveTimeout)
    clearTimeout(accountsBoardRefreshTimeout)
    const rect = event.currentTarget.getBoundingClientRect()
    accountsBoard = { x: rect.right + 6, y: rect.bottom }
    accountsBoardRefreshTimeout = setTimeout(() => {
      accountsBoardRefreshTimeout = null
      for (const tool of tools()) {
        void refreshAccounts(tool.id)
        void refreshAccountRelationships(tool.id)
        if (tool.capabilities.usage) {
          void refreshUsage(tool.id, { maxAgeMs: ACCOUNTS_BOARD_REFRESH_TTL_MS })
        }
      }
    }, 300)
  }

  function keepAccountsBoardOpen() {
    clearTimeout(accountsBoardLeaveTimeout)
    accountsBoardLeaveTimeout = null
  }

  function scheduleAccountsBoardClose() {
    clearTimeout(accountsBoardLeaveTimeout)
    accountsBoardLeaveTimeout = setTimeout(closeAccountsBoard, 200)
  }

  function closeAccountsBoard() {
    clearTimeout(accountsBoardLeaveTimeout)
    clearTimeout(accountsBoardRefreshTimeout)
    accountsBoardLeaveTimeout = null
    accountsBoardRefreshTimeout = null
    accountsBoard = null
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
  function noteAccountNotApplied(project, result, tool) {
    const notice = launchAccountNotice(result, { project, tool })
    if (notice) showSidebarNotice(notice)
  }

  function ctxLaunchTool(mode, tool = tools()[0]?.id, accountId = null, { choose = 'auto' } = {}) {
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
      choose,
      launch: (projectId, launchMode, launchTool, launchAccountId) =>
        launchCliSession(projectId, launchMode, launchTool, launchAccountId).then((r) => {
          console.log('[cmd-center] launch OK:', r)
          noteAccountNotApplied(project, r, tool)
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
        stopClaudeSession(pane, launchTool)
          .then(() => launchCliSession(projectId, launchMode, launchTool, launchAccountId))
          .then((result) => {
            // A restart is a launch: an account it could not enforce is
            // reported the same way a fresh launch reports it.
            noteAccountNotApplied(project, result, launchTool)
            return result
          }),
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
   *
   * `onChoose`, where the caller offers it, is the row that reopens the chooser
   * itself — the only way back to a side-by-side usage comparison once the
   * project remembers an account.
   */
  function withAccountSubmenu(item, tool, onPick, mode = 'fresh', sessions = [], onChoose = null) {
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
        onChoose,
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
      () => ctxLaunchTool(mode, tool, null, { choose: 'always' }),
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

<aside
  bind:this={asideEl}
  class="sidebar-rail w-[252px] bg-brand-950 rounded-lg flex flex-col shrink-0 border border-white/[0.06] overflow-hidden"
>

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
    class="sidebar-rail-scroll flex-1 overflow-y-auto px-1.5 pt-1"
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
      {utilityOpen}
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

  <!-- Footer — the key cluster (Projects · Accounts · Settings) bottom-left.
       The daemon readout on the right is deliberately un-key-like: a vital
       sign, not a door. -->
  <div class="h-[44px] flex items-center gap-1 px-3 border-t border-white/[0.06]">
    <button
      class="w-7 h-7 flex items-center justify-center rounded-md transition-colors {projectsKeyTone} {railKeyFocus}"
      aria-label="Projects"
      aria-expanded={projectsOpen}
      title="Projects"
      data-testid="manage-projects-btn"
      onclick={handleToggleProjects}
    >
      <svg class="w-4 h-4" fill="none" viewBox={RAIL_ICONS.projects.viewBox} stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d={RAIL_ICONS.projects.path}/></svg>
    </button>
    <button
      class="relative w-7 h-7 flex items-center justify-center rounded-md transition-colors {accountsKeyTone} {railKeyFocus}"
      aria-label={accountsKeyLabel}
      aria-expanded={accountsOpen}
      title={accountsKeyTitle}
      onclick={handleToggleAccounts}
      onmouseenter={showAccountsBoard}
      onmouseleave={scheduleAccountsBoardClose}
      data-testid="accounts-toggle"
    >
      <svg class="h-4 w-4" fill="none" viewBox={RAIL_ICONS.accounts.viewBox} stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d={RAIL_ICONS.accounts.path}/></svg>
      {#if accountSignal.visible}
        <span
          class="rail-badge absolute -right-1 -top-1 {accountBadgeTone}"
          data-testid="accounts-signal"
          aria-label={accountSignalDescription}
          title={accountSignalDescription}
        >
          {accountSignal.magnitude}
        </span>
      {/if}
    </button>
    <button
      class="w-7 h-7 flex items-center justify-center rounded-md transition-colors {settingsKeyTone} {railKeyFocus}"
      aria-label="Settings"
      aria-expanded={settingsOpen}
      title="Settings"
      onclick={handleToggleSettings}
      data-testid="settings-toggle"
    >
      <svg class="w-4 h-4" fill="none" viewBox={RAIL_ICONS.settings.viewBox} stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d={RAIL_ICONS.settings.path}/></svg>
    </button>
    {#if daemonStatus && daemonStatus !== 'not_configured'}
      <span class="ml-auto flex items-center gap-1.5 text-[11px] font-medium" data-testid="daemon-status">
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
  </div>

  <!-- Pulled-row bridge: the selected row's panel material continuing across
       the frame gutter into the main panel. Fixed-position so it escapes the
       rail's overflow clip (a scroll container cannot paint a child across
       its own x-axis); the driver above sizes the clip to the list viewport
       and slides the strip to the measured row. -->
  <div
    class="sidebar-bridge-clip"
    bind:this={bridgeClipEl}
    aria-hidden="true"
    data-testid="sidebar-bridge"
  >
    <div class="sidebar-bridge-strip" bind:this={bridgeStripEl}>
      <span class="sidebar-bridge-scoop sidebar-bridge-scoop-top"></span>
      <span class="sidebar-bridge-scoop sidebar-bridge-scoop-bottom"></span>
    </div>
  </div>

  <!-- In-rail lane cover: when the list overflows, the classic scrollbar
       takes 8px of layout and the pulled row stops short of the rail edge.
       This strip restores the drawer law ("flush to the rail's right edge");
       the list stacks above it, so the thumb travels over the material like
       an overlay scrollbar. -->
  <div
    class="sidebar-bridge-lane-clip"
    bind:this={laneClipEl}
    aria-hidden="true"
    data-testid="sidebar-bridge-lane"
  >
    <div class="sidebar-bridge-strip" bind:this={laneStripEl}></div>
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

{#if accountsBoard}
  <AccountUsageBoard
    x={accountsBoard.x}
    y={accountsBoard.y}
    dark={true}
    onManage={handleOpenAccounts}
    onClose={closeAccountsBoard}
    onMouseEnter={keepAccountsBoardOpen}
    onMouseLeave={scheduleAccountsBoardClose}
  />
{/if}
