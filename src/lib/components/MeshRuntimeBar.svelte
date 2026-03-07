<script>
  let {
    teamName = '',
    lead = null,
    agents = [],
    teamRuntimeState = 'active',
    dark = false,
    actionsDisabled = false,
    onAddAgent = () => {},
    onDisband = () => {},
    onResumeTeam = () => {},
  } = $props()

  let overflowOpen = $state(false)

  const panelTone = $derived(dark ? 'bg-zinc-900/80 backdrop-blur-sm' : 'bg-white/80 backdrop-blur-sm')
  const borderTone = $derived(dark ? 'border border-white/8' : 'border border-brand-200/60')
  const titleTone = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const summaryTone = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const stateTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const primaryTone = $derived(
    actionsDisabled
      ? 'bg-brand-600/60 text-white'
      : 'bg-brand-600 text-white hover:bg-brand-700'
  )
  const secondaryTone = $derived(
    dark
      ? 'border-white/10 bg-white/[0.04] text-zinc-200 hover:bg-white/[0.08]'
      : 'border-brand-200/70 bg-white text-zinc-700 hover:bg-brand-50'
  )
  const menuTone = $derived(
    dark
      ? 'border-white/10 bg-zinc-950/95 shadow-[0_18px_44px_rgba(0,0,0,0.45)]'
      : 'border-brand-200/70 bg-white/95 shadow-[0_18px_44px_rgba(15,23,42,0.14)]'
  )
  const menuItemTone = $derived(
    dark ? 'text-zinc-200 hover:bg-white/[0.06]' : 'text-zinc-700 hover:bg-brand-50'
  )
  const dangerTone = $derived(
    dark ? 'text-danger-300 hover:bg-danger-500/12' : 'text-danger-600 hover:bg-danger-500/10'
  )
  const hintTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')

  const members = $derived.by(() => [lead, ...(Array.isArray(agents) ? agents : [])].filter(Boolean))

  const statusCounts = $derived.by(() => {
    const counts = { active: 0, idle: 0, offline: 0 }
    for (const member of members) {
      const status = String(member?.status ?? member?.sessionStatus ?? '').trim().toLowerCase()
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

  const totalMembers = $derived(members.length)
  const isColdResume = $derived(teamRuntimeState === 'coldResume')
  const isDegraded = $derived(teamRuntimeState === 'degraded')
  const isActive = $derived(teamRuntimeState === 'active')

  const summaryLine = $derived.by(() => {
    const parts = [`${totalMembers} member${totalMembers === 1 ? '' : 's'}`]
    parts.push(`${statusCounts.active} active`)
    if (statusCounts.idle > 0) parts.push(`${statusCounts.idle} idle`)
    if (statusCounts.offline > 0) parts.push(`${statusCounts.offline} stopped`)
    return parts.join(' • ')
  })

  const stateCopy = $derived.by(() => {
    if (isColdResume) return 'Team ready to resume'
    if (isDegraded) {
      return statusCounts.offline === 1 ? '1 member stopped' : `${statusCounts.offline} members stopped`
    }
    return 'Team running normally'
  })

  const primaryLabel = $derived.by(() => {
    if (!isActive) return isColdResume ? 'Resume Team' : `Resume Offline (${statusCounts.offline})`
    return 'Add Agent'
  })

  function closeOverflow() {
    overflowOpen = false
  }

  function handlePrimaryAction() {
    if (actionsDisabled) return
    if (isActive) {
      onAddAgent()
      return
    }
    onResumeTeam('continue')
  }

  function handleAddAgent() {
    if (actionsDisabled) return
    onAddAgent()
  }

  function handleDisband() {
    if (actionsDisabled) return
    closeOverflow()
    onDisband()
  }
</script>

<header
  class="relative flex w-full flex-col gap-3 rounded-xl px-4 py-3 {panelTone} {borderTone}"
  data-testid="mesh-runtime-bar"
>
  <div class="flex items-start justify-between gap-3">
    <div class="min-w-0 flex-1 space-y-1">
      <h2 class="truncate text-sm font-semibold leading-tight {titleTone}" data-testid="mesh-runtime-title">
        {teamName.trim() || 'Untitled Team'}
      </h2>
      <p class="text-xs font-medium {summaryTone}" data-testid="mesh-runtime-summary-line">
        {summaryLine}
      </p>
      <p class="text-[11px] {stateTone}" data-testid="mesh-runtime-state-copy">
        {stateCopy}
      </p>
    </div>

    <div class="flex shrink-0 items-center gap-2">
      <button
        class="inline-flex min-w-[8.5rem] items-center justify-center rounded-md px-3 py-1.5 text-xs font-semibold transition-colors disabled:cursor-not-allowed disabled:opacity-60 {primaryTone}"
        type="button"
        onclick={handlePrimaryAction}
        disabled={actionsDisabled}
        data-testid="mesh-runtime-primary-action"
      >
        {#if actionsDisabled && !isActive}
          Resuming Team...
        {:else}
          {primaryLabel}
        {/if}
      </button>

      {#if !isActive}
        <button
          class="inline-flex items-center justify-center rounded-md border px-3 py-1.5 text-xs font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-60 {secondaryTone}"
          type="button"
          onclick={handleAddAgent}
          disabled={actionsDisabled}
          data-testid="mesh-runtime-add-agent"
        >
          Add Agent
        </button>
      {/if}

      <div class="relative">
        <button
          class="inline-flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-60 {secondaryTone}"
          type="button"
          onclick={() => {
            if (actionsDisabled) return
            overflowOpen = !overflowOpen
          }}
          aria-expanded={overflowOpen}
          aria-haspopup="menu"
          disabled={actionsDisabled}
          data-testid="mesh-runtime-more-toggle"
        >
          More
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>
        </button>

        {#if overflowOpen}
          <div
            class="absolute right-0 top-[calc(100%+0.4rem)] z-20 flex min-w-48 flex-col gap-1 rounded-xl p-1.5 {menuTone}"
            role="menu"
            data-testid="mesh-runtime-more-menu"
          >
            <button
              class="rounded-lg px-3 py-2 text-left text-xs font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50 {menuItemTone}"
              type="button"
              disabled={true}
              title="Stop-all lifecycle action is planned for a later phase."
              data-testid="mesh-runtime-stop-all"
            >
              Stop All Members
            </button>

            <button
              class="rounded-lg px-3 py-2 text-left text-xs font-medium transition-colors {dangerTone}"
              type="button"
              onclick={handleDisband}
              disabled={actionsDisabled}
              data-testid="mesh-runtime-disband"
            >
              Disband Team...
            </button>

            <p class="px-3 pb-1 text-[10px] {hintTone}">
              Destructive actions require confirmation.
            </p>
          </div>
        {/if}
      </div>
    </div>
  </div>
</header>
