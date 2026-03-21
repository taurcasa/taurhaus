<!--
  Design B: "Spotlight"
  Conference speaker / team page feel — each member gets a prominent portrait card.
  Empty slots are invitation cards. Role picker is an inline search bar at top.
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
  const allMembers = $derived(team ? [team.lead, ...team.agents] : [])
  const availableRoles = $derived(
    MOCK_ROLES.filter(r => {
      if (!team) return true
      if (team.lead?.id === r.id) return false
      return !team.agents.some(a => a.id === r.id)
    })
  )

  const bg = $derived(dark ? 'bg-[#090e11]' : 'bg-[#f8fcfb]')
  const textPrimary = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const textMuted = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')

  function toolGradient(tool) {
    if (tool === 'claude') return dark
      ? 'bg-gradient-to-br from-brand-900/60 to-brand-950/80 border-brand-700/30'
      : 'bg-gradient-to-br from-brand-50 to-brand-100/80 border-brand-200/60'
    if (tool === 'codex') return dark
      ? 'bg-gradient-to-br from-emerald-900/50 to-emerald-950/70 border-emerald-700/25'
      : 'bg-gradient-to-br from-emerald-50 to-emerald-100/80 border-emerald-200/60'
    if (tool === 'gemini') return dark
      ? 'bg-gradient-to-br from-blue-900/50 to-blue-950/70 border-blue-700/25'
      : 'bg-gradient-to-br from-blue-50 to-blue-100/80 border-blue-200/60'
    return dark ? 'bg-white/[0.03] border-white/[0.08]' : 'bg-zinc-50 border-zinc-200'
  }

  function toolAccent(tool) {
    if (tool === 'claude') return dark ? 'text-brand-400' : 'text-brand-600'
    if (tool === 'codex') return dark ? 'text-emerald-400' : 'text-emerald-600'
    if (tool === 'gemini') return dark ? 'text-blue-400' : 'text-blue-600'
    return textMuted
  }

  function toolBadge(tool) {
    if (tool === 'claude') return dark ? 'bg-brand-500/15 text-brand-300' : 'bg-brand-100 text-brand-700'
    if (tool === 'codex') return dark ? 'bg-emerald-500/15 text-emerald-300' : 'bg-emerald-100 text-emerald-700'
    if (tool === 'gemini') return dark ? 'bg-blue-500/15 text-blue-300' : 'bg-blue-100 text-blue-700'
    return ''
  }

  function toolLetter(tool) {
    return { claude: 'C', codex: 'X', gemini: 'G' }[tool] ?? '?'
  }
</script>

<div class="{bg} rounded-2xl p-6 min-h-[600px] font-sans">
  <!-- Header with inline search -->
  <div class="mb-6 flex items-start justify-between gap-4">
    <div class="flex-1">
      <p class="text-[10px] font-bold uppercase tracking-[0.2em] {textMuted}">Team Roster</p>
      {#if team}
        <h1 class="mt-1 text-2xl font-bold {textPrimary}">{team.name}</h1>
        <p class="mt-1 text-sm {textSecondary}">{team.description}</p>
      {:else}
        <h1 class="mt-1 text-2xl font-bold {textPrimary}">Build Your Team</h1>
        <p class="mt-1 text-sm {textSecondary}">Choose a starting lineup or add members one by one.</p>
      {/if}
    </div>
    <div class="flex items-center gap-2">
      {#if state === 'ready'}
        <button class="h-10 rounded-xl bg-brand-600 px-5 text-sm font-semibold text-white shadow-md transition hover:bg-brand-500">
          Initialize Team
        </button>
      {/if}
    </div>
  </div>

  <!-- Quick start presets -->
  {#if state === 'empty'}
    <div class="mb-6 grid grid-cols-4 gap-3">
      {#each MOCK_PRESETS as preset}
        <button class="rounded-2xl border p-4 text-left transition {dark ? 'border-white/[0.06] bg-white/[0.02] hover:bg-white/[0.05] hover:border-brand-500/30' : 'border-zinc-200/60 bg-white hover:bg-brand-50/40 hover:border-brand-300/40'}">
          <p class="text-[13px] font-bold {textPrimary}">{preset.name}</p>
          <p class="mt-1 text-[11px] {textSecondary}">{preset.leads} lead + {preset.agents} agents</p>
          <div class="mt-2 flex gap-1">
            {#each preset.tools as tool}
              <span class="rounded-md px-1.5 py-0.5 text-[9px] font-bold uppercase {toolBadge(tool)}">{tool}</span>
            {/each}
          </div>
        </button>
      {/each}
    </div>
  {/if}

  <!-- Member cards grid -->
  <div class="grid gap-4 {state === 'ready' ? 'grid-cols-3' : 'grid-cols-3'}">
    {#if team}
      <!-- Lead — larger card spanning first position -->
      <div class="relative overflow-hidden rounded-3xl border p-5 {toolGradient(team.lead.tool)}">
        <div class="absolute right-3 top-3">
          <span class="rounded-full px-2.5 py-1 text-[9px] font-bold uppercase tracking-wider {dark ? 'bg-brand-500/20 text-brand-300' : 'bg-brand-100 text-brand-700'}">Lead</span>
        </div>
        <div class="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl border-2 {dark ? 'border-white/10 bg-white/[0.06]' : 'border-brand-200/50 bg-white/70'}">
          <span class="text-3xl font-bold {toolAccent(team.lead.tool)}">{toolLetter(team.lead.tool)}</span>
        </div>
        <h3 class="text-[16px] font-bold {textPrimary}">{team.lead.name}</h3>
        <p class="mt-1 text-[11px] font-medium uppercase tracking-wide {toolAccent(team.lead.tool)}">{team.lead.tool} · {team.lead.model}</p>
        <p class="mt-3 text-[12px] leading-relaxed {textSecondary}">{team.lead.summary}</p>
        <div class="mt-4 flex items-center gap-2">
          <span class="rounded-lg border px-2 py-1 text-[10px] {dark ? 'border-white/10 text-zinc-400' : 'border-zinc-200 text-zinc-500'}">this project</span>
        </div>
      </div>

      <!-- Agent cards -->
      {#each team.agents as agent}
        <div class="overflow-hidden rounded-3xl border p-5 transition {toolGradient(agent.tool)}">
          <div class="mb-4 flex h-12 w-12 items-center justify-center rounded-xl border {dark ? 'border-white/10 bg-white/[0.06]' : 'border-zinc-200/40 bg-white/70'}">
            <span class="text-xl font-bold {toolAccent(agent.tool)}">{toolLetter(agent.tool)}</span>
          </div>
          <h3 class="text-[14px] font-bold {textPrimary}">{agent.name}</h3>
          <p class="mt-1 text-[11px] font-medium uppercase tracking-wide {toolAccent(agent.tool)}">{agent.tool} · {agent.model}</p>
          <p class="mt-3 text-[12px] leading-relaxed {textSecondary}">{agent.summary}</p>
          <div class="mt-4 flex items-center gap-2">
            <span class="rounded-lg border px-2 py-1 text-[10px] {dark ? 'border-white/10 text-zinc-400' : 'border-zinc-200 text-zinc-500'}">this project</span>
          </div>
        </div>
      {/each}
    {/if}

    <!-- Empty invitation cards -->
    {#if !team}
      {#each [{ label: 'Lead', icon: '★', prominent: true }, { label: 'Agent 1', icon: '+' }, { label: 'Agent 2', icon: '+' }, { label: 'Agent 3', icon: '+' }, { label: 'Agent 4', icon: '+' }] as slot}
        <button class="flex flex-col items-center justify-center rounded-3xl border-2 border-dashed py-12 transition {slot.prominent ? (dark ? 'border-brand-500/25 hover:border-brand-500/40 hover:bg-brand-500/5' : 'border-brand-300/40 hover:border-brand-400/60 hover:bg-brand-50/60') : (dark ? 'border-white/[0.06] hover:border-white/[0.12] hover:bg-white/[0.02]' : 'border-zinc-200/60 hover:border-zinc-300 hover:bg-zinc-50')}">
          <span class="text-3xl {slot.prominent ? (dark ? 'text-brand-500' : 'text-brand-400') : textMuted}">{slot.icon}</span>
          <p class="mt-2 text-[12px] font-semibold {slot.prominent ? (dark ? 'text-brand-400' : 'text-brand-600') : textMuted}">{slot.label}</p>
        </button>
      {/each}
    {:else}
      <!-- One more add card -->
      <button class="flex flex-col items-center justify-center rounded-3xl border-2 border-dashed py-12 transition {dark ? 'border-white/[0.06] hover:border-white/[0.12] hover:bg-white/[0.02]' : 'border-zinc-200/60 hover:border-zinc-300 hover:bg-zinc-50'}">
        <span class="text-3xl {textMuted}">+</span>
        <p class="mt-2 text-[12px] font-semibold {textMuted}">Add member</p>
      </button>
    {/if}
  </div>

  <!-- Role picker bar (always visible) -->
  <div class="mt-6 rounded-2xl border p-4 {dark ? 'border-white/[0.06] bg-white/[0.02]' : 'border-zinc-200/60 bg-zinc-50/80'}">
    <div class="mb-3 flex items-center gap-3">
      <div class="flex-1 rounded-xl border px-3 py-2.5 text-sm {dark ? 'border-white/[0.08] bg-black/20 text-zinc-400' : 'border-zinc-200 bg-white text-zinc-400'}">
        Search roles...
      </div>
      <div class="flex gap-1.5">
        {#each ['All', 'Leads', 'Agents'] as filter}
          <button class="rounded-lg px-2.5 py-1.5 text-[10px] font-bold uppercase tracking-wider transition {filter === 'All' ? (dark ? 'bg-white/[0.08] text-white' : 'bg-brand-100 text-brand-700') : (dark ? 'text-zinc-500 hover:text-zinc-300' : 'text-zinc-400 hover:text-zinc-600')}">
            {filter}
          </button>
        {/each}
      </div>
    </div>
    <div class="flex flex-wrap gap-2">
      {#each availableRoles.slice(0, 6) as role}
        <button class="inline-flex items-center gap-2 rounded-xl border px-3 py-2 text-left transition {dark ? 'border-white/[0.06] bg-white/[0.03] hover:bg-white/[0.06]' : 'border-zinc-200 bg-white hover:bg-zinc-50'}">
          <span class="flex h-6 w-6 items-center justify-center rounded-lg text-[10px] font-bold {toolBadge(role.tool)}">{toolLetter(role.tool)}</span>
          <span class="text-[12px] font-semibold {textPrimary}">{role.name}</span>
          <span class="text-[10px] {textMuted}">{role.kind}</span>
        </button>
      {/each}
      {#if availableRoles.length > 6}
        <span class="inline-flex items-center px-2 text-[11px] {textMuted}">+{availableRoles.length - 6} more</span>
      {/if}
    </div>
  </div>
</div>
