<script>
  let {
    teamName = '',
    agents = [],
    dark = false,
    onAddAgent = () => {},
    onDisband = () => {},
    onOverflow = () => {},
  } = $props()

  let showOverflowMenu = $state(false)

  const panelTone = $derived(dark ? 'border-zinc-700/70 bg-zinc-900/60' : 'border-zinc-200 bg-brand-50/80')
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

  function handleWindowClick(event) {
    if (!showOverflowMenu) return
    const target = event?.target
    if (!(target instanceof Element)) return
    if (
      target.closest('[data-testid="mesh-runtime-overflow-button"]') ||
      target.closest('[data-testid="mesh-runtime-overflow-menu"]')
    ) {
      return
    }
    showOverflowMenu = false
  }

  function toggleOverflowMenu() {
    showOverflowMenu = !showOverflowMenu
    onOverflow(showOverflowMenu)
  }
</script>

<svelte:window onclick={handleWindowClick} />

<footer
  class="flex items-center gap-3 rounded-lg border px-4 py-2.5 {panelTone}"
  data-testid="mesh-runtime-bar"
>
  <div class="min-w-0 flex-1">
    <h2 class="truncate text-[15px] font-semibold leading-tight {titleTone}" data-testid="mesh-runtime-title">
      {teamName.trim() || 'Untitled Team'}
    </h2>
  </div>

  <div class="flex flex-wrap items-center justify-center gap-1.5" data-testid="mesh-runtime-summary">
    <span class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium {statusPillTone}">
      <span class="h-1.5 w-1.5 rounded-full bg-success-400"></span>
      {statusCounts.active} active
    </span>
    <span class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium {statusPillTone}">
      <span class="h-1.5 w-1.5 rounded-full bg-warning-400"></span>
      {statusCounts.idle} idle
    </span>
    <span class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium {statusPillTone}">
      <span class="h-1.5 w-1.5 rounded-full bg-zinc-400"></span>
      {statusCounts.offline} offline
    </span>
    <span class="sr-only">{statusSummary}</span>
  </div>

  <div class="flex items-center gap-2">
    <button
      class="rounded-md bg-brand-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-brand-700"
      type="button"
      onclick={() => {
        showOverflowMenu = false
        onAddAgent()
      }}
      data-testid="mesh-runtime-add-agent"
    >
      + Add Agent
    </button>

    <div class="relative">
      <button
        class="rounded-md border px-3 py-1.5 text-xs transition-colors {secondaryTone}"
        type="button"
        onclick={toggleOverflowMenu}
        aria-label="Runtime menu"
        data-testid="mesh-runtime-overflow-button"
      >
        ⋯
      </button>

      {#if showOverflowMenu}
        <div
          class="absolute right-0 top-full mt-1 z-10 min-w-[140px] rounded-md border py-1 shadow-lg {dark ? 'border-zinc-700 bg-zinc-900' : 'border-zinc-200 bg-white'}"
          data-testid="mesh-runtime-overflow-menu"
        >
          <button
            class="block w-full px-3 py-1.5 text-left text-xs text-danger-500 transition-colors hover:bg-danger-500/10"
            type="button"
            onclick={() => {
              showOverflowMenu = false
              onDisband()
            }}
            data-testid="mesh-runtime-disband"
          >
            Disband Team
          </button>
        </div>
      {/if}
    </div>
  </div>
</footer>
