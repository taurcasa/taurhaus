<script>
  import { composeTeam, getTeamPreset, listTeamPresets } from '../ipc.js'
  import { createAsyncGuard } from '../asyncGuard.js'
  import { collectDuplicateNames } from '../meshValidation.js'
  import { normalizeProjectOption, projectBasename } from '../projectOptions.js'
  import MeshEmptyState from './MeshEmptyState.svelte'
  import TeamComposer from './TeamComposer.svelte'
  import TemplateCatalog from './TemplateCatalog.svelte'
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
  const frameTone = $derived(
    dark ? 'border-zinc-700/70 bg-zinc-900/60' : 'border-zinc-200 bg-zinc-50/80'
  )
  const panelTone = $derived(
    dark ? 'border-zinc-700/60 bg-zinc-900/40' : 'border-zinc-200 bg-white'
  )

  const toolOptions = [
    { value: 'claude', label: 'Claude' },
    { value: 'codex', label: 'Codex' },
    { value: 'gemini', label: 'Gemini' },
  ]

  const modelOptionsByTool = {
    claude: ['opus', 'sonnet', 'haiku'],
    codex: ['gpt-5.3-codex', 'gpt-5-mini'],
    gemini: ['gemini-2.5-pro', 'gemini-2.0-flash'],
  }

  function inferTeamName(path) {
    return `${projectBasename(path)}-team`
  }

  const projectOptions = $derived(
    (availableProjects ?? [])
      .map((project) => normalizeProjectOption(project, { unnamedLabel: 'Unnamed' }))
      .filter((p) => p.id)
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
  let templatePresetsLoading = $state(false)
  let templateError = $state('')
  let templateNotice = $state('')
  let presetSummaries = $state([])
  let selectedPresetId = $state('')
  let selectedPreset = $state(null)
  let showTeamComposer = $state(false)
  let showTemplateCatalog = $state(false)
  let startTeamInFlight = $state(false)
  const presetApplyGuard = createAsyncGuard()
  const presetCatalogGuard = createAsyncGuard()
  const quickPresetIds = ['fullstack-dev', 'research-dev', 'review-team']

  function defaultLead() {
    return {
      name: 'team-lead',
      cliTool: 'claude',
      model: 'opus',
      description: 'Team lead',
      projectId: projectPath,
      roleId: null,
      instructions: null,
      behavioralContract: null,
      capabilities: null,
    }
  }

  let lead = $state(defaultLead())

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
      roleId: null,
      instructions: null,
      behavioralContract: null,
      capabilities: null,
    }
  }

  let agents = $state([defaultAgent()])

  const quickPresets = $derived.by(() => {
    return quickPresetIds.map((presetId) => {
      const preset = presetSummaries.find((entry) => entry.presetId === presetId)
      if (preset) return { ...preset, missing: false }
      return {
        presetId,
        name: presetId,
        description: 'Unavailable in current catalog',
        leadRoleId: '',
        missing: true,
      }
    })
  })

  function toolLabel(tool) {
    const option = toolOptions.find((entry) => entry.value === tool)
    return option?.label ?? String(tool || '')
  }

  function normalizePresetSummary(value) {
    return {
      presetId: value?.presetId ?? value?.preset_id ?? '',
      name: value?.name ?? '',
      description: value?.description ?? '',
      leadRoleId: value?.leadRoleId ?? value?.lead_role_id ?? '',
      agentCount: Math.max(0, Number(value?.agentCount ?? value?.agent_count ?? 0)),
      leadCount: Math.max(0, Number(value?.leadCount ?? value?.lead_count ?? 1)),
      tools: Array.isArray(value?.tools) ? value.tools : [],
      builtIn: Boolean(value?.builtIn ?? value?.built_in ?? false),
    }
  }

  function normalizePreset(value) {
    if (!value || typeof value !== 'object') return null
    const agentSlots = value?.agentSlots ?? value?.agent_slots ?? []
    return {
      presetId: value?.presetId ?? value?.preset_id ?? '',
      name: value?.name ?? '',
      description: value?.description ?? '',
      leadRoleId: value?.leadRoleId ?? value?.lead_role_id ?? '',
      agentSlots: Array.isArray(agentSlots)
        ? agentSlots.map((slot) => ({
            roleId: slot?.roleId ?? slot?.role_id ?? '',
            count: Number(slot?.count ?? 0),
            projectBinding: slot?.projectBinding ?? slot?.project_binding ?? 'lead_project',
            overrides: slot?.overrides ?? null,
          }))
        : [],
    }
  }

  function normalizeResolvedMember(value) {
    return {
      name: value?.name ?? '',
      roleId: value?.roleId ?? value?.role_id ?? '',
      roleKind: String(value?.roleKind ?? value?.role_kind ?? 'agent').toLowerCase(),
      cliTool: String(value?.cliTool ?? value?.cli_tool ?? 'codex').toLowerCase(),
      model: value?.model ?? '',
      instructions: value?.instructions ?? '',
      behavioralContract: value?.behavioralContract ?? value?.behavioral_contract ?? null,
      capabilities: Array.isArray(value?.capabilities) ? value.capabilities : [],
      projectId: value?.projectId ?? value?.project_id ?? '',
    }
  }

  function mapAgentFromPayload(agent) {
    return {
      id: nextAgentId++,
      name: String(agent?.name ?? '').trim(),
      cliTool: String(agent?.cliTool ?? agent?.cli_tool ?? 'codex').toLowerCase(),
      model: String(agent?.model ?? modelOptionsByTool.codex[0]),
      projectId: String(agent?.projectId ?? agent?.project_id ?? projectPath ?? ''),
      description: String(agent?.description ?? ''),
      roleId: agent?.roleId ?? agent?.role_id ?? null,
      instructions: agent?.instructions ?? null,
      behavioralContract: agent?.behavioralContract ?? agent?.behavioral_contract ?? null,
      capabilities: Array.isArray(agent?.capabilities) ? agent.capabilities : null,
    }
  }

  function applyInitializedPayload(payload, notice) {
    if (payload?.lead) {
      lead = {
        name: String(payload.lead.name ?? 'team-lead'),
        cliTool: String(payload.lead.cliTool ?? payload.lead.cli_tool ?? 'claude').toLowerCase(),
        model: String(payload.lead.model ?? 'opus'),
        description: String(payload.lead.description ?? 'Team lead'),
        projectId: String(payload.lead.projectId ?? payload.lead.project_id ?? projectPath ?? ''),
        roleId: payload.lead.roleId ?? payload.lead.role_id ?? null,
        instructions: payload.lead.instructions ?? null,
        behavioralContract:
          payload.lead.behavioralContract ?? payload.lead.behavioral_contract ?? null,
        capabilities: Array.isArray(payload.lead.capabilities) ? payload.lead.capabilities : null,
      }
    } else {
      lead = defaultLead()
    }

    if (Array.isArray(payload?.agents)) {
      agents = payload.agents.map(mapAgentFromPayload)
    } else {
      agents = [defaultAgent()]
    }

    if (notice) templateNotice = notice
    showTeamComposer = false
    showTemplateCatalog = false
  }

  function applyComposedRoster(composed, notice) {
    const roster = Array.isArray(composed?.roster) ? composed.roster.map(normalizeResolvedMember) : []
    const leadMember = roster.find((entry) => entry.roleKind === 'lead')
    const agentMembers = roster.filter((entry) => entry.roleKind !== 'lead')

    applyInitializedPayload(
      {
        lead: leadMember
          ? {
              name: leadMember.name || 'team-lead',
              cliTool: leadMember.cliTool || 'claude',
              model: leadMember.model || 'opus',
              projectId: leadMember.projectId || projectPath,
              description: leadMember.roleId || 'Team lead',
              roleId: leadMember.roleId || null,
              instructions: leadMember.instructions || null,
              behavioralContract: leadMember.behavioralContract ?? null,
              capabilities: leadMember.capabilities?.length ? leadMember.capabilities : null,
            }
          : defaultLead(),
        agents: agentMembers.map((entry) => ({
          name: entry.name,
          cliTool: entry.cliTool,
          model: entry.model,
          projectId: entry.projectId || projectPath,
          description: entry.roleId || null,
          roleId: entry.roleId || null,
          instructions: entry.instructions || null,
          behavioralContract: entry.behavioralContract ?? null,
          capabilities: entry.capabilities?.length ? entry.capabilities : null,
        })),
      },
      notice
    )
  }

  async function loadTemplatePresets() {
    const sequence = presetCatalogGuard.next()
    templatePresetsLoading = true
    templateError = ''
    try {
      const presets = await listTeamPresets()
      if (!presetCatalogGuard.isCurrent(sequence)) return
      presetSummaries = (presets ?? []).map(normalizePresetSummary).filter((entry) => entry.presetId)
    } catch (error) {
      if (!presetCatalogGuard.isCurrent(sequence)) return
      presetSummaries = []
      templateError = error?.message || 'Failed to load templates.'
    } finally {
      if (presetCatalogGuard.isCurrent(sequence)) {
        templatePresetsLoading = false
      }
    }
  }

  function resetTemplateTransientState() {
    startTeamInFlight = false
    templateError = ''
    templateNotice = ''
    showTeamComposer = false
    showTemplateCatalog = false
  }

  async function applyPreset(presetId) {
    const sequence = presetApplyGuard.next()
    resetTemplateTransientState()
    selectedPresetId = presetId
    selectedPreset = null
    try {
      const preset = normalizePreset(await getTeamPreset(presetId))
      if (!presetApplyGuard.isCurrent(sequence)) return
      if (!preset) {
        templateError = 'Preset not found.'
        return
      }
      selectedPreset = preset
      const composed = await composeTeam({
        leadRoleId: preset.leadRoleId,
        agentSlots: preset.agentSlots ?? [],
        projectName,
      })
      if (!presetApplyGuard.isCurrent(sequence)) return
      applyComposedRoster(composed, `Applied preset: ${preset.name}`)
    } catch (error) {
      if (!presetApplyGuard.isCurrent(sequence)) return
      templateError = error?.message || 'Failed to apply preset.'
    }
  }

  function startCustomTemplateFlow() {
    resetTemplateTransientState()
    presetApplyGuard.invalidate()
    showTeamComposer = true
  }

  function openTemplateCatalog() {
    resetTemplateTransientState()
    presetApplyGuard.invalidate()
    showTemplateCatalog = true
  }

  function applyCompositionPayload(payload, notice = 'Applied composed team') {
    resetTemplateTransientState()
    presetApplyGuard.invalidate()
    applyInitializedPayload(payload, notice)
  }

  $effect(() => {
    startTeamInFlight = false
    if (!teamName.trim()) {
      teamName = inferTeamName(projectPath)
    }
  })

  $effect(() => {
    if (!lead.projectId && projectPath) {
      lead = {
        ...lead,
        projectId: projectPath,
      }
    }
  })

  $effect(() => {
    try {
      onboardingDismissed = localStorage.getItem('mesh-onboarding-dismissed') === 'true'
    } catch {}
  })

  $effect(() => {
    void loadTemplatePresets()
    return () => {
      presetCatalogGuard.invalidate()
      presetApplyGuard.invalidate()
    }
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
    const names = [lead.name, ...agents.map((a) => a.name)]
      .map((n) => n.trim().toLowerCase())
      .filter(Boolean)
    return collectDuplicateNames(names)
  })

  const hasDuplicateNames = $derived(duplicateNames.length > 0)

  function agentDisplayName(agent, index) {
    return agent.name.trim() || `${projectName}-dev${agents.length > 1 ? `-${index + 1}` : ''}`
  }

  function startTeam() {
    if (startTeamInFlight) return
    startTeamInFlight = true
    oninitialize({
      teamName: teamName.trim() || inferTeamName(projectPath),
      teamDescription: teamDescription.trim() || null,
      leadMode: 'launch_new',
      lead: {
        name: lead.name.trim() || 'team-lead',
        cliTool: lead.cliTool,
        model: lead.model,
        projectId: lead.projectId || projectPath,
        description: lead.description || 'Team lead',
        roleId: lead.roleId,
        instructions: lead.instructions,
        behavioralContract: lead.behavioralContract,
        capabilities: lead.capabilities,
      },
      agents: agents.map((agent, i) => ({
        name: agentDisplayName(agent, i),
        cliTool: agent.cliTool,
        model: agent.model,
        projectId: agent.projectId || projectPath,
        description: agent.description.trim() || null,
        roleId: agent.roleId,
        instructions: agent.instructions,
        behavioralContract: agent.behavioralContract,
        capabilities: agent.capabilities,
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
      class="relative flex items-start gap-2 rounded-md border px-3 py-2 text-xs leading-relaxed {frameTone} {t.textMuted}"
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

  <section class="rounded-lg border p-3 {panelTone}" data-testid="mesh-template-picker">
    <MeshEmptyState
      presets={quickPresets.filter((preset) => !preset.missing)}
      dark={dark}
      onSelectPreset={(preset) => {
        if (!preset?.presetId) return
        void applyPreset(preset.presetId)
      }}
      onBrowseTemplates={() => {
        openTemplateCatalog()
      }}
      onStartCustom={() => {
        startCustomTemplateFlow()
      }}
    />
    {#if templatePresetsLoading}
      <p class="text-xs {t.textMuted}" data-testid="mesh-template-loading">Loading templates...</p>
    {/if}
    {#if templateError}
      <p class="text-xs text-danger-500" data-testid="mesh-template-error">{templateError}</p>
    {/if}
    {#if templateNotice}
      <p class="text-xs text-brand-500" data-testid="mesh-template-notice">{templateNotice}</p>
    {/if}
  </section>

  {#if showTemplateCatalog}
    <TemplateCatalog
      dark={dark}
      onComposeApply={(payload) => {
        applyCompositionPayload(payload, 'Applied catalog composition')
      }}
      onSaveComposedPreset={async () => {
        await loadTemplatePresets()
      }}
    />
  {/if}

  {#if showTeamComposer}
    <TeamComposer
      dark={dark}
      projectPath={projectPath}
      projectName={projectName}
      initialPreset={selectedPreset}
      onApply={(payload) => {
        applyCompositionPayload(payload)
      }}
      onClose={() => {
        showTeamComposer = false
      }}
    />
  {/if}

  <div
    class="rounded-lg border {panelTone}"
    data-testid="mesh-roster-preview"
  >
    <div class="px-3 py-1.5 border-b {t.keyline}">
      <span class="text-xs font-medium {t.textMuted}">
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
        <span class="text-xs font-medium {t.textPrimary}">{lead.name || 'team-lead'}</span>
        <span class="text-xs {dark ? 'text-zinc-400' : 'text-zinc-600'}">
          {toolLabel(lead.cliTool)} · {lead.model}
        </span>
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
            aria-label={`Agent ${index + 1} name`}
            data-testid={`mesh-agent-name-input-${index}`}
          />
          <select
            class={inlineSelect}
            style:background-image={chevronSvg}
            style:background-repeat="no-repeat"
            style:background-position="right 4px center"
            value={agent.cliTool}
            onchange={(e) => updateAgentTool(index, e.currentTarget.value)}
            aria-label={`Agent ${index + 1} tool`}
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
            aria-label={`Agent ${index + 1} model`}
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
            aria-label={`Agent ${index + 1} project`}
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
    <p class="text-xs text-warning-500" data-testid="mesh-duplicate-name-error">
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
        class="h-8 inline-flex items-center rounded-md bg-brand-600 px-4 text-xs font-medium text-white hover:bg-brand-500 transition-colors disabled:cursor-not-allowed disabled:opacity-50"
        type="button"
        onclick={startTeam}
        disabled={startTeamInFlight}
        data-testid="mesh-create-team-button"
      >{startTeamInFlight ? 'Starting…' : 'Start Team'}</button>
    </div>
  </div>
</section>
