<script>
  import {
    deleteTeamPreset,
    deleteRoleTemplate,
    getRoleTemplate,
    getTeamPreset,
    listRoleTemplates,
    listTeamPresets,
    upsertTeamPreset,
    upsertRoleTemplate,
  } from '../ipc.js'
  import { getToolIcon, getToolName } from '../toolLogos.js'
  import { themeTokens } from '../themeTokens.js'
  import ConfirmDialog from './ConfirmDialog.svelte'
  import PresetCard from './PresetCard.svelte'
  import RoleEditor from './RoleEditor.svelte'
  import SlideOver from './SlideOver.svelte'
  import TeamCustomizerPanel from './TeamCustomizerPanel.svelte'
  import TemplateHistoryPanel from './TemplateHistoryPanel.svelte'

  let {
    open = false,
    dark = false,
    onClose = () => {},
    onSelectPreset = () => {},
    onSelectRole = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const tabBase = 'px-2 py-1 text-xs border-b-2 transition-colors'
  const tabActive = $derived(dark ? `font-medium ${t.textPrimary} border-brand-500` : `font-medium ${t.textPrimary} border-brand-500`)
  const tabInactive = $derived(`${t.textMuted} border-transparent hover:text-zinc-500`)
  const inputTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500'
      : 'border-zinc-300 bg-white text-zinc-900 placeholder:text-zinc-400'
  )
  const cardTone = $derived(
    dark
      ? 'border-zinc-700/70 bg-zinc-900/70 hover:border-brand-700/60'
      : 'border-zinc-200 bg-white hover:border-brand-300'
  )
  const actionSecondary = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800/80'
      : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'
  )
  const toneMuted = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')

  let loading = $state(false)
  let errorMessage = $state('')
  let roleTemplates = $state([])
  let teamPresets = $state([])
  let searchQuery = $state('')
  let activeTab = $state('roles')

  let detailKind = $state('')
  let detailLoading = $state(false)
  let selectedRole = $state(null)
  let selectedPreset = $state(null)
  let historyTemplateId = $state('')
  let historyTemplateKind = $state('')
  let roleEditorOpen = $state(false)
  let roleEditorRole = $state(null)
  let deleteRoleId = $state('')
  let deleteRoleName = $state('')
  let presetEditorOpen = $state(false)
  let presetEditorMode = $state('create')
  let presetEditorDraft = $state(null)
  let presetEditorTeamConfig = $state(null)
  let deletePresetId = $state('')
  let deletePresetName = $state('')

  const filteredRoleTemplates = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase()
    return roleTemplates.filter((role) => {
      if (!query) return true
      return (
        String(role.name ?? '').toLowerCase().includes(query) ||
        String(role.roleId ?? '').toLowerCase().includes(query) ||
        String(role.model ?? '').toLowerCase().includes(query)
      )
    })
  })

  const hasCustomRoles = $derived.by(() => roleTemplates.some((role) => !role.builtIn))

  const filteredTeamPresets = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase()
    return teamPresets.filter((preset) => {
      if (!query) return true
      return (
        String(preset.name ?? '').toLowerCase().includes(query) ||
        String(preset.presetId ?? '').toLowerCase().includes(query) ||
        String(preset.description ?? '').toLowerCase().includes(query)
      )
    })
  })

  function normalizeRoleTemplate(value) {
    return {
      roleId: value?.roleId ?? value?.role_id ?? '',
      name: value?.name ?? '',
      kind: String(value?.kind ?? 'agent').toLowerCase(),
      cliTool: String(value?.cliTool ?? value?.cli_tool ?? 'claude').toLowerCase(),
      model: value?.model ?? '',
      capabilities: Array.isArray(value?.capabilities) ? value.capabilities : [],
      instructions: value?.instructions ?? '',
      behavioralContract:
        value?.behavioralContract ?? value?.behavioral_contract ?? [],
      builtIn: Boolean(value?.builtIn ?? value?.built_in),
      readOnly: Boolean(value?.readOnly ?? value?.read_only),
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
      readOnly: Boolean(value?.readOnly ?? value?.read_only),
    }
  }

  function resetDetail() {
    detailKind = ''
    selectedRole = null
    selectedPreset = null
    detailLoading = false
  }

  function resetRoleEditor() {
    roleEditorOpen = false
    roleEditorRole = null
  }

  function setTab(tab) {
    activeTab = tab
    resetDetail()
  }

  function isCustomRole(role) {
    return !Boolean(role?.builtIn)
  }

  function isCustomPreset(preset) {
    return !Boolean(preset?.builtIn || preset?.readOnly)
  }

  function toSlug(value) {
    return String(value ?? '')
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
  }

  function ensureUniquePresetId(baseId, currentId = '') {
    const normalizedBase = toSlug(baseId) || 'custom-preset'
    const existing = new Set((teamPresets ?? []).map((preset) => preset?.presetId).filter(Boolean))
    existing.delete(currentId)

    if (!existing.has(normalizedBase)) {
      return normalizedBase
    }

    let idx = 2
    while (existing.has(`${normalizedBase}-${idx}`)) {
      idx += 1
    }
    return `${normalizedBase}-${idx}`
  }

  function resolveRoleById(roleId) {
    return roleTemplates.find((role) => role.roleId === roleId) ?? null
  }

  function defaultLeadRoleId() {
    return (
      roleTemplates.find((role) => role.kind === 'lead')?.roleId ??
      roleTemplates[0]?.roleId ??
      'claude-orchestrator'
    )
  }

  function defaultAgentRoleId() {
    return (
      roleTemplates.find((role) => role.kind === 'agent')?.roleId ??
      roleTemplates.find((role) => role.roleId !== defaultLeadRoleId())?.roleId ??
      roleTemplates[0]?.roleId ??
      'codex-developer'
    )
  }

  function normalizePresetDraft(source = {}) {
    const leadRoleId = source?.leadRoleId ?? source?.lead_role_id ?? defaultLeadRoleId()
    const slots = Array.isArray(source?.agentSlots ?? source?.agent_slots)
      ? (source?.agentSlots ?? source?.agent_slots)
      : []
    const agentSlots = slots.length > 0
      ? slots.map((slot) => ({
        roleId: slot?.roleId ?? slot?.role_id ?? defaultAgentRoleId(),
        count: Math.max(1, Number(slot?.count ?? 1)),
        projectBinding: slot?.projectBinding ?? slot?.project_binding ?? 'lead_project',
        projectId: slot?.projectId ?? slot?.project_id ?? null,
      }))
      : [{
        roleId: defaultAgentRoleId(),
        count: 1,
        projectBinding: 'lead_project',
        projectId: null,
      }]

    return {
      presetId: source?.presetId ?? source?.preset_id ?? ensureUniquePresetId('custom-preset'),
      name: source?.name ?? 'New Preset',
      description: source?.description ?? 'Custom team preset',
      version: source?.version ?? '1.0.0',
      leadRoleId,
      agentSlots,
      defaults: {
        teamNamePattern: source?.defaults?.teamNamePattern ?? source?.defaults?.team_name_pattern ?? '{project}-team',
        tmuxLayout: source?.defaults?.tmuxLayout ?? source?.defaults?.tmux_layout ?? 'tiled',
      },
    }
  }

  function presetDraftToTeamConfig(presetDraft) {
    const draft = normalizePresetDraft(presetDraft)
    const leadRole = resolveRoleById(draft.leadRoleId)
    const agentRoleCounts = new Map()
    const agents = []
    let nextAgent = 1

    for (const slot of draft.agentSlots) {
      const role = resolveRoleById(slot.roleId)
      for (let idx = 0; idx < slot.count; idx += 1) {
        const previous = agentRoleCounts.get(slot.roleId) ?? 0
        agentRoleCounts.set(slot.roleId, previous + 1)
        const roleSequence = agentRoleCounts.get(slot.roleId)
        const roleName = role?.name || 'agent'
        agents.push({
          id: `agent-${nextAgent}`,
          name: slot.count > 1 ? `${roleName}-${roleSequence}` : roleName,
          tool: role?.cliTool ?? 'codex',
          model: role?.model ?? 'gpt-5.3-codex',
          projectId: '',
          description: slot.roleId,
        })
        nextAgent += 1
      }
    }

    return {
      teamName: draft.name,
      description: draft.description,
      presetId: draft.presetId,
      lead: {
        id: 'lead',
        name: leadRole?.name || 'team-lead',
        tool: leadRole?.cliTool ?? 'claude',
        model: leadRole?.model ?? 'claude-opus-4-6',
        projectId: '',
        description: draft.leadRoleId,
      },
      agents,
    }
  }

  function capabilityTestId(roleId, capability) {
    const normalized = String(capability ?? '')
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
    return `role-capability-${roleId}-${normalized}`
  }

  async function refreshRoles() {
    const roles = await listRoleTemplates()
    roleTemplates = (roles ?? []).map(normalizeRoleTemplate)
  }

  async function refreshPresets() {
    const presets = await listTeamPresets()
    teamPresets = (presets ?? []).map(normalizeTeamPreset)
  }

  async function loadCatalog() {
    loading = true
    errorMessage = ''
    try {
      await Promise.all([refreshRoles(), refreshPresets()])
    } catch (error) {
      roleTemplates = []
      teamPresets = []
      errorMessage = error?.message || 'Failed to load template catalog.'
    } finally {
      loading = false
    }
  }

  function openCreateRoleEditor() {
    resetDetail()
    roleEditorRole = null
    roleEditorOpen = true
  }

  async function openEditRoleEditor(role) {
    resetDetail()
    errorMessage = ''
    try {
      const detail = await getRoleTemplate(role.roleId)
      const merged = normalizeRoleTemplate({ ...role, ...detail })
      roleEditorRole = {
        ...merged,
        tool: merged.cliTool,
      }
    } catch {
      roleEditorRole = {
        ...role,
        tool: role.cliTool,
      }
    }
    roleEditorOpen = true
  }

  async function handleRoleSave(roleData) {
    errorMessage = ''
    try {
      await upsertRoleTemplate(roleData)
      resetRoleEditor()
      await refreshRoles()
    } catch (error) {
      errorMessage = error?.message || 'Failed to save role template.'
    }
  }

  function requestRoleDelete(role) {
    deleteRoleId = role.roleId
    deleteRoleName = role.name
  }

  function cancelRoleDelete() {
    deleteRoleId = ''
    deleteRoleName = ''
  }

  async function confirmRoleDelete() {
    if (!deleteRoleId) return

    const targetRoleId = deleteRoleId
    cancelRoleDelete()
    errorMessage = ''
    try {
      await deleteRoleTemplate(targetRoleId)
      if (selectedRole?.roleId === targetRoleId) {
        resetDetail()
      }
      await refreshRoles()
    } catch (error) {
      errorMessage = error?.message || 'Failed to delete role template.'
    }
  }

  function closePresetEditor() {
    presetEditorOpen = false
    presetEditorMode = 'create'
    presetEditorDraft = null
    presetEditorTeamConfig = null
  }

  function openCreatePresetEditor() {
    resetDetail()
    const draft = normalizePresetDraft({
      presetId: ensureUniquePresetId('custom-preset'),
      name: 'New Preset',
      description: 'Custom team preset',
      leadRoleId: defaultLeadRoleId(),
      agentSlots: [{
        roleId: defaultAgentRoleId(),
        count: 1,
        projectBinding: 'lead_project',
        projectId: null,
      }],
    })
    presetEditorMode = 'create'
    presetEditorDraft = draft
    presetEditorTeamConfig = presetDraftToTeamConfig(draft)
    presetEditorOpen = true
  }

  async function openPresetEditorForMutation(preset, mode) {
    if (!preset?.presetId) return

    resetDetail()
    errorMessage = ''
    let detail = null
    try {
      detail = await getTeamPreset(preset.presetId)
    } catch {
      detail = null
    }

    const merged = normalizePresetDraft({
      ...preset,
      ...(detail ?? {}),
    })
    if (mode === 'duplicate') {
      merged.name = `Copy of ${merged.name}`
      merged.presetId = ensureUniquePresetId(`copy-of-${merged.presetId || merged.name}`)
    }
    if (mode === 'create') {
      merged.presetId = ensureUniquePresetId(merged.presetId || merged.name)
    }

    presetEditorMode = mode
    presetEditorDraft = merged
    presetEditorTeamConfig = presetDraftToTeamConfig(merged)
    presetEditorOpen = true
  }

  async function savePresetFromCustomizer(payload) {
    if (!presetEditorDraft) return

    const name = String(payload?.teamName ?? presetEditorDraft.name ?? '').trim() || 'New Preset'
    const description = String(payload?.description ?? presetEditorDraft.description ?? '').trim() || 'Custom team preset'
    const currentId = presetEditorMode === 'edit' ? presetEditorDraft.presetId : ''
    const desiredId = presetEditorMode === 'edit'
      ? (presetEditorDraft.presetId || ensureUniquePresetId(name))
      : ensureUniquePresetId(name, currentId)

    const draft = normalizePresetDraft({
      ...presetEditorDraft,
      presetId: desiredId,
      name,
      description,
    })

    errorMessage = ''
    try {
      await upsertTeamPreset({
        schema: {
          kind: 'team_preset',
          version: 1,
        },
        presetId: draft.presetId,
        name: draft.name,
        description: draft.description,
        version: draft.version,
        leadRoleId: draft.leadRoleId,
        agentSlots: draft.agentSlots,
        defaults: draft.defaults,
      })
      closePresetEditor()
      await refreshPresets()
    } catch (error) {
      errorMessage = error?.message || 'Failed to save team preset.'
    }
  }

  function requestPresetDelete(preset) {
    if (!preset?.presetId || !isCustomPreset(preset)) return
    deletePresetId = preset.presetId
    deletePresetName = preset.name
  }

  function cancelPresetDelete() {
    deletePresetId = ''
    deletePresetName = ''
  }

  async function confirmPresetDelete() {
    if (!deletePresetId) return

    const targetPresetId = deletePresetId
    cancelPresetDelete()
    errorMessage = ''
    try {
      await deleteTeamPreset(targetPresetId)
      if (selectedPreset?.presetId === targetPresetId) {
        resetDetail()
      }
      await refreshPresets()
    } catch (error) {
      errorMessage = error?.message || 'Failed to delete team preset.'
    }
  }

  async function inspectRole(role) {
    detailKind = 'role'
    detailLoading = true
    errorMessage = ''
    try {
      const detail = await getRoleTemplate(role.roleId)
      selectedRole = { ...role, ...detail }
      selectedPreset = null
      historyTemplateId = role.roleId
      historyTemplateKind = 'role'
    } catch (error) {
      selectedRole = { ...role }
      selectedPreset = null
      errorMessage = error?.message || 'Failed to load role template details.'
    } finally {
      detailLoading = false
    }
  }

  async function inspectPreset(preset) {
    detailKind = 'preset'
    detailLoading = true
    errorMessage = ''
    try {
      const detail = await getTeamPreset(preset.presetId)
      selectedPreset = { ...preset, ...detail }
      selectedRole = null
      historyTemplateId = preset.presetId
      historyTemplateKind = 'preset'
    } catch (error) {
      selectedPreset = { ...preset }
      selectedRole = null
      errorMessage = error?.message || 'Failed to load team preset details.'
    } finally {
      detailLoading = false
    }
  }

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

  function capabilityChipTone() {
    return dark
      ? 'border border-zinc-700 text-zinc-300 bg-zinc-900/80'
      : 'border border-zinc-200 text-zinc-700 bg-zinc-50'
  }

  $effect(() => {
    if (!open) return
    void loadCatalog()
  })
</script>

<SlideOver {open} title="Templates" width={420} {dark} onClose={onClose}>
  {#snippet children()}
    <section class="space-y-3" data-testid="template-browser-panel">
      <header class="space-y-2">
        <div class="flex items-center gap-1.5 border-b {t.keyline}">
          <button
            class="{tabBase} {activeTab === 'roles' ? tabActive : tabInactive}"
            onclick={() => setTab('roles')}
            data-testid="catalog-tab-roles"
          >
            Roles
          </button>
          <button
            class="{tabBase} {activeTab === 'presets' ? tabActive : tabInactive}"
            onclick={() => setTab('presets')}
            data-testid="catalog-tab-presets"
          >
            Presets
          </button>
          <button
            class="{tabBase} {activeTab === 'history' ? tabActive : tabInactive}"
            onclick={() => setTab('history')}
            data-testid="catalog-tab-history"
          >
            History
          </button>
        </div>

        {#if activeTab !== 'history'}
          <label class="flex flex-col gap-1">
            <span class="text-[11px] uppercase tracking-wide {t.textMuted}">Filter</span>
            <input
              class="h-8 rounded-md border px-2 text-xs {inputTone}"
              placeholder={activeTab === 'roles' ? 'Search roles by name, id, or model' : 'Search presets by name, id, or description'}
              value={searchQuery}
              oninput={(event) => {
                searchQuery = event.currentTarget.value
              }}
              data-testid="template-browser-search-input"
            />
          </label>
        {/if}
      </header>

      {#if errorMessage}
        <p class="rounded-md border border-danger-400/40 bg-danger-500/10 px-2 py-1 text-xs text-danger-400">
          {errorMessage}
        </p>
      {/if}

      {#if loading}
        <p class="text-xs {t.textMuted}" data-testid="template-browser-loading">Loading templates...</p>
      {:else if activeTab === 'roles'}
        {#if detailKind === 'role'}
          <section class="rounded-md border p-3 space-y-2 {cardTone}" data-testid="template-role-detail">
            <button
              class="inline-flex items-center gap-1 rounded px-2 py-1 text-[11px] {toneMuted} hover:text-brand-500"
              onclick={resetDetail}
              data-testid="template-role-back"
            >
              ← Back
            </button>

            {#if detailLoading}
              <p class="text-xs {toneMuted}">Loading role details...</p>
            {:else if selectedRole}
              <h3 class="text-sm font-semibold {t.textPrimary}">
                {selectedRole.name} ({selectedRole.roleId})
              </h3>
              <p class="text-xs {t.textSecondary}">
                {selectedRole.instructions || 'No role instructions available.'}
              </p>
              <button
                class="rounded-md bg-brand-600 px-2 py-1 text-xs font-medium text-white hover:bg-brand-700"
                onclick={() => onSelectRole(selectedRole)}
                data-testid={`role-select-${selectedRole.roleId}`}
              >
                Use role
              </button>
            {/if}
          </section>
        {:else if filteredRoleTemplates.length === 0}
          <p class="rounded-md border px-2 py-2 text-xs {t.textMuted} {cardTone}">
            No role templates match the current filter.
          </p>
        {:else}
          <div class="flex items-center justify-between">
            <p class="text-xs font-medium {t.textSecondary}">Role Templates</p>
            <button
              class="rounded border px-2 py-1 text-[11px] font-medium {actionSecondary}"
              onclick={openCreateRoleEditor}
              data-testid="role-create-button"
            >
              + Create
            </button>
          </div>

          {#if !hasCustomRoles}
            <p class="rounded-md border px-2 py-2 text-xs {t.textMuted} {cardTone}" data-testid="role-custom-empty-state">
              No custom roles yet. Create one or capture from a live team.
            </p>
          {/if}

          <div class="space-y-2" data-testid="template-role-list">
            {#each filteredRoleTemplates as role}
              <article class="rounded-md border p-2 transition-colors {cardTone}" data-testid={`role-template-card-${role.roleId}`}>
                <div class="flex items-start justify-between gap-2">
                  <div class="min-w-0">
                    <p class="truncate text-[13px] font-medium {t.textPrimary}">{role.name}</p>
                    <p class="text-[10px] {t.textMuted}">{role.roleId}</p>
                  </div>
                  <span class="rounded-full px-1.5 py-0.5 text-[10px] {roleKindBadgeTone(role.kind)}">{role.kind}</span>
                </div>

                <div class="mt-1 flex flex-wrap items-center gap-1 text-[11px] {t.textSecondary}">
                  <span
                    class="inline-flex items-center gap-1 rounded-full border px-1.5 py-0.5 text-[10px] {capabilityChipTone()}"
                    data-testid={`role-tool-badge-${role.roleId}`}
                  >
                    <svg class="h-3 w-3 shrink-0" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
                      <path d={getToolIcon(role.cliTool).path}></path>
                    </svg>
                    <span>{getToolName(role.cliTool)}</span>
                  </span>
                  <span
                    class="inline-flex items-center rounded-full border px-1.5 py-0.5 text-[10px] {capabilityChipTone()}"
                    data-testid={`role-model-badge-${role.roleId}`}
                  >
                    {role.model || 'unspecified'}
                  </span>
                </div>

                <div class="mt-1 flex flex-wrap gap-1">
                  {#each role.capabilities as capability}
                    <span
                      class="rounded-full px-1.5 py-0.5 text-[10px] {capabilityChipTone()}"
                      data-testid={capabilityTestId(role.roleId, capability)}
                    >
                      {capability}
                    </span>
                  {/each}
                </div>

                <div class="mt-2 flex flex-wrap justify-end gap-1">
                  <button
                    class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                    onclick={() => onSelectRole(role)}
                    data-testid={`role-use-${role.roleId}`}
                  >
                    Use
                  </button>
                  <button
                    class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                    onclick={() => {
                      void inspectRole(role)
                    }}
                    data-testid={`role-inspect-${role.roleId}`}
                  >
                    Inspect
                  </button>
                  {#if isCustomRole(role)}
                    <button
                      class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                      onclick={() => {
                        void openEditRoleEditor(role)
                      }}
                      data-testid={`role-edit-${role.roleId}`}
                    >
                      Edit
                    </button>
                    <button
                      class="rounded border px-1.5 py-0.5 text-[10px] border-danger-400/50 text-danger-500 hover:bg-danger-500/10"
                      onclick={() => requestRoleDelete(role)}
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
      {:else if activeTab === 'presets'}
        {#if detailKind === 'preset'}
          <section class="rounded-md border p-3 space-y-2 {cardTone}" data-testid="template-preset-detail">
            <button
              class="inline-flex items-center gap-1 rounded px-2 py-1 text-[11px] {toneMuted} hover:text-brand-500"
              onclick={resetDetail}
              data-testid="template-preset-back"
            >
              ← Back
            </button>

            {#if detailLoading}
              <p class="text-xs {toneMuted}">Loading preset details...</p>
            {:else if selectedPreset}
              <h3 class="text-sm font-semibold {t.textPrimary}">
                {selectedPreset.name} ({selectedPreset.presetId})
              </h3>
              <p class="text-xs {t.textSecondary}">
                {selectedPreset.description || 'No preset description provided.'}
              </p>
              <p class="text-[11px] {toneMuted}">
                {(selectedPreset.agentSlots ?? []).length} slot(s) configured.
              </p>
              <button
                class="rounded-md bg-brand-600 px-2 py-1 text-xs font-medium text-white hover:bg-brand-700"
                onclick={() => onSelectPreset(selectedPreset)}
                data-testid={`preset-select-${selectedPreset.presetId}`}
              >
                Use preset
              </button>
            {/if}
          </section>
        {:else}
          <div class="flex items-center justify-between">
            <p class="text-xs font-medium {t.textSecondary}">Team Presets</p>
            <button
              class="rounded border px-2 py-1 text-[11px] font-medium {actionSecondary}"
              onclick={openCreatePresetEditor}
              data-testid="template-preset-create"
            >
              + Create
            </button>
          </div>

          {#if filteredTeamPresets.length === 0}
            <p class="rounded-md border px-2 py-2 text-xs {t.textMuted} {cardTone}">
              No team presets match the current filter.
            </p>
          {:else}
            <div class="space-y-2" data-testid="template-preset-list">
              {#each filteredTeamPresets as preset}
                <article class="space-y-1.5 rounded-md border p-2 transition-colors {cardTone}">
                  <PresetCard
                    dark={dark}
                    name={preset.name}
                    description={preset.description}
                    leadCount={Math.max(1, Number(preset.roleCount ?? 1) - Number(preset.agentCount ?? 0))}
                    agentCount={preset.agentCount}
                    tools={preset.tools}
                    builtIn={preset.builtIn}
                    onSelect={() => {
                      onSelectPreset(preset)
                    }}
                    onInspect={() => {
                      void inspectPreset(preset)
                    }}
                    testId={`template-browser-preset-${preset.presetId}`}
                  />

                  <div class="flex flex-wrap justify-end gap-1">
                    <button
                      class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                      onclick={() => onSelectPreset(preset)}
                      data-testid={`template-preset-use-${preset.presetId}`}
                    >
                      Use
                    </button>
                    <button
                      class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                      onclick={() => {
                        void inspectPreset(preset)
                      }}
                      data-testid={`template-preset-inspect-${preset.presetId}`}
                    >
                      Inspect
                    </button>
                    {#if isCustomPreset(preset)}
                      <button
                        class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                        onclick={() => {
                          void openPresetEditorForMutation(preset, 'edit')
                        }}
                        data-testid={`template-preset-edit-${preset.presetId}`}
                      >
                        Edit
                      </button>
                      <button
                        class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                        onclick={() => {
                          void openPresetEditorForMutation(preset, 'duplicate')
                        }}
                        data-testid={`template-preset-duplicate-${preset.presetId}`}
                      >
                        Duplicate
                      </button>
                      <button
                        class="rounded border px-1.5 py-0.5 text-[10px] border-danger-400/50 text-danger-500 hover:bg-danger-500/10"
                        onclick={() => requestPresetDelete(preset)}
                        data-testid={`template-preset-delete-${preset.presetId}`}
                      >
                        Delete
                      </button>
                    {/if}
                  </div>
                </article>
              {/each}
            </div>
          {/if}
        {/if}
      {:else}
        <TemplateHistoryPanel
          dark={dark}
          selectedTemplateId={historyTemplateId}
          selectedTemplateKind={historyTemplateKind}
        />
      {/if}
    </section>
  {/snippet}
</SlideOver>

<RoleEditor
  open={roleEditorOpen}
  {dark}
  role={roleEditorRole}
  onSave={handleRoleSave}
  onCancel={resetRoleEditor}
  onDelete={(roleId) => {
    const role = roleTemplates.find((entry) => entry.roleId === roleId)
    if (role) requestRoleDelete(role)
    resetRoleEditor()
  }}
/>

<TeamCustomizerPanel
  open={presetEditorOpen}
  {dark}
  teamConfig={presetEditorTeamConfig}
  onClose={closePresetEditor}
  onSave={savePresetFromCustomizer}
  onReset={closePresetEditor}
/>

{#if deleteRoleId}
  <ConfirmDialog
    {dark}
    open={true}
    title="Delete role template?"
    message={`Delete ${deleteRoleName || deleteRoleId}? This cannot be undone.`}
    confirmLabel="Delete"
    variant="danger"
    onconfirm={confirmRoleDelete}
    oncancel={cancelRoleDelete}
  />
{/if}

{#if deletePresetId}
  <ConfirmDialog
    {dark}
    open={true}
    title="Delete team preset?"
    message={`Delete ${deletePresetName || deletePresetId}? This cannot be undone.`}
    confirmLabel="Delete"
    variant="danger"
    onconfirm={confirmPresetDelete}
    oncancel={cancelPresetDelete}
  />
{/if}
