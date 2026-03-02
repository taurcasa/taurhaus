<script>
  import { themeTokens } from '../themeTokens.js'

  let {
    dark = false,
    projectPath = '',
    availableProjects = [],
    preflightWarnings = [],
    oninitialize = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const selectScheme = $derived(dark ? '[color-scheme:dark]' : '[color-scheme:light]')

  const toolOptions = [
    { value: 'claude', label: 'Claude' },
    { value: 'codex', label: 'Codex' },
    { value: 'gemini', label: 'Gemini' },
  ]

  const modelOptionsByTool = {
    claude: ['opus', 'sonnet', 'haiku'],
    codex: ['gpt-5.3', 'gpt-5-mini'],
    gemini: ['gemini-2.5-pro', 'gemini-2.0-flash'],
  }

  function projectBasename(path) {
    return String(path || '').split(/[\\/]+/).filter(Boolean).at(-1) || 'project'
  }

  function inferTeamName(path) {
    return `${projectBasename(path)}-team`
  }

  function normalizeProjectOption(project) {
    if (typeof project === 'string') {
      return { id: project, label: projectBasename(project) }
    }
    if (project && typeof project === 'object') {
      const id = project.path || project.id || project.name || ''
      const label = project.name || projectBasename(project.path || project.id) || 'Unnamed'
      return { id, label }
    }
    return { id: '', label: '' }
  }

  const projectOptions = $derived(
    (availableProjects ?? []).map(normalizeProjectOption).filter((p) => p.id)
  )

  const projectName = $derived(projectBasename(projectPath))

  const warnings = $derived.by(() => {
    return (preflightWarnings ?? [])
      .map((w) => {
        if (typeof w === 'string') return { id: w, message: w }
        return { id: w?.agentName ?? w?.message ?? 'w', message: w?.message ?? String(w ?? '') }
      })
      .filter((w) => w.message)
  })

  const warningText = $derived.by(() => {
    if (warnings.length === 0) return ''
    if (warnings.length === 1) return softenWarningMessage(warnings[0].message)
    return 'Some tools may need installation. You can still start \u2014 agents will report issues.'
  })

  let nextAgentId = 1
  let teamName = $state('')
  let teamDescription = $state('')
  let onboardingDismissed = $state(false)
  let showCustomize = $state(false)

  const leadName = 'team-lead'
  const leadModel = 'opus'

  function defaultAgentProjectId() {
    const match = projectOptions.find((p) => p.id === projectPath)
    if (match) return match.id
    if (projectOptions.length === 1) return projectOptions[0].id
    return ''
  }

  function defaultAgent() {
    return {
      id: nextAgentId++,
      name: '',
      cliTool: 'codex',
      model: modelOptionsByTool.codex[0],
      projectId: defaultAgentProjectId(),
      description: '',
    }
  }

  let agents = $state([defaultAgent()])

  $effect(() => {
    if (!teamName.trim()) {
      teamName = inferTeamName(projectPath)
    }
  })

  $effect(() => {
    try {
      onboardingDismissed = localStorage.getItem('mesh-onboarding-dismissed') === 'true'
    } catch {}
  })

  function modelsForTool(tool) {
    return modelOptionsByTool[tool] ?? ['default']
  }

  function updateAgent(index, patch) {
    agents = agents.map((a, i) => (i !== index ? a : { ...a, ...patch }))
  }

  function updateAgentTool(index, tool) {
    updateAgent(index, { cliTool: tool, model: modelsForTool(tool)[0] ?? '' })
  }

  function addAgent() {
    agents = [...agents, defaultAgent()]
  }

  function removeAgent(index) {
    if (agents.length <= 1) return
    agents = agents.filter((_, i) => i !== index)
  }

  function softenWarningMessage(message) {
    const normalized = String(message || '').trim()
    if (!normalized) return 'Some setup checks need attention.'
    const friendly = {
      mesh_daemon_not_running: 'Mesh daemon is not running. Agents may need one extra retry.',
      mesh_binary_not_found: 'Mesh CLI is not installed. Install it before starting.',
      tmux_missing: 'tmux is unavailable. Install tmux to run team sessions.',
    }
    const key = normalized.toLowerCase()
    if (friendly[key]) return friendly[key]
    return normalized
      .replace(/[_-]+/g, ' ')
      .replace(/\b[A-Z][A-Z0-9_/-]{2,}\b/g, (tok) => tok.toLowerCase())
      .replace(/\s+/g, ' ')
      .replace(/^./, (c) => c.toUpperCase())
      .trim()
  }

  function dismissOnboarding() {
    onboardingDismissed = true
    try {
      localStorage.setItem('mesh-onboarding-dismissed', 'true')
    } catch {}
  }

  const duplicateNames = $derived.by(() => {
    const counts = new Map()
    const names = [leadName, ...agents.map((a) => a.name)]
      .map((n) => n.trim().toLowerCase())
      .filter(Boolean)
    for (const name of names) {
      counts.set(name, (counts.get(name) ?? 0) + 1)
    }
    return new Set([...counts.entries()].filter(([, count]) => count > 1).map(([name]) => name))
  })

  const hasDuplicateNames = $derived(duplicateNames.size > 0)

  function agentDisplayName(agent, index) {
    return agent.name.trim() || `${projectName}-dev${agents.length > 1 ? `-${index + 1}` : ''}`
  }

  function startTeam() {
    oninitialize({
      teamName: teamName.trim() || inferTeamName(projectPath),
      teamDescription: teamDescription.trim() || null,
      leadMode: 'launch_new',
      lead: {
        name: leadName,
        cliTool: 'claude',
        model: leadModel,
        projectId: projectPath,
        description: 'Team lead',
      },
      agents: agents.map((agent, i) => ({
        name: agentDisplayName(agent, i),
        cliTool: agent.cliTool,
        model: agent.model,
        projectId: agent.projectId || projectPath,
        description: agent.description.trim() || null,
      })),
    })
  }

  // Inline select style — custom chevron, pill-like appearance
  const chevronSvg = $derived(
    dark
      ? `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 10 10'%3E%3Cpath d='M3 4l2 2 2-2' fill='none' stroke='%2371717a' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")`
      : `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 10 10'%3E%3Cpath d='M3 4l2 2 2-2' fill='none' stroke='%2352525b' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")`
  )
  const inlineSelect = $derived(
    dark
      ? `appearance-none bg-zinc-800/80 text-xs text-zinc-300 rounded px-1.5 py-1 pr-4 border-none focus:ring-1 focus:ring-brand-500 focus:outline-none ${selectScheme} cursor-pointer`
      : `appearance-none bg-zinc-200/80 text-xs text-zinc-700 rounded px-1.5 py-1 pr-4 border-none focus:ring-1 focus:ring-brand-500 focus:outline-none ${selectScheme} cursor-pointer`
  )
</script>

<section class="space-y-3" data-testid="mesh-setup-form">
  <header class="space-y-0.5">
    <h2 class="text-sm font-semibold {t.textPrimary}" data-testid="mesh-setup-title">
      Start a team
    </h2>
    <p class="text-xs {t.textMuted}" data-testid="mesh-setup-description">
      Launch AI agents to work on <span class="font-medium {t.textSecondary}">{projectName}</span>
    </p>
  </header>

  {#if !onboardingDismissed}
    <div
      class="relative flex items-start gap-2 rounded-md px-3 py-2 text-[11px] leading-relaxed {dark ? 'bg-white/[0.03] text-zinc-500' : 'bg-zinc-50 text-zinc-500 border border-zinc-200'}"
      data-testid="mesh-onboarding-banner"
    >
      <span class="shrink-0 mt-px {dark ? 'text-brand-400/70' : 'text-brand-600'}">ℹ</span>
      <p>Mesh coordinates multiple AI agents on your project. Each agent runs in its own terminal session.</p>
      <button
        class="shrink-0 ml-auto p-1 rounded {dark ? 'text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.04]' : 'text-zinc-400 hover:text-zinc-600 hover:bg-zinc-100'} transition-colors"
        type="button"
        onclick={dismissOnboarding}
        aria-label="Dismiss onboarding"
        data-testid="mesh-onboarding-dismiss"
      >
        <svg class="h-3.5 w-3.5" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
        </svg>
      </button>
    </div>
  {/if}

  <div
    class="rounded-lg border {dark ? 'border-zinc-700/60 bg-white/[0.02]' : 'border-zinc-300 bg-white shadow-sm'}"
    data-testid="mesh-roster-preview"
  >
    <div class="px-3 py-1.5 border-b {dark ? 'border-zinc-700/40' : 'border-zinc-200'}">
      <span class="text-xs font-medium {dark ? 'text-zinc-400' : 'text-zinc-600'}">
        Your team
      </span>
    </div>

    <div class="px-3 py-1">
      <div
        class="flex items-center gap-2 py-2.5 {dark ? 'hover:bg-white/[0.02]' : 'hover:bg-zinc-50/50'} transition-colors rounded-sm"
      >
        <svg class="h-3.5 w-3.5 text-brand-500 shrink-0" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path d="m8 2 1.6 3.3 3.6.5-2.6 2.5.6 3.5L8 10.1 4.8 11.8l.6-3.5L2.8 5.8l3.6-.5L8 2Z" fill="currentColor" />
        </svg>
        <span class="text-xs font-medium {t.textPrimary}">You</span>
        <span class="text-xs {dark ? 'text-zinc-400' : 'text-zinc-600'}">Claude · {leadModel}</span>
        <span
          class="ml-auto text-[10px] font-medium px-1.5 py-0.5 rounded {dark ? 'bg-white/[0.06] text-zinc-400' : 'bg-zinc-100 text-zinc-500'}"
        >Lead</span>
      </div>

      {#each agents as agent, index (agent.id)}
        <div
          class="flex items-center gap-2 py-2.5 border-t {dark ? 'border-zinc-700/30 hover:bg-white/[0.02]' : 'border-zinc-200/60 hover:bg-zinc-50/50'} transition-colors rounded-sm"
          data-testid="mesh-agent-card"
        >
          <svg class="h-3.5 w-3.5 text-zinc-400 shrink-0" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <circle cx="8" cy="8" r="4.5" stroke="currentColor" stroke-width="1.2" />
            <circle cx="8" cy="8" r="1.2" fill="currentColor" />
          </svg>
          <input
            class="w-28 {dark ? 'bg-zinc-800/50 placeholder:text-zinc-500' : 'bg-zinc-100 placeholder:text-zinc-400'} text-xs font-medium {t.textPrimary} rounded px-1.5 py-1 border-none focus:ring-1 focus:ring-brand-500 focus:outline-none"
            value={agent.name}
            placeholder={agentDisplayName(agent, index)}
            oninput={(e) => updateAgent(index, { name: e.currentTarget.value })}
            data-testid={`mesh-agent-name-input-${index}`}
          />
          <select
            class={inlineSelect}
            style:background-image={chevronSvg}
            style:background-repeat="no-repeat"
            style:background-position="right 4px center"
            value={agent.cliTool}
            onchange={(e) => updateAgentTool(index, e.currentTarget.value)}
            data-testid={`mesh-agent-tool-select-${index}`}
          >
            {#each toolOptions as tool}
              <option value={tool.value}>{tool.label}</option>
            {/each}
          </select>
          <select
            class={inlineSelect}
            style:background-image={chevronSvg}
            style:background-repeat="no-repeat"
            style:background-position="right 4px center"
            value={agent.model}
            onchange={(e) => updateAgent(index, { model: e.currentTarget.value })}
            data-testid={`mesh-agent-model-select-${index}`}
          >
            {#each modelsForTool(agent.cliTool) as model}
              <option value={model}>{model}</option>
            {/each}
          </select>
          <select
            class={inlineSelect}
            style:background-image={chevronSvg}
            style:background-repeat="no-repeat"
            style:background-position="right 4px center"
            value={agent.projectId}
            onchange={(e) => updateAgent(index, { projectId: e.currentTarget.value })}
            data-testid={`mesh-agent-project-select-${index}`}
          >
            <option value="">Select project</option>
            {#each projectOptions as p}
              <option value={p.id}>{p.label}</option>
            {/each}
          </select>
          {#if agents.length > 1}
            <button
              class="shrink-0 ml-auto p-1 rounded {dark ? 'text-zinc-500 hover:text-danger-400 hover:bg-danger-500/10' : 'text-zinc-400 hover:text-danger-500 hover:bg-danger-50'} transition-colors"
              type="button"
              onclick={() => removeAgent(index)}
              aria-label={`Remove agent ${index + 1}`}
              data-testid={`mesh-agent-remove-button-${index}`}
            >
              <svg class="h-3.5 w-3.5" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              </svg>
            </button>
          {/if}
        </div>
      {/each}

      <div class="py-1.5 border-t {dark ? 'border-zinc-700/30' : 'border-zinc-200/60'}">
        <button
          class="text-xs rounded px-2 py-1 -mx-1 {dark ? 'text-zinc-500 hover:text-brand-400 hover:bg-white/[0.03]' : 'text-zinc-400 hover:text-brand-600 hover:bg-zinc-50'} transition-colors"
          type="button"
          onclick={addAgent}
          data-testid="mesh-add-agent-button"
        >+ Add agent</button>
      </div>
    </div>
  </div>

  {#if hasDuplicateNames}
    <p class="text-xs text-danger-500" data-testid="mesh-duplicate-name-error">
      Duplicate member names. Each name must be unique.
    </p>
  {/if}

  {#if warningText}
    <p
      class="text-xs {dark ? 'text-warning-300/70' : 'text-warning-700/70'}"
      data-testid="mesh-setup-warnings"
    >⚠ {warningText}</p>
  {/if}

  <div class="space-y-2.5">
    {#if showCustomize}
      <div class="space-y-2" data-testid="mesh-team-basics">
        <div class="flex items-center gap-3">
          <span class="text-xs {t.textMuted} w-20 shrink-0">Team name</span>
          <input
            class="flex-1 text-xs {t.textPrimary} {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} rounded px-1.5 py-1 border-none focus:ring-1 focus:ring-brand-500 focus:outline-none"
            bind:value={teamName}
            data-testid="mesh-team-name-input"
          />
          <button
            class="text-xs rounded px-2 py-1 border {dark ? 'border-zinc-700/60 text-brand-400 hover:bg-white/[0.03]' : 'border-zinc-300 text-brand-600 hover:bg-zinc-50'} transition-colors shrink-0"
            type="button"
            onclick={() => { showCustomize = false }}
            data-testid="mesh-advanced-toggle"
          >Done</button>
        </div>
        <div class="flex items-center gap-3">
          <span class="text-xs {t.textMuted} w-20 shrink-0">Description</span>
          <input
            class="flex-1 text-xs {t.textPrimary} {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} rounded px-1.5 py-1 border-none focus:ring-1 focus:ring-brand-500 focus:outline-none"
            placeholder="Optional — describe the team's purpose"
            bind:value={teamDescription}
            data-testid="mesh-team-description-input"
          />
        </div>
      </div>
    {:else}
      <div class="flex items-center gap-2">
        <span class="text-[11px] {dark ? t.textMuted : 'text-zinc-600'}">{teamName || inferTeamName(projectPath)}</span>
        <button
          class="text-xs rounded px-2 py-1 border {dark ? 'border-zinc-700/60 text-brand-400 hover:bg-white/[0.03]' : 'border-zinc-300 text-brand-600 hover:bg-zinc-50'} transition-colors"
          type="button"
          onclick={() => { showCustomize = true }}
          data-testid="mesh-advanced-toggle"
        >Customize…</button>
      </div>
    {/if}

    <div class="flex justify-end">
      <button
        class="h-8 inline-flex items-center rounded-md bg-brand-600 px-4 text-xs font-medium text-white hover:bg-brand-500 transition-colors"
        type="button"
        onclick={startTeam}
        data-testid="mesh-create-team-button"
      >Start Team</button>
    </div>
  </div>
</section>
