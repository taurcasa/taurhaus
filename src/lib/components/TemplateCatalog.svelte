<script>
  import {
    composeTeam,
    getRoleTemplate,
    getTeamPreset,
    listRoleTemplates,
    listTeamPresets,
  } from '../ipc.js'
  import TeamComposer from './TeamComposer.svelte'
  import { getToolIcon, getToolName } from '../toolLogos.js'
  import { themeTokens } from '../themeTokens.js'

  let {
    dark = false,
    onCreateRoleTemplate = () => {},
    onCreateTeamPreset = () => {},
    onEditTemplate = () => {},
    onDeleteTemplate = () => {},
    onImportTemplates = () => {},
    onComposePreview = () => {},
    onComposeApply = () => {},
    onSaveComposedPreset = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const frameTone = $derived(
    dark
      ? 'border-brand-900/80 bg-brand-950/50'
      : 'border-brand-200 bg-brand-50/70'
  )
  const cardTone = $derived(
    dark
      ? 'border-zinc-700/70 bg-zinc-900/70 hover:border-brand-700/60'
      : 'border-zinc-200 bg-white hover:border-brand-300'
  )
  const inputTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500'
      : 'border-zinc-300 bg-white text-zinc-900 placeholder:text-zinc-400'
  )
  const actionSecondary = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800/80'
      : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'
  )

  let loading = $state(false)
  let errorMessage = $state('')
  let roleTemplates = $state([])
  let teamPresets = $state([])
  let searchQuery = $state('')
  let selectedTool = $state('all')
  let selectedCapability = $state('all')
  let selectedRoleId = $state('')
  let selectedPresetId = $state('')
  let selectedRole = $state(null)
  let selectedPreset = $state(null)
  let detailLoading = $state(false)
  let showComposer = $state(false)
  let composerInitialPreset = $state(null)

  const toolOptions = [
    { value: 'all', label: 'All tools' },
    { value: 'claude', label: 'Claude' },
    { value: 'codex', label: 'Codex' },
    { value: 'gemini', label: 'Gemini' },
  ]

  const capabilityOptions = $derived.by(() => {
    const values = new Set()
    for (const role of roleTemplates) {
      for (const capability of role.capabilities ?? []) values.add(capability)
    }
    for (const preset of teamPresets) {
      for (const capability of preset.capabilities ?? []) values.add(capability)
    }
    return ['all', ...Array.from(values).sort((left, right) => left.localeCompare(right))]
  })

  const filteredRoleTemplates = $derived.by(() => {
    return roleTemplates.filter((role) => {
      if (selectedTool !== 'all' && role.cliTool !== selectedTool) return false
      if (selectedCapability !== 'all' && !(role.capabilities ?? []).includes(selectedCapability)) return false
      if (!searchQuery.trim()) return true
      const query = searchQuery.trim().toLowerCase()
      return (
        String(role.name ?? '').toLowerCase().includes(query) ||
        String(role.roleId ?? '').toLowerCase().includes(query) ||
        String(role.model ?? '').toLowerCase().includes(query)
      )
    })
  })

  const filteredTeamPresets = $derived.by(() => {
    return teamPresets.filter((preset) => {
      if (selectedTool !== 'all' && !(preset.tools ?? []).includes(selectedTool)) return false
      if (
        selectedCapability !== 'all' &&
        !(preset.capabilities ?? []).includes(selectedCapability)
      ) {
        return false
      }
      if (!searchQuery.trim()) return true
      const query = searchQuery.trim().toLowerCase()
      return (
        String(preset.name ?? '').toLowerCase().includes(query) ||
        String(preset.presetId ?? '').toLowerCase().includes(query) ||
        String(preset.description ?? '').toLowerCase().includes(query)
      )
    })
  })

  function roleKindBadgeTone(kind) {
    if (kind === 'lead') {
      return dark
        ? 'border border-brand-500/40 bg-brand-500/10 text-brand-300'
        : 'border border-brand-300 bg-brand-100 text-brand-700'
    }
    return dark
      ? 'border border-zinc-600 bg-zinc-800 text-zinc-300'
      : 'border border-zinc-300 bg-zinc-100 text-zinc-700'
  }

  function builtInBadgeTone() {
    return dark
      ? 'border border-zinc-600 bg-zinc-800/90 text-zinc-300'
      : 'border border-zinc-300 bg-zinc-100 text-zinc-700'
  }

  function normalizeRoleTemplate(value) {
    return {
      roleId: value?.roleId ?? value?.role_id ?? '',
      name: value?.name ?? '',
      kind: String(value?.kind ?? 'agent').toLowerCase(),
      cliTool: String(value?.cliTool ?? value?.cli_tool ?? 'claude').toLowerCase(),
      model: value?.model ?? '',
      capabilities: Array.isArray(value?.capabilities) ? value.capabilities : [],
      builtIn: Boolean(value?.builtIn ?? value?.built_in),
      readOnly: Boolean(value?.readOnly ?? value?.read_only ?? value?.builtIn ?? value?.built_in),
    }
  }

  function normalizeTeamPreset(value) {
    return {
      presetId: value?.presetId ?? value?.preset_id ?? '',
      name: value?.name ?? '',
      description: value?.description ?? '',
      leadRoleId: value?.leadRoleId ?? value?.lead_role_id ?? '',
      roleCount: value?.roleCount ?? value?.role_count ?? 0,
      agentCount: value?.agentCount ?? value?.agent_count ?? 0,
      tools: Array.isArray(value?.tools) ? value.tools : [],
      capabilities: Array.isArray(value?.capabilities) ? value.capabilities : [],
      builtIn: Boolean(value?.builtIn ?? value?.built_in),
      readOnly: Boolean(value?.readOnly ?? value?.read_only ?? value?.builtIn ?? value?.built_in),
    }
  }

  async function loadCatalog() {
    loading = true
    errorMessage = ''
    try {
      const [roles, presets] = await Promise.all([listRoleTemplates(), listTeamPresets()])
      roleTemplates = (roles ?? []).map(normalizeRoleTemplate)
      teamPresets = (presets ?? []).map(normalizeTeamPreset)
    } catch (error) {
      errorMessage = error?.message || 'Failed to load template catalog.'
      roleTemplates = []
      teamPresets = []
    } finally {
      loading = false
    }
  }

  async function inspectRoleTemplate(roleId) {
    selectedRoleId = roleId
    selectedPresetId = ''
    detailLoading = true
    try {
      selectedRole = await getRoleTemplate(roleId)
      selectedPreset = null
    } catch (error) {
      errorMessage = error?.message || 'Failed to load role template details.'
      selectedRole = null
    } finally {
      detailLoading = false
    }
  }

  async function inspectTeamPreset(presetId) {
    selectedPresetId = presetId
    selectedRoleId = ''
    detailLoading = true
    try {
      selectedPreset = await getTeamPreset(presetId)
      selectedRole = null
    } catch (error) {
      errorMessage = error?.message || 'Failed to load team preset details.'
      selectedPreset = null
    } finally {
      detailLoading = false
    }
  }

  async function previewComposition(preset) {
    const request = {
      leadRoleId: preset.leadRoleId,
      agentSlots: preset.agentSlots ?? [],
      projectName: 'project',
    }
    const composed = await composeTeam(request)
    composerInitialPreset = preset
    showComposer = true
    onComposePreview({ preset, composed })
  }

  function toolIcon(tool) {
    return getToolIcon(tool)
  }

  function toolLabel(tool) {
    return getToolName(tool)
  }

  function capitalize(value) {
    const text = String(value ?? '')
    if (!text) return ''
    return text.charAt(0).toUpperCase() + text.slice(1)
  }

  function capabilityChipTone() {
    return dark
      ? 'border border-zinc-700 text-zinc-300 bg-zinc-900/80'
      : 'border border-zinc-200 text-zinc-700 bg-zinc-50'
  }

  $effect(() => {
    void loadCatalog()
  })
</script>

<section
  class="space-y-3 rounded-lg border p-3 {frameTone}"
  data-testid="template-catalog"
>
  <header class="flex items-center justify-between gap-3 border-b pb-2 {t.keyline}">
    <div>
      <h2 class="text-sm font-semibold {t.textPrimary}" data-testid="template-catalog-title">
        Template Catalog
      </h2>
      <p class="text-[11px] {t.textMuted}" data-testid="template-catalog-subtitle">
        Browse role templates and team presets before composing a roster.
      </p>
    </div>
    <div class="flex items-center gap-1.5">
      <button
        class="rounded-md border px-2 py-1 text-[11px] {actionSecondary}"
        onclick={() => onImportTemplates()}
        data-testid="template-import-button"
      >
        Import
      </button>
      <button
        class="rounded-md bg-brand-600 px-2 py-1 text-[11px] font-medium text-white hover:bg-brand-700"
        onclick={() => onCreateRoleTemplate()}
        data-testid="template-create-role-button"
      >
        New Role
      </button>
      <button
        class="rounded-md bg-brand-600 px-2 py-1 text-[11px] font-medium text-white hover:bg-brand-700"
        onclick={() => onCreateTeamPreset()}
        data-testid="template-create-preset-button"
      >
        New Preset
      </button>
    </div>
  </header>

  <div class="grid grid-cols-1 gap-2 md:grid-cols-3" data-testid="template-catalog-filters">
    <label class="flex flex-col gap-1">
      <span class="text-[10px] uppercase tracking-wide {t.textMuted}">Search</span>
      <input
        class="h-8 rounded-md border px-2 text-xs {inputTone}"
        placeholder="Filter by name, id, or model"
        value={searchQuery}
        oninput={(event) => {
          searchQuery = event.currentTarget.value
        }}
        data-testid="template-search-input"
      />
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-[10px] uppercase tracking-wide {t.textMuted}">CLI Tool</span>
      <select
        class="h-8 rounded-md border px-2 text-xs {inputTone}"
        value={selectedTool}
        onchange={(event) => {
          selectedTool = event.currentTarget.value
        }}
        data-testid="template-tool-filter"
      >
        {#each toolOptions as option}
          <option value={option.value}>{option.label}</option>
        {/each}
      </select>
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-[10px] uppercase tracking-wide {t.textMuted}">Capability</span>
      <select
        class="h-8 rounded-md border px-2 text-xs {inputTone}"
        value={selectedCapability}
        onchange={(event) => {
          selectedCapability = event.currentTarget.value
        }}
        data-testid="template-capability-filter"
      >
        {#each capabilityOptions as capability}
          <option value={capability}>
            {capability === 'all' ? 'All capabilities' : capability}
          </option>
        {/each}
      </select>
    </label>
  </div>

  {#if errorMessage}
    <p class="rounded-md border border-danger-400/40 bg-danger-500/10 px-2 py-1 text-xs text-danger-400">
      {errorMessage}
    </p>
  {/if}

  {#if loading}
    <p class="text-xs {t.textMuted}" data-testid="template-catalog-loading">Loading templates...</p>
  {:else}
    <div class="grid grid-cols-1 gap-3 xl:grid-cols-2">
      <section class="space-y-2" data-testid="role-template-section">
        <div class="flex items-center justify-between">
          <h3 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">
            Role Templates ({filteredRoleTemplates.length})
          </h3>
        </div>

        {#if filteredRoleTemplates.length === 0}
          <p class="rounded-md border px-2 py-2 text-xs {t.textMuted} {cardTone}">
            No role templates match the current filters.
          </p>
        {:else}
          <div class="space-y-2">
            {#each filteredRoleTemplates as role}
              <article
                class="rounded-md border p-2 transition-colors {cardTone} {selectedRoleId === role.roleId ? (dark ? 'ring-1 ring-brand-500/50' : 'ring-1 ring-brand-400/70') : ''}"
                data-testid={`role-template-card-${role.roleId}`}
              >
                <div class="flex items-start justify-between gap-2">
                  <div class="min-w-0">
                    <p class="truncate text-[13px] font-medium {t.textPrimary}">{role.name}</p>
                    <p class="text-[10px] {t.textMuted}">{role.roleId}</p>
                  </div>
                  <div class="flex items-center gap-1">
                    <span class="rounded-full px-1.5 py-0.5 text-[10px] {roleKindBadgeTone(role.kind)}">
                      {role.kind}
                    </span>
                    {#if role.builtIn}
                      <span class="rounded-full px-1.5 py-0.5 text-[10px] {builtInBadgeTone()}">
                        Built-in
                      </span>
                    {/if}
                  </div>
                </div>

                <div class="mt-1 flex items-center gap-1 text-[11px] {t.textSecondary}">
                  <svg
                    class="h-3 w-3 shrink-0"
                    viewBox={toolIcon(role.cliTool).viewBox}
                    fill="currentColor"
                    aria-hidden="true"
                  >
                    <path d={toolIcon(role.cliTool).path}></path>
                  </svg>
                  <span>{toolLabel(role.cliTool)}</span>
                  <span class={t.textMuted}>|</span>
                  <span>{role.model}</span>
                </div>

                <div class="mt-1 flex flex-wrap gap-1">
                  {#each role.capabilities as capability}
                    <span class="rounded-full px-1.5 py-0.5 text-[10px] {capabilityChipTone()}">
                      {capability}
                    </span>
                  {/each}
                </div>

                <div class="mt-2 flex items-center justify-end gap-1">
                  <button
                    class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                    onclick={() => {
                      void inspectRoleTemplate(role.roleId)
                    }}
                    data-testid={`role-inspect-${role.roleId}`}
                  >
                    Inspect
                  </button>
                  {#if role.readOnly}
                    <span class="text-[10px] {t.textMuted}" data-testid={`role-readonly-${role.roleId}`}>
                      Read-only
                    </span>
                  {:else}
                    <button
                      class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                      onclick={(event) => {
                        event.stopPropagation()
                        onEditTemplate({ kind: 'role', id: role.roleId })
                      }}
                      data-testid={`role-edit-${role.roleId}`}
                    >
                      Edit
                    </button>
                    <button
                      class="rounded border border-danger-500/40 px-1.5 py-0.5 text-[10px] text-danger-500 hover:bg-danger-500/10"
                      onclick={(event) => {
                        event.stopPropagation()
                        onDeleteTemplate({ kind: 'role', id: role.roleId })
                      }}
                      data-testid={`role-delete-${role.roleId}`}
                    >
                      Delete
                    </button>
                  {/if}
                </div>
              </article>
            {/each}
          </div>
        {/if}
      </section>

      <section class="space-y-2" data-testid="team-preset-section">
        <div class="flex items-center justify-between">
          <h3 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">
            Team Presets ({filteredTeamPresets.length})
          </h3>
        </div>

        {#if filteredTeamPresets.length === 0}
          <p class="rounded-md border px-2 py-2 text-xs {t.textMuted} {cardTone}">
            No team presets match the current filters.
          </p>
        {:else}
          <div class="space-y-2">
            {#each filteredTeamPresets as preset}
              <article
                class="rounded-md border p-2 transition-colors {cardTone} {selectedPresetId === preset.presetId ? (dark ? 'ring-1 ring-brand-500/50' : 'ring-1 ring-brand-400/70') : ''}"
                data-testid={`team-preset-card-${preset.presetId}`}
              >
                <div class="flex items-start justify-between gap-2">
                  <div class="min-w-0">
                    <p class="truncate text-[13px] font-medium {t.textPrimary}">{preset.name}</p>
                    <p class="text-[10px] {t.textMuted}">{preset.presetId}</p>
                  </div>
                  <div class="flex items-center gap-1">
                    {#if preset.builtIn}
                      <span class="rounded-full px-1.5 py-0.5 text-[10px] {builtInBadgeTone()}">
                        Built-in
                      </span>
                    {/if}
                  </div>
                </div>

                <p class="mt-1 text-[11px] {t.textSecondary}">
                  {preset.description}
                </p>
                <p class="mt-1 text-[10px] {t.textMuted}">
                  {preset.roleCount} role type(s) | {preset.agentCount} agent(s) | lead {preset.leadRoleId}
                </p>

                <div class="mt-1 flex flex-wrap gap-1">
                  {#each preset.tools ?? [] as tool}
                    <span class="rounded-full px-1.5 py-0.5 text-[10px] {capabilityChipTone()}">{tool}</span>
                  {/each}
                </div>

                <div class="mt-2 flex items-center justify-end gap-1">
                  <button
                    class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                    onclick={() => {
                      void inspectTeamPreset(preset.presetId)
                    }}
                    data-testid={`preset-inspect-${preset.presetId}`}
                  >
                    Inspect
                  </button>
                  <button
                    class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                    onclick={async (event) => {
                      event.stopPropagation()
                      await previewComposition(await getTeamPreset(preset.presetId))
                    }}
                    data-testid={`preset-preview-${preset.presetId}`}
                  >
                    Preview
                  </button>
                  {#if preset.readOnly}
                    <span class="text-[10px] {t.textMuted}" data-testid={`preset-readonly-${preset.presetId}`}>
                      Read-only
                    </span>
                  {:else}
                    <button
                      class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                      onclick={(event) => {
                        event.stopPropagation()
                        onEditTemplate({ kind: 'preset', id: preset.presetId })
                      }}
                      data-testid={`preset-edit-${preset.presetId}`}
                    >
                      Edit
                    </button>
                    <button
                      class="rounded border border-danger-500/40 px-1.5 py-0.5 text-[10px] text-danger-500 hover:bg-danger-500/10"
                      onclick={(event) => {
                        event.stopPropagation()
                        onDeleteTemplate({ kind: 'preset', id: preset.presetId })
                      }}
                      data-testid={`preset-delete-${preset.presetId}`}
                    >
                      Delete
                    </button>
                  {/if}
                </div>
              </article>
            {/each}
          </div>
        {/if}
      </section>
    </div>

    <section class="rounded-md border p-2 {cardTone}" data-testid="template-detail-panel">
      <h4 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">
        Template Details
      </h4>

      {#if detailLoading}
        <p class="mt-2 text-xs {t.textMuted}">Loading details...</p>
      {:else if selectedRole}
        <div class="mt-2 space-y-1">
          <p class="text-[13px] font-medium {t.textPrimary}">
            {selectedRole.name} ({selectedRole.roleId})
          </p>
          <p class="text-[11px] {t.textSecondary}">
            {selectedRole.instructions}
          </p>
          <div class="text-[10px] {t.textMuted}">
            <p>Communication: {(selectedRole.behavioralContract?.communication ?? []).length} bullets</p>
            <p>Execution: {(selectedRole.behavioralContract?.execution ?? []).length} bullets</p>
            <p>Escalation: {(selectedRole.behavioralContract?.escalation ?? []).length} bullets</p>
          </div>
        </div>
      {:else if selectedPreset}
        <div class="mt-2 space-y-1">
          <p class="text-[13px] font-medium {t.textPrimary}">
            {selectedPreset.name} ({selectedPreset.presetId})
          </p>
          <p class="text-[11px] {t.textSecondary}">
            {selectedPreset.description}
          </p>
          <p class="text-[10px] {t.textMuted}">
            {(selectedPreset.agentSlots ?? []).length} slot(s) configured.
          </p>
        </div>
      {:else}
        <p class="mt-2 text-xs {t.textMuted}">
          Select a role template or team preset to inspect details.
        </p>
      {/if}
    </section>

    {#if showComposer}
      <section class="rounded-md border p-2 {cardTone}" data-testid="template-composer-panel">
        <TeamComposer
          dark={dark}
          initialPreset={composerInitialPreset}
          onApply={(payload) => {
            onComposeApply(payload)
          }}
          onSavePreset={(payload) => {
            onSaveComposedPreset(payload)
          }}
          onClose={() => {
            showComposer = false
          }}
        />
      </section>
    {/if}
  {/if}
</section>
