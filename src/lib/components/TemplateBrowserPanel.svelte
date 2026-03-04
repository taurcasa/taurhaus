<script>
  import {
    getRoleTemplate,
    getTeamPreset,
    listRoleTemplates,
    listTeamPresets,
  } from '../ipc.js'
  import { getToolIcon, getToolName } from '../toolLogos.js'
  import { themeTokens } from '../themeTokens.js'
  import PresetCard from './PresetCard.svelte'
  import SlideOver from './SlideOver.svelte'
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
    }
  }

  function resetDetail() {
    detailKind = ''
    selectedRole = null
    selectedPreset = null
    detailLoading = false
  }

  function setTab(tab) {
    activeTab = tab
    resetDetail()
  }

  async function loadCatalog() {
    loading = true
    errorMessage = ''
    try {
      const [roles, presets] = await Promise.all([listRoleTemplates(), listTeamPresets()])
      roleTemplates = (roles ?? []).map(normalizeRoleTemplate)
      teamPresets = (presets ?? []).map(normalizeTeamPreset)
    } catch (error) {
      roleTemplates = []
      teamPresets = []
      errorMessage = error?.message || 'Failed to load template catalog.'
    } finally {
      loading = false
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

                <div class="mt-1 flex items-center gap-1 text-[11px] {t.textSecondary}">
                  <svg class="h-3 w-3 shrink-0" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
                    <path d={getToolIcon(role.cliTool).path}></path>
                  </svg>
                  <span>{getToolName(role.cliTool)}</span>
                  <span class={t.textMuted}>|</span>
                  <span>{role.model}</span>
                </div>

                <div class="mt-1 flex flex-wrap gap-1">
                  {#each role.capabilities as capability}
                    <span class="rounded-full px-1.5 py-0.5 text-[10px] {capabilityChipTone()}">{capability}</span>
                  {/each}
                </div>

                <div class="mt-2 flex justify-end">
                  <button
                    class="rounded border px-1.5 py-0.5 text-[10px] {actionSecondary}"
                    onclick={() => {
                      void inspectRole(role)
                    }}
                    data-testid={`role-inspect-${role.roleId}`}
                  >
                    Inspect
                  </button>
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
        {:else if filteredTeamPresets.length === 0}
          <p class="rounded-md border px-2 py-2 text-xs {t.textMuted} {cardTone}">
            No team presets match the current filter.
          </p>
        {:else}
          <div class="grid grid-cols-1 gap-2 sm:grid-cols-2" data-testid="template-preset-list">
            {#each filteredTeamPresets as preset}
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
            {/each}
          </div>
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
