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

  const borderTone = $derived(dark ? 'border-zinc-700/70' : 'border-zinc-200')
  const titleTone = $derived(dark ? 'text-zinc-200' : 'text-zinc-900')
  const mutedTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const ghostTone = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800/80'
      : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'
  )
  const overflowTone = $derived(
    dark
      ? 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/70'
      : 'text-zinc-500 hover:text-zinc-900 hover:bg-zinc-100'
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
  class="flex h-12 items-center justify-between gap-2 border-t px-3 {borderTone}"
  data-testid="mesh-runtime-bar"
>
  <div class="min-w-0">
    <h2 class="truncate text-sm font-semibold {titleTone}" data-testid="mesh-runtime-title">
      {teamName.trim() || 'Untitled Team'}
    </h2>
    <p class="truncate text-[11px] {mutedTone}" data-testid="mesh-runtime-summary">{statusSummary}</p>
  </div>

  <div class="flex items-center gap-1.5">
    <button
      class="h-8 rounded-md border px-2.5 text-xs transition-colors {ghostTone}"
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
        class="h-8 rounded-md px-2 text-xs transition-colors {overflowTone}"
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
