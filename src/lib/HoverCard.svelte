<script>
  import { sessionBadge, hasLiveSession, toolIcon } from './sessionIndicator.js'
  import { getProjectActivity, getRecentCommits } from './ipc.js'
  import { formatDuration } from './format.js'

  let {
    project = null,
    sessions = [],
    anchorEl = null,
  } = $props()

  let cardEl = $state(null)
  let posX = $state(0)
  let posY = $state(0)
  let historicalStats = $state(null)
  let recentCommits = $state(null)

  const liveSessions = $derived((sessions || []).filter(s => hasLiveSession(s)))

  const activityLabels = {
    active: 'Active',
    recent: 'Recent',
    stale: 'Stale',
    dormant: 'Dormant',
  }

  const activityDotColors = {
    active: 'bg-success-300',
    recent: 'bg-info-300',
    stale: 'bg-warning-300',
    dormant: 'bg-zinc-500',
  }

  function timeSince(timestamp) {
    return formatDuration(Date.now() - timestamp)
  }

  // Fetch historical stats and recent commits when project changes
  $effect(() => {
    if (!project?.path) {
      historicalStats = null
      recentCommits = null
      return
    }
    const path = project.path
    const id = project.id

    getProjectActivity(path).then(stats => {
      if (project?.path === path) historicalStats = stats
    }).catch(() => {
      historicalStats = null
    })

    getRecentCommits(id, 3).then(commits => {
      if (project?.id === id) recentCommits = commits
    }).catch(() => {
      recentCommits = null
    })
  })

  // Position the card anchored to the right edge of the sidebar row
  $effect(() => {
    if (!anchorEl || !cardEl) return

    const anchor = anchorEl.getBoundingClientRect()
    const card = cardEl.getBoundingClientRect()
    const vw = window.innerWidth
    const vh = window.innerHeight

    let x = anchor.right + 8
    let y = anchor.top + anchor.height / 2 - card.height / 2

    if (x + card.width > vw - 8) {
      x = anchor.left - card.width - 8
    }

    if (y + card.height > vh - 8) {
      y = vh - card.height - 8
    }
    if (y < 8) y = 8

    posX = x
    posY = y
  })
</script>

{#if project}
  <div
    bind:this={cardEl}
    class="fixed z-[90] w-[280px] rounded-lg border border-white/[0.08] bg-brand-950 shadow-xl shadow-black/40 py-3 px-3.5 pointer-events-none"
    style="left: {posX}px; top: {posY}px;"
    role="tooltip"
  >
    <!-- Header: name + branch -->
    <div class="flex items-baseline justify-between gap-3 mb-1.5">
      <span class="text-[14px] font-semibold text-white truncate">{project.name}</span>
      {#if project.branch}
        <span class="font-mono text-[11px] text-white/30 shrink-0 truncate max-w-[100px]">{project.branch}</span>
      {/if}
    </div>

    <!-- Status line: activity state · dirty · historical -->
    <div class="flex items-center gap-1.5 text-[11px] text-white/45 flex-wrap">
      <span class="flex items-center gap-1.5">
        <span class="w-[5px] h-[5px] rounded-full {activityDotColors[project.activity_state] || 'bg-zinc-500'}"></span>
        {activityLabels[project.activity_state] || 'Unknown'}
      </span>
      {#if project.is_dirty}
        <span class="text-white/20">·</span>
        <span class="text-warning-300/70">Dirty</span>
      {/if}
      {#if historicalStats && historicalStats.session_count > 0}
        <span class="text-white/20">·</span>
        <span class="text-white/30">{formatDuration(historicalStats.total_active_ms)} across {historicalStats.session_count} session{historicalStats.session_count === 1 ? '' : 's'}</span>
      {/if}
    </div>

    <!-- Recent commits -->
    {#if recentCommits && recentCommits.length > 0}
      <div class="h-px bg-white/[0.06] mt-2.5 mb-2"></div>

      <div class="space-y-1">
        {#each recentCommits as commit}
          <div class="flex items-baseline gap-2 text-[11px]">
            <span class="font-mono text-white/20 shrink-0">{commit.hash}</span>
            <span class="text-white/40 truncate flex-1">{commit.message}</span>
            <span class="text-white/20 shrink-0">{commit.date}</span>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Sessions -->
    {#if liveSessions.length > 0}
      <div class="h-px bg-white/[0.06] mt-2.5 mb-2"></div>

      {#each liveSessions as s}
        {@const badge = sessionBadge(s)}
        {@const icon = toolIcon(s)}
        {@const isWorking = s.state === 'active'}
        {@const statusColor = isWorking ? 'text-success-400' : 'text-warning-300'}

        <div class="mb-2.5 last:mb-0">
          <!-- Session header: icon + tool name + status + duration -->
          <div class="flex items-center gap-1.5 mb-0.5">
            <svg class="w-[11px] h-[11px] shrink-0 {statusColor}" viewBox={icon.viewBox} fill="currentColor" aria-hidden="true">
              <path d={icon.path}/>
            </svg>
            <span class="text-[12px] font-medium {statusColor}">{badge.toolLabel}</span>
            <span class="text-[11px] text-white/35">{isWorking ? 'working' : 'waiting for input'}</span>
            {#if s._duration != null}
              <span class="text-[11px] text-white/25 ml-auto shrink-0">{formatDuration(s._duration)}</span>
            {/if}
          </div>

          <!-- Stats line: active time + idle since -->
          {#if s._duration != null}
            <div class="flex items-center gap-1.5 text-[10px] text-white/30 pl-[17px]">
              <span>Active {formatDuration(s._activeMs)} ({s._activePercent}%)</span>
              {#if !isWorking && s._lastTransition}
                <span class="text-white/15">·</span>
                <span>idle {timeSince(s._lastTransition)}</span>
              {/if}
            </div>
          {/if}

          <!-- Technical metadata: sid · tmux · pid -->
          <div class="flex items-center gap-1.5 text-[10px] font-mono text-white/20 pl-[17px] mt-0.5">
            {#if s.session_id}
              <span>{s.session_id.slice(0, 8)}</span>
            {/if}
            {#if s.tmux_session}
              {#if s.session_id}<span class="text-white/10">·</span>{/if}
              <span>{s.tmux_session}{s.tmux_window != null ? `:${s.tmux_window}` : ''}</span>
            {/if}
            {#if s.pid}
              {#if s.session_id || s.tmux_session}<span class="text-white/10">·</span>{/if}
              <span>pid {s.pid}</span>
            {/if}
          </div>
        </div>
      {/each}
    {:else}
      <div class="h-px bg-white/[0.06] mt-2.5 mb-2"></div>
      <div class="text-[11px] text-white/20">No active sessions</div>
    {/if}
  </div>
{/if}
