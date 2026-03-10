<script>
  import { getProjectTasks, getTaskDetail } from './ipc.js'
  import { groupTasksByStatus } from './taskHelpers.js'
  import { TOOL_ICONS, TOOL_NAMES } from './toolLogos.js'
  import { themeTokens } from './themeTokens.js'
  import { createAsyncGuard } from './asyncGuard.js'
  import { getProjectContext } from './context/ProjectContext.js'
  import TaskDetailPanel from './TaskDetailPanel.svelte'
  import SessionHistory from './SessionHistory.svelte'

  /** @type {{ projectId?: string|null, projectPath: string, isActive?: boolean, dark: boolean, codeTheme?: string, position: object|null, navTarget: object|null, onClearNavTarget?: () => void }} */
  let { projectId = null, projectPath, isActive = true, dark, codeTheme = 'github-light', position = $bindable(null), navTarget = null, onClearNavTarget } = $props()
  const projectContext = getProjectContext()

  // Sub-tab state: 'active' (Kanban) or 'history' (SessionHistory)
  let activeSubTab = $state('active')

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens
  const cardHover     = $derived(dark ? 'hover:bg-zinc-900' : 'hover:bg-zinc-100/80')
  const cardBorder    = $derived(dark ? 'border-zinc-800/60' : 'border-zinc-200/80')
  const cardSelectedBorder = $derived(dark ? 'border-brand-500/60' : 'border-brand-500/50')
  const cardSelectedBg     = $derived(dark ? 'bg-zinc-900/80' : 'bg-zinc-50')
  const columnBg      = $derived(dark ? 'bg-zinc-900/30' : 'bg-zinc-50/40')
  const subTabActive  = $derived(dark ? 'text-zinc-100 border-brand-500' : 'text-zinc-900 border-brand-500')
  const subTabInactive = $derived(dark ? 'text-zinc-500 border-transparent hover:text-zinc-400' : 'text-zinc-400 border-transparent hover:text-zinc-500')

  // Task data state
  let tasks = $state([])
  let errors = $state([])
  let loading = $state(true)

  // Selection + detail panel state
  let selectedTask = $state(null)
  let taskDetail = $state(null)
  let taskDetailError = $state(null)
  const taskListFetchGuard = createAsyncGuard()

  const SOURCE_LABELS = TOOL_NAMES

  // Column definitions (static keys + labels; dot color resolved via $derived)
  const COLUMNS = [
    { key: 'in_progress', label: 'In Progress' },
    { key: 'pending', label: 'Pending' },
    { key: 'completed', label: 'Completed' },
  ]

  /** Dot color per column key, reactive to dark mode. */
  const dotColors = $derived({
    in_progress: 'bg-success-400',
    pending: 'bg-info-400',
    completed: dark ? 'bg-zinc-600' : 'bg-zinc-400',
  })

  // Group + sort tasks by status.
  // Memoized helper returns stable references for identical task-array inputs.
  const grouped = $derived.by(() => groupTasksByStatus(tasks))
  const activeBoardRevealKey = $derived.by(() => {
    if (activeSubTab !== 'active' || loading) return null
    const projectRef = projectId || projectPath || 'unknown'
    return `${projectRef}:${tasks.length}:${errors.length}`
  })
  const historyRevealKey = $derived.by(() => {
    if (activeSubTab !== 'history') return null
    return `${projectId || projectPath || 'unknown'}:history`
  })

  // Pending restore target — applied once tasks finish loading
  let pendingRestore = $state(null)

  // Sync position outward for Shell's per-project position memory
  $effect(() => {
    position = {
      activeSubTab,
      selectedTaskId: selectedTask?.id ?? null,
      selectedTaskSource: selectedTask?.source ?? null,
      selectedTaskSourceKey: selectedTask?.source_key ?? null,
    }
  })

  // Handle restore target from Shell (separate channel from position sync)
  $effect(() => {
    if (!navTarget) return
    if (navTarget.activeSubTab) activeSubTab = navTarget.activeSubTab
    if (navTarget.selectedTaskId) {
      pendingRestore = {
        id: navTarget.selectedTaskId,
        source: navTarget.selectedTaskSource,
        source_key: navTarget.selectedTaskSourceKey ?? null,
      }
    }
    onClearNavTarget?.()
  })

  // Apply pending restore once tasks are loaded
  $effect(() => {
    if (!pendingRestore || loading || tasks.length === 0) return
    const match = tasks.find(t => isSameTaskIdentity(t, pendingRestore))
    if (match) selectTask(match)
    pendingRestore = null
  })

  // Fetch tasks on mount and when projectPath changes.
  // In Tauri mode, listen for backend-pushed task change events (event-driven).
  // In Vite-only mode, fall back to polling for mock data.
  $effect(() => {
    if (!projectPath) return
    const projectRef = projectId || projectPath
    if (!projectRef) return
    let destroyed = false
    let unlisten = null
    let interval = null

    // Initial fetch only when tab is visible to avoid hidden background churn
    if (isActive) fetchTasks({ background: false })

    // Event-driven updates in Tauri mode
    const isTauriEnv = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
    if (isTauriEnv) {
      import('@tauri-apps/api/event').then(({ listen }) => {
        if (destroyed) return
        listen('project-tasks-changed', (event) => {
          const eventProjectId = event?.payload?.project_id ?? null
          const projectMatches = !projectId || !eventProjectId || eventProjectId === projectId
          if (!destroyed && !document.hidden && isActive && projectMatches) {
            fetchTasks({ background: true })
          }
        }).then(fn => {
          if (destroyed) {
            fn()
            return
          }
          unlisten = fn
        })
      })
    } else {
      // Vite-only mode — poll for mock data
      interval = setInterval(() => {
        if (!document.hidden && isActive) fetchTasks({ background: true })
      }, 5000)
    }

    async function fetchTasks({ background = false } = {}) {
      const fetchSequence = taskListFetchGuard.next()
      const expectedProjectPath = projectPath
      const expectedProjectRef = projectRef
      const showLoading = !background || tasks.length === 0
      if (showLoading) loading = true
      try {
        const result = await getProjectTasks(expectedProjectRef)
        if (
          destroyed
          || !taskListFetchGuard.isCurrent(fetchSequence)
          || projectPath !== expectedProjectPath
          || (projectId || projectPath) !== expectedProjectRef
        ) {
          return
        }
        tasks = result.tasks || []
        errors = result.errors || []
        if (showLoading) loading = false
      } catch (e) {
        if (
          destroyed
          || !taskListFetchGuard.isCurrent(fetchSequence)
          || projectPath !== expectedProjectPath
          || (projectId || projectPath) !== expectedProjectRef
        ) {
          return
        }
        errors = [['fetch', e.message || 'Failed to load tasks']]
        if (showLoading) loading = false
      }
    }

    return () => {
      destroyed = true
      taskListFetchGuard.invalidate()
      if (unlisten) {
        unlisten()
        unlisten = null
      }
      if (interval) clearInterval(interval)
    }
  })

  /** Handle card click: toggle selection and fetch detail. */
  function selectTask(task) {
    if (isSameTaskIdentity(selectedTask, task)) {
      // Clicking same card again — deselect
      selectedTask = null
      taskDetail = null
      taskDetailError = null
    } else {
      selectedTask = task
      taskDetail = null // Show loading state immediately
      taskDetailError = null
      fetchDetail(task)
    }
  }

  /** Fetch enriched detail for a task. */
  async function fetchDetail(task) {
    try {
      const sourceKey = task.source_key || `legacy-${task.source}`
      const projectRef = projectId || projectPath
      const detail = await getTaskDetail(projectRef, task.id, task.source, sourceKey)
      // Only apply if this task is still selected
      if (isSameTaskIdentity(selectedTask, task)) {
        taskDetail = detail
        taskDetailError = null
      }
    } catch (e) {
      console.error(
        `[tasks] failed to load task detail (project=${projectPath}, task=${task?.id}, source=${task?.source}):`,
        e
      )
      if (isSameTaskIdentity(selectedTask, task)) {
        taskDetail = { task, session: null, commits: [], files_changed: [] }
        taskDetailError = 'Task detail failed to load. Showing basic task info.'
      }
    }
  }

  /** Close the detail panel. */
  function closeDetail() {
    selectedTask = null
    taskDetail = null
    taskDetailError = null
  }

  /** Close panel when clicking the board background (not a card). */
  function handleBoardClick(e) {
    if (!selectedTask) return
    // Don't close if click was on a task card or history task (or inside one)
    if (e.target.closest('[data-testid="task-row"]') || e.target.closest('[data-testid="history-task"]')) return
    closeDetail()
  }

  /** Check if a task is currently selected. */
  function isSelected(task) {
    return isSameTaskIdentity(selectedTask, task)
  }

  function isSameTaskIdentity(a, b) {
    if (!a || !b) return false
    if (a.id !== b.id || a.source !== b.source) return false
    const aKey = a.source_key || null
    const bKey = b.source_key || null
    if (aKey && bKey) return aKey === bKey
    return true
  }

  /** Switch sub-tab, clearing any open detail panel. */
  function switchSubTab(tab) {
    if (activeSubTab === tab) return
    activeSubTab = tab
    closeDetail()
  }

  function navigateToCommit(hash) {
    projectContext?.navigateToCommit?.(hash)
  }

  function navigateToFile(path) {
    projectContext?.navigateToFile?.(path)
  }

  function navigateToCommitRange(after, before) {
    projectContext?.navigateToCommitRange?.(after, before)
  }
</script>

<div class="flex-1 flex overflow-hidden">
  <!-- Board area (flex-1 to compress when detail panel opens) -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="flex-1 flex flex-col overflow-hidden min-w-0" onclick={handleBoardClick}>

  <!-- Header bar with sub-tab switcher -->
  <div class="flex items-center justify-between px-5 pt-4 pb-3 shrink-0">
    <div class="flex items-center gap-1" role="tablist" data-testid="sub-tab-list">
      <button
        role="tab"
        aria-selected={activeSubTab === 'active'}
        class="px-2.5 py-1 text-[11px] font-semibold uppercase tracking-[0.06em] border-b-2 transition-colors cursor-pointer
          {activeSubTab === 'active' ? subTabActive : subTabInactive}"
        data-testid="sub-tab-active"
        onclick={() => switchSubTab('active')}
      >Active{#if activeSubTab === 'active' && tasks.length > 0}<span class="ml-1.5 font-normal normal-case tracking-normal {t.textTertiary}">&middot; {tasks.length}</span>{/if}</button>
      <button
        role="tab"
        aria-selected={activeSubTab === 'history'}
        class="px-2.5 py-1 text-[11px] font-semibold uppercase tracking-[0.06em] border-b-2 transition-colors cursor-pointer
          {activeSubTab === 'history' ? subTabActive : subTabInactive}"
        data-testid="sub-tab-history"
        onclick={() => switchSubTab('history')}
      >History</button>
    </div>
  </div>

  {#if activeSubTab === 'active'}
    {#if loading}
      <!-- Loading skeleton — three column placeholders -->
      <div class="flex-1 flex gap-3 px-5 pb-5 overflow-hidden" data-testid="tasks-loading">
        {#each Array(3) as _}
          <div class="flex-1 rounded-lg {columnBg} p-3">
            <div class="h-3 w-20 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse mb-4"></div>
            {#each Array(3) as __}
              <div class="rounded-lg {t.cardBg} border {cardBorder} p-3 mb-2">
                <div class="h-3 w-full rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse mb-2"></div>
                <div class="h-2.5 w-3/4 rounded {dark ? 'bg-zinc-800/30' : 'bg-zinc-100/60'} animate-pulse"></div>
              </div>
            {/each}
          </div>
        {/each}
      </div>
    {:else if tasks.length === 0}
      <!-- Empty state -->
      <div class="flex-1 flex items-center justify-center" data-testid="tasks-empty">
        <div class="text-center max-w-xs">
          <svg class="w-12 h-12 {t.textMuted} mx-auto opacity-30" fill="none" viewBox="0 0 24 24" stroke-width="1" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12h3.75M9 15h3.75M9 18h3.75m3 .75H18a2.25 2.25 0 002.25-2.25V6.108c0-1.135-.845-2.098-1.976-2.192a48.424 48.424 0 00-1.123-.08m-5.801 0c-.065.21-.1.433-.1.664 0 .414.336.75.75.75h4.5a.75.75 0 00.75-.75 2.25 2.25 0 00-.1-.664m-5.8 0A2.251 2.251 0 0113.5 2.25H15c1.012 0 1.867.668 2.15 1.586m-5.8 0c-.376.023-.75.05-1.124.08C9.095 4.01 8.25 4.973 8.25 6.108V8.25m0 0H4.875c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V9.375c0-.621-.504-1.125-1.125-1.125H8.25z" />
          </svg>
          <p class="mt-4 text-[15px] font-medium {t.textMuted}">No tasks tracked</p>
          <p class="mt-2 text-[13px] leading-relaxed {t.textTertiary}">Tasks appear automatically when Claude, Codex, or Gemini create plans or task lists in your project.</p>
        </div>
      </div>
    {:else if activeBoardRevealKey}
      {#key activeBoardRevealKey}
        <div class="flex-1 flex flex-col min-h-0 content-enter">
          <!-- Error indicators (per-source) -->
          {#if taskDetailError}
            <div class="px-5 pb-2 shrink-0">
              <div class="flex items-center gap-2 px-3 py-1.5 rounded text-[11px] {dark ? 'bg-warning-300/10 text-warning-300' : 'bg-warning-50 text-warning-600'}" data-testid="task-detail-error">
                <svg class="w-3 h-3 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.007M4.93 19.5h14.14c1.54 0 2.502-1.667 1.732-3L13.732 4.25c-.77-1.333-2.694-1.333-3.464 0L3.198 16.5c-.77 1.333.192 3 1.732 3Z" />
                </svg>
                <span>{taskDetailError}</span>
              </div>
            </div>
          {/if}

          {#if errors.length > 0}
            <div class="px-5 pb-2 space-y-1 shrink-0">
              {#each errors as [source, message]}
                <div class="flex items-center gap-2 px-3 py-1.5 rounded text-[11px] {dark ? 'bg-warning-300/10 text-warning-300' : 'bg-warning-50 text-warning-600'}">
                  <svg class="w-3 h-3 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
                  </svg>
                  <span>{SOURCE_LABELS[source] || source}: {message}</span>
                </div>
              {/each}
            </div>
          {/if}

          <!-- Kanban columns -->
          <div class="flex-1 flex gap-3 px-5 pb-5 overflow-hidden min-h-0">
            {#each COLUMNS as col}
              {@const colTasks = grouped[col.key] || []}
              <section class="flex-1 flex flex-col min-w-0 rounded-lg {columnBg} min-h-0" data-testid="kanban-column">
                <!-- Column header -->
                <div class="flex items-center gap-2 px-3 pt-3 pb-2 shrink-0">
                  <span class="w-[6px] h-[6px] rounded-full {dotColors[col.key]}"></span>
                  <span class="text-[11px] font-semibold uppercase tracking-[0.06em] {t.textTertiary}">{col.label}</span>
                  <span class="text-[10px] {t.textMuted}">{colTasks.length}</span>
                </div>

                <!-- Scrollable card list -->
                <div class="flex-1 overflow-y-auto px-2 pb-2 min-h-0">
                  {#each colTasks as task}
                    {@render taskCard(task)}
                  {:else}
                    <div class="px-2 py-6 text-center">
                      <span class="text-[11px] {t.textMuted}">No tasks</span>
                    </div>
                  {/each}
                </div>
              </section>
            {/each}
          </div>
        </div>
      {/key}
    {/if}
  {:else}
    <!-- History sub-tab — SessionHistory accordion -->
    {#if historyRevealKey}
      {#key historyRevealKey}
        <div class="flex-1 overflow-hidden content-enter" data-testid="history-tab-content">
          <SessionHistory
            {projectPath}
            {dark}
            {projectId}
            isActive={isActive && activeSubTab === 'history'}
            onSelectTask={selectTask}
            onNavigateToCommit={navigateToCommit}
            onNavigateToFile={navigateToFile}
            onNavigateToCommitRange={navigateToCommitRange}
          />
        </div>
      {/key}
    {/if}
  {/if}
  </div>

  <!-- Detail panel (slides in from right) -->
  {#if selectedTask}
    <TaskDetailPanel
      task={selectedTask}
      detail={taskDetail}
      {dark}
      {codeTheme}
      allTasks={tasks}
      onClose={closeDetail}
      onNavigateTask={selectTask}
    />
  {/if}
</div>

{#snippet taskCard(task)}
  {@const icon = TOOL_ICONS[task.source] || TOOL_ICONS.claude}
  {@const selected = isSelected(task)}
  <button
    class="w-full text-left rounded-lg border px-3 py-2.5 mb-2 transition-all cursor-pointer
      active:scale-[0.98] motion-reduce:active:scale-100
      {selected ? `${cardSelectedBg} ${cardSelectedBorder} border-l-2` : `${t.cardBg} ${cardBorder} ${cardHover}`}"
    data-testid="task-row"
    onclick={() => selectTask(task)}
  >
    <!-- Top line: tool icon + subject -->
    <div class="flex items-start gap-2">
      <span class="w-[14px] h-[14px] shrink-0 mt-0.5 {t.textTertiary}" aria-label={SOURCE_LABELS[task.source] || task.source}>
        <svg class="w-[12px] h-[12px]" viewBox={icon.viewBox} fill="currentColor" aria-hidden="true">
          <path d={icon.path}/>
        </svg>
      </span>
      <span class="text-[13px] leading-snug {task.status === 'completed' ? `${t.textMuted} line-through` : t.textBody}">{task.subject}</span>
    </div>

    {#if task.status === 'in_progress' && task.active_form}
      <p class="text-[11px] {t.textTertiary} mt-1 ml-[22px] truncate" data-testid="task-active-form">{task.active_form}</p>
    {/if}

    <!-- Description -->
    {#if task.description}
      <p class="text-[11px] {t.textTertiary} mt-1.5 ml-[22px] line-clamp-2">{task.description}</p>
    {/if}

    <!-- Metadata: blocked_by + owner -->
    {#if task.blocked_by.length > 0 || task.owner}
      <div class="flex items-center gap-2 mt-1.5 ml-[22px]">
        {#if task.blocked_by.length > 0}
          <span class="text-[10px] {t.textMuted}">blocked by: {task.blocked_by.map(id => `#${id}`).join(', ')}</span>
        {/if}
        {#if task.owner}
          <span class="text-[10px] {t.textMuted}">{task.owner}</span>
        {/if}
      </div>
    {/if}
  </button>
{/snippet}
