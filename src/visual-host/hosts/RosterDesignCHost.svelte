<!--
  Design C: "The Split"
  Two-column layout reimagined — roles on left as compact visual picks,
  team on right as a growing lineup. But the left side is beautiful,
  not a settings panel. Roles are colorful tool-branded cards.
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
  const memberCount = $derived(team ? 1 + team.agents.length : 0)

  const bg = $derived(dark ? 'bg-[#090e11]' : 'bg-[#f8fcfb]')
  const textPrimary = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const textMuted = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const panelBg = $derived(dark ? 'bg-white/[0.02] border-white/[0.06]' : 'bg-white/80 border-zinc-200/60')
  const cardBg = $derived(dark ? 'bg-white/[0.04] border-white/[0.08]' : 'bg-white border-zinc-200/60')

  function toolGrad(tool) {
    if (tool === 'claude') return dark
      ? 'from-brand-900/40 to-transparent border-l-brand-500/50'
      : 'from-brand-50/80 to-transparent border-l-brand-400/50'
    if (tool === 'codex') return dark
      ? 'from-emerald-900/35 to-transparent border-l-emerald-500/50'
      : 'from-emerald-50/80 to-transparent border-l-emerald-400/50'
    if (tool === 'gemini') return dark
      ? 'from-blue-900/35 to-transparent border-l-blue-500/50'
      : 'from-blue-50/80 to-transparent border-l-blue-400/50'
    return ''
  }

  function toolAccent(tool) {
    if (tool === 'claude') return dark ? 'text-brand-400' : 'text-brand-600'
    if (tool === 'codex') return dark ? 'text-emerald-400' : 'text-emerald-600'
    if (tool === 'gemini') return dark ? 'text-blue-400' : 'text-blue-600'
    return textMuted
  }

  function toolBadge(tool) {
    if (tool === 'claude') return dark ? 'bg-brand-500/15 text-brand-300 border-brand-500/20' : 'bg-brand-50 text-brand-700 border-brand-200'
    if (tool === 'codex') return dark ? 'bg-emerald-500/15 text-emerald-300 border-emerald-500/20' : 'bg-emerald-50 text-emerald-700 border-emerald-200'
    if (tool === 'gemini') return dark ? 'bg-blue-500/15 text-blue-300 border-blue-500/20' : 'bg-blue-50 text-blue-700 border-blue-200'
    return ''
  }

  function toolLetter(tool) {
    return { claude: 'C', codex: 'X', gemini: 'G' }[tool] ?? '?'
  }
</script>

<div class="{bg} rounded-2xl min-h-[600px] font-sans flex gap-4 p-4">

  <!-- LEFT: Available Roles — always visible, beautifully styled -->
  <aside class="w-[280px] shrink-0 flex flex-col rounded-2xl border p-4 {panelBg}">
    <div class="mb-4">
      <p class="text-[10px] font-bold uppercase tracking-[0.2em] {textMuted}">Available Roles</p>
      <p class="mt-1 text-[11px] {textSecondary}">Click to add to your team</p>
    </div>

    <!-- Tool filter pills -->
    <div class="mb-3 flex gap-1.5">
      {#each [['all', 'All'], ['claude', 'C'], ['codex', 'X'], ['gemini', 'G']] as [id, label]}
        <button class="flex h-7 items-center justify-center rounded-lg px-2.5 text-[10px] font-bold transition {id === 'all' ? (dark ? 'bg-white/[0.1] text-white' : 'bg-brand-100 text-brand-700') : (dark ? 'bg-white/[0.04] text-zinc-500 hover:text-zinc-300' : 'bg-zinc-100 text-zinc-400 hover:text-zinc-600')}">
          {label}
        </button>
      {/each}
    </div>

    <!-- Search -->
    <div class="mb-3 rounded-xl border px-3 py-2 text-[12px] {dark ? 'border-white/[0.08] bg-black/20 text-zinc-500' : 'border-zinc-200 bg-white text-zinc-400'}">
      Search roles...
    </div>

    <!-- Role list -->
    <div class="flex-1 space-y-1.5 overflow-y-auto">
      {#each availableRoles as role}
        <button class="w-full rounded-xl border-l-[3px] bg-gradient-to-r p-2.5 text-left transition hover:scale-[1.01] {toolGrad(role.tool)}">
          <div class="flex items-center gap-2.5">
            <span class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border text-xs font-bold {toolBadge(role.tool)}">
              {toolLetter(role.tool)}
            </span>
            <div class="min-w-0 flex-1">
              <p class="truncate text-[12px] font-semibold {textPrimary}">{role.name}</p>
              <p class="truncate text-[10px] {toolAccent(role.tool)}">{role.tool} · {role.model}</p>
            </div>
            <span class="shrink-0 rounded-full border px-1.5 py-0.5 text-[8px] font-bold uppercase {role.kind === 'lead' ? (dark ? 'border-brand-500/20 text-brand-400' : 'border-brand-200 text-brand-600') : (dark ? 'border-white/[0.08] text-zinc-500' : 'border-zinc-200 text-zinc-400')}">{role.kind}</span>
          </div>
        </button>
      {/each}
    </div>

    <!-- Presets at bottom -->
    <div class="mt-4 border-t pt-3 {dark ? 'border-white/[0.06]' : 'border-zinc-200/60'}">
      <p class="mb-2 text-[10px] font-bold uppercase tracking-[0.2em] {textMuted}">Quick Start</p>
      <div class="grid grid-cols-2 gap-1.5">
        {#each MOCK_PRESETS as preset}
          <button class="rounded-lg border px-2 py-1.5 text-left transition {dark ? 'border-white/[0.06] bg-white/[0.02] hover:bg-white/[0.05]' : 'border-zinc-200 bg-white hover:bg-zinc-50'}">
            <p class="text-[11px] font-semibold {textPrimary}">{preset.name}</p>
            <p class="text-[9px] {textMuted}">{preset.leads + preset.agents} members</p>
          </button>
        {/each}
      </div>
    </div>
  </aside>

  <!-- RIGHT: Team lineup -->
  <main class="flex-1 flex flex-col rounded-2xl border p-5 {panelBg}">
    <!-- Team header -->
    <div class="mb-5">
      <div class="flex items-center justify-between">
        <div>
          <p class="text-[10px] font-bold uppercase tracking-[0.2em] {textMuted}">Your Team</p>
          {#if team}
            <h1 class="mt-1 text-xl font-bold {textPrimary}">{team.name}</h1>
            <p class="mt-0.5 text-[12px] {textSecondary}">{team.description}</p>
          {:else}
            <h1 class="mt-1 text-xl font-bold {textPrimary}">New Team</h1>
            <p class="mt-0.5 text-[12px] {textSecondary}">Pick roles from the left to build your lineup.</p>
          {/if}
        </div>
        <div class="flex items-center gap-3">
          {#if team}
            <div class="flex items-center gap-1.5 rounded-full border px-3 py-1 {dark ? 'border-white/[0.08]' : 'border-zinc-200'}">
              <span class="text-[12px] font-bold {textPrimary}">{memberCount}</span>
              <span class="text-[11px] {textMuted}">members</span>
            </div>
          {/if}
          <button class="h-9 rounded-xl px-4 text-[12px] font-bold transition {state === 'ready' ? 'bg-brand-600 text-white hover:bg-brand-500' : (dark ? 'bg-white/[0.06] text-zinc-400' : 'bg-zinc-100 text-zinc-400')} {state === 'ready' ? '' : 'cursor-default'}">
            {state === 'ready' ? 'Initialize' : 'Build first...'}
          </button>
        </div>
      </div>

      <!-- Progress indicator -->
      <div class="mt-4 flex gap-1">
        <div class="h-1 flex-1 rounded-full {team?.lead ? (dark ? 'bg-brand-500' : 'bg-brand-500') : (dark ? 'bg-white/[0.06]' : 'bg-zinc-200')}"></div>
        {#each [0, 1, 2, 3] as i}
          <div class="h-1 flex-1 rounded-full {team?.agents?.[i] ? (dark ? 'bg-brand-500/70' : 'bg-brand-400') : (dark ? 'bg-white/[0.06]' : 'bg-zinc-200')}"></div>
        {/each}
      </div>
    </div>

    <!-- Team members -->
    <div class="flex-1 space-y-3">
      {#if team}
        <!-- Lead -->
        <div class="rounded-2xl border-l-[3px] bg-gradient-to-r p-4 {toolGrad(team.lead.tool)} border {cardBg}">
          <div class="flex items-start gap-4">
            <div class="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl border {toolBadge(team.lead.tool)}">
              <span class="text-xl font-bold">{toolLetter(team.lead.tool)}</span>
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <span class="text-[9px] font-bold uppercase tracking-wider {dark ? 'text-brand-400' : 'text-brand-600'}">Lead</span>
              </div>
              <h3 class="mt-0.5 text-[15px] font-bold {textPrimary}">{team.lead.name}</h3>
              <p class="mt-1 text-[11px] {textSecondary}">{team.lead.summary}</p>
              <div class="mt-2 flex items-center gap-3 text-[10px] {textMuted}">
                <span class="font-medium uppercase {toolAccent(team.lead.tool)}">{team.lead.tool} · {team.lead.model}</span>
                <span>·</span>
                <span>this project</span>
              </div>
            </div>
          </div>
        </div>

        <!-- Agents -->
        {#each team.agents as agent, i}
          <div class="rounded-2xl border-l-[3px] bg-gradient-to-r p-3.5 {toolGrad(agent.tool)} border {cardBg}">
            <div class="flex items-center gap-3.5">
              <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border {toolBadge(agent.tool)}">
                <span class="text-sm font-bold">{toolLetter(agent.tool)}</span>
              </div>
              <div class="min-w-0 flex-1">
                <p class="text-[13px] font-bold {textPrimary}">{agent.name}</p>
                <p class="mt-0.5 text-[11px] {textSecondary}">{agent.summary}</p>
              </div>
              <span class="text-[10px] font-medium uppercase {toolAccent(agent.tool)}">{agent.tool} · {agent.model}</span>
            </div>
          </div>
        {/each}

      {:else}
        <!-- Empty state — visual invitation -->
        <div class="flex flex-1 flex-col items-center justify-center rounded-2xl border-2 border-dashed py-16 {dark ? 'border-white/[0.06]' : 'border-zinc-200/60'}">
          <div class="mb-4 flex gap-2">
            {#each ['claude', 'codex', 'gemini'] as tool}
              <span class="flex h-10 w-10 items-center justify-center rounded-xl border {toolBadge(tool)}">
                <span class="text-sm font-bold">{toolLetter(tool)}</span>
              </span>
            {/each}
          </div>
          <p class="text-[14px] font-semibold {textPrimary}">Your team starts here</p>
          <p class="mt-1 text-[12px] {textSecondary}">Pick a preset on the left, or click individual roles to build.</p>
        </div>
      {/if}
    </div>
  </main>
</div>
