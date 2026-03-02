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
  const fieldTone = $derived(
    dark
      ? 'border-zinc-700/80 text-zinc-100 placeholder:text-zinc-600 focus:border-brand-500'
      : 'border-zinc-300 text-zinc-900 placeholder:text-zinc-400 focus:border-brand-500'
  )
  const fixedFieldTone = $derived(
    dark ? 'border-zinc-700/80 text-zinc-100' : 'border-zinc-300 text-zinc-900'
  )
  const quickAddButton = $derived(
    dark
      ? 'rounded-md border border-dashed border-zinc-700 px-2 py-1 text-[11px] text-zinc-400 hover:border-brand-500/60 hover:text-brand-300 hover:bg-brand-500/10'
      : 'rounded-md border border-dashed border-zinc-300 px-2 py-1 text-[11px] text-zinc-600 hover:border-brand-500/60 hover:text-brand-700 hover:bg-brand-500/10'
  )
  const actionBase = 'rounded-md px-2 py-1 text-[11px] transition-colors'
  const actionBrand = `${actionBase} text-brand-500 hover:text-brand-400 hover:bg-brand-500/10`
  const actionDanger = `${actionBase} text-danger-500/70 hover:text-danger-500 hover:bg-danger-500/10`
  const formFieldBase =
    'w-full bg-transparent border-b rounded-none px-1 py-1.5 text-sm transition-colors focus:outline-none'
  const primaryCta = 'h-8 inline-flex items-center rounded-md bg-brand-600 px-3 text-xs font-medium text-white hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50'

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
  let onboardingDismissed = $state(false)

  function defaultAgentProjectId() {
    if (projectOptions.length === 1) {
      return projectOptions[0].id
    }
    return ''
  }

  function blankAgent() {
    return {
      id: nextAgentId++,
      name: '',
      cliTool: 'codex',
      model: modelOptionsByTool.codex[0],
      projectId: defaultAgentProjectId(),
      description: '',
    }
  }

  let agents = $state([blankAgent()])

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
  const reviewAgents = $derived.by(() => {
    return agents
      .map((agent, index) => {
        const toolLabel =
          toolOptions.find((toolOption) => toolOption.value === agent.cliTool)?.label ?? agent.cliTool
        return `${agent.name.trim() || `agent-${index + 1}`} (${toolLabel})`
      })
      .join(', ')
  })
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

  function dismissOnboarding() {
    onboardingDismissed = true
    try {
      localStorage.setItem('mesh-onboarding-dismissed', 'true')
    } catch {}
  }

  function quickStart() {
    const projectName = String(projectPath || '')
      .split('/')
      .filter(Boolean)
      .at(-1) || 'project'
    const payload = {
      teamName: `${projectName}-team`,
      teamDescription: null,
      leadMode: 'attach_existing',
      lead: {
        name: 'team-lead',
        cliTool: 'claude',
        model: 'opus',
        projectId: projectPath,
        description: 'Team lead',
      },
      agents: [
        {
          name: `${projectName}-dev`,
          cliTool: 'codex',
          model: 'gpt-5.3',
          projectId: projectPath,
          description: 'Development agent',
        },
      ],
    }
    oninitialize(payload)
  }
</script>

<section class="space-y-4" data-testid="mesh-setup-form">
  <header class="space-y-1 pb-3 border-b {t.keyline}">
    <h2 class="text-sm font-semibold {t.textPrimary}" data-testid="mesh-setup-title">Mesh Team Setup</h2>
    <p class="text-xs {t.textMuted}" data-testid="mesh-setup-description">
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

  {#if !onboardingDismissed}
    <div
      class="relative rounded-md px-3 py-2.5 text-xs space-y-0.5 {dark ? 'bg-brand-500/10 text-brand-200' : 'bg-brand-50 text-brand-800'}"
      data-testid="mesh-onboarding-banner"
    >
      <button
        class="absolute top-2 right-2 text-[10px] opacity-60 hover:opacity-100"
        onclick={dismissOnboarding}
        data-testid="mesh-onboarding-dismiss"
      >
        ✕
      </button>
      <p class="font-medium">What is Mesh?</p>
      <p class="{dark ? 'text-brand-300/80' : 'text-brand-700/80'}">
        Mesh coordinates multiple AI agents across your projects. Define a team below, click
        Initialize, then each agent works in its own terminal session.
      </p>
    </div>
  {/if}

  <div class="space-y-3 pt-1" data-testid="mesh-team-basics">
    <div class="flex items-center gap-2">
      <span class="h-3 w-0.5 rounded-full bg-brand-500/80"></span>
      <h3 class="text-[11px] uppercase {t.textMuted}">Team Basics</h3>
    </div>
    <div class="grid grid-cols-1 gap-2">
      <label class="space-y-1 text-xs {t.textMuted}">
        <span>Team name</span>
        <input
          class="{formFieldBase} {fieldTone}"
          bind:value={teamName}
          data-testid="mesh-team-name-input"
        />
      </label>
      <label class="space-y-1 text-xs {t.textMuted}">
        <span>Team description</span>
        <input
          class="{formFieldBase} {fieldTone}"
          placeholder="Optional — describe the team's purpose"
          bind:value={teamDescription}
          data-testid="mesh-team-description-input"
        />
      </label>
    </div>
  </div>

  <div class="pt-4 space-y-3" data-testid="mesh-lead-card">
    <div class="flex items-center gap-2">
      <span class="h-3 w-0.5 rounded-full bg-brand-500/80"></span>
      <h3 class="text-[11px] uppercase {t.textMuted}">Team Lead</h3>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-2">
      <label class="space-y-1 text-xs {t.textMuted}">
        <span>Name</span>
        <input
          class="{formFieldBase} {fieldTone}"
          bind:value={leadName}
          data-testid="mesh-lead-name-input"
        />
      </label>

      <div class="space-y-1 text-xs {t.textMuted}">
        <span>CLI tool</span>
        <div
          class="w-full border-b rounded-none px-1 py-1.5 text-sm font-medium {fixedFieldTone}"
          data-testid="mesh-lead-tool-fixed"
        >
          Claude
        </div>
      </div>
    </div>

    <label class="block space-y-1 text-xs {t.textMuted}">
      <span>Model</span>
      <select
        class="{formFieldBase} {fieldTone} {selectScheme}"
        bind:value={leadModel}
        data-testid="mesh-lead-model-select"
      >
        {#each modelOptionsByTool.claude as model}
          <option value={model}>{model}</option>
        {/each}
      </select>
    </label>

    <fieldset class="space-y-1.5 text-xs {t.textMuted}">
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

  <div class="pt-4 space-y-3" data-testid="mesh-agent-section">
    <div class="flex items-center justify-between gap-2">
      <div class="flex items-center gap-2">
        <span class="h-3 w-0.5 rounded-full bg-brand-500/80"></span>
        <h3 class="text-[11px] uppercase {t.textMuted}">Agents</h3>
      </div>
      <button
        class={actionBrand}
        type="button"
        onclick={() => addAgent()}
        data-testid="mesh-add-agent-button"
      >
        + Agent
      </button>
    </div>

    <div class="flex flex-wrap items-center gap-1.5" data-testid="mesh-quick-add-roles">
      <span class="text-[11px] {t.textMuted}">Quick add:</span>
      {#each quickRoles as role}
        <button
          type="button"
          class={quickAddButton}
          title={role.description}
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

    <div class="space-y-1">
      {#each agents as agent, index (agent.id)}
        <article class="py-3 space-y-2.5 rounded-md -mx-2 px-2 {index > 0 ? `border-t ${t.keyline}` : ''} {dark ? 'hover:bg-zinc-900' : 'hover:bg-zinc-50'}" data-testid="mesh-agent-card">
          <div class="flex items-center justify-between gap-2">
            <span class="text-[11px] {t.textMuted}">Agent {index + 1}</span>
            <button
              type="button"
              class="{actionDanger} disabled:opacity-50"
              onclick={() => removeAgent(index)}
              disabled={agents.length <= 1}
              data-testid={`mesh-agent-remove-button-${index}`}
            >
              Remove
            </button>
          </div>

          <div class="grid grid-cols-1 md:grid-cols-2 gap-2">
            <label class="space-y-1 text-xs {t.textMuted}">
              <span>Name</span>
              <input
                class="{formFieldBase} {fieldTone}"
                value={agent.name}
                oninput={(event) => updateAgent(index, { name: event.currentTarget.value })}
                data-testid={`mesh-agent-name-input-${index}`}
              />
            </label>

            <label class="space-y-1 text-xs {t.textMuted}">
              <span>CLI tool</span>
              <select
                class="{formFieldBase} {fieldTone} {selectScheme}"
                value={agent.cliTool}
                onchange={(event) => updateAgentTool(index, event.currentTarget.value)}
                data-testid={`mesh-agent-tool-select-${index}`}
              >
                {#each toolOptions as tool}
                  <option value={tool.value}>{tool.label}</option>
                {/each}
              </select>
            </label>

            <label class="space-y-1 text-xs {t.textMuted}">
              <span>Model</span>
              <select
                class="{formFieldBase} {fieldTone} {selectScheme}"
                value={agent.model}
                onchange={(event) => updateAgent(index, { model: event.currentTarget.value })}
                data-testid={`mesh-agent-model-select-${index}`}
              >
                {#each modelsForTool(agent.cliTool) as model}
                  <option value={model}>{model}</option>
                {/each}
              </select>
            </label>

            <label class="space-y-1 text-xs {t.textMuted}">
              <span>Target project</span>
              <select
                class="{formFieldBase} {fieldTone} {selectScheme}"
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

          <label class="space-y-1 text-xs {t.textMuted}">
            <span>Description</span>
            <input
              class="{formFieldBase} {fieldTone}"
              value={agent.description}
              oninput={(event) => updateAgent(index, { description: event.currentTarget.value })}
              data-testid={`mesh-agent-description-input-${index}`}
            />
          </label>
        </article>
      {/each}
    </div>
  </div>

  <div class="pt-4 space-y-1.5" data-testid="mesh-review-panel">
    <div class="flex items-center gap-2">
      <span class="h-3 w-0.5 rounded-full bg-brand-500/80"></span>
      <h3 class="text-[11px] uppercase {t.textMuted}">Review</h3>
    </div>
    <p class="text-xs {t.textMuted}">Team: <span class="{t.textPrimary} font-medium">{teamName || '—'}</span></p>
    <p class="text-xs {t.textMuted}">
      Lead: <span class="{t.textPrimary} font-medium">{leadName || '—'}</span> · Claude ({leadModel})
    </p>
    <p class="text-xs {t.textMuted}">Agents: <span class="{t.textPrimary} font-medium">{agents.length}</span></p>
    <p class="text-xs {t.textMuted}" data-testid="mesh-review-agents-detail">
      Members: <span class="{t.textPrimary} font-medium">{reviewAgents || '—'}</span>
    </p>
  </div>

  <div class="flex justify-end gap-2">
    <button
      class="h-8 inline-flex items-center rounded-md px-3 text-xs font-medium transition-colors {dark ? 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700' : 'bg-zinc-100 text-zinc-700 hover:bg-zinc-200'}"
      type="button"
      onclick={quickStart}
      data-testid="mesh-quick-start-button"
    >
      Quick Start
    </button>
    <button
      class={primaryCta}
      type="button"
      onclick={submitInitialize}
      disabled={!canInitialize}
      data-testid="mesh-create-team-button"
    >
      Initialize Team
    </button>
  </div>
</section>
