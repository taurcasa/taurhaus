<script>
  import SlideOver from './SlideOver.svelte'
  import AgentCard from './AgentCard.svelte'
  import ValidationBar from './ValidationBar.svelte'
  import { getModelCatalogContext } from '../context/ModelCatalogContext.js'
  import {
    EMPTY_MODEL_CATALOG,
    defaultEffortFor,
    defaultModelFor,
    parseLegacyModel,
  } from '../modelCatalog.js'
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
    modelCatalog = null,
    onClose = () => {},
    onSave = () => {},
    onReset = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const modelCatalogContext = getModelCatalogContext()
  const catalog = $derived(modelCatalog ?? modelCatalogContext?.catalog ?? EMPTY_MODEL_CATALOG)
  const inputTone = $derived(
    dark
      ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20'
      : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'
  )
  const sectionTone = $derived(
    dark
      ? 'border-white/[0.06] bg-white/[0.03]'
      : 'border-brand-200/40 bg-brand-50/50'
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
  let hydratedConfig = $state(undefined)
  let hydratedSelectedRoleId = $state(undefined)

  let showSavePresetDialog = $state(false)
  let newPresetName = $state('')
  let newPresetDescription = $state('')
  let isSavingPreset = $state(false)
  let presetSaveMessage = $state('')
  let presetSaveError = $state(false)
  let presetSaveTimer = null

  const projectOptions = $derived.by(() =>
    (availableProjects ?? [])
      .map((project) => normalizeProjectOption(project, { stringLabel: 'raw', objectFallbackLabel: 'raw' }))
      .filter((project) => project.id)
  )

  function normalizeOptionalTool(value) {
    const normalized = String(value ?? '').trim().toLowerCase()
    return normalized || ''
  }

  function selectedLeadDefaults() {
    const selectedRole = context?.selectedRole
    const tool = normalizeOptionalTool(selectedRole?.cliTool ?? selectedRole?.tool)
    const parsed = parseLegacyModel(selectedRole?.model)
    const model = parsed.model || (tool ? defaultModelFor(catalog, tool) : '')
    const declaredEffort =
      selectedRole?.reasoningEffort ?? selectedRole?.reasoning_effort ?? parsed.reasoningEffort

    return {
      tool,
      model,
      // Only a catalog-supplied model brings the catalog's default effort: a role
      // that names a model but no effort keeps inheriting the CLI global setting.
      reasoningEffort:
        declaredEffort ??
        (!parsed.model && tool ? defaultEffortFor(catalog, tool, model) : null),
      roleId: selectedRole?.roleId ?? null,
      description: String(selectedRole?.name ?? 'Team lead'),
    }
  }

  function defaultLead() {
    const defaults = selectedLeadDefaults()

    return {
      id: 'lead',
      name: 'team-lead',
      tool: defaults.tool,
      model: defaults.model,
      reasoningEffort: defaults.reasoningEffort,
      projectId: projectPath || projectOptions[0]?.id || '',
      description: defaults.description,
      roleId: defaults.roleId,
    }
  }

  function defaultAgent(index) {
    const model = defaultModelFor(catalog, 'codex')
    return {
      id: `agent-${index + 1}`,
      name: `agent-${index + 1}`,
      tool: 'codex',
      model,
      reasoningEffort: defaultEffortFor(catalog, 'codex', model),
      projectId: projectPath || projectOptions[0]?.id || '',
      description: '',
      roleId: null,
      slotIndex: null,
    }
  }

  function hydrateFromConfig(config) {
    localTeamName = String(config?.teamName ?? '').trim()
    localDescription = String(config?.description ?? '')
    const defaults = selectedLeadDefaults()

    const incomingLead = config?.lead
    lead = incomingLead
      ? {
          id: String(incomingLead.id ?? 'lead'),
          name: String(incomingLead.name ?? 'team-lead'),
          tool: normalizeOptionalTool(incomingLead.tool ?? incomingLead.cliTool ?? defaults.tool),
          model: String(parseLegacyModel(incomingLead.model).model || defaults.model || ''),
          reasoningEffort:
            incomingLead.reasoningEffort ??
            incomingLead.reasoning_effort ??
            parseLegacyModel(incomingLead.model).reasoningEffort ??
            defaults.reasoningEffort ??
            null,
          projectId: String(incomingLead.projectId ?? incomingLead.project_id ?? projectPath ?? ''),
          description: String(incomingLead.description ?? defaults.description),
          roleId: incomingLead.roleId ?? defaults.roleId ?? null,
        }
      : defaultLead()

    const incomingAgents = Array.isArray(config?.agents) ? config.agents : []
    agents = incomingAgents.map((agent, index) => ({
      id: String(agent.id ?? `agent-${index + 1}`),
      name: String(agent.name ?? `agent-${index + 1}`),
      tool: String(agent.tool ?? agent.cliTool ?? 'codex').toLowerCase(),
      model: String(parseLegacyModel(agent.model).model || ''),
      reasoningEffort:
        agent.reasoningEffort ??
        agent.reasoning_effort ??
        parseLegacyModel(agent.model).reasoningEffort ??
        null,
      projectId: String(agent.projectId ?? agent.project_id ?? projectPath ?? ''),
      description: String(agent.description ?? ''),
      roleId: agent.roleId ?? null,
      // Which preset slot this row came from, so a save can put the edit back on
      // that slot's overrides instead of rebuilding the preset from role defaults.
      slotIndex: Number.isInteger(agent.slotIndex) ? agent.slotIndex : null,
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

  function fallbackLeadRoleId() {
    return lead?.roleId ?? selectedLeadDefaults().roleId ?? null
  }

  function fallbackAgentRoleId(tool) {
    const normalized = String(tool || '').toLowerCase()
    if (normalized === 'claude') return 'claude-reviewer'
    if (normalized === 'gemini') return 'custom-doc-writer'
    return 'codex-developer'
  }

  // Without overrides the saved preset only remembers the role, so reloading it
  // restores the role defaults and throws away the model and effort the roster
  // actually selected.
  function slotOverridesFor(agent) {
    const model = String(agent?.model ?? '').trim()
    const reasoningEffort = String(agent?.reasoningEffort ?? '').trim()
    if (!model && !reasoningEffort) return null
    return { model: model || null, reasoningEffort: reasoningEffort || null }
  }

  function clearPresetSaveTimer() {
    if (!presetSaveTimer) return
    clearTimeout(presetSaveTimer)
    presetSaveTimer = null
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
      const leadRoleId = fallbackLeadRoleId()
      if (!leadRoleId) {
        presetSaveError = true
        presetSaveMessage = 'Lead role selection is required to save a preset.'
        return
      }
      const agentSlots = agents.map((agent) => ({
        roleId: agent.roleId || fallbackAgentRoleId(agent.tool),
        count: 1,
        projectBinding: 'lead_project',
        projectId: null,
        overrides: slotOverridesFor(agent),
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
      clearPresetSaveTimer()
      presetSaveTimer = setTimeout(() => {
        showSavePresetDialog = false
        presetSaveMessage = ''
        presetSaveError = false
        newPresetName = ''
        newPresetDescription = ''
        presetSaveTimer = null
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
        cliTool: normalizeOptionalTool(lead?.tool),
        model: String(lead?.model ?? '').trim(),
        reasoningEffort: lead?.reasoningEffort ?? null,
        projectId: String(lead?.projectId ?? '').trim(),
        description: String(lead?.description ?? '').trim(),
        roleId: lead?.roleId ?? null,
      },
      agents: agents.map((agent) => ({
        name: String(agent.name ?? '').trim(),
        cliTool: String(agent.tool ?? 'codex').toLowerCase(),
        model: String(agent.model ?? '').trim(),
        reasoningEffort: agent.reasoningEffort ?? null,
        projectId: String(agent.projectId ?? '').trim(),
        description: String(agent.description ?? '').trim(),
        roleId: agent.roleId ?? null,
        slotIndex: Number.isInteger(agent.slotIndex) ? agent.slotIndex : null,
      })),
      ...payload,
    })
  }

  $effect(() => {
    if (!open) return
    const selectedRoleId = context?.selectedRole?.roleId ?? null
    if (teamConfig === hydratedConfig && selectedRoleId === hydratedSelectedRoleId) return
    hydratedConfig = teamConfig
    hydratedSelectedRoleId = selectedRoleId
    hydrateFromConfig(teamConfig)
  })

  $effect(() => {
    return () => {
      clearPresetSaveTimer()
    }
  })
</script>

<SlideOver {open} title="Customize Team" width={460} {dark} onClose={onClose}>
  {#snippet children()}
    <section class="space-y-4 pb-20" data-testid="team-customizer-panel">
      <!-- Header Section -->
      <div class="space-y-3 p-3 rounded-xl border transition-all animate-in fade-in slide-in-from-bottom-1 duration-200 {sectionTone}" data-testid="team-customizer-header">
        <label class="space-y-1.5 block">
          <span class="text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Team name</span>
          <input
            class="h-10 w-full rounded-lg border px-3 text-base transition-all outline-none {inputTone}"
            value={localTeamName}
            oninput={(event) => {
              localTeamName = event.currentTarget.value
            }}
            data-testid="team-customizer-name-input"
          />
        </label>
        <label class="space-y-1.5 block">
          <span class="text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Description</span>
          <input
            class="h-10 w-full rounded-lg border px-3 text-sm transition-all outline-none {inputTone}"
            value={localDescription}
            oninput={(event) => {
              localDescription = event.currentTarget.value
            }}
            data-testid="team-customizer-description-input"
          />
        </label>
      </div>

      <div class="px-1">
        <ValidationBar issues={validationIssues} {dark} />
      </div>

      {#if context?.selectedRole}
        <div class="px-3 py-2 rounded-lg bg-brand-500/5 border border-brand-500/10 animate-in fade-in zoom-in-95 duration-200">
          <p class="text-[11px] font-medium text-brand-500" data-testid="team-customizer-selected-role">
            Selected role: <span class="font-bold">{context.selectedRole.name || context.selectedRole.roleId}</span>
          </p>
        </div>
      {/if}

      <!-- Lead Section -->
      {#if lead}
        <div class="animate-in fade-in slide-in-from-bottom-1 duration-200 delay-75">
          <AgentCard
            testId="team-customizer-lead"
            role="lead"
            name={lead.name}
            tool={lead.tool}
            model={lead.model}
            reasoningEffort={lead.reasoningEffort}
            modelCatalog={catalog}
            projectId={lead.projectId}
            description={lead.description}
            {dark}
            onSave={updateLead}
          />
        </div>
      {/if}

      <!-- Agents Section -->
      <section class="space-y-3">
        {#each agents as agent, i (agent.id)}
          <div class="animate-in fade-in slide-in-from-bottom-1 duration-200" style:transition-delay={`${100 + (i * 50)}ms`}>
            <AgentCard
              testId={`team-customizer-agent-${agent.id}`}
              role="agent"
              name={agent.name}
              tool={agent.tool}
              model={agent.model}
              reasoningEffort={agent.reasoningEffort}
              modelCatalog={catalog}
              projectId={agent.projectId}
              description={agent.description}
              {dark}
              onSave={(payload) => updateAgent(agent.id, payload)}
              onRemove={() => removeAgent(agent.id)}
            />
          </div>
        {/each}
      </section>

      <div class="flex justify-center pt-1 animate-in fade-in duration-300 delay-200">
        <button
          class="h-10 px-6 rounded-lg border-2 border-dashed font-bold text-xs transition-all active:scale-95 flex items-center gap-2 {ghostTone}"
          onclick={addAgent}
          data-testid="team-customizer-add-agent"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="M12 5v14"/></svg>
          Add Agent Slot
        </button>
      </div>

      <!-- Save as Preset Drawer -->
      <div class="pt-4 mt-4 border-t {t.keyline}">
        {#if !showSavePresetDialog}
          <button
            class="w-full h-12 rounded-xl border border-dashed text-xs font-bold transition-all hover:border-brand-500/50 hover:bg-brand-500/5 active:scale-[0.99] flex items-center justify-center gap-2 {dark ? 'text-zinc-400 border-zinc-800' : 'text-zinc-500 border-zinc-200'}"
            onclick={() => {
              clearPresetSaveTimer()
              showSavePresetDialog = true
              newPresetName = localTeamName
              presetSaveError = false
              presetSaveMessage = ''
            }}
            disabled={hasErrors}
            data-testid="team-customizer-save-preset-trigger"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/></svg>
            Save as New Preset
          </button>
        {:else}
          <div class="space-y-4 p-4 -mx-4 border-t animate-in slide-in-from-bottom-2 duration-200 {dark ? 'bg-brand-950/40 border-brand-500/20' : 'bg-brand-50/80 border-brand-200'}" data-testid="save-preset-dialog">
            <header class="flex items-center justify-between">
              <p class="text-[10px] font-bold uppercase tracking-wider text-brand-500">Save as New Preset</p>
              <button 
                class="text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors"
                onclick={() => {
                  clearPresetSaveTimer()
                  showSavePresetDialog = false
                }}
                aria-label="Close"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
              </button>
            </header>
            
            <div class="space-y-3">
              <label class="space-y-1.5 block">
                <span class="text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Preset Name*</span>
                <input
                  class="h-10 w-full rounded-lg border px-3 text-sm {inputTone}"
                  bind:value={newPresetName}
                  placeholder="e.g. My Feature Team"
                  data-testid="save-preset-name-input"
                />
              </label>
              <label class="space-y-1.5 block">
                <span class="text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Description</span>
                <textarea
                  class="w-full rounded-lg border px-3 py-2 text-sm transition-all outline-none resize-none {inputTone}"
                  rows="2"
                  bind:value={newPresetDescription}
                  placeholder="Optional description"
                  data-testid="save-preset-description-input"
                ></textarea>
              </label>
            </div>
            
            {#if presetSaveMessage}
              <div class={`p-2 rounded-lg border animate-in fade-in zoom-in-95 duration-200 ${presetSaveError ? 'bg-danger-500/10 border-danger-500/20' : 'bg-success-500/10 border-success-500/20'}`}>
                <p class={`text-[11px] font-bold text-center ${presetSaveError ? 'text-danger-500' : 'text-success-600'}`} data-testid="save-preset-feedback">
                  {presetSaveMessage}
                </p>
              </div>
            {/if}

            <div class="flex justify-end gap-3 pt-1">
              <button
                class="h-10 px-4 rounded-lg text-xs font-bold transition-all active:scale-95 {dark ? 'text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]' : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100'}"
                onclick={() => {
                  clearPresetSaveTimer()
                  showSavePresetDialog = false
                  presetSaveMessage = ''
                  presetSaveError = false
                }}
                data-testid="save-preset-cancel"
              >
                Cancel
              </button>
              <button
                class="h-10 px-6 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 shadow-lg shadow-brand-500/20 disabled:opacity-50 disabled:pointer-events-none transition-all"
                onclick={handleSaveAsPreset}
                disabled={!newPresetName.trim() || isSavingPreset}
                data-testid="save-preset-confirm"
              >
                {isSavingPreset ? 'Saving...' : 'Save Preset'}
              </button>
            </div>
          </div>
        {/if}
      </div>

      <!-- Footer Sticky Actions -->
      <div class="fixed bottom-0 right-0 left-0 p-4 border-t backdrop-blur-md transition-all z-10 {dark ? 'bg-brand-950/80 border-white/[0.06]' : 'bg-white/80 border-brand-200/60'}" style="width: inherit; border-bottom-right-radius: inherit;">
        <div class="flex items-center justify-between max-w-full">
          <button
            class="h-10 px-4 rounded-lg text-xs font-bold text-danger-500 hover:bg-danger-500/10 transition-all active:scale-95"
            onclick={onReset}
            data-testid="team-customizer-reset"
          >
            Reset to Empty
          </button>
          <button
            class="h-10 px-8 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 shadow-lg shadow-brand-500/20 disabled:opacity-50 disabled:pointer-events-none transition-all"
            onclick={handleSave}
            disabled={hasErrors}
            data-testid="team-customizer-save"
          >
            Apply Changes
          </button>
        </div>
      </div>
    </section>
  {/snippet}
</SlideOver>
