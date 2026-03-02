<script>
  let {
    dark = false,
    projectPath = '',
    availableProjects = [],
    preflightWarnings = [],
    oninitialize = () => {},
  } = $props()

  const keyline = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const textPrimary = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textMuted = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const inputBg = $derived(
    dark
      ? 'bg-zinc-900/70 border-zinc-800 text-zinc-100 placeholder:text-zinc-600'
      : 'bg-white border-zinc-300 text-zinc-900 placeholder:text-zinc-400'
  )
  const subtleButton = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:border-zinc-600 hover:text-zinc-200'
      : 'border-zinc-300 text-zinc-700 hover:border-zinc-400 hover:text-zinc-900'
  )

  const toolOptions = [
    { value: 'claude', label: 'Claude', icon: 'C' },
    { value: 'codex', label: 'Codex', icon: 'CX' },
    { value: 'gemini', label: 'Gemini', icon: 'G' },
  ]

  const modelOptionsByTool = {
    claude: ['opus', 'sonnet', 'haiku'],
    codex: ['gpt-5.3', 'gpt-5-mini'],
    gemini: ['gemini-2.5-pro', 'gemini-2.0-flash'],
  }

  function inferTeamName(path) {
    const name = String(path || '').split('/').filter(Boolean).at(-1) || 'project'
    return `${name}-team`
  }

  function normalizeProjectOption(project) {
    if (typeof project === 'string') {
      return { id: project, label: project }
    }
    if (project && typeof project === 'object') {
      const id = project.id || project.path || project.name || ''
      const label = project.name || project.path || project.id || 'Unnamed project'
      return { id, label }
    }
    return { id: '', label: '' }
  }

  const projectOptions = $derived(
    (availableProjects ?? []).map(normalizeProjectOption).filter((project) => project.id)
  )

  const warnings = $derived.by(() => {
    return (preflightWarnings ?? [])
      .map((warning) => {
        if (typeof warning === 'string') {
          return { id: warning, tool: '', message: warning }
        }
        return {
          id: warning?.agentName ?? warning?.agent_name ?? warning?.message ?? 'warning',
          tool: warning?.cliTool ?? warning?.cli_tool ?? '',
          message: warning?.message ?? String(warning ?? ''),
        }
      })
      .filter((warning) => warning.message)
  })

  let nextAgentId = 1
  let teamName = $state('')
  let teamDescription = $state('')
  let leadName = $state('team-lead')
  let leadModel = $state('opus')
  let leadSessionMode = $state('use_current')

  function blankAgent() {
    return {
      id: nextAgentId++,
      name: '',
      cliTool: 'codex',
      model: modelOptionsByTool.codex[0],
      projectId: projectOptions[0]?.id ?? '',
      description: '',
    }
  }

  let agents = $state([blankAgent()])

  $effect(() => {
    if (!teamName.trim()) {
      teamName = inferTeamName(projectPath)
    }
  })

  function modelsForTool(tool) {
    return modelOptionsByTool[tool] ?? ['default']
  }

  function updateAgent(index, patch) {
    agents = agents.map((agent, agentIndex) => {
      if (agentIndex !== index) return agent
      return { ...agent, ...patch }
    })
  }

  function updateAgentTool(index, tool) {
    const models = modelsForTool(tool)
    updateAgent(index, { cliTool: tool, model: models[0] ?? '' })
  }

  function addAgent(prefill = {}) {
    const defaults = blankAgent()
    agents = [
      ...agents,
      {
        ...defaults,
        ...prefill,
        model: prefill.cliTool ? modelsForTool(prefill.cliTool)[0] : defaults.model,
      },
    ]
  }

  function removeAgent(index) {
    if (agents.length <= 1) return
    agents = agents.filter((_, agentIndex) => agentIndex !== index)
  }

  const quickRoles = [
    { label: 'Frontend', name: 'frontend-dev', description: 'Owns UI implementation' },
    { label: 'Backend', name: 'backend-dev', description: 'Owns API and services' },
    { label: 'QA', name: 'qa-engineer', description: 'Owns validation and test coverage' },
    { label: 'Docs', name: 'docs-writer', description: 'Owns documentation and handoff notes' },
  ]

  function quickAdd(role) {
    addAgent({
      name: role.name,
      description: role.description,
    })
  }

  const duplicateNames = $derived.by(() => {
    const counts = new Map()
    const names = [leadName, ...agents.map((agent) => agent.name)]
      .map((name) => name.trim().toLowerCase())
      .filter(Boolean)
    for (const name of names) {
      counts.set(name, (counts.get(name) ?? 0) + 1)
    }
    return new Set([...counts.entries()].filter(([, count]) => count > 1).map(([name]) => name))
  })

  const leadNameDuplicate = $derived(duplicateNames.has(leadName.trim().toLowerCase()))
  const hasDuplicateNames = $derived(duplicateNames.size > 0)
  const hasMissingRequired = $derived.by(() => {
    if (!teamName.trim() || !leadName.trim()) return true
    if (agents.length === 0) return true
    return agents.some(
      (agent) =>
        !agent.name.trim() || !agent.cliTool.trim() || !agent.model.trim() || !agent.projectId.trim()
    )
  })
  const canInitialize = $derived(!hasDuplicateNames && !hasMissingRequired)

  function submitInitialize() {
    if (!canInitialize) return
    const payload = {
      teamName: teamName.trim(),
      teamDescription: teamDescription.trim() || null,
      leadMode: leadSessionMode === 'launch_new' ? 'launch_new' : 'attach_existing',
      lead: {
        name: leadName.trim(),
        cliTool: 'claude',
        model: leadModel,
        projectId: projectPath,
        description: 'Team lead',
      },
      agents: agents.map((agent) => ({
        name: agent.name.trim(),
        cliTool: agent.cliTool,
        model: agent.model,
        projectId: agent.projectId,
        description: agent.description.trim() || null,
      })),
    }
    oninitialize(payload)
  }
</script>

<section class="space-y-4" data-testid="mesh-setup-form">
  <header class="space-y-1 pb-2 border-b {keyline}">
    <h2 class="text-base font-semibold {textPrimary}" data-testid="mesh-setup-title">Mesh Team Setup</h2>
    <p class="text-sm {textMuted}" data-testid="mesh-setup-description">
      Define agents, assign projects and tools, initialize once, then coordinate in CLI.
    </p>
  </header>

  {#if warnings.length > 0}
    <div class="space-y-1.5" data-testid="mesh-setup-warnings">
      {#each warnings as warning (warning.id)}
        <p class="text-xs text-warning-600">
          {#if warning.tool}
            <span class="uppercase tracking-[0.06em] text-[10px] mr-1">{warning.tool}</span>
          {/if}
          {warning.message}
        </p>
      {/each}
    </div>
  {/if}

  <div class="space-y-3 pt-1" data-testid="mesh-team-basics">
    <h3 class="text-[11px] font-semibold uppercase tracking-[0.06em] {textMuted}">Team Basics</h3>
    <div class="grid grid-cols-1 gap-2">
      <label class="space-y-1 text-xs {textMuted}">
        <span>Team name</span>
        <input
          class="w-full rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
          bind:value={teamName}
          data-testid="mesh-team-name-input"
        />
      </label>
      <label class="space-y-1 text-xs {textMuted}">
        <span>Team description</span>
        <textarea
          class="w-full min-h-[68px] rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
          bind:value={teamDescription}
          data-testid="mesh-team-description-input"
        ></textarea>
      </label>
    </div>
  </div>

  <div class="pt-3 border-t {keyline} space-y-3" data-testid="mesh-lead-card">
    <h3 class="text-[11px] font-semibold uppercase tracking-[0.06em] {textMuted}">Team Lead</h3>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-2">
      <label class="space-y-1 text-xs {textMuted}">
        <span>Name</span>
        <input
          class="w-full rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
          bind:value={leadName}
          data-testid="mesh-lead-name-input"
        />
      </label>

      <div class="space-y-1 text-xs {textMuted}">
        <span>CLI tool</span>
        <div
          class="w-full rounded-md border px-2.5 py-1.5 text-sm font-medium {inputBg}"
          data-testid="mesh-lead-tool-fixed"
        >
          Claude
        </div>
      </div>
    </div>

    <label class="block space-y-1 text-xs {textMuted}">
      <span>Model</span>
      <select
        class="w-full rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
        bind:value={leadModel}
        data-testid="mesh-lead-model-select"
      >
        {#each modelOptionsByTool.claude as model}
          <option value={model}>{model}</option>
        {/each}
      </select>
    </label>

    <fieldset class="space-y-1.5 text-xs {textMuted}">
      <legend class="mb-0.5">Session mode</legend>
      <div class="flex items-center gap-4">
        <label class="inline-flex items-center gap-2">
          <input
            type="radio"
            name="lead-session-mode"
            value="use_current"
            bind:group={leadSessionMode}
            data-testid="mesh-lead-session-use-current"
          />
          Use current session
        </label>
        <label class="inline-flex items-center gap-2">
          <input
            type="radio"
            name="lead-session-mode"
            value="launch_new"
            bind:group={leadSessionMode}
            data-testid="mesh-lead-session-launch-new"
          />
          Launch new session
        </label>
      </div>
    </fieldset>

    {#if !leadName.trim()}
      <p class="text-xs text-danger-500" data-testid="mesh-lead-required-error">Lead name is required.</p>
    {/if}
    {#if leadNameDuplicate}
      <p class="text-xs text-danger-500" data-testid="mesh-lead-duplicate-error">
        Lead name must be unique across the team.
      </p>
    {/if}
  </div>

  <div class="pt-3 border-t {keyline} space-y-3" data-testid="mesh-agent-section">
    <div class="flex items-center justify-between gap-2">
      <h3 class="text-[11px] font-semibold uppercase tracking-[0.06em] {textMuted}">Agents</h3>
      <button
        class="rounded-md border border-brand-500/50 px-2.5 py-1 text-xs text-brand-500 hover:border-brand-500 hover:text-brand-400"
        type="button"
        onclick={() => addAgent()}
        data-testid="mesh-add-agent-button"
      >
        Add Agent
      </button>
    </div>

    <div class="flex flex-wrap gap-1.5" data-testid="mesh-quick-add-roles">
      {#each quickRoles as role}
        <button
          type="button"
          class="rounded-md border px-2 py-1 text-xs {subtleButton}"
          onclick={() => quickAdd(role)}
          data-testid={`mesh-quick-add-${role.label.toLowerCase()}`}
        >
          {role.label}
        </button>
      {/each}
    </div>

    {#if hasDuplicateNames}
      <p class="text-xs text-danger-500" data-testid="mesh-duplicate-name-error">
        Duplicate member names detected. Each lead/agent name must be unique.
      </p>
    {/if}

    <div class="divide-y {keyline} border-y {keyline}">
      {#each agents as agent, index (agent.id)}
        <article class="py-3 space-y-2" data-testid="mesh-agent-card">
          <div class="flex items-center justify-between gap-2">
            <h4 class="text-xs font-semibold {textPrimary}">Agent {index + 1}</h4>
            <button
              type="button"
              class="rounded-md border border-danger-400/40 px-2 py-1 text-[11px] text-danger-500 hover:border-danger-500 disabled:opacity-50"
              onclick={() => removeAgent(index)}
              disabled={agents.length <= 1}
              data-testid={`mesh-agent-remove-button-${index}`}
            >
              Remove
            </button>
          </div>

          <div class="grid grid-cols-1 md:grid-cols-2 gap-2">
            <label class="space-y-1 text-xs {textMuted}">
              <span>Name</span>
              <input
                class="w-full rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
                value={agent.name}
                oninput={(event) => updateAgent(index, { name: event.currentTarget.value })}
                data-testid={`mesh-agent-name-input-${index}`}
              />
            </label>

            <label class="space-y-1 text-xs {textMuted}">
              <span>CLI tool</span>
              <select
                class="w-full rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
                value={agent.cliTool}
                onchange={(event) => updateAgentTool(index, event.currentTarget.value)}
                data-testid={`mesh-agent-tool-select-${index}`}
              >
                {#each toolOptions as tool}
                  <option value={tool.value}>{tool.icon} {tool.label}</option>
                {/each}
              </select>
            </label>

            <label class="space-y-1 text-xs {textMuted}">
              <span>Model</span>
              <select
                class="w-full rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
                value={agent.model}
                onchange={(event) => updateAgent(index, { model: event.currentTarget.value })}
                data-testid={`mesh-agent-model-select-${index}`}
              >
                {#each modelsForTool(agent.cliTool) as model}
                  <option value={model}>{model}</option>
                {/each}
              </select>
            </label>

            <label class="space-y-1 text-xs {textMuted}">
              <span>Target project</span>
              <select
                class="w-full rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
                value={agent.projectId}
                onchange={(event) => updateAgent(index, { projectId: event.currentTarget.value })}
                data-testid={`mesh-agent-project-select-${index}`}
              >
                <option value="">Select project</option>
                {#each projectOptions as project}
                  <option value={project.id}>{project.label}</option>
                {/each}
              </select>
            </label>
          </div>

          <label class="space-y-1 text-xs {textMuted}">
            <span>Description</span>
            <input
              class="w-full rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
              value={agent.description}
              oninput={(event) => updateAgent(index, { description: event.currentTarget.value })}
              data-testid={`mesh-agent-description-input-${index}`}
            />
          </label>
        </article>
      {/each}
    </div>
  </div>

  <div class="pt-3 border-t {keyline} space-y-1.5" data-testid="mesh-review-panel">
    <h3 class="text-[11px] font-semibold uppercase tracking-[0.06em] {textMuted}">Review</h3>
    <p class="text-xs {textMuted}">Team: <span class="{textPrimary} font-medium">{teamName || '—'}</span></p>
    <p class="text-xs {textMuted}">Lead: <span class="{textPrimary} font-medium">{leadName || '—'}</span> ({leadModel})</p>
    <p class="text-xs {textMuted}">Agents: <span class="{textPrimary} font-medium">{agents.length}</span></p>
  </div>

  <div class="flex justify-end">
    <button
      class="inline-flex items-center rounded-md bg-brand-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50"
      type="button"
      onclick={submitInitialize}
      disabled={!canInitialize}
      data-testid="mesh-create-team-button"
    >
      Initialize Team
    </button>
  </div>
</section>
