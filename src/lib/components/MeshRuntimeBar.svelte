<script>
  let {
    teamName = '',
    agents = [],
    dark = false,
    onAddAgent = () => {},
    onDisband = () => {},
  } = $props()

  const panelTone = $derived(dark ? 'bg-zinc-900/80 backdrop-blur-sm' : 'bg-white/80 backdrop-blur-sm')
  const borderTone = $derived(dark ? 'border-b border-white/8' : 'border-b border-brand-200/60')
  const titleTone = $derived(dark ? 'text-zinc-200' : 'text-zinc-900')
  const statusPillTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-900/80 text-zinc-200'
      : 'border-zinc-200 bg-white/90 text-zinc-700'
  )
  const secondaryTone = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800/80'
      : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'
  )
  const dangerTone = $derived(
    dark
      ? 'border-danger-500/40 text-danger-300 hover:bg-danger-500/15'
      : 'border-danger-400/60 text-danger-600 hover:bg-danger-500/10'
  )

  const statusCounts = $derived.by(() => {
    const counts = { active: 0, idle: 0, offline: 0 }
    for (const agent of Array.isArray(agents) ? agents : []) {
      const status = String(agent?.status || '').toLowerCase()
      if (status === 'active') {
        counts.active += 1
      } else if (status === 'idle') {
        counts.idle += 1
      } else {
        counts.offline += 1
      }
    }
    return counts
  })

  const statusSummary = $derived.by(() => {
    const parts = []
    if (statusCounts.active > 0) parts.push(`${statusCounts.active} active`)
    if (statusCounts.idle > 0) parts.push(`${statusCounts.idle} idle`)
    if (statusCounts.offline > 0) parts.push(`${statusCounts.offline} offline`)
    return parts.length > 0 ? parts.join(', ') : 'No agents'
  })

</script>

<footer
  class="flex w-full min-h-12 items-center justify-between gap-3 rounded-lg px-4 py-2.5 {panelTone} {borderTone}"
  data-testid="mesh-runtime-bar"
>
  <div class="min-w-0 flex flex-1 items-center gap-2.5">
    <h2 class="truncate text-sm font-semibold leading-tight {titleTone}" data-testid="mesh-runtime-title">
      {teamName.trim() || 'Untitled Team'}
    </h2>

    <div class="flex flex-wrap items-center gap-1.5" data-testid="mesh-runtime-summary">
      <span class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium {statusPillTone}">
        <span class="h-1.5 w-1.5 rounded-full bg-success-400"></span>
        {statusCounts.active} Active
      </span>
      <span class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium {statusPillTone}">
        <span class="h-1.5 w-1.5 rounded-full bg-warning-400"></span>
        {statusCounts.idle} Idle
      </span>
      <span class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium {statusPillTone}">
        <span class="h-1.5 w-1.5 rounded-full bg-zinc-400"></span>
        {statusCounts.offline} Offline
      </span>
      <span class="sr-only">{statusSummary}</span>
    </div>
  </div>

  <div class="flex items-center gap-2">
    <button
      class="inline-flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors {dangerTone}"
      type="button"
      onclick={onDisband}
      aria-label="Disband team"
      data-testid="mesh-runtime-disband"
    >
      <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="M19 6l-1 14H6L5 6"/><path d="m10 11 4 4"/><path d="m14 11-4 4"/></svg>
      Disband
    </button>

    <button
      class="rounded-md bg-brand-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-brand-700"
      type="button"
      onclick={onAddAgent}
      data-testid="mesh-runtime-add-agent"
    >
      + Add Agent
    </button>
  </div>
</footer>
