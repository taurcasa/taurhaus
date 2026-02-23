<script>
  import { getArchivedSessions } from './ipc.js'
  import { formatDuration } from './format.js'

  /** @type {{ projectPath: string, dark: boolean, onSelectTask?: (task: any) => void }} */
  let { projectPath, dark, onSelectTask } = $props()

  // Dark mode tokens
  const textPrimary   = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const textTertiary  = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const textMuted     = $derived(dark ? 'text-zinc-600' : 'text-zinc-500')
  const textBody      = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const headerBg      = $derived(dark ? 'bg-zinc-900/40' : 'bg-zinc-50/60')
  const headerHover   = $derived(dark ? 'hover:bg-zinc-900/60' : 'hover:bg-zinc-100/80')
  const keyline       = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const detailBg      = $derived(dark ? 'bg-zinc-900/20' : 'bg-zinc-50/30')
  const hashBg        = $derived(dark ? 'bg-zinc-800' : 'bg-zinc-200/80')

  // Tool icon SVG paths (same as TaskBoard)
  const TOOL_ICONS = {
    claude: { viewBox: '0 0 16 16', path: 'M3.127 10.604l3.135-1.76.053-.153-.053-.085H6.11l-.525-.032-1.791-.048-1.554-.065-1.505-.08-.38-.081L0 7.832l.036-.234.32-.214.455.04 1.009.069 1.513.105 1.097.064 1.626.17h.259l.036-.105-.089-.065-.068-.064-1.566-1.062-1.695-1.121-.887-.646-.48-.327-.243-.306-.104-.67.435-.48.585.04.15.04.593.456 1.267.981 1.654 1.218.242.202.097-.068.012-.049-.109-.181-.9-1.626-.96-1.655-.428-.686-.113-.411a2 2 0 01-.068-.484l.496-.674L4.446 0l.662.089.279.242.411.94.666 1.48 1.033 2.014.302.597.162.553.06.17h.105v-.097l.085-1.134.157-1.392.154-1.792.052-.504.25-.605.497-.327.387.186.319.456-.045.294-.19 1.23-.37 1.93-.243 1.29h.142l.161-.16.654-.868 1.097-1.372.484-.545.565-.601.363-.287h.686l.505.751-.226.775-.707.895-.585.759-.839 1.13-.524.904.048.072.125-.012 1.897-.403 1.024-.186 1.223-.21.553.258.06.263-.218.536-1.307.323-1.533.307-2.284.54-.028.02.032.04 1.029.098.44.024h1.077l2.005.15.525.346.315.424-.053.323-.807.411-3.631-.863-.872-.218h-.12v.073l.726.71 1.331 1.202 1.667 1.55.084.383-.214.302-.226-.032-1.464-1.101-.565-.497-1.28-1.077h-.084v.113l.295.432 1.557 2.34.08.718-.112.234-.404.141-.444-.08-.911-1.28-.94-1.44-.759-1.291-.093.053-.448 4.821-.21.246-.484.186-.403-.307-.214-.496.214-.98.258-1.28.21-1.016.19-1.263.112-.42-.008-.028-.092.012-.953 1.307-1.448 1.957-1.146 1.227-.274.109-.477-.247.045-.44.266-.39 1.586-2.018.956-1.25.617-.723-.004-.105h-.036l-4.212 2.736-.75.096-.324-.302.04-.496.154-.162 1.267-.871z' },
    codex: { viewBox: '0 0 16 16', path: 'M14.949 6.547a3.94 3.94 0 00-.348-3.273 4.11 4.11 0 00-4.4-1.934A4.1 4.1 0 008.423.2 4.15 4.15 0 006.305.086a4.1 4.1 0 00-1.891.948 4.04 4.04 0 00-1.158 1.753 4.1 4.1 0 00-1.563.679A4 4 0 00.554 4.72a3.99 3.99 0 00.502 4.731 3.94 3.94 0 00.346 3.274 4.11 4.11 0 004.402 1.933c.382.425.852.764 1.377.995.526.231 1.095.35 1.67.346 1.78.002 3.358-1.132 3.901-2.804a4.1 4.1 0 001.563-.68 4 4 0 001.14-1.253 3.99 3.99 0 00-.506-4.716m-6.097 8.406a3.05 3.05 0 01-1.945-.694l.096-.054 3.23-1.838a.53.53 0 00.265-.455v-4.49l1.366.778q.02.011.025.035v3.722c-.003 1.653-1.361 2.992-3.037 2.996m-6.53-2.75a2.95 2.95 0 01-.36-2.01l.095.057L5.29 12.09a.53.53 0 00.527 0l3.949-2.246v1.555a.05.05 0 01-.022.041L6.473 13.3c-1.454.826-3.311.335-4.15-1.098m-.85-6.94A3.02 3.02 0 013.07 3.949v3.785a.51.51 0 00.262.451l3.93 2.237-1.366.779a.05.05 0 01-.048 0L2.585 9.342a2.98 2.98 0 01-1.113-4.094zm11.216 2.571L8.747 5.576l1.362-.776a.05.05 0 01.048 0l3.265 1.86a3 3 0 011.173 1.207 2.96 2.96 0 01-.27 3.2 3.05 3.05 0 01-1.36.997V8.279a.52.52 0 00-.276-.445m1.36-2.015l-.097-.057-3.226-1.855a.53.53 0 00-.53 0L6.249 6.153V4.598a.04.04 0 01.019-.04L9.533 2.7a3.07 3.07 0 013.257.139c.474.325.843.778 1.066 1.303.223.526.289 1.103.191 1.664zM5.503 8.575L4.139 7.8a.05.05 0 01-.026-.037V4.049c0-.57.166-1.127.476-1.607s.752-.864 1.275-1.105a3.08 3.08 0 013.234.41l-.096.054-3.23 1.838a.53.53 0 00-.265.455zm.742-1.577l1.758-1 1.762 1v2l-1.755 1-1.762-1z' },
    gemini: { viewBox: '0 0 65 65', path: 'M32.447 0c.68 0 1.273.465 1.439 1.125a38.904 38.904 0 001.999 5.905c2.152 5 5.105 9.376 8.854 13.125 3.751 3.75 8.126 6.703 13.125 8.855a38.98 38.98 0 005.906 1.999c.66.166 1.124.758 1.124 1.438 0 .68-.464 1.273-1.125 1.439a38.902 38.902 0 00-5.905 1.999c-5 2.152-9.375 5.105-13.125 8.854-3.749 3.751-6.702 8.126-8.854 13.125a38.973 38.973 0 00-2 5.906 1.485 1.485 0 01-1.438 1.124c-.68 0-1.272-.464-1.438-1.125a38.913 38.913 0 00-2-5.905c-2.151-5-5.103-9.375-8.854-13.125-3.75-3.749-8.125-6.702-13.125-8.854a38.973 38.973 0 00-5.905-2A1.485 1.485 0 010 32.448c0-.68.465-1.272 1.125-1.438a38.903 38.903 0 005.905-2c5-2.151 9.376-5.104 13.125-8.854 3.75-3.749 6.703-8.125 8.855-13.125a38.972 38.972 0 001.999-5.905A1.485 1.485 0 0132.447 0z' },
  }

  const SOURCE_LABELS = { claude: 'Claude', codex: 'Codex', gemini: 'Gemini' }

  // Data state
  let sessions = $state([])
  let dataErrors = $state([])
  let loading = $state(true)

  // Expand/collapse state — Set of session_ids that are open
  let expanded = $state(new Set())

  function toggleSession(sessionId) {
    if (expanded.has(sessionId)) {
      expanded = new Set([...expanded].filter(id => id !== sessionId))
    } else {
      expanded = new Set([...expanded, sessionId])
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
      <div class="text-center">
        <svg class="w-10 h-10 {textMuted} mx-auto opacity-40" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <p class="mt-3 text-[13px] {textMuted}">No completed work yet</p>
        <p class="mt-1 text-[11px] {textTertiary}">Finished tasks appear here after sessions end</p>
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
        <div class="rounded-lg overflow-hidden border {keyline}">
          <!-- Session header (click target) -->
          <button
            class="w-full text-left flex items-center gap-3 px-4 py-3 rounded-lg transition-colors cursor-pointer
              {headerBg} {headerHover} {textPrimary}"
            data-testid="session-header"
            onclick={() => toggleSession(session.session_id)}
            aria-expanded={open}
          >
            <!-- Chevron -->
            <svg
              class="w-3 h-3 shrink-0 {textTertiary} transition-transform duration-200 {open ? 'rotate-90' : ''}"
              fill="none" viewBox="0 0 24 24" stroke-width="2.5" stroke="currentColor"
            >
              <path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" />
            </svg>

            <!-- Date + duration -->
            <span class="text-[13px] font-semibold">{formatDate(session.started_at)}</span>
            {#if session.duration_ms}
              <span class="text-[11px] {textTertiary}">{formatDuration(session.duration_ms)}</span>
            {/if}

            <!-- Spacer -->
            <span class="flex-1"></span>

            <!-- Task count + commit count pills -->
            <span class="text-[11px] {textTertiary}">
              {session.tasks.length} task{session.tasks.length !== 1 ? 's' : ''}
            </span>
            <span class="text-[11px] {textMuted}">&middot;</span>
            <span class="text-[11px] {textTertiary}">
              {session.commit_count} commit{session.commit_count !== 1 ? 's' : ''}
            </span>

            <!-- Source tool icons -->
            {#each session.sources as source}
              {@const icon = TOOL_ICONS[source]}
              {#if icon}
                <span class="w-3 h-3 shrink-0 {textTertiary}" aria-label={SOURCE_LABELS[source] || source}>
                  <svg class="w-3 h-3" viewBox={icon.viewBox} fill="currentColor" aria-hidden="true">
                    <path d={icon.path}/>
                  </svg>
                </span>
              {/if}
            {/each}
          </button>

          <!-- Expandable detail (CSS grid animation) -->
          {#if open}
            <div
              class="px-4 pb-3 pt-1 {detailBg} rounded-b-lg"
              data-testid="session-detail"
            >
              <!-- Tasks sub-section -->
              <div class="mb-3">
                <h4 class="text-[10px] font-semibold uppercase tracking-[0.06em] {textTertiary} mb-1.5">Tasks</h4>
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
                      <span class="text-[12px] {textBody} truncate">{task.subject}</span>
                      <!-- Source icon -->
                      {#if TOOL_ICONS[task.source]}
                        <span class="w-2.5 h-2.5 shrink-0 {textMuted} ml-auto">
                          <svg class="w-2.5 h-2.5" viewBox={TOOL_ICONS[task.source].viewBox} fill="currentColor" aria-hidden="true">
                            <path d={TOOL_ICONS[task.source].path}/>
                          </svg>
                        </span>
                      {/if}
                    </button>
                  {/each}
                </div>
              </div>

              <!-- Commits sub-section (count only — full list shown in TaskDetailPanel) -->
              {#if session.commit_count > 0}
                <div class="mb-2">
                  <span class="text-[10px] font-semibold uppercase tracking-[0.06em] {textTertiary}">{session.commit_count} commit{session.commit_count !== 1 ? 's' : ''}</span>
                </div>
              {/if}

              <!-- Files changed footer -->
              {#if session.file_count > 0}
                <div class="text-[10px] {textMuted}">
                  {session.file_count} file{session.file_count !== 1 ? 's' : ''} changed
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
