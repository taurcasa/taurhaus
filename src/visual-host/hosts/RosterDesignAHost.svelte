<!--
  Design A: "The Bench"
  Sports roster feel — team on a board at top, available roles as a visible bench below.
  The bench is always visible. Roles flow from bench → board on click.
-->
<script>
  import { MOCK_ROLES, MOCK_PRESETS, MOCK_TEAM_PARTIAL, MOCK_TEAM_READY } from '../../test/visual/fixtures/rosterDesigns.fixtures.js'

  let { scenario, theme = 'dark' } = $props()
  const dark = $derived(theme === 'dark')
  const state = $derived(scenario?.state ?? 'empty')

  const team = $derived(
    state === 'ready' ? MOCK_TEAM_READY
    : state === 'partial' ? MOCK_TEAM_PARTIAL
    : null
  )
  const availableRoles = $derived(
    MOCK_ROLES.filter(r => {
      if (!team) return true
      if (team.lead?.id === r.id) return false
      return !team.agents.some(a => a.id === r.id)
    })
  )

  const bg = $derived(dark ? 'bg-[#090e11]' : 'bg-[#f8fcfb]')
  const cardBg = $derived(dark ? 'bg-white/[0.04] border-white/[0.08]' : 'bg-white border-brand-200/40')
  const cardBgHover = $derived(dark ? 'hover:bg-white/[0.07] hover:border-white/[0.14]' : 'hover:bg-brand-50/60 hover:border-brand-400/40')
  const textPrimary = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const textMuted = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const accentBg = $derived(dark ? 'bg-brand-500/10 border-brand-500/20' : 'bg-brand-50 border-brand-200')
  const sectionBg = $derived(dark ? 'bg-white/[0.02] border-white/[0.06]' : 'bg-zinc-50/80 border-zinc-200/60')

  function toolColor(tool) {
    if (tool === 'claude') return dark ? 'text-brand-400' : 'text-brand-600'
    if (tool === 'codex') return dark ? 'text-emerald-400' : 'text-emerald-600'
    if (tool === 'agy') return dark ? 'text-blue-400' : 'text-blue-600'
    return textMuted
  }

  function toolBadgeBg(tool) {
    if (tool === 'claude') return dark ? 'bg-brand-500/15 border-brand-500/25 text-brand-300' : 'bg-brand-50 border-brand-200 text-brand-700'
    if (tool === 'codex') return dark ? 'bg-emerald-500/15 border-emerald-500/25 text-emerald-300' : 'bg-emerald-50 border-emerald-200 text-emerald-700'
    if (tool === 'agy') return dark ? 'bg-blue-500/15 border-blue-500/25 text-blue-300' : 'bg-blue-50 border-blue-200 text-blue-700'
    return ''
  }

  function toolInitial(tool) {
    if (tool === 'claude') return 'C'
    if (tool === 'codex') return 'X'
    if (tool === 'agy') return 'G'
    return '?'
  }
</script>

<div class="{bg} rounded-2xl p-6 min-h-[600px] font-sans">
  <!-- Header -->
  <div class="mb-6">
    <div class="flex items-center justify-between">
      <div>
        <p class="text-[10px] font-bold uppercase tracking-[0.2em] {textMuted}">Build your team</p>
        {#if team}
          <h1 class="mt-1 text-2xl font-bold {textPrimary}">{team.name}</h1>
          <p class="mt-1 text-sm {textSecondary}">{team.description}</p>
        {:else}
          <h1 class="mt-1 text-2xl font-bold {textPrimary}">New Team</h1>
          <p class="mt-1 text-sm {textSecondary}">Pick a preset or start adding roles from the bench.</p>
        {/if}
      </div>
      {#if team?.lead && (state === 'partial' || state === 'ready')}
        <button class="h-10 rounded-xl bg-brand-600 px-5 text-sm font-semibold text-white transition hover:bg-brand-500 {state === 'ready' ? '' : 'opacity-60'}">
          {state === 'ready' ? 'Initialize Team' : 'Add more agents...'}
        </button>
      {/if}
    </div>

    <!-- Preset chips -->
    {#if state === 'empty'}
      <div class="mt-4 flex flex-wrap gap-2">
        {#each MOCK_PRESETS as preset}
          <button class="inline-flex h-9 items-center gap-2 rounded-xl border px-3.5 text-[12px] font-semibold transition {cardBg} {cardBgHover}">
            <span class={textPrimary}>{preset.name}</span>
            <span class="text-[10px] {textMuted}">{preset.agents + preset.leads}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- THE BOARD — Team roster -->
  <section class="mb-6 rounded-2xl border p-5 {sectionBg}">
    <p class="mb-4 text-[10px] font-bold uppercase tracking-[0.2em] {textMuted}">Your Team</p>

    {#if !team}
      <!-- Empty board -->
      <div class="grid grid-cols-3 gap-3">
        <!-- Lead slot -->
        <div class="col-span-3 flex items-center justify-center rounded-2xl border-2 border-dashed py-10 {dark ? 'border-brand-500/20' : 'border-brand-300/40'}">
          <div class="text-center">
            <div class="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-2xl {accentBg}">
              <span class="text-xl font-bold {dark ? 'text-brand-400' : 'text-brand-600'}">★</span>
            </div>
            <p class="text-sm font-semibold {textPrimary}">Lead position</p>
            <p class="mt-1 text-[12px] {textSecondary}">Pick a lead role from the bench below</p>
          </div>
        </div>
        <!-- Agent slots -->
        {#each [1, 2, 3] as _}
          <div class="flex items-center justify-center rounded-2xl border-2 border-dashed py-8 {dark ? 'border-white/[0.06]' : 'border-zinc-200/60'}">
            <div class="text-center">
              <p class="text-2xl {textMuted}">+</p>
              <p class="mt-1 text-[11px] {textMuted}">Agent</p>
            </div>
          </div>
        {/each}
      </div>

    {:else}
      <!-- Populated board -->
      <div class="space-y-3">
        <!-- Lead card — prominent -->
        <div class="flex items-center gap-4 rounded-2xl border p-4 {cardBg}">
          <div class="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl border-2 {toolBadgeBg(team.lead.tool)}">
            <span class="text-xl font-bold">{toolInitial(team.lead.tool)}</span>
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <span class="text-[10px] font-bold uppercase tracking-wider {dark ? 'text-brand-400' : 'text-brand-600'}">Lead</span>
              <span class="rounded-full border px-2 py-0.5 text-[9px] font-bold uppercase {toolBadgeBg(team.lead.tool)}">{team.lead.tool}</span>
            </div>
            <p class="mt-1 text-[15px] font-bold {textPrimary}">{team.lead.name}</p>
            <p class="mt-0.5 text-[12px] {textSecondary}">{team.lead.summary}</p>
          </div>
          <span class="text-[11px] font-medium {textMuted}">{team.lead.model}</span>
        </div>

        <!-- Agent cards — grid -->
        <div class="grid gap-3 {team.agents.length > 2 ? 'grid-cols-2' : 'grid-cols-2'}">
          {#each team.agents as agent, i}
            <div class="flex items-center gap-3 rounded-2xl border p-3.5 {cardBg}">
              <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border {toolBadgeBg(agent.tool)}">
                <span class="text-sm font-bold">{toolInitial(agent.tool)}</span>
              </div>
              <div class="min-w-0 flex-1">
                <p class="text-[13px] font-semibold {textPrimary}">{agent.name}</p>
                <p class="mt-0.5 text-[11px] {textSecondary}">{agent.tool} · {agent.model}</p>
              </div>
              <span class="flex h-7 w-7 items-center justify-center rounded-lg text-[11px] font-bold {dark ? 'bg-white/[0.06] text-zinc-400' : 'bg-zinc-100 text-zinc-500'}">
                {i + 1}
              </span>
            </div>
          {/each}

          <!-- Add more slot -->
          <button class="flex items-center justify-center rounded-2xl border-2 border-dashed py-6 transition {dark ? 'border-white/[0.06] hover:border-white/[0.12]' : 'border-zinc-200/60 hover:border-zinc-300'}">
            <div class="text-center">
              <p class="text-xl {textMuted}">+</p>
              <p class="mt-1 text-[11px] {textMuted}">Add from bench</p>
            </div>
          </button>
        </div>
      </div>
    {/if}
  </section>

  <!-- THE BENCH — Available roles -->
  <section class="rounded-2xl border p-5 {sectionBg}">
    <div class="mb-3 flex items-center justify-between">
      <div>
        <p class="text-[10px] font-bold uppercase tracking-[0.2em] {textMuted}">The Bench</p>
        <p class="mt-1 text-[12px] {textSecondary}">{availableRoles.length} roles available — click to add</p>
      </div>
      <div class="flex gap-1.5">
        {#each ['All', 'Claude', 'Codex', 'Antigravity'] as filter}
          <button class="rounded-lg border px-2.5 py-1 text-[10px] font-semibold transition {filter === 'All' ? (dark ? 'bg-white/[0.08] border-white/[0.12] text-white' : 'bg-brand-50 border-brand-200 text-brand-700') : `${cardBg} ${textMuted}`}">
            {filter}
          </button>
        {/each}
      </div>
    </div>

    <div class="grid grid-cols-2 gap-2">
      {#each availableRoles as role}
        <button class="flex items-center gap-3 rounded-xl border p-3 text-left transition {cardBg} {cardBgHover}">
          <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border {toolBadgeBg(role.tool)}">
            <span class="text-xs font-bold">{toolInitial(role.tool)}</span>
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <p class="truncate text-[12px] font-semibold {textPrimary}">{role.name}</p>
              <span class="rounded-full border px-1.5 py-0.5 text-[8px] font-bold uppercase {role.kind === 'lead' ? (dark ? 'border-brand-500/20 text-brand-400' : 'border-brand-200 text-brand-600') : (dark ? 'border-white/[0.08] text-zinc-500' : 'border-zinc-200 text-zinc-400')}">{role.kind}</span>
            </div>
            <p class="mt-0.5 truncate text-[11px] {textSecondary}">{role.summary}</p>
          </div>
          <span class="shrink-0 text-lg {textMuted}">+</span>
        </button>
      {/each}
    </div>
  </section>
</div>
