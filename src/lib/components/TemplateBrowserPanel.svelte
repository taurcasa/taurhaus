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
  const tabBase = 'px-3 py-2 text-[11px] font-bold uppercase tracking-wider transition-all duration-200 border-b-2'
  const tabActive = $derived(dark ? `text-brand-400 border-brand-500 bg-brand-500/5` : `text-brand-600 border-brand-500 bg-brand-50/50`)
  const tabInactive = $derived(`text-zinc-500 border-transparent hover:text-zinc-400 hover:bg-zinc-500/5`)
  
  const inputTone = $derived(
    dark
      ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20'
      : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'
  )
  
  const cardTone = $derived(
    dark
      ? 'bg-white/[0.03] border-white/[0.06] hover:bg-white/[0.05] hover:border-brand-500/30'
      : 'bg-brand-50/50 border-brand-200/40 hover:bg-brand-50/80 hover:border-brand-500/30'
  )
  
  const actionSecondary = $derived(
    dark
      ? 'bg-white/[0.05] border-white/[0.08] text-zinc-300 hover:text-white hover:bg-white/[0.1] active:scale-95'
      : 'bg-zinc-100 border-zinc-200 text-zinc-700 hover:bg-zinc-200 active:scale-95'
  )
  
  const toneMuted = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')

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
    <section class="space-y-4 animate-in fade-in duration-200" data-testid="template-browser-panel">
      <header class="space-y-4">
        <div class="flex items-center gap-1 border-b {t.keyline} -mx-4 px-4 bg-black/5 dark:bg-white/5">
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
          <div class="px-1">
            <label class="space-y-1.5 block">
              <span class="text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Filter</span>
              <div class="relative">
                <input
                  class="h-10 w-full rounded-lg border px-3 pr-10 text-sm transition-all outline-none {inputTone}"
                  placeholder={activeTab === 'roles' ? 'Search roles by name, id, or model' : 'Search presets by name, id, or description'}
                  value={searchQuery}
                  oninput={(event) => {
                    searchQuery = event.currentTarget.value
                  }}
                  data-testid="template-browser-search-input"
                />
                <div class="absolute right-3 top-1/2 -translate-y-1/2 text-zinc-500">
                  <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
                </div>
              </div>
            </label>
          </div>
        {/if}
      </header>

      {#if errorMessage}
        <div class="p-2 rounded-lg bg-danger-500/10 border border-danger-500/20 animate-in fade-in zoom-in-95 duration-200">
          <p class="text-[11px] font-medium text-danger-500 text-center">
            {errorMessage}
          </p>
        </div>
      {/if}

      {#if loading}
        <div class="flex flex-col items-center justify-center py-12 space-y-3 opacity-50">
          <div class="w-6 h-6 border-2 border-brand-500 border-t-transparent rounded-full animate-spin"></div>
          <p class="text-[11px] font-bold uppercase tracking-widest text-brand-500" data-testid="template-browser-loading">Loading templates...</p>
        </div>
      {:else if activeTab === 'roles'}
        {#if detailKind === 'role'}
          <section class="rounded-xl border p-4 space-y-4 animate-in fade-in slide-in-from-left-2 duration-200 {cardTone}" data-testid="template-role-detail">
            <button
              class="inline-flex items-center gap-1.5 h-8 px-2.5 rounded-lg text-[11px] font-bold uppercase tracking-wide {actionSecondary}"
              onclick={resetDetail}
              data-testid="template-role-back"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>
              Back
            </button>

            {#if detailLoading}
              <p class="text-xs text-center py-4 {toneMuted}">Loading role details...</p>
            {:else if selectedRole}
              <div class="space-y-2">
                <div class="flex items-start justify-between">
                  <h3 class="text-base font-bold {t.textPrimary}">
                    {selectedRole.name}
                  </h3>
                  <span class="rounded-full px-2 py-0.5 text-[10px] font-bold {roleKindBadgeTone(selectedRole.kind)}">{selectedRole.kind}</span>
                </div>
                <p class="text-[10px] font-mono {toneMuted}">{selectedRole.roleId}</p>
              </div>
              
              <div class="p-3 rounded-lg bg-black/5 dark:bg-white/5 border border-white/5">
                <p class="text-xs leading-relaxed {t.textSecondary}">
                  {selectedRole.instructions || 'No role instructions available.'}
                </p>
              </div>
              
              <button
                class="w-full h-10 rounded-lg bg-brand-600 px-4 py-1 text-xs font-bold text-white hover:bg-brand-500 shadow-lg shadow-brand-500/20 active:scale-95 transition-all"
                onclick={() => onSelectRole(selectedRole)}
                data-testid={`role-select-${selectedRole.roleId}`}
              >
                Use this Role
              </button>
            {/if}
          </section>
        {:else if filteredRoleTemplates.length === 0}
          <div class="flex flex-col items-center justify-center py-12 border-2 border-dashed rounded-xl {dark ? 'border-zinc-800' : 'border-zinc-200'}">
            <p class="text-xs {t.textMuted}">
              No role templates match the current filter.
            </p>
          </div>
        {:else}
          <div class="flex items-center justify-between px-1">
            <p class="text-[10px] font-bold uppercase tracking-wider {t.textMuted}">Role Templates</p>
            <button
              class="h-8 px-3 rounded-lg text-[11px] font-bold text-white bg-brand-600 hover:bg-brand-500 active:scale-95 transition-all shadow-lg shadow-brand-500/10"
              onclick={openCreateRoleEditor}
              data-testid="role-create-button"
            >
              + Create
            </button>
          </div>

          {#if !hasCustomRoles}
            <div class="p-4 rounded-xl border-2 border-dashed flex flex-col items-center justify-center text-center space-y-2 {dark ? 'border-zinc-800 bg-white/[0.01]' : 'border-zinc-200 bg-black/[0.01]'}" data-testid="role-custom-empty-state">
              <p class="text-xs {t.textMuted}">No custom roles yet. Create one or capture from a live team.</p>
            </div>
          {/if}

          <div class="space-y-3" data-testid="template-role-list">
            {#each filteredRoleTemplates as role, i}
              <article class="group rounded-xl border p-3 transition-all animate-in fade-in slide-in-from-bottom-1 duration-200 {cardTone}" style:transition-delay={`${i * 30}ms`} data-testid={`role-template-card-${role.roleId}`}>
                <div class="flex items-start justify-between gap-2">
                  <div class="min-w-0">
                    <p class="truncate text-[14px] font-bold {t.textPrimary}">{role.name}</p>
                    <p class="text-[10px] font-mono {toneMuted}">{role.roleId}</p>
                  </div>
                  <span class="rounded-full px-2 py-0.5 text-[9px] font-bold uppercase tracking-tight {roleKindBadgeTone(role.kind)}">{role.kind}</span>
                </div>

                <div class="mt-3 flex flex-wrap items-center gap-1.5">
                  <span
                    class="inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[10px] font-bold {capabilityChipTone()}"
                    data-testid={`role-tool-badge-${role.roleId}`}
                  >
                    <svg class="h-3 w-3 shrink-0" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
                      <path d={getToolIcon(role.cliTool).path}></path>
                    </svg>
                    <span class="uppercase tracking-tighter opacity-80">{getToolName(role.cliTool)}</span>
                  </span>
                  <span
                    class="inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-bold {capabilityChipTone()}"
                    data-testid={`role-model-badge-${role.roleId}`}
                  >
                    {role.model || 'unspecified'}
                  </span>
                </div>

                {#if role.capabilities.length > 0}
                  <div class="mt-2 flex flex-wrap gap-1">
                    {#each role.capabilities as capability}
                      <span
                        class="rounded-full px-2 py-0.5 text-[9px] font-bold border border-transparent bg-black/5 dark:bg-white/5 {t.textSecondary}"
                        data-testid={capabilityTestId(role.roleId, capability)}
                      >
                        {capability}
                      </span>
                    {/each}
                  </div>
                {/if}

                <div class="mt-4 flex flex-wrap justify-end gap-2 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
                  <div class="flex gap-1.5">
                    <button
                      class="h-8 px-3 rounded-lg text-[11px] font-bold {actionSecondary}"
                      onclick={() => onSelectRole(role)}
                      data-testid={`role-use-${role.roleId}`}
                    >
                      Use
                    </button>
                    <button
                      class="h-8 px-3 rounded-lg text-[11px] font-bold {actionSecondary}"
                      onclick={() => {
                        void inspectRole(role)
                      }}
                      data-testid={`role-inspect-${role.roleId}`}
                    >
                      Inspect
                    </button>
                  </div>
                  
                  {#if isCustomRole(role)}
                    <div class="flex gap-1.5 ml-auto">
                      <button
                        class="h-8 w-8 flex items-center justify-center rounded-lg {actionSecondary}"
                        onclick={() => {
                          void openEditRoleEditor(role)
                        }}
                        aria-label="Edit role"
                        title="Edit role"
                        data-testid={`role-edit-${role.roleId}`}
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/><path d="m15 5 4 4"/></svg>
                      </button>
                      <button
                        class="h-8 w-8 flex items-center justify-center rounded-lg border border-danger-500/20 text-danger-500 hover:bg-danger-500/10 active:scale-95 transition-all"
                        onclick={() => requestRoleDelete(role)}
                        aria-label="Delete role"
                        title="Delete role"
                        data-testid={`role-delete-${role.roleId}`}
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
                      </button>
                    </div>
                  {/if}
                </div>
              </article>
            {/each}
          </div>
        {/if}
      {:else if activeTab === 'presets'}
        {#if detailKind === 'preset'}
          <section class="rounded-xl border p-4 space-y-4 animate-in fade-in slide-in-from-left-2 duration-200 {cardTone}" data-testid="template-preset-detail">
            <button
              class="inline-flex items-center gap-1.5 h-8 px-2.5 rounded-lg text-[11px] font-bold uppercase tracking-wide {actionSecondary}"
              onclick={resetDetail}
              data-testid="template-preset-back"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>
              Back
            </button>

            {#if detailLoading}
              <p class="text-xs text-center py-4 {toneMuted}">Loading preset details...</p>
            {:else if selectedPreset}
              <div class="space-y-2">
                <h3 class="text-base font-bold {t.textPrimary}">
                  {selectedPreset.name}
                </h3>
                <p class="text-[10px] font-mono {toneMuted}">{selectedPreset.presetId}</p>
              </div>
              
              <div class="p-3 rounded-lg bg-black/5 dark:bg-white/5 border border-white/5">
                <p class="text-xs leading-relaxed {t.textSecondary}">
                  {selectedPreset.description || 'No preset description provided.'}
                </p>
              </div>
              
              <div class="flex items-center gap-2 px-1">
                <span class="text-[10px] font-bold uppercase tracking-widest text-brand-500">Configuration:</span>
                <span class="text-[11px] font-medium {t.textSecondary}">
                  {(selectedPreset.agentSlots ?? []).length} slot(s) configured.
                </span>
              </div>
              
              <button
                class="w-full h-10 rounded-lg bg-brand-600 px-4 py-1 text-xs font-bold text-white hover:bg-brand-500 shadow-lg shadow-brand-500/20 active:scale-95 transition-all"
                onclick={() => onSelectPreset(selectedPreset)}
                data-testid={`preset-select-${selectedPreset.presetId}`}
              >
                Use this Preset
              </button>
            {/if}
          </section>
        {:else}
          <div class="flex items-center justify-between px-1">
            <p class="text-[10px] font-bold uppercase tracking-wider {t.textMuted}">Team Presets</p>
            <button
              class="h-8 px-3 rounded-lg text-[11px] font-bold text-white bg-brand-600 hover:bg-brand-500 active:scale-95 transition-all shadow-lg shadow-brand-500/10"
              onclick={openCreatePresetEditor}
              data-testid="template-preset-create"
            >
              + Create
            </button>
          </div>

          {#if filteredTeamPresets.length === 0}
            <div class="flex flex-col items-center justify-center py-12 border-2 border-dashed rounded-xl {dark ? 'border-zinc-800' : 'border-zinc-200'}">
              <p class="text-xs {t.textMuted}">
                No team presets match the current filter.
              </p>
            </div>
          {:else}
            <div class="space-y-3" data-testid="template-preset-list">
              {#each filteredTeamPresets as preset, i}
                <article class="group space-y-3 rounded-xl border p-3 transition-all animate-in fade-in slide-in-from-bottom-1 duration-200 {cardTone}" style:transition-delay={`${i * 30}ms`}>
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

                  <div class="flex flex-wrap justify-end gap-2 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
                    <div class="flex gap-1.5">
                      <button
                        class="h-8 px-3 rounded-lg text-[11px] font-bold {actionSecondary}"
                        onclick={() => onSelectPreset(preset)}
                        data-testid={`template-preset-use-${preset.presetId}`}
                      >
                        Use
                      </button>
                      <button
                        class="h-8 px-3 rounded-lg text-[11px] font-bold {actionSecondary}"
                        onclick={() => {
                          void inspectPreset(preset)
                        }}
                        data-testid={`template-preset-inspect-${preset.presetId}`}
                      >
                        Inspect
                      </button>
                    </div>
                    
                    {#if isCustomPreset(preset)}
                      <div class="flex gap-1.5 ml-auto">
                        <button
                          class="h-8 w-8 flex items-center justify-center rounded-lg {actionSecondary}"
                          onclick={() => {
                            void openPresetEditorForMutation(preset, 'edit')
                          }}
                          aria-label="Edit preset"
                          title="Edit preset"
                          data-testid={`template-preset-edit-${preset.presetId}`}
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/><path d="m15 5 4 4"/></svg>
                        </button>
                        <button
                          class="h-8 w-8 flex items-center justify-center rounded-lg {actionSecondary}"
                          onclick={() => {
                            void openPresetEditorForMutation(preset, 'duplicate')
                          }}
                          aria-label="Duplicate preset"
                          title="Duplicate preset"
                          data-testid={`template-preset-duplicate-${preset.presetId}`}
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>
                        </button>
                        <button
                          class="h-8 w-8 flex items-center justify-center rounded-lg border border-danger-500/20 text-danger-500 hover:bg-danger-500/10 active:scale-95 transition-all"
                          onclick={() => requestPresetDelete(preset)}
                          aria-label="Delete preset"
                          title="Delete preset"
                          data-testid={`template-preset-delete-${preset.presetId}`}
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
                        </button>
                      </div>
                    {/if}
                  </div>
                </article>
              {/each}
            </div>
          {/if}
        {/if}
      {:else}
        <div class="animate-in fade-in slide-in-from-right-2 duration-200">
          <TemplateHistoryPanel
            dark={dark}
            selectedTemplateId={historyTemplateId}
            selectedTemplateKind={historyTemplateKind}
          />
        </div>
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
