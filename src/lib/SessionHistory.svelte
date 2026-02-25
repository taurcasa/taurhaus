<script>
  import { getArchivedSessions, getCommitsInRange } from './ipc.js'
  import { formatDuration } from './format.js'
  import { TOOL_ICONS, TOOL_NAMES } from './toolLogos.js'
  import { themeTokens } from './themeTokens.js'

  /** @type {{ projectPath: string, dark: boolean, onSelectTask?: (task: any) => void, onNavigateToCommit?: (hash: string) => void, onNavigateToFile?: (path: string) => void, onNavigateToCommitRange?: (after: string, before: string) => void }} */
  let { projectPath, dark, onSelectTask, onNavigateToCommit, onNavigateToFile, onNavigateToCommitRange } = $props()

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens
  const headerBg      = $derived(dark ? 'bg-zinc-900/40' : 'bg-zinc-50/60')
  const headerHover   = $derived(dark ? 'hover:bg-zinc-900/60' : 'hover:bg-zinc-100/80')
  const detailBg      = $derived(dark ? 'bg-zinc-900/20' : 'bg-zinc-50/30')
  const hashBg        = $derived(dark ? 'bg-zinc-800' : 'bg-zinc-200/80')

  const SOURCE_LABELS = TOOL_NAMES

  // Data state
  let sessions = $state([])
  let dataErrors = $state([])
  let loading = $state(true)

  // Expand/collapse state — Set of session_ids that are open
  let expanded = $state(new Set())

  // Lazy-loaded detail data per session: Map<session_id, { commits, files, loading, error }>
  let expandedData = $state(new Map())

  function toggleSession(sessionId) {
    if (expanded.has(sessionId)) {
      expanded = new Set([...expanded].filter(id => id !== sessionId))
    } else {
      expanded = new Set([...expanded, sessionId])
      // Lazy-load if not already cached
      if (!expandedData.has(sessionId)) {
        loadSessionDetail(sessionId)
      }
    }
  }

  async function loadSessionDetail(sessionId) {
    const session = sessions.find(s => s.session_id === sessionId)
    if (!session) return
    expandedData = new Map(expandedData).set(sessionId, { commits: [], files: [], loading: true, error: null })
    try {
      const result = await getCommitsInRange(projectPath, session.started_at, session.ended_at)
      expandedData = new Map(expandedData).set(sessionId, {
        commits: result.commits || [],
        files: result.files || [],
        loading: false,
        error: null,
      })
    } catch (e) {
      expandedData = new Map(expandedData).set(sessionId, {
        commits: [],
        files: [],
        loading: false,
        error: e.message || 'Failed to load details',
      })
    }
  }

  function isExpanded(sessionId) {
    return expanded.has(sessionId)
  }

  /** Format an ISO date string to a readable date. */
  function formatDate(iso) {
    if (!iso) return 'Unknown date'
    try {
      const d = new Date(iso)
      return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
    } catch {
      return 'Unknown date'
    }
  }

  /** Format an ISO date string as a relative time ("2h ago", "3d ago"). */
  function formatRelativeTime(iso) {
    if (!iso) return null
    try {
      const ms = Date.now() - new Date(iso).getTime()
      if (ms < 0) return null
      const mins = Math.floor(ms / 60000)
      if (mins < 1) return 'just now'
      if (mins < 60) return `${mins}m ago`
      const hours = Math.floor(mins / 60)
      if (hours < 24) return `${hours}h ago`
      const days = Math.floor(hours / 24)
      return `${days}d ago`
    } catch {
      return null
    }
  }

  // Fetch on mount
  $effect(() => {
    if (!projectPath) return
    let cancelled = false

    async function fetchData() {
      try {
        const result = await getArchivedSessions(projectPath)
        if (cancelled) return
        sessions = result.sessions || []
        dataErrors = result.errors || []
        loading = false
      } catch (e) {
        if (cancelled) return
        dataErrors = [e.message || 'Failed to load session history']
        loading = false
      }
    }

    fetchData()

    return () => { cancelled = true }
  })
</script>

<div class="flex-1 flex flex-col overflow-hidden">
  {#if loading}
    <!-- Loading skeleton: 3-4 collapsed session header placeholders -->
    <div class="flex-1 overflow-y-auto px-5 py-4 space-y-2" data-testid="history-loading">
      {#each Array(4) as _}
        <div class="rounded-lg {headerBg} px-4 py-3">
          <div class="flex items-center gap-3">
            <div class="h-3 w-3 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
            <div class="h-3 w-40 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
            <div class="h-2.5 w-20 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse ml-auto"></div>
          </div>
        </div>
      {/each}
    </div>

  {:else if sessions.length === 0}
    <!-- Empty state -->
    <div class="flex-1 flex items-center justify-center" data-testid="history-empty">
      <div class="text-center max-w-xs">
        <svg class="w-12 h-12 {t.textMuted} mx-auto opacity-30" fill="none" viewBox="0 0 24 24" stroke-width="1" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <p class="mt-4 text-[15px] font-medium {t.textMuted}">No completed work yet</p>
        <p class="mt-2 text-[13px] leading-relaxed {t.textTertiary}">Session summaries appear here after AI tools finish their work.</p>
      </div>
    </div>

  {:else}
    <!-- Error indicators -->
    {#if dataErrors.length > 0}
      <div class="px-5 pt-4 pb-2 space-y-1 shrink-0" data-testid="history-errors">
        {#each dataErrors as message}
          <div class="flex items-center gap-2 px-3 py-1.5 rounded text-[11px] {dark ? 'bg-warning-300/10 text-warning-300' : 'bg-warning-50 text-warning-600'}">
            <svg class="w-3 h-3 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
            </svg>
            <span>{message}</span>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Session accordion list -->
    <div class="flex-1 overflow-y-auto px-5 py-4 space-y-1.5">
      {#each sessions as session (session.session_id)}
        {@const open = isExpanded(session.session_id)}
        <div class="rounded-lg overflow-hidden border {t.keyline}">
          <!-- Session header (click target) -->
          <button
            class="w-full text-left flex items-center gap-3 px-4 py-3 rounded-lg transition-colors cursor-pointer
              {headerBg} {headerHover} {t.textPrimary}"
            data-testid="session-header"
            onclick={() => toggleSession(session.session_id)}
            aria-expanded={open}
          >
            <!-- Chevron -->
            <svg
              class="w-3 h-3 shrink-0 {t.textTertiary} transition-transform duration-200 {open ? 'rotate-90' : ''}"
              fill="none" viewBox="0 0 24 24" stroke-width="2.5" stroke="currentColor"
            >
              <path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" />
            </svg>

            <!-- Date + duration + last archived -->
            <span class="text-[13px] font-semibold">{formatDate(session.started_at)}</span>
            {#if session.duration_ms}
              <span class="text-[11px] {t.textTertiary}">{formatDuration(session.duration_ms)}</span>
            {/if}
            {#if formatRelativeTime(session.last_archived_at)}
              <span class="text-[10px] {t.textMuted}" data-testid="last-archived">archived {formatRelativeTime(session.last_archived_at)}</span>
            {/if}

            <!-- Spacer -->
            <span class="flex-1"></span>

            <!-- Task count + commit count pills -->
            <span class="text-[11px] {t.textTertiary}">
              {session.tasks.length} task{session.tasks.length !== 1 ? 's' : ''}
            </span>
            <span class="text-[11px] {t.textMuted}">&middot;</span>
            <span class="text-[11px] {t.textTertiary}">
              {session.commit_count} commit{session.commit_count !== 1 ? 's' : ''}
            </span>

            <!-- Source tool icons -->
            {#each session.sources as source}
              {@const icon = TOOL_ICONS[source]}
              {#if icon}
                <span class="w-3 h-3 shrink-0 {t.textTertiary}" aria-label={SOURCE_LABELS[source] || source}>
                  <svg class="w-3 h-3" viewBox={icon.viewBox} fill="currentColor" aria-hidden="true">
                    <path d={icon.path}/>
                  </svg>
                </span>
              {/if}
            {/each}
          </button>

          <!-- Expandable detail (CSS grid animation) -->
          {#if open}
            {@const detail = expandedData.get(session.session_id)}
            <div
              class="px-4 pb-3 pt-1 {detailBg} rounded-b-lg"
              data-testid="session-detail"
            >
              <!-- Tasks sub-section -->
              <div class="mb-3">
                <h4 class="text-[10px] font-semibold uppercase tracking-[0.06em] {t.textTertiary} mb-1.5">Tasks</h4>
                <div class="space-y-1">
                  {#each session.tasks as task}
                    <button
                      class="w-full text-left flex items-center gap-2 px-2 py-1 rounded transition-colors cursor-pointer
                        {dark ? 'hover:bg-zinc-800/50' : 'hover:bg-zinc-100/80'}"
                      data-testid="history-task"
                      onclick={() => onSelectTask?.(task)}
                    >
                      <!-- Checkmark icon -->
                      <svg class="w-3 h-3 shrink-0 text-success-400" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                      </svg>
                      <span class="text-[12px] {t.textBody} truncate">{task.subject}</span>
                      <!-- Source icon -->
                      {#if TOOL_ICONS[task.source]}
                        <span class="w-2.5 h-2.5 shrink-0 {t.textMuted} ml-auto">
                          <svg class="w-2.5 h-2.5" viewBox={TOOL_ICONS[task.source].viewBox} fill="currentColor" aria-hidden="true">
                            <path d={TOOL_ICONS[task.source].path}/>
                          </svg>
                        </span>
                      {/if}
                    </button>
                  {/each}
                </div>
              </div>

              <!-- Commits sub-section (lazy-loaded) -->
              {#if detail?.loading}
                <div class="mb-3 space-y-1" data-testid="session-commits-loading">
                  {#each Array(3) as _}
                    <div class="flex items-center gap-2 h-[22px]">
                      <div class="h-2 w-14 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
                      <div class="h-2 flex-1 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse"></div>
                    </div>
                  {/each}
                </div>
              {:else if detail?.commits?.length > 0}
                <div class="mb-3" data-testid="session-commits">
                  <h4 class="text-[10px] font-semibold uppercase tracking-[0.06em] {t.textTertiary} mb-1.5">Commits</h4>
                  <div class="space-y-0.5">
                    {#each detail.commits.slice(0, 5) as commit}
                      <button
                        class="w-full text-left flex items-center gap-2 px-2 py-0.5 rounded transition-colors {t.fileBg}"
                        onclick={() => onNavigateToCommit?.(commit.hash)}
                        data-testid="session-commit"
                      >
                        <span class="font-mono text-[11px] {t.hashColor} w-[58px] shrink-0">{commit.hash}</span>
                        <span class="text-[12px] {t.textBody} truncate">{commit.message}</span>
                      </button>
                    {/each}
                    {#if detail.commits.length > 5}
                      <span class="text-[10px] {t.textTertiary} px-2">+ {detail.commits.length - 5} more</span>
                    {/if}
                  </div>
                </div>
              {:else if session.commit_count > 0}
                <div class="mb-2">
                  <span class="text-[10px] font-semibold uppercase tracking-[0.06em] {t.textTertiary}">{session.commit_count} commit{session.commit_count !== 1 ? 's' : ''}</span>
                </div>
              {/if}

              <!-- Files sub-section (lazy-loaded) -->
              {#if detail && !detail.loading && detail.files?.length > 0}
                <div class="mb-3" data-testid="session-files">
                  <h4 class="text-[10px] font-semibold uppercase tracking-[0.06em] {t.textTertiary} mb-1.5">Files changed</h4>
                  <div class="space-y-0.5">
                    {#each detail.files.slice(0, 8) as filePath}
                      <button
                        class="w-full text-left flex items-center gap-2 px-2 py-0.5 rounded text-[11px] font-mono {t.textBody} transition-colors {t.fileBg}"
                        onclick={() => onNavigateToFile?.(filePath)}
                        data-testid="session-file"
                      >
                        {filePath}
                      </button>
                    {/each}
                    {#if detail.files.length > 8}
                      <span class="text-[10px] {t.textTertiary} px-2">+ {detail.files.length - 8} more files</span>
                    {/if}
                  </div>
                </div>
              {:else if session.file_count > 0 && (!detail || detail.loading)}
                <div class="text-[10px] {t.textMuted}">
                  {session.file_count} file{session.file_count !== 1 ? 's' : ''} changed
                </div>
              {/if}

              <!-- View in Git button -->
              {#if session.started_at && session.ended_at}
                <button
                  class="mt-1 text-[11px] {t.linkColor} transition-colors"
                  onclick={() => onNavigateToCommitRange?.(session.started_at, session.ended_at)}
                  data-testid="view-in-git"
                >View in Git &rarr;</button>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
