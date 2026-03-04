<script>
  import SlideOver from './SlideOver.svelte'
  import AgentCard from './AgentCard.svelte'
  import ValidationBar from './ValidationBar.svelte'
  import { collectDuplicateNames } from '../meshValidation.js'
  import { normalizeProjectOption } from '../projectOptions.js'
  import { themeTokens } from '../themeTokens.js'
  import { upsertTeamPreset } from '../ipc.js'

  let {
    open = false,
    dark = false,
    projectPath = '',
    availableProjects = [],
    teamConfig = null,
    context = null,
    onClose = () => {},
    onSave = () => {},
    onReset = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const inputTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500'
      : 'border-brand-200 bg-white text-brand-900 placeholder:text-brand-700/60'
  )
  const sectionTone = $derived(
    dark
      ? 'border-zinc-700/70 bg-zinc-900/40'
      : 'border-brand-200 bg-linear-to-b from-brand-50 to-[#e6f7f4]'
  )
  const ghostTone = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800'
      : 'border-brand-200 text-brand-700 hover:bg-brand-50'
  )

  let localTeamName = $state('')
  let localDescription = $state('')
  let lead = $state(null)
  let agents = $state([])
  let nextAgentId = $state(1)
  let hydratedConfig = $state(null)

  let showSavePresetDialog = $state(false)
  let newPresetName = $state('')
  let newPresetDescription = $state('')
  let isSavingPreset = $state(false)
  let presetSaveMessage = $state('')
  let presetSaveError = $state(false)

  const projectOptions = $derived.by(() =>
    (availableProjects ?? [])
      .map((project) => normalizeProjectOption(project, { stringLabel: 'raw', objectFallbackLabel: 'raw' }))
      .filter((project) => project.id)
  )

  function defaultLead() {
    return {
      id: 'lead',
      name: 'team-lead',
      tool: 'claude',
      model: 'opus',
      projectId: projectPath || projectOptions[0]?.id || '',
      description: 'Team lead',
      roleId: null,
    }
  }

  function defaultAgent(index) {
    return {
      id: `agent-${index + 1}`,
      name: `agent-${index + 1}`,
      tool: 'codex',
      model: 'gpt-5.3-codex',
      projectId: projectPath || projectOptions[0]?.id || '',
      description: '',
      roleId: null,
    }
  }

  function hydrateFromConfig(config) {
    localTeamName = String(config?.teamName ?? '').trim()
    localDescription = String(config?.description ?? '')

    const incomingLead = config?.lead
    lead = incomingLead
      ? {
          id: String(incomingLead.id ?? 'lead'),
          name: String(incomingLead.name ?? 'team-lead'),
          tool: String(incomingLead.tool ?? incomingLead.cliTool ?? 'claude').toLowerCase(),
          model: String(incomingLead.model ?? 'opus'),
          projectId: String(incomingLead.projectId ?? incomingLead.project_id ?? projectPath ?? ''),
          description: String(incomingLead.description ?? 'Team lead'),
          roleId: incomingLead.roleId ?? null,
        }
      : defaultLead()

    const incomingAgents = Array.isArray(config?.agents) ? config.agents : []
    agents = incomingAgents.map((agent, index) => ({
      id: String(agent.id ?? `agent-${index + 1}`),
      name: String(agent.name ?? `agent-${index + 1}`),
      tool: String(agent.tool ?? agent.cliTool ?? 'codex').toLowerCase(),
      model: String(agent.model ?? 'gpt-5.3-codex'),
      projectId: String(agent.projectId ?? agent.project_id ?? projectPath ?? ''),
      description: String(agent.description ?? ''),
      roleId: agent.roleId ?? null,
    }))
    nextAgentId = agents.length + 1
  }

  const validationIssues = $derived.by(() => {
    const issues = []
    if (!localTeamName.trim()) {
      issues.push({ severity: 'error', member: 'Team', message: 'Team name is required.' })
    }
    if (!lead?.name?.trim()) {
      issues.push({ severity: 'error', member: 'Lead', message: 'Lead name is required.' })
    }
    const duplicates = collectDuplicateNames([lead?.name, ...agents.map((agent) => agent.name)])
    for (const duplicate of duplicates) {
      issues.push({ severity: 'error', member: duplicate, message: 'Duplicate member name.' })
    }
    return issues
  })

  const hasErrors = $derived(validationIssues.some((issue) => issue.severity === 'error'))

  function addAgent() {
    agents = [...agents, defaultAgent(nextAgentId - 1)]
    nextAgentId += 1
  }

  function updateLead(payload) {
    lead = {
      ...lead,
      ...payload,
    }
  }

  function updateAgent(id, payload) {
    agents = agents.map((agent) => (agent.id === id ? { ...agent, ...payload } : agent))
  }

  function removeAgent(id) {
    agents = agents.filter((agent) => agent.id !== id)
  }

  function slugifyPresetId(value) {
    const slug = String(value || '')
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9\s_-]+/g, '')
      .replace(/[\s_]+/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '')
    return slug || 'custom-preset'
  }

  function fallbackLeadRoleId(tool) {
    const normalized = String(tool || '').toLowerCase()
    if (normalized === 'codex') return 'codex-developer'
    if (normalized === 'gemini') return 'custom-doc-writer'
    return 'claude-orchestrator'
  }

  function fallbackAgentRoleId(tool) {
    const normalized = String(tool || '').toLowerCase()
    if (normalized === 'claude') return 'claude-reviewer'
    if (normalized === 'gemini') return 'custom-doc-writer'
    return 'codex-developer'
  }

  async function handleSaveAsPreset() {
    if (hasErrors || !newPresetName.trim()) return
    if ((agents ?? []).length === 0) {
      presetSaveError = true
      presetSaveMessage = 'At least one agent is required to save a preset.'
      return
    }

    isSavingPreset = true
    presetSaveMessage = ''
    presetSaveError = false

    try {
      const leadRoleId = lead?.roleId || fallbackLeadRoleId(lead?.tool)
      const agentSlots = agents.map((agent) => ({
        roleId: agent.roleId || fallbackAgentRoleId(agent.tool),
        count: 1,
        projectBinding: 'lead_project',
        projectId: null,
        overrides: null,
      }))

      await upsertTeamPreset({
        schema: {
          kind: 'team_preset',
          version: 1,
        },
        presetId: slugifyPresetId(newPresetName),
        name: newPresetName.trim(),
        description: newPresetDescription.trim() || 'Custom team preset',
        version: '1.0.0',
        leadRoleId,
        agentSlots,
        defaults: {
          teamNamePattern: '{project}-team',
          tmuxLayout: 'tiled',
        },
      })

      presetSaveMessage = 'Preset saved to catalog'
      presetSaveError = false
      setTimeout(() => {
        showSavePresetDialog = false
        presetSaveMessage = ''
        presetSaveError = false
        newPresetName = ''
        newPresetDescription = ''
      }, 2000)
    } catch (err) {
      presetSaveError = true
      presetSaveMessage = err?.message || 'Failed to save preset.'
    } finally {
      isSavingPreset = false
    }
  }

  function handleSave(payload) {
    onSave({
      teamName: localTeamName.trim(),
      description: localDescription.trim(),
      presetId: teamConfig?.presetId ?? '',
      lead: {
        name: String(lead?.name ?? '').trim(),
        cliTool: String(lead?.tool ?? 'claude').toLowerCase(),
        model: String(lead?.model ?? '').trim(),
        projectId: String(lead?.projectId ?? '').trim(),
        description: String(lead?.description ?? '').trim(),
      },
      agents: agents.map((agent) => ({
        name: String(agent.name ?? '').trim(),
        cliTool: String(agent.tool ?? 'codex').toLowerCase(),
        model: String(agent.model ?? '').trim(),
        projectId: String(agent.projectId ?? '').trim(),
        description: String(agent.description ?? '').trim(),
      })),
      ...payload,
    })
  }

  $effect(() => {
    if (!open) return
    if (teamConfig === hydratedConfig) return
    hydratedConfig = teamConfig
    hydrateFromConfig(teamConfig)
  })
</script>

<SlideOver {open} title="Customize Team" width={460} {dark} onClose={onClose}>
  {#snippet children()}
    <section class="space-y-3" data-testid="team-customizer-panel">
      <div class="space-y-2 rounded-[12px] border p-3 {sectionTone}" data-testid="team-customizer-header">
        <label class="space-y-1 block">
          <span class="text-[10px] font-medium uppercase tracking-wide {t.textMuted}">Team name</span>
          <input
            class="h-9 w-full rounded-[12px] border px-2.5 text-sm {inputTone}"
            value={localTeamName}
            oninput={(event) => {
              localTeamName = event.currentTarget.value
            }}
            data-testid="team-customizer-name-input"
          />
        </label>
        <label class="space-y-1 block">
          <span class="text-[10px] font-medium uppercase tracking-wide {t.textMuted}">Description</span>
          <input
            class="h-9 w-full rounded-[12px] border px-2.5 text-sm {inputTone}"
            value={localDescription}
            oninput={(event) => {
              localDescription = event.currentTarget.value
            }}
            data-testid="team-customizer-description-input"
          />
        </label>
      </div>

      <ValidationBar issues={validationIssues} {dark} />

      {#if context?.selectedRole}
        <p class="text-xs {dark ? 'text-zinc-400' : 'text-brand-700'}" data-testid="team-customizer-selected-role">
          Selected role from catalog: {context.selectedRole.name || context.selectedRole.roleId}
        </p>
      {/if}

      {#if lead}
        <AgentCard
          testId="team-customizer-lead"
          role="lead"
          name={lead.name}
          tool={lead.tool}
          model={lead.model}
          projectId={lead.projectId}
          description={lead.description}
          {dark}
          onSave={updateLead}
        />
      {/if}

      <section class="space-y-2">
        {#each agents as agent (agent.id)}
          <AgentCard
            testId={`team-customizer-agent-${agent.id}`}
            role="agent"
            name={agent.name}
            tool={agent.tool}
            model={agent.model}
            projectId={agent.projectId}
            description={agent.description}
            {dark}
            onSave={(payload) => updateAgent(agent.id, payload)}
            onRemove={() => removeAgent(agent.id)}
          />
        {/each}
      </section>

      <button
        class="rounded-[12px] border px-3 py-1.5 text-xs transition-colors {ghostTone}"
        onclick={addAgent}
        data-testid="team-customizer-add-agent"
      >
        + Agent
      </button>

      {#if !showSavePresetDialog}
        <div class="pt-2">
          <button
            class="w-full rounded-[12px] border border-dashed py-3 text-xs font-medium transition-colors {ghostTone}"
            onclick={() => {
              showSavePresetDialog = true
              newPresetName = localTeamName
              presetSaveError = false
              presetSaveMessage = ''
            }}
            disabled={hasErrors}
            data-testid="team-customizer-save-preset-trigger"
          >
            Save as New Preset
          </button>
        </div>
      {:else}
        <div class="space-y-3 rounded-[12px] border p-3 {sectionTone}" data-testid="save-preset-dialog">
          <p class="text-[10px] font-bold uppercase tracking-wider {t.textMuted}">Save as New Preset</p>
          <label class="space-y-1 block">
            <span class="text-[10px] font-medium uppercase tracking-wide {t.textMuted}">Preset Name*</span>
            <input
              class="h-9 w-full rounded-[12px] border px-2.5 text-sm {inputTone}"
              bind:value={newPresetName}
              placeholder="e.g. My Feature Team"
              data-testid="save-preset-name-input"
            />
          </label>
          <label class="space-y-1 block">
            <span class="text-[10px] font-medium uppercase tracking-wide {t.textMuted}">Description</span>
            <input
              class="h-9 w-full rounded-[12px] border px-2.5 text-sm {inputTone}"
              bind:value={newPresetDescription}
              placeholder="Optional description"
              data-testid="save-preset-description-input"
            />
          </label>
          
          {#if presetSaveMessage}
            <p class="text-xs font-medium {presetSaveError ? 'text-danger-500' : 'text-success-600'}" data-testid="save-preset-feedback">
              {presetSaveMessage}
            </p>
          {/if}

          <div class="flex justify-end gap-2 pt-1">
            <button
              class="rounded-[12px] px-3 py-1.5 text-xs transition-colors {ghostTone}"
              onclick={() => {
                showSavePresetDialog = false
                presetSaveMessage = ''
                presetSaveError = false
              }}
              data-testid="save-preset-cancel"
            >
              Cancel
            </button>
            <button
              class="rounded-[12px] bg-brand-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-brand-700 disabled:opacity-50"
              onclick={handleSaveAsPreset}
              disabled={!newPresetName.trim() || isSavingPreset}
              data-testid="save-preset-confirm"
            >
              {isSavingPreset ? 'Saving...' : 'Save Preset'}
            </button>
          </div>
        </div>
      {/if}

      <div class="flex justify-end gap-2 border-t pt-3 {t.keyline}">
        <button
          class="rounded-[12px] border px-3 py-1.5 text-xs transition-colors {ghostTone}"
          onclick={onReset}
          data-testid="team-customizer-reset"
        >
          Reset to Empty
        </button>
        <button
          class="rounded-[12px] bg-brand-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50"
          onclick={handleSave}
          disabled={hasErrors}
          data-testid="team-customizer-save"
        >
          Apply
        </button>
      </div>
    </section>
  {/snippet}
</SlideOver>
