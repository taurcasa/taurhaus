<script>
  import { sessionBadge, hasLiveSession } from './sessionIndicator.js'
  import { getProjectActivity } from './ipc.js'
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

  const liveSessions = $derived((sessions || []).filter(s => hasLiveSession(s)))

  // Activity state labels
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

  /** Time since a timestamp, formatted as relative duration. */
  function timeSince(timestamp) {
    return formatDuration(Date.now() - timestamp)
  }

  // Fetch historical stats when project changes
  $effect(() => {
    if (!project?.path) {
      historicalStats = null
      return
    }
    const path = project.path
    getProjectActivity(path).then(stats => {
      // Guard against stale responses
      if (project?.path === path) {
        historicalStats = stats
      }
    }).catch(() => {
      historicalStats = null
    })
  })

  // Position the card anchored to the right edge of the sidebar row
  $effect(() => {
    if (!anchorEl || !cardEl) return

    const anchor = anchorEl.getBoundingClientRect()
    const card = cardEl.getBoundingClientRect()
    const vw = window.innerWidth
    const vh = window.innerHeight

    // Right of the anchor, vertically centered
    let x = anchor.right + 8
    let y = anchor.top + anchor.height / 2 - card.height / 2

    // If it would go off-screen right, flip to left
    if (x + card.width > vw - 8) {
      x = anchor.left - card.width - 8
    }

    // Clamp vertically
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
    class="fixed z-[90] w-[260px] rounded-lg border border-white/[0.08] bg-brand-950 shadow-xl shadow-black/40 py-3 px-3.5 pointer-events-none"
    style="left: {posX}px; top: {posY}px;"
    role="tooltip"
  >
    <!-- Project info -->
    <div class="flex items-center gap-2 mb-1.5">
      <span class="text-[14px] font-semibold text-white truncate">{project.name}</span>
    </div>

    <div class="flex items-center gap-3 text-[12px] text-white/55">
      <span class="flex items-center gap-1.5">
        <span class="w-[6px] h-[6px] rounded-full {activityDotColors[project.activity_state] || 'bg-zinc-500'}"></span>
        {activityLabels[project.activity_state] || 'Unknown'}
      </span>
      {#if project.branch}
        <span class="font-mono text-[11px] text-white/35">{project.branch}</span>
      {/if}
    </div>

    {#if project.is_dirty}
      <div class="mt-1.5 flex items-center gap-1.5 text-[11px] text-warning-300/80">
        <span class="w-[5px] h-[5px] rounded-full bg-warning-400"></span>
        Working tree dirty
      </div>
    {/if}

    <!-- Historical LLM time -->
    {#if historicalStats && historicalStats.session_count > 0}
      <div class="mt-1.5 text-[11px] text-white/35">
        LLM time: {formatDuration(historicalStats.total_active_ms)} across {historicalStats.session_count} session{historicalStats.session_count === 1 ? '' : 's'}
      </div>
    {/if}

    <!-- Separator -->
    <div class="h-px bg-white/[0.06] my-2.5"></div>

    <!-- Session info -->
    {#if liveSessions.length > 0}
      {#each liveSessions as s}
        {@const badge = sessionBadge(s)}
        <div class="mb-2 last:mb-0">
          <div class="flex items-center gap-2 mb-1">
            <span
              class="inline-flex items-center justify-center text-[9px] font-semibold tracking-[0.08em] px-1.5 h-[16px] {badge.badgeClass}"
            >{badge.label}</span>
            <span class="text-[12px] text-white/55">
              {badge.toolLabel} {s.state === 'idle' ? '— waiting for input' : '— working'}
            </span>
          </div>

          <!-- Duration stats -->
          {#if s._duration != null}
            <div class="space-y-0.5 text-[11px] text-white/40 mb-1">
              <div>Running for {formatDuration(s._duration)}</div>
              <div>Active: {formatDuration(s._activeMs)} ({s._activePercent}%)</div>
              {#if s.state === 'idle' && s._lastTransition}
                <div>Idle since {timeSince(s._lastTransition)} ago</div>
              {/if}
            </div>
          {/if}

          <div class="space-y-0.5 text-[11px] font-mono text-white/25">
            {#if s.session_id}
              <div>sid: {s.session_id.slice(0, 12)}</div>
            {/if}
            {#if s.tmux_session}
              <div>tmux: {s.tmux_session}{s.tmux_window != null ? `:${s.tmux_window}` : ''}</div>
            {/if}
            {#if s.pid}
              <div>pid: {s.pid}</div>
            {/if}
          </div>
        </div>
      {/each}
    {:else}
      <div class="text-[11px] text-white/25">No active session</div>
    {/if}
  </div>
{/if}
