<script>
  import PresetCard from './PresetCard.svelte'
  import ValidationBar from './ValidationBar.svelte'
  import { themeTokens } from '../themeTokens.js'
  import {
    MODEL_OPTIONS_BY_TOOL,
    applyNamePattern,
    defaultModelForTool,
    normalizeTool,
    resolveDefaultNamePattern,
    resolveRoleModel,
    resolveRoleTool,
  } from '../meshDefaults.js'
  import { collectDuplicateNames } from '../meshValidation.js'
  import { normalizeProjectOption } from '../projectOptions.js'
  import { getToolIcon, getToolName } from '../toolLogos.js'
  import { projectNameFromPath } from './meshTabUtils.js'

  let {
    dark = false,
    mode = 'empty',
    teamName = '',
    teamConfig = null,
    roleTemplates = [],
    presets = [],
    availableProjects = [],
    onBuildCustom = () => {},
    onBrowseCatalog = () => {},
    onTeamNameChange = () => {},
    onDescriptionChange = () => {},
    onApplyPreset = () => {},
    onAssignLeadRole = () => {},
    onClearLead = () => {},
    onAppendAgentRole = () => {},
    onUpdateLead = () => {},
    onUpdateAgent = () => {},
    onRemoveAgent = () => {},
    onReorderAgent = () => {},
    onMoveAgentToEnd = () => {},
    onInitialize = () => {},
    onReset = () => {},
    onSavePreset = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const panelTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.03]'
      : 'border-brand-200/50 bg-white/90'
  )
  const surfaceTone = $derived(
    dark
      ? 'border-white/[0.08] bg-black/15'
      : 'border-zinc-200 bg-white'
  )
  const inputTone = $derived(
    dark
      ? 'border-white/[0.08] bg-zinc-950/60 text-zinc-100 placeholder-zinc-500'
      : 'border-brand-200/60 bg-white text-zinc-900 placeholder-zinc-400'
  )
  const ghostTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.04] text-zinc-300 hover:bg-white/[0.08]'
      : 'border-zinc-200 bg-zinc-50 text-zinc-700 hover:bg-zinc-100'
  )
  const leadDropTone = $derived(
    dark
      ? 'border-brand-500/40 bg-brand-500/8'
      : 'border-brand-400/60 bg-brand-50'
  )
  const invalidDropTone = $derived(
    dark
      ? 'border-danger-500/50 bg-danger-500/10'
      : 'border-danger-400/60 bg-danger-50'
  )

  let searchQuery = $state('')
  let activeToolFilter = $state('all')
  let activeKindFilter = $state('all')
  let draggingCatalogRoleId = $state('')
  let draggingRosterAgentId = $state('')
  let leadDropState = $state('idle')
  let agentDropState = $state('idle')
  let reorderTargetAgentId = $state('')
  let catalogSearchInput = $state(null)

  const normalizedTeam = $derived(teamConfig ?? { description: '', lead: null, agents: [] })
  const normalizedRoles = $derived.by(() =>
    (roleTemplates ?? [])
      .filter((role) => role && (role.roleId || role.name))
      .map((role) => {
        const tool = resolveRoleTool(role, role.kind === 'lead' ? 'claude' : 'codex')
        const model = resolveRoleModel(role, tool)
        return {
          ...role,
          roleId: String(role.roleId ?? ''),
          name: String(role.name ?? role.roleId ?? 'Unnamed role'),
          kind: String(role.kind ?? 'agent').trim().toLowerCase() === 'lead' ? 'lead' : 'agent',
          cliTool: tool,
          model,
          summary: String(
            role.behaviorSummary ??
            role.behavior_summary ??
            role.contextSummary ??
            role.context_summary ??
            role.instructions ??
            ''
          ).trim(),
        }
      })
  )
  const filteredRoles = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase()
    return normalizedRoles.filter((role) => {
      if (activeToolFilter !== 'all' && role.cliTool !== activeToolFilter) return false
      if (activeKindFilter !== 'all' && role.kind !== activeKindFilter) return false
      if (!query) return true
      return (
        role.name.toLowerCase().includes(query)
        || role.roleId.toLowerCase().includes(query)
        || role.cliTool.toLowerCase().includes(query)
        || role.model.toLowerCase().includes(query)
      )
    })
  })
  const leadRoles = $derived(filteredRoles.filter((role) => role.kind === 'lead'))
  const agentRoles = $derived(filteredRoles.filter((role) => role.kind !== 'lead'))
  const visibleRoleCount = $derived(filteredRoles.length)
  const toolFilterCounts = $derived.by(() => ({
    all: normalizedRoles.length,
    claude: normalizedRoles.filter((role) => role.cliTool === 'claude').length,
    codex: normalizedRoles.filter((role) => role.cliTool === 'codex').length,
    gemini: normalizedRoles.filter((role) => role.cliTool === 'gemini').length,
  }))
  const kindFilterCounts = $derived.by(() => ({
    all: normalizedRoles.length,
    lead: normalizedRoles.filter((role) => role.kind === 'lead').length,
    agent: normalizedRoles.filter((role) => role.kind === 'agent').length,
  }))
  const agents = $derived(Array.isArray(normalizedTeam.agents) ? normalizedTeam.agents : [])
  const availableProjectOptions = $derived.by(() =>
    (availableProjects ?? [])
      .map((project) => normalizeProjectOption(project, { stringLabel: 'raw', objectFallbackLabel: 'raw' }))
      .filter((project) => project.id)
  )
  const validationIssues = $derived.by(() => {
    const issues = []
    if (!String(teamName ?? '').trim()) {
      issues.push({ severity: 'error', member: 'Team', message: 'Team name is required.' })
    }
    if (!normalizedTeam?.lead?.name?.trim()) {
      issues.push({ severity: 'error', member: 'Lead', message: 'Lead role is required.' })
    }
    const duplicates = collectDuplicateNames([
      normalizedTeam?.lead?.name,
      ...agents.map((agent) => agent.name),
    ])
    for (const duplicate of duplicates) {
      issues.push({ severity: 'error', member: duplicate, message: 'Duplicate member name.' })
    }
    return issues
  })
  const canInitialize = $derived(
    Boolean(normalizedTeam?.lead) && !validationIssues.some((issue) => issue.severity === 'error')
  )

  function slugify(value) {
    return String(value || '')
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '') || 'member'
  }

  function rosterProjectName() {
    return projectNameFromPath(
      availableProjectOptions[0]?.id ?? normalizedTeam?.lead?.projectId ?? ''
    )
  }

  function nextInstanceName(role) {
    if (role.kind === 'lead') return 'team-lead'
    const basePattern = resolveDefaultNamePattern(role) ?? `${role.roleId || slugify(role.name)}-{n}`
    const existing = new Set(agents.map((agent) => String(agent.name ?? '').trim()))
    const projectName = rosterProjectName()
    let index = 1
    while (index < 100) {
      const candidate = applyNamePattern(basePattern, index, projectName)
      const resolved = candidate || `${role.roleId || slugify(role.name)}-${index}`
      if (!existing.has(resolved)) return resolved
      index += 1
    }
    return `${role.roleId || slugify(role.name)}-${Date.now()}`
  }

  function createDragGhost(label, tool) {
    const ghost = document.createElement('div')
    ghost.style.position = 'fixed'
    ghost.style.top = '-1000px'
    ghost.style.left = '-1000px'
    ghost.style.padding = '8px 10px'
    ghost.style.borderRadius = '10px'
    ghost.style.border = '1px solid rgba(34,197,94,0.35)'
    ghost.style.background = dark ? 'rgba(10, 16, 18, 0.96)' : 'rgba(255,255,255,0.98)'
    ghost.style.color = dark ? '#f4f4f5' : '#0f172a'
    ghost.style.font = '600 12px Geist, sans-serif'
    ghost.style.display = 'flex'
    ghost.style.gap = '8px'
    ghost.style.alignItems = 'center'
    ghost.textContent = `${getToolName(tool)} ${label}`
    document.body.appendChild(ghost)
    return ghost
  }

  function readTransfer(event) {
    const types = [
      'application/x-taurhaus-mesh-role',
      'application/x-taurhaus-roster-agent',
      'text/plain',
    ]
    for (const type of types) {
      const raw = event.dataTransfer?.getData(type)
      if (!raw) continue
      try {
        const payload = JSON.parse(raw)
        if (payload && typeof payload === 'object') return payload
      } catch {
        continue
      }
    }
    return null
  }

  function roleById(roleId) {
    return normalizedRoles.find((role) => role.roleId === roleId) ?? null
  }

  function canDropLead(payload) {
    if (!payload || payload.type !== 'catalog-role') return false
    return roleById(payload.roleId)?.kind === 'lead'
  }

  function canDropAgent(payload) {
    if (!payload) return false
    if (payload.type === 'catalog-role') return roleById(payload.roleId)?.kind !== 'lead'
    return payload.type === 'roster-agent'
  }

  function handleCatalogDragStart(event, role) {
    draggingCatalogRoleId = role.roleId
    const payload = JSON.stringify({ type: 'catalog-role', roleId: role.roleId })
    event.dataTransfer?.setData('application/x-taurhaus-mesh-role', payload)
    event.dataTransfer?.setData('text/plain', payload)
    event.dataTransfer.effectAllowed = 'copy'
    const ghost = createDragGhost(nextInstanceName(role), role.cliTool)
    event.dataTransfer?.setDragImage(ghost, 12, 12)
    requestAnimationFrame(() => ghost.remove())
  }

  function handleCatalogDragEnd() {
    draggingCatalogRoleId = ''
    leadDropState = 'idle'
    agentDropState = 'idle'
  }

  function handleRosterDragStart(event, agentId) {
    draggingRosterAgentId = agentId
    const payload = JSON.stringify({ type: 'roster-agent', agentId })
    event.dataTransfer?.setData('application/x-taurhaus-roster-agent', payload)
    event.dataTransfer?.setData('text/plain', payload)
    event.dataTransfer.effectAllowed = 'move'
  }

  function handleRosterDragEnd() {
    draggingRosterAgentId = ''
    reorderTargetAgentId = ''
    agentDropState = 'idle'
  }

  function handleLeadDragOver(event) {
    const payload = readTransfer(event)
    const valid = canDropLead(payload)
    leadDropState = valid ? 'valid' : 'invalid'
    if (!valid) return
    event.preventDefault()
    event.dataTransfer.dropEffect = 'copy'
  }

  function handleLeadDrop(event) {
    const payload = readTransfer(event)
    leadDropState = 'idle'
    if (!canDropLead(payload)) return
    event.preventDefault()
    onAssignLeadRole(payload.roleId)
  }

  function handleAgentDropZoneOver(event) {
    const payload = readTransfer(event)
    const valid = canDropAgent(payload)
    agentDropState = valid ? 'valid' : 'invalid'
    if (!valid) return
    event.preventDefault()
    event.dataTransfer.dropEffect = payload.type === 'roster-agent' ? 'move' : 'copy'
  }

  function handleAgentDropZoneLeave() {
    agentDropState = 'idle'
  }

  function handleAgentDropZoneDrop(event) {
    const payload = readTransfer(event)
    agentDropState = 'idle'
    if (!canDropAgent(payload)) return
    event.preventDefault()
    if (payload.type === 'catalog-role') {
      onAppendAgentRole(payload.roleId)
      return
    }
    onMoveAgentToEnd(payload.agentId)
  }

  function handleAgentCardDragOver(event, agentId) {
    const payload = readTransfer(event)
    if (!payload || payload.type !== 'roster-agent' || payload.agentId === agentId) return
    event.preventDefault()
    reorderTargetAgentId = agentId
    event.dataTransfer.dropEffect = 'move'
  }

  function handleAgentCardDrop(event, agentId) {
    const payload = readTransfer(event)
    reorderTargetAgentId = ''
    if (!payload || payload.type !== 'roster-agent' || payload.agentId === agentId) return
    event.preventDefault()
    onReorderAgent(payload.agentId, agentId)
  }

  function focusCatalogSearch() {
    catalogSearchInput?.focus?.()
    onBrowseCatalog()
  }

  function roleCardTone(role) {
    if (draggingCatalogRoleId === role.roleId) return 'opacity-50'
    return ''
  }

  function toggleToolFilter(tool) {
    activeToolFilter = activeToolFilter === tool ? 'all' : tool
  }

  function toggleKindFilter(kind) {
    activeKindFilter = activeKindFilter === kind ? 'all' : kind
  }

  function filterButtonTone(active) {
    if (active) {
      return dark
        ? 'border-brand-400/50 bg-brand-500/18 text-zinc-100 shadow-[0_0_0_1px_rgba(45,212,191,0.14)]'
        : 'border-brand-400/60 bg-brand-50 text-brand-900 shadow-[0_0_0_1px_rgba(15,118,110,0.08)]'
    }
    return dark
      ? 'border-white/[0.08] bg-white/[0.03] text-zinc-400 hover:bg-white/[0.06]'
      : 'border-zinc-200 bg-white text-zinc-600 hover:bg-zinc-50'
  }

  function roleMedallionTone(tool) {
    switch (tool) {
      case 'claude':
        return dark
          ? 'border-amber-400/35 bg-amber-500/12 text-amber-200'
          : 'border-amber-300/70 bg-amber-50 text-amber-800'
      case 'gemini':
        return dark
          ? 'border-sky-400/35 bg-sky-500/12 text-sky-200'
          : 'border-sky-300/70 bg-sky-50 text-sky-800'
      default:
        return dark
          ? 'border-emerald-400/35 bg-emerald-500/12 text-emerald-200'
          : 'border-emerald-300/70 bg-emerald-50 text-emerald-800'
    }
  }

  function roleChipTone(role) {
    if (role.kind === 'lead') {
      return dark
        ? 'border-brand-400/40 bg-brand-500/14 text-brand-200'
        : 'border-brand-300/60 bg-brand-50 text-brand-800'
    }
    return dark
      ? 'border-white/[0.08] bg-white/[0.05] text-zinc-300'
      : 'border-zinc-200 bg-zinc-50 text-zinc-700'
  }
</script>

<section
  class="grid gap-3 lg:grid-cols-[minmax(300px,0.8fr)_minmax(420px,1.2fr)]"
  data-testid={mode === 'empty' ? 'mesh-mode-empty' : 'mesh-mode-setup'}
>
  {#if mode === 'empty'}
    <div class="sr-only" data-testid="mesh-empty-state">Mesh builder empty state</div>
  {/if}
  <aside class="rounded-[20px] border p-3 {panelTone}" data-testid="mesh-builder-catalog">
    <div class="flex items-start justify-between gap-2">
      <div>
        <p class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Role Catalog</p>
        <h3 class="mt-1 text-sm font-semibold {t.textPrimary}">Build a roster inline</h3>
      </div>
      <div class="flex items-center gap-1.5">
        <button
          class="h-8 rounded-lg border px-2.5 text-[10px] font-bold uppercase tracking-[0.08em] {ghostTone}"
          type="button"
          onclick={focusCatalogSearch}
          data-testid="mesh-template-browse-catalog"
        >
          Browse Catalog
        </button>
        <button
          class="h-8 rounded-lg border px-2.5 text-[10px] font-bold uppercase tracking-[0.08em] {ghostTone}"
          type="button"
          onclick={onBuildCustom}
          data-testid="mesh-template-build-custom"
        >
          Build Custom
        </button>
      </div>
    </div>

    <label class="mt-4 block">
      <span class="sr-only">Search roles</span>
      <input
        bind:this={catalogSearchInput}
        class="h-10 w-full rounded-xl border px-3 text-sm outline-none {inputTone}"
        placeholder="Search roles by name, id, or tool"
        value={searchQuery}
        oninput={(event) => {
          searchQuery = event.currentTarget.value
        }}
        data-testid="mesh-builder-role-search"
      />
    </label>

    <div class="mt-3 space-y-3">
      <section class="space-y-2" data-testid="mesh-builder-filter-tools">
        <div class="flex items-center justify-between">
          <p class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Quick Filters</p>
          <span class="text-[10px] {t.textMuted}">{visibleRoleCount} visible</span>
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            class="inline-flex h-9 items-center gap-2 rounded-xl border px-3 text-[11px] font-semibold transition {filterButtonTone(activeToolFilter === 'all')}"
            type="button"
            onclick={() => {
              activeToolFilter = 'all'
            }}
            data-testid="mesh-builder-filter-tool-all"
          >
            <span>All tools</span>
            <span class="text-[10px] {t.textMuted}">{toolFilterCounts.all}</span>
          </button>
          {#each ['claude', 'codex', 'gemini'] as tool}
            <button
              class="inline-flex h-9 items-center gap-2 rounded-xl border px-3 text-[11px] font-semibold transition {filterButtonTone(activeToolFilter === tool)}"
              type="button"
              onclick={() => toggleToolFilter(tool)}
              data-testid={`mesh-builder-filter-tool-${tool}`}
            >
              <span class="inline-flex h-5 w-5 items-center justify-center rounded-full border {roleMedallionTone(tool)}">
                <svg
                  class="h-3 w-3"
                  viewBox={getToolIcon(tool, 'sidebarSmall').viewBox}
                  fill="currentColor"
                  aria-hidden="true"
                >
                  <path d={getToolIcon(tool, 'sidebarSmall').path}></path>
                </svg>
              </span>
              <span>{getToolName(tool)}</span>
              <span class="text-[10px] {t.textMuted}">{toolFilterCounts[tool]}</span>
            </button>
          {/each}
        </div>
        <div class="flex flex-wrap gap-2" data-testid="mesh-builder-filter-kinds">
          <button
            class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[10px] font-bold uppercase tracking-[0.12em] transition {filterButtonTone(activeKindFilter === 'all')}"
            type="button"
            onclick={() => {
              activeKindFilter = 'all'
            }}
            data-testid="mesh-builder-filter-kind-all"
          >
            All roles
            <span class="text-[10px] normal-case tracking-normal {t.textMuted}">{kindFilterCounts.all}</span>
          </button>
          <button
            class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[10px] font-bold uppercase tracking-[0.12em] transition {filterButtonTone(activeKindFilter === 'lead')}"
            type="button"
            onclick={() => toggleKindFilter('lead')}
            data-testid="mesh-builder-filter-kind-lead"
          >
            Lead
            <span class="text-[10px] normal-case tracking-normal {t.textMuted}">{kindFilterCounts.lead}</span>
          </button>
          <button
            class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[10px] font-bold uppercase tracking-[0.12em] transition {filterButtonTone(activeKindFilter === 'agent')}"
            type="button"
            onclick={() => toggleKindFilter('agent')}
            data-testid="mesh-builder-filter-kind-agent"
          >
            Agent
            <span class="text-[10px] normal-case tracking-normal {t.textMuted}">{kindFilterCounts.agent}</span>
          </button>
        </div>
      </section>

      {#if visibleRoleCount === 0}
        <div class="rounded-[18px] border border-dashed p-5 text-center {surfaceTone}" data-testid="mesh-builder-empty-results">
          <p class="text-[12px] font-semibold {t.textPrimary}">No roles match these filters</p>
          <p class="mt-1 text-[11px] {t.textSecondary}">Clear a tool or kind filter, or widen the search query.</p>
        </div>
      {/if}

      {#if activeKindFilter !== 'agent' && leadRoles.length > 0}
      <section data-testid="mesh-builder-role-section-leads">
        <div class="mb-2 flex items-center justify-between">
          <p class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Leads</p>
          <span class="text-[10px] {t.textMuted}">{leadRoles.length}</span>
        </div>
        <div class="grid gap-2 [grid-template-columns:repeat(auto-fit,minmax(188px,1fr))]">
          {#each leadRoles as role (role.roleId)}
            <button
              class="group relative flex min-h-[92px] w-full flex-col gap-2 overflow-hidden rounded-[18px] border p-2.5 text-left transition {surfaceTone} {roleCardTone(role)}"
              type="button"
              draggable="true"
              onclick={() => onAssignLeadRole(role.roleId)}
              ondragstart={(event) => handleCatalogDragStart(event, role)}
              ondragend={handleCatalogDragEnd}
              data-testid={`mesh-builder-role-${role.roleId}`}
            >
              <div class="flex items-start justify-between gap-2">
                <span class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-xl border {roleMedallionTone(role.cliTool)}">
                  <svg class="h-4 w-4" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
                    <path d={getToolIcon(role.cliTool).path}></path>
                  </svg>
                </span>
                <div class="flex items-center gap-1.5">
                  <span class="rounded-full border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] {roleChipTone(role)}">
                    Lead
                  </span>
                  <span class="text-[12px] font-bold {t.textMuted}" aria-hidden="true">⋮⋮</span>
                </div>
              </div>
              <div class="min-w-0">
                <p class="truncate text-[12px] font-semibold {t.textPrimary}">{role.name}</p>
                <p class="mt-1 text-[10px] font-medium uppercase tracking-[0.12em] {t.textMuted}">
                  {getToolName(role.cliTool)} · {role.model}
                </p>
                <p class="mt-2 text-[11px] leading-4 {t.textSecondary}">
                  {role.summary || 'Direction-setting lead role.'}
                </p>
              </div>
            </button>
          {/each}
        </div>
      </section>
      {/if}

      {#if activeKindFilter !== 'lead' && agentRoles.length > 0}
      <section data-testid="mesh-builder-role-section-agents">
        <div class="mb-2 flex items-center justify-between">
          <p class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Agents</p>
          <span class="text-[10px] {t.textMuted}">{agentRoles.length}</span>
        </div>
        <div class="grid gap-2 [grid-template-columns:repeat(auto-fit,minmax(188px,1fr))]">
          {#each agentRoles as role (role.roleId)}
            <button
              class="group relative flex min-h-[92px] w-full flex-col gap-2 overflow-hidden rounded-[18px] border p-2.5 text-left transition {surfaceTone} {roleCardTone(role)}"
              type="button"
              draggable="true"
              onclick={() => onAppendAgentRole(role.roleId)}
              ondragstart={(event) => handleCatalogDragStart(event, role)}
              ondragend={handleCatalogDragEnd}
              data-testid={`mesh-builder-role-${role.roleId}`}
            >
              <div class="flex items-start justify-between gap-2">
                <span class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-xl border {roleMedallionTone(role.cliTool)}">
                  <svg class="h-4 w-4" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
                    <path d={getToolIcon(role.cliTool).path}></path>
                  </svg>
                </span>
                <div class="flex items-center gap-1.5">
                  <span class="rounded-full border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] {roleChipTone(role)}">
                    Agent
                  </span>
                  <span class="text-[12px] font-bold {t.textMuted}" aria-hidden="true">⋮⋮</span>
                </div>
              </div>
              <div class="min-w-0">
                <p class="truncate text-[12px] font-semibold {t.textPrimary}">{role.name}</p>
                <p class="mt-1 text-[10px] font-medium uppercase tracking-[0.12em] {t.textMuted}">
                  {getToolName(role.cliTool)} · {role.model}
                </p>
                <p class="mt-2 text-[11px] leading-4 {t.textSecondary}">
                  {role.summary || 'Execution-focused specialist role.'}
                </p>
              </div>
            </button>
          {/each}
        </div>
      </section>
      {/if}

      <section data-testid="mesh-builder-preset-section">
        <div class="mb-2 flex items-center justify-between">
          <p class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Presets</p>
          <span class="text-[10px] {t.textMuted}">{(presets ?? []).length}</span>
        </div>
        <div class="space-y-2">
          {#each presets as preset (preset.presetId ?? preset.name)}
            <PresetCard
              name={preset.name}
              description={preset.description}
              leadCount={preset.leadCount ?? 1}
              agentCount={preset.agentCount ?? 0}
              tools={preset.tools ?? []}
              builtIn={preset.builtIn ?? false}
              dark={dark}
              testId={`mesh-template-preset-${preset.presetId ?? preset.name}`}
              onSelect={() => onApplyPreset(preset)}
              onInspect={focusCatalogSearch}
            />
          {/each}
        </div>
      </section>
    </div>
  </aside>

  <main class="rounded-[20px] border p-4 {panelTone}" data-testid="mesh-builder-roster">
    <div class="space-y-3">
      <div class="grid gap-3 lg:grid-cols-[minmax(220px,1fr)_minmax(0,1fr)]">
        <label class="space-y-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Team Name</span>
          <input
            class="h-10 w-full rounded-xl border px-3 text-sm outline-none {inputTone}"
            value={teamName}
            oninput={(event) => onTeamNameChange(event.currentTarget.value)}
            data-testid="mesh-builder-team-name-input"
          />
        </label>
        <label class="space-y-1.5">
          <span class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Description</span>
          <input
            class="h-10 w-full rounded-xl border px-3 text-sm outline-none {inputTone}"
            value={normalizedTeam.description ?? ''}
            oninput={(event) => onDescriptionChange(event.currentTarget.value)}
            data-testid="mesh-builder-team-description-input"
          />
        </label>
      </div>

      <ValidationBar issues={validationIssues} {dark} />

      <section class="space-y-2" data-testid="mesh-builder-lead-section">
        <div class="flex items-center justify-between">
          <p class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Lead</p>
          {#if normalizedTeam.lead}
            <button
              class="rounded-lg border px-2 py-1 text-[10px] font-bold {ghostTone}"
              type="button"
              onclick={onClearLead}
              data-testid="mesh-builder-lead-clear"
            >
              Clear
            </button>
          {/if}
        </div>

        <div
          class="rounded-2xl border p-3 transition {leadDropState === 'valid' ? leadDropTone : leadDropState === 'invalid' ? invalidDropTone : surfaceTone}"
          role="group"
          aria-label="Lead drop zone"
          ondragover={handleLeadDragOver}
          ondragleave={() => {
            leadDropState = 'idle'
          }}
          ondrop={handleLeadDrop}
          data-testid="mesh-builder-lead-slot"
        >
          {#if normalizedTeam.lead}
            <div class="space-y-3" data-testid="mesh-builder-lead-card">
              <div class="flex items-start gap-3">
                <svg class="mt-0.5 h-4 w-4 shrink-0 {t.textSecondary}" viewBox={getToolIcon(normalizeTool(normalizedTeam.lead.tool)).viewBox} fill="currentColor" aria-hidden="true">
                  <path d={getToolIcon(normalizeTool(normalizedTeam.lead.tool)).path}></path>
                </svg>
                <div class="min-w-0 flex-1">
                  <p class="text-[12px] font-semibold {t.textPrimary}">
                    {normalizedTeam.lead.roleName || normalizedTeam.lead.roleId || 'Lead'}
                  </p>
                  <p class="mt-1 text-[10px] uppercase tracking-wide {t.textMuted}">
                    {getToolName(normalizeTool(normalizedTeam.lead.tool))} · {normalizedTeam.lead.model || defaultModelForTool(normalizedTeam.lead.tool)}
                  </p>
                </div>
              </div>

              <div class="grid gap-2 lg:grid-cols-3">
                <label class="space-y-1">
                  <span class="text-[10px] font-medium uppercase tracking-wide {t.textMuted}">Name</span>
                  <input
                    class="h-9 w-full rounded-lg border px-3 text-xs outline-none {inputTone}"
                    value={normalizedTeam.lead.name ?? ''}
                    oninput={(event) => onUpdateLead({ name: event.currentTarget.value })}
                    data-testid="mesh-builder-lead-name-input"
                  />
                </label>
                <label class="space-y-1">
                  <span class="text-[10px] font-medium uppercase tracking-wide {t.textMuted}">Model</span>
                  <select
                    class="h-9 w-full rounded-lg border px-3 text-xs outline-none {inputTone}"
                    value={normalizedTeam.lead.model ?? defaultModelForTool(normalizedTeam.lead.tool)}
                    onchange={(event) => onUpdateLead({ model: event.currentTarget.value })}
                    data-testid="mesh-builder-lead-model-input"
                  >
                    {#each MODEL_OPTIONS_BY_TOOL[normalizeTool(normalizedTeam.lead.tool)] ?? [defaultModelForTool(normalizedTeam.lead.tool)] as option}
                      <option value={option}>{option}</option>
                    {/each}
                  </select>
                </label>
                <label class="space-y-1">
                  <span class="text-[10px] font-medium uppercase tracking-wide {t.textMuted}">Project</span>
                  <input
                    class="h-9 w-full rounded-lg border px-3 text-xs outline-none {inputTone}"
                    value={normalizedTeam.lead.projectId ?? ''}
                    oninput={(event) => onUpdateLead({ projectId: event.currentTarget.value })}
                    data-testid="mesh-builder-lead-project-input"
                  />
                </label>
              </div>
            </div>
          {:else}
            <div class="rounded-2xl border border-dashed p-8 text-center" data-testid="mesh-builder-lead-empty">
              <p class="text-[12px] font-semibold {t.textPrimary}">Drop lead role here</p>
              <p class="mt-1 text-[11px] {t.textSecondary}">Only lead roles can occupy this slot.</p>
            </div>
          {/if}
        </div>
      </section>

      <section class="space-y-2" data-testid="mesh-builder-agents-section">
        <div class="flex items-center justify-between">
          <p class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Agents</p>
          <span class="text-[10px] {t.textMuted}">{agents.length}</span>
        </div>

        <div class="space-y-2">
          {#each agents as agent, index (agent.id)}
            <article
              class="rounded-2xl border p-3 transition {reorderTargetAgentId === agent.id ? leadDropTone : surfaceTone}"
              draggable="true"
              ondragstart={(event) => handleRosterDragStart(event, agent.id)}
              ondragend={handleRosterDragEnd}
              ondragover={(event) => handleAgentCardDragOver(event, agent.id)}
              ondrop={(event) => handleAgentCardDrop(event, agent.id)}
              data-testid={`mesh-builder-agent-card-${agent.id}`}
            >
              <div class="flex items-start gap-3" data-testid="mesh-node-agent">
                <svg class="mt-0.5 h-4 w-4 shrink-0 {t.textSecondary}" viewBox={getToolIcon(normalizeTool(agent.tool)).viewBox} fill="currentColor" aria-hidden="true">
                  <path d={getToolIcon(normalizeTool(agent.tool)).path}></path>
                </svg>
                <div class="min-w-0 flex-1">
                  <div class="flex items-center justify-between gap-2">
                    <p class="truncate text-[12px] font-semibold {t.textPrimary}">
                      {agent.roleName || agent.roleId || agent.name}
                    </p>
                    <button
                      class="rounded-lg border px-2 py-1 text-[10px] font-bold {ghostTone}"
                      type="button"
                      onclick={() => onRemoveAgent(agent.id)}
                      data-testid={`mesh-builder-agent-remove-${agent.id}`}
                    >
                      x
                    </button>
                  </div>
                  <p class="mt-1 text-[10px] uppercase tracking-wide {t.textMuted}">
                    {getToolName(normalizeTool(agent.tool))} · {agent.model || defaultModelForTool(agent.tool)}
                  </p>
                </div>
              </div>

              <div class="mt-3 grid gap-2 lg:grid-cols-3">
                <label class="space-y-1">
                  <span class="text-[10px] font-medium uppercase tracking-wide {t.textMuted}">Name</span>
                  <input
                    class="h-9 w-full rounded-lg border px-3 text-xs outline-none {inputTone}"
                    value={agent.name ?? ''}
                    oninput={(event) => onUpdateAgent(agent.id, { name: event.currentTarget.value })}
                    data-testid={`mesh-builder-agent-name-input-${agent.id}`}
                  />
                </label>
                <label class="space-y-1">
                  <span class="text-[10px] font-medium uppercase tracking-wide {t.textMuted}">Model</span>
                  <select
                    class="h-9 w-full rounded-lg border px-3 text-xs outline-none {inputTone}"
                    value={agent.model ?? defaultModelForTool(agent.tool)}
                    onchange={(event) => onUpdateAgent(agent.id, { model: event.currentTarget.value })}
                    data-testid={`mesh-builder-agent-model-input-${agent.id}`}
                  >
                    {#each MODEL_OPTIONS_BY_TOOL[normalizeTool(agent.tool)] ?? [defaultModelForTool(agent.tool)] as option}
                      <option value={option}>{option}</option>
                    {/each}
                  </select>
                </label>
                <label class="space-y-1">
                  <span class="text-[10px] font-medium uppercase tracking-wide {t.textMuted}">Project</span>
                  <input
                    class="h-9 w-full rounded-lg border px-3 text-xs outline-none {inputTone}"
                    value={agent.projectId ?? ''}
                    oninput={(event) => onUpdateAgent(agent.id, { projectId: event.currentTarget.value })}
                    data-testid={`mesh-builder-agent-project-input-${agent.id}`}
                  />
                </label>
              </div>
            </article>
          {/each}

          <div
            class="rounded-2xl border border-dashed p-5 text-center transition {agentDropState === 'valid' ? leadDropTone : agentDropState === 'invalid' ? invalidDropTone : surfaceTone}"
            role="group"
            aria-label="Agent drop zone"
            ondragover={handleAgentDropZoneOver}
            ondragleave={handleAgentDropZoneLeave}
            ondrop={handleAgentDropZoneDrop}
            data-testid="mesh-builder-agent-dropzone"
          >
            <p class="text-[12px] font-semibold {t.textPrimary}">Drop agent role here</p>
            <p class="mt-1 text-[11px] {t.textSecondary}">
              Drag from the catalog to append a new agent, or drag a roster card here to move it to the end.
            </p>
          </div>
        </div>
      </section>

      <footer class="flex items-center justify-between gap-3 border-t pt-4 {dark ? 'border-white/[0.08]' : 'border-zinc-200'}" data-testid="mesh-action-bar">
        <button
          class="text-[11px] font-bold uppercase tracking-wide {t.textMuted} hover:text-brand-500"
          type="button"
          onclick={onSavePreset}
          data-testid="mesh-builder-save-preset"
        >
          Save as Preset
        </button>

        <div class="flex items-center gap-2">
          <button
            class="h-10 rounded-lg border px-4 text-[11px] font-bold {ghostTone}"
            type="button"
            onclick={onReset}
            data-testid="mesh-action-reset"
          >
            Reset
          </button>
          <button
            class="h-10 rounded-lg bg-brand-600 px-4 text-[11px] font-bold text-white transition hover:bg-brand-500 disabled:cursor-not-allowed disabled:opacity-50"
            type="button"
            onclick={onInitialize}
            disabled={!canInitialize}
            data-testid="mesh-action-initialize"
          >
            Initialize Team
          </button>
        </div>
      </footer>
    </div>
  </main>
</section>
