<script>
  import { statusBadgeClass, statusLabel } from './taskHelpers.js'
  import { TOOL_ICONS, TOOL_NAMES } from './toolLogos.js'
  import { themeTokens } from './themeTokens.js'
  import MarkdownRenderer from './MarkdownRenderer.svelte'

  /** @type {{ task: object, detail: object|null, dark: boolean, codeTheme?: string, allTasks: object[], onClose: () => void, onNavigateTask?: (task: object) => void }} */
  let { task, detail, dark, codeTheme = 'github-light', allTasks = [], onClose, onNavigateTask } = $props()

  /** Look up a task by ID in the loaded task list. */
  function resolveTask(id) {
    return allTasks.find(t => t.id === id)
  }

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens
  const sectionBg     = $derived(dark ? 'bg-zinc-900/50' : 'bg-zinc-50/80')
  const fileBg        = $derived(dark ? 'bg-zinc-800/50' : 'bg-zinc-100/80')
  const divideColor   = $derived(dark ? 'divide-zinc-800' : 'divide-zinc-200')
  const hashPillBg    = $derived(dark ? 'bg-brand-950/80' : 'bg-brand-50')
  const depChipBg     = $derived(dark ? 'bg-zinc-800' : 'bg-zinc-100')
  const depChipText   = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const depChipHover  = $derived(dark ? 'hover:bg-zinc-700' : 'hover:bg-zinc-200')

  const SOURCE_LABELS = TOOL_NAMES

  // Resolve tool icon reactively from props
  const toolIcon = $derived(TOOL_ICONS[task.source] || TOOL_ICONS.claude)
  const toolLabel = $derived(SOURCE_LABELS[task.source] || task.source)

  // Close on Escape key
  function handleKeydown(e) {
    if (e.key === 'Escape') onClose()
  }

  /** Split a file path into directory and filename. */
  function splitPath(filePath) {
    const lastSlash = filePath.lastIndexOf('/')
    if (lastSlash === -1) return { dir: '', name: filePath }
    return { dir: filePath.slice(0, lastSlash + 1), name: filePath.slice(lastSlash + 1) }
  }

  /** Format a time range as "Feb 22, 03:59 - 04:05 (6m)". */
  function formatTimeRange(start, end) {
    try {
      const s = new Date(start)
      const e = new Date(end)
      const date = s.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
      const startTime = s.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false })
      const endTime = e.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false })
      const durationMs = e - s
      const durationMin = Math.round(durationMs / 60000)
      const durStr = durationMin < 60
        ? `${durationMin}m`
        : `${Math.floor(durationMin / 60)}h ${durationMin % 60}m`
      return `${date}, ${startTime} - ${endTime} (${durStr})`
    } catch {
      return `${start} - ${end}`
    }
  }

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

  function formatArchivedReason(reason) {
    if (!reason) return null
    if (reason === 'completed_and_removed') return 'source removed'
    return String(reason).replaceAll('_', ' ')
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<aside
  class="w-[360px] shrink-0 {t.mainBg} border-l {t.keyline} flex flex-col overflow-hidden task-detail-enter"
  data-testid="task-detail-panel"
  aria-label="Task detail"
>
  <!-- Header -->
  <header class="flex items-start gap-3 px-4 pt-4 pb-3 border-b {t.keyline} shrink-0">
    <div class="flex-1 min-w-0">
      <h3 class="text-[15px] font-semibold leading-snug {t.textPrimary}">{task.subject}</h3>
      <div class="flex items-center gap-2 mt-1.5">
        <!-- Source tool icon + label -->
        <span class="flex items-center gap-1 {t.textTertiary}">
          <svg class="w-[12px] h-[12px]" viewBox={toolIcon.viewBox} fill="currentColor" aria-hidden="true">
            <path d={toolIcon.path}/>
          </svg>
          <span class="text-[11px]">{toolLabel}</span>
        </span>
        <!-- Status badge -->
        <span class="text-[10px] font-medium px-1.5 py-0.5 rounded {statusBadgeClass(task.status)}">
          {statusLabel(task.status)}
        </span>
      </div>
    </div>
    <!-- Close button -->
    <button
      class="p-1 rounded {t.textTertiary} hover:text-zinc-300 transition-colors shrink-0"
      onclick={onClose}
      aria-label="Close detail panel"
      data-testid="detail-close"
    >
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  </header>

  <!-- Scrollable content -->
  <div class="flex-1 overflow-y-auto px-4">

    <!-- Loading state -->
    {#if !detail}
      <div class="space-y-3 py-3" data-testid="detail-loading">
        {#each Array(3) as _}
          <div class="h-3 w-full rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
        {/each}
      </div>

    {:else}

      <!-- Sections with keyline dividers between them -->
      <div class="divide-y {divideColor}" data-testid="detail-sections">

        <!-- Description -->
        {#if detail.task.description}
          <section class="py-3 text-[13px]" data-testid="detail-description">
            <MarkdownRenderer source={detail.task.description} {dark} {codeTheme} />
          </section>
        {/if}

        <!-- Session info -->
        {#if detail.session}
          <section class="py-3" data-testid="detail-session">
            <h4 class="text-[11px] font-semibold uppercase tracking-[0.06em] {t.textTertiary} mb-2">Session</h4>
            <div class="{sectionBg} rounded-md px-3 py-2 space-y-1">
              <div class="flex items-center justify-between">
                <span class="text-[11px] font-mono {t.hashColor}">{detail.session.id.slice(0, 8)}</span>
              </div>
              <div class="text-[11px] {t.textMuted}">
                {formatTimeRange(detail.session.started_at, detail.session.ended_at)}
              </div>
            </div>
          </section>
        {/if}

        {#if detail.task.archived_at || detail.task.archived_reason || detail.task.last_status}
          <section class="py-3" data-testid="detail-archive-context">
            <h4 class="text-[11px] font-semibold uppercase tracking-[0.06em] {t.textTertiary} mb-2">Archive Context</h4>
            <div class="{sectionBg} rounded-md px-3 py-2 space-y-1.5">
              {#if formatArchivedReason(detail.task.archived_reason)}
                <div class="text-[11px] {t.textMuted}">
                  Archived reason: <span class="{t.textBody}">{formatArchivedReason(detail.task.archived_reason)}</span>
                </div>
              {/if}
              {#if detail.task.archived_at}
                <div class="text-[11px] {t.textMuted}">
                  Archived at: <span class="{t.textBody}">{formatRelativeTime(detail.task.archived_at) || detail.task.archived_at}</span>
                </div>
              {/if}
              {#if detail.task.last_status}
                <div class="text-[11px] {t.textMuted}">
                  Last status: <span class="{t.textBody}">{statusLabel(detail.task.last_status)}</span>
                </div>
              {/if}
            </div>
          </section>
        {/if}

        <!-- Commits -->
        {#if detail.commits.length > 0}
          <section class="py-3" data-testid="detail-commits">
            <h4 class="text-[11px] font-semibold uppercase tracking-[0.06em] {t.textTertiary} mb-2">
              Commits ({detail.commits.length})
            </h4>
            <div class="space-y-1.5">
              {#each detail.commits as commit}
                <div class="flex items-start gap-2">
                  <code class="text-[11px] font-mono {t.hashColor} {hashPillBg} px-1.5 py-0.5 rounded shrink-0" data-testid="commit-hash">{commit.hash}</code>
                  <span class="text-[12px] {t.textSecondary} truncate flex-1 pt-px">{commit.message}</span>
                  <span class="text-[10px] {t.textMuted} shrink-0 pt-0.5">{commit.date}</span>
                </div>
              {/each}
            </div>
          </section>
        {/if}

        <!-- Files Changed -->
        {#if detail.files_changed.length > 0}
          <section class="py-3" data-testid="detail-files">
            <h4 class="text-[11px] font-semibold uppercase tracking-[0.06em] {t.textTertiary} mb-2">
              Files Changed ({detail.files_changed.length})
            </h4>
            <div class="space-y-0.5">
              {#each detail.files_changed as filePath}
                {@const parts = splitPath(filePath)}
                <div class="{fileBg} rounded px-2.5 py-1.5 font-mono text-[11px]">
                  {#if parts.dir}<span class="{t.textMuted}" data-testid="file-dir">{parts.dir}</span>{/if}<span class="{t.textSecondary}" data-testid="file-name">{parts.name}</span>
                </div>
              {/each}
            </div>
          </section>
        {/if}

        <!-- Dependencies -->
        {#if detail.task.blocked_by?.length > 0 || detail.task.blocks?.length > 0}
          <section class="py-3" data-testid="detail-dependencies">
            <h4 class="text-[11px] font-semibold uppercase tracking-[0.06em] {t.textTertiary} mb-2">Dependencies</h4>
            <div class="space-y-2">
              {#if detail.task.blocked_by?.length > 0}
                <div class="flex items-start gap-1.5 flex-wrap">
                  <span class="text-[11px] {t.textMuted} py-0.5">Blocked by</span>
                  {#each detail.task.blocked_by as id}
                    {@const resolved = resolveTask(id)}
                    {#if resolved && onNavigateTask}
                      <button
                        class="text-[11px] font-mono {depChipText} {depChipBg} {depChipHover} px-1.5 py-0.5 rounded cursor-pointer transition-colors text-left max-w-full truncate"
                        data-testid="dep-chip"
                        onclick={() => onNavigateTask(resolved)}
                      >#{id} · {resolved.subject}</button>
                    {:else}
                      <span class="text-[11px] font-mono {depChipText} {depChipBg} px-1.5 py-0.5 rounded opacity-60" data-testid="dep-chip">#{id}</span>
                    {/if}
                  {/each}
                </div>
              {/if}
              {#if detail.task.blocks?.length > 0}
                <div class="flex items-start gap-1.5 flex-wrap">
                  <span class="text-[11px] {t.textMuted} py-0.5">Blocks</span>
                  {#each detail.task.blocks as id}
                    {@const resolved = resolveTask(id)}
                    {#if resolved && onNavigateTask}
                      <button
                        class="text-[11px] font-mono {depChipText} {depChipBg} {depChipHover} px-1.5 py-0.5 rounded cursor-pointer transition-colors text-left max-w-full truncate"
                        data-testid="dep-chip"
                        onclick={() => onNavigateTask(resolved)}
                      >#{id} · {resolved.subject}</button>
                    {:else}
                      <span class="text-[11px] font-mono {depChipText} {depChipBg} px-1.5 py-0.5 rounded opacity-60" data-testid="dep-chip">#{id}</span>
                    {/if}
                  {/each}
                </div>
              {/if}
            </div>
          </section>
        {/if}

        <!-- Owner -->
        {#if detail.task.owner}
          <section class="py-3" data-testid="detail-owner">
            <h4 class="text-[11px] font-semibold uppercase tracking-[0.06em] {t.textTertiary} mb-2">Owner</h4>
            <span class="text-[12px] {t.textBody}">{detail.task.owner}</span>
          </section>
        {/if}

      </div>

    {/if}

  </div>
</aside>

<style>
  .task-detail-enter {
    animation: slideIn 150ms ease-out;
  }

  @keyframes slideIn {
    from { transform: translateX(100%); opacity: 0; }
    to { transform: translateX(0); opacity: 1; }
  }

  @media (prefers-reduced-motion: reduce) {
    .task-detail-enter {
      animation: none;
    }
  }

  /* Scale MarkdownRenderer prose to match panel's 13px content scale */
  :global([data-testid="detail-description"] .th-prose) {
    font-size: 13px;
  }
</style>
