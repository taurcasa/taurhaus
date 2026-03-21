<script>
  import { onDestroy, onMount, tick } from 'svelte'
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
  import {
    latestRoleVersions,
    ROLE_VERSION_VISIBILITY_STORAGE_KEY,
  } from './roleVersioning.js'

  const CATALOG_DENSITY_STORAGE_KEY = 'taurhaus.mesh.roleCatalogDensity'
  const PINNED_ROLE_IDS_STORAGE_KEY = 'taurhaus.mesh.pinnedRoleIds'

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
  const rosterSectionTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.02]'
      : 'border-zinc-200/90 bg-zinc-50/70'
  )
  const presetRowTone = $derived(
    dark
      ? 'border-white/[0.08] bg-black/15 hover:bg-white/[0.05]'
      : 'border-zinc-200 bg-white hover:bg-zinc-50'
  )
  const presetBadgeTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.06] text-zinc-300'
      : 'border-zinc-200 bg-zinc-50 text-zinc-600'
  )

  let searchQuery = $state('')
  let activeToolFilter = $state('all')
  let activeKindFilter = $state('all')
  let showAllRoleVersions = $state(false)
  let pinnedRoleIds = $state([])
  let catalogCollapsed = $state(false)
  let editingTeamName = $state(false)
  let editingDescription = $state(false)
  let leadDetailsExpanded = $state(false)
  let expandedAgentIds = $state([])
  let highlightedRosterSection = $state('')
  let draggingCatalogRoleId = $state('')
  let draggingRosterAgentId = $state('')
  let leadDropState = $state('idle')
  let agentDropState = $state('idle')
  let reorderTargetAgentId = $state('')
  let catalogSearchInput = $state(null)
  let catalogDensityPreference = $state(null)
  let teamNameInput = $state(null)
  let teamDescriptionInput = $state(null)
  let rosterFeedbackTimer = null

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
            role.focusArea ??
            role.focus_area ??
            role.contextSummary ??
            role.context_summary ??
            role.instructions ??
            ''
          ).trim(),
        }
      })
  )
  const catalogRoles = $derived(showAllRoleVersions ? normalizedRoles : latestRoleVersions(normalizedRoles))
  const pinnedRoles = $derived.by(() => {
    const roleMap = new Map(normalizedRoles.map((role) => [role.roleId, role]))
    return pinnedRoleIds
      .map((roleId) => roleMap.get(roleId) ?? null)
      .filter(Boolean)
  })
  const filteredRoles = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase()
    return catalogRoles.filter((role) => {
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
  const catalogDensityMode = $derived(
    catalogDensityPreference ?? (visibleRoleCount > 8 ? 'compact' : 'expanded')
  )
  const toolFilterCounts = $derived.by(() => ({
    all: catalogRoles.length,
    claude: catalogRoles.filter((role) => role.cliTool === 'claude').length,
    codex: catalogRoles.filter((role) => role.cliTool === 'codex').length,
    gemini: catalogRoles.filter((role) => role.cliTool === 'gemini').length,
  }))
  const kindFilterCounts = $derived.by(() => ({
    all: catalogRoles.length,
    lead: catalogRoles.filter((role) => role.kind === 'lead').length,
    agent: catalogRoles.filter((role) => role.kind === 'agent').length,
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
  const firstValidationIssue = $derived(validationIssues[0] ?? null)
  const initializeButtonTitle = $derived(
    canInitialize
      ? 'Initialize this team'
      : firstValidationIssue?.message ?? 'Resolve the roster issues before initializing.'
  )
  const teamNameLabel = $derived(String(teamName ?? '').trim() || 'Name this team')
  const teamNameDisplayLabel = $derived(String(teamName ?? '').trim() || 'New Team')
  const teamDescriptionLabel = $derived(
    String(normalizedTeam?.description ?? '').trim()
      || 'Add a one-line brief so the roster has the right context before initialization.'
  )
  const teamDescriptionDisplayLabel = $derived(
    String(normalizedTeam?.description ?? '').trim()
      || 'Pick roles from the left to build your lineup.'
  )
  const memberCount = $derived((normalizedTeam?.lead ? 1 : 0) + agents.length)
  const presetCountLabel = $derived.by(() => {
    const count = Array.isArray(presets) ? presets.length : 0
    return `${count} preset${count === 1 ? '' : 's'}`
  })

  onMount(() => {
    try {
      const storedValue = window.localStorage.getItem(CATALOG_DENSITY_STORAGE_KEY)
      if (storedValue === 'compact' || storedValue === 'expanded') {
        catalogDensityPreference = storedValue
      }

      showAllRoleVersions =
        window.localStorage.getItem(ROLE_VERSION_VISIBILITY_STORAGE_KEY) === 'true'

      const pinnedValue = window.localStorage.getItem(PINNED_ROLE_IDS_STORAGE_KEY)
      if (pinnedValue) {
        const parsed = JSON.parse(pinnedValue)
        pinnedRoleIds = Array.isArray(parsed)
          ? parsed.filter((roleId) => typeof roleId === 'string' && roleId.trim())
          : []
      }

      catalogCollapsed = false
    } catch {
      // Ignore localStorage failures and fall back to the smart default.
    }
  })

  onDestroy(() => {
    if (rosterFeedbackTimer) {
      clearTimeout(rosterFeedbackTimer)
    }
  })

  $effect(() => {
    if (!normalizedTeam?.lead && leadDetailsExpanded) {
      leadDetailsExpanded = false
    }
  })

  $effect(() => {
    const validAgentIds = new Set(agents.map((agent) => agent.id))
    const nextExpandedAgentIds = expandedAgentIds.filter((agentId) => validAgentIds.has(agentId))

    if (
      nextExpandedAgentIds.length !== expandedAgentIds.length
      || nextExpandedAgentIds.some((agentId, index) => agentId !== expandedAgentIds[index])
    ) {
      expandedAgentIds = nextExpandedAgentIds
    }
  })

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

  async function focusCatalogSearch() {
    await tick()
    catalogSearchInput?.focus?.()
    onBrowseCatalog()
  }

  async function startEditingTeamName() {
    editingTeamName = true
    await tick()
    teamNameInput?.focus?.()
    teamNameInput?.select?.()
  }

  async function startEditingDescription() {
    editingDescription = true
    await tick()
    teamDescriptionInput?.focus?.()
    teamDescriptionInput?.select?.()
  }

  function finishEditingTeamName() {
    editingTeamName = false
  }

  function finishEditingDescription() {
    editingDescription = false
  }

  function handleBuildCustom() {
    onBuildCustom()
  }

  function handleReset() {
    editingTeamName = false
    editingDescription = false
    leadDetailsExpanded = false
    expandedAgentIds = []
    onReset()
  }

  function triggerRosterFeedback(section = 'all') {
    highlightedRosterSection = section
    if (rosterFeedbackTimer) {
      clearTimeout(rosterFeedbackTimer)
    }
    rosterFeedbackTimer = setTimeout(() => {
      highlightedRosterSection = ''
      rosterFeedbackTimer = null
    }, 700)
  }

  function roleCardTone(role) {
    if (draggingCatalogRoleId === role.roleId) return 'opacity-50'
    return ''
  }

  function teamCardTone(tool) {
    const normalizedTool = normalizeTool(tool)
    if (normalizedTool === 'claude') {
      return dark
        ? 'border-brand-400/22 bg-brand-500/8'
        : 'border-brand-300/65 bg-brand-50/85'
    }
    if (normalizedTool === 'gemini') {
      return dark
        ? 'border-sky-400/22 bg-sky-500/8'
        : 'border-sky-300/65 bg-sky-50/85'
    }
    return dark
      ? 'border-emerald-400/22 bg-emerald-500/8'
      : 'border-emerald-300/65 bg-emerald-50/85'
  }

  function toggleToolFilter(tool) {
    activeToolFilter = activeToolFilter === tool ? 'all' : tool
  }

  function projectSelectLabel(projectId) {
    const option = availableProjectOptions.find((project) => project.id === projectId)
    return option?.label ?? 'Choose project'
  }

  function toggleKindFilter(kind) {
    activeKindFilter = activeKindFilter === kind ? 'all' : kind
  }

  function toggleRoleVersionVisibility() {
    showAllRoleVersions = !showAllRoleVersions
    try {
      window.localStorage.setItem(
        ROLE_VERSION_VISIBILITY_STORAGE_KEY,
        showAllRoleVersions ? 'true' : 'false'
      )
    } catch {
      // Ignore localStorage failures and keep the in-memory preference.
    }
  }

  function persistPinnedRoleIds(nextRoleIds) {
    pinnedRoleIds = nextRoleIds
    try {
      window.localStorage.setItem(PINNED_ROLE_IDS_STORAGE_KEY, JSON.stringify(nextRoleIds))
    } catch {
      // Ignore localStorage failures and keep the in-memory preference.
    }
  }

  function isRolePinned(roleId) {
    return pinnedRoleIds.includes(roleId)
  }

  function togglePinnedRole(roleId) {
    if (!roleId) return
    if (isRolePinned(roleId)) {
      persistPinnedRoleIds(pinnedRoleIds.filter((entry) => entry !== roleId))
      return
    }
    persistPinnedRoleIds([...pinnedRoleIds, roleId])
  }

  function handlePinToggle(event, roleId) {
    event.stopPropagation()
    event.preventDefault()
    togglePinnedRole(roleId)
  }

  function assignRole(role) {
    if (!role?.roleId) return
    if (role.kind === 'lead') {
      triggerRosterFeedback('lead')
      onAssignLeadRole(role.roleId)
      return
    }
    triggerRosterFeedback('agents')
    onAppendAgentRole(role.roleId)
  }

  function handlePresetApply(preset) {
    leadDetailsExpanded = false
    expandedAgentIds = []
    triggerRosterFeedback('all')
    onApplyPreset(preset)
  }

  function toggleLeadDetails() {
    leadDetailsExpanded = !leadDetailsExpanded
  }

  function isAgentExpanded(agentId) {
    return expandedAgentIds.includes(agentId)
  }

  function toggleAgentDetails(agentId) {
    if (!agentId) return
    if (expandedAgentIds.includes(agentId)) {
      expandedAgentIds = expandedAgentIds.filter((entry) => entry !== agentId)
      return
    }
    expandedAgentIds = [...expandedAgentIds, agentId]
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

  function densityButtonTone(active) {
    if (active) {
      return dark
        ? 'border-brand-400/50 bg-brand-500/18 text-zinc-100'
        : 'border-brand-400/60 bg-brand-50 text-brand-900'
    }
    return dark
      ? 'border-white/[0.08] bg-white/[0.03] text-zinc-400 hover:bg-white/[0.06]'
      : 'border-zinc-200 bg-white text-zinc-500 hover:bg-zinc-50'
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

  function roleKindLabel(role) {
    return role.kind === 'lead' ? 'Lead' : 'Agent'
  }

  function roleSummaryText(role) {
    return role.summary || (role.kind === 'lead'
      ? 'Direction-setting lead role.'
      : 'Execution-focused specialist role.')
  }

  function presetTools(preset) {
    if (!Array.isArray(preset?.tools)) return []
    return [...new Set(preset.tools.map((tool) => String(tool || '').toLowerCase()).filter(Boolean))]
  }

  function presetAgentCount(preset) {
    return Math.max(0, Number(preset?.agentCount ?? 0))
  }

  function presetLeadCount(preset) {
    const explicitLeadCount = Number(preset?.leadCount)
    if (Number.isFinite(explicitLeadCount)) {
      return Math.max(0, explicitLeadCount)
    }
    const agentCount = presetAgentCount(preset)
    const roleCount = Number(preset?.roleCount ?? agentCount + 1)
    return Math.max(1, roleCount - agentCount)
  }

  function presetMemberSummary(preset) {
    const agentCount = presetAgentCount(preset)
    const leadCount = presetLeadCount(preset)
    return `${agentCount} agent${agentCount === 1 ? '' : 's'} · ${leadCount} lead${leadCount === 1 ? '' : 's'}`
  }

  function pinButtonTone(active) {
    if (active) {
      return dark
        ? 'border-amber-400/50 bg-amber-500/16 text-amber-200 opacity-100'
        : 'border-amber-300/70 bg-amber-50 text-amber-800 opacity-100'
    }
    return dark
      ? 'border-white/[0.08] bg-black/25 text-zinc-300 opacity-100 hover:text-amber-200'
      : 'border-zinc-200 bg-white text-zinc-500 opacity-100 hover:text-amber-700'
  }

  function setCatalogDensityPreference(mode) {
    catalogDensityPreference = mode
    try {
      window.localStorage.setItem(CATALOG_DENSITY_STORAGE_KEY, mode)
    } catch {
      // Ignore localStorage failures and keep the in-memory preference.
    }
  }
</script>

<section
  class="mx-auto flex w-full max-w-[1180px] flex-col gap-3"
  data-testid={mode === 'empty' ? 'mesh-mode-empty' : 'mesh-mode-setup'}
>
  {#if mode === 'empty'}
    <div class="sr-only" data-testid="mesh-empty-state">Mesh builder empty state</div>
  {/if}

  <main class="space-y-3" data-testid="mesh-builder-roster">
    <div class="grid gap-3 md:grid-cols-[minmax(0,1.22fr)_minmax(340px,0.94fr)]">
      <section
        class="flex min-h-0 flex-col space-y-4 rounded-[28px] border p-4 shadow-sm backdrop-blur {panelTone} md:max-h-[calc(100vh-11rem)]"
        data-testid="mesh-builder-catalog"
        data-collapsed="false"
      >
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="space-y-1">
            <div class="flex items-center gap-2">
              <h2 class="text-[16px] font-semibold {t.textPrimary}">Available Roles</h2>
              <span class="rounded-full border px-2 py-0.5 text-[10px] font-medium {presetBadgeTone}">
                {visibleRoleCount} visible
              </span>
            </div>
            <p class="text-[12px] {t.textSecondary}">
              Search, filter, and add from the live role catalog.
            </p>
          </div>

          <div class="flex items-center gap-2">
            <button
              class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[11px] font-medium transition {ghostTone}"
              type="button"
              onclick={focusCatalogSearch}
              data-testid="mesh-template-browse-catalog"
            >
              Browse catalog
            </button>
            {#if mode === 'empty'}
              <button
                class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[11px] font-medium transition {ghostTone}"
                type="button"
                onclick={handleBuildCustom}
                data-testid="mesh-template-build-custom"
              >
                Start from scratch
              </button>
            {/if}
          </div>
        </div>

        <div class="space-y-3" data-testid="mesh-builder-catalog-content">
          <label class="block">
            <span class="sr-only">Search roles</span>
            <div class="flex h-11 items-center gap-3 rounded-[18px] border px-3 {surfaceTone}">
              <svg class="h-4 w-4 shrink-0 {t.textMuted}" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                <circle cx="7" cy="7" r="4.5"></circle>
                <path d="M10.5 10.5 14 14" stroke-linecap="round"></path>
              </svg>
              <input
                bind:this={catalogSearchInput}
                class="h-full min-w-0 flex-1 bg-transparent text-[13px] outline-none {dark ? 'text-zinc-100 placeholder-zinc-500' : 'text-zinc-900 placeholder-zinc-400'}"
                placeholder="Search roles"
                value={searchQuery}
                oninput={(event) => {
                  searchQuery = event.currentTarget.value
                }}
                data-testid="mesh-builder-role-search"
              />
              <div class="flex items-center gap-1" data-testid="mesh-builder-density-toggle">
                <button
                  class="inline-flex h-7 w-7 items-center justify-center rounded-lg border transition {densityButtonTone(catalogDensityMode === 'compact')}"
                  type="button"
                  aria-label="Use compact role density"
                  aria-pressed={catalogDensityMode === 'compact'}
                  title="Compact density"
                  onclick={() => setCatalogDensityPreference('compact')}
                  data-testid="mesh-builder-density-compact"
                >
                  <svg class="h-3.5 w-3.5" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" aria-hidden="true">
                    <path d="M3 4.25h10M3 8h10M3 11.75h10" stroke-linecap="round"></path>
                  </svg>
                </button>
                <button
                  class="inline-flex h-7 w-7 items-center justify-center rounded-lg border transition {densityButtonTone(catalogDensityMode === 'expanded')}"
                  type="button"
                  aria-label="Use expanded role density"
                  aria-pressed={catalogDensityMode === 'expanded'}
                  title="Expanded density"
                  onclick={() => setCatalogDensityPreference('expanded')}
                  data-testid="mesh-builder-density-expanded"
                >
                  <svg class="h-3.5 w-3.5" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" aria-hidden="true">
                    <rect x="2.5" y="2.5" width="4.5" height="4.5" rx="1"></rect>
                    <rect x="9" y="2.5" width="4.5" height="4.5" rx="1"></rect>
                    <rect x="2.5" y="9" width="4.5" height="4.5" rx="1"></rect>
                    <rect x="9" y="9" width="4.5" height="4.5" rx="1"></rect>
                  </svg>
                </button>
              </div>
            </div>
          </label>

          <section class="space-y-3" data-testid="mesh-builder-filter-tools">
            <div class="flex flex-wrap gap-2">
              <button
                class="inline-flex h-9 items-center gap-2 rounded-full border px-3 text-[11px] font-medium transition {filterButtonTone(activeToolFilter === 'all')}"
                type="button"
                onclick={() => {
                  activeToolFilter = 'all'
                }}
                data-testid="mesh-builder-filter-tool-all"
              >
                <span>All</span>
                <span class="text-[10px] {t.textMuted}">{toolFilterCounts.all}</span>
              </button>
              {#each ['claude', 'codex', 'gemini'] as tool}
                <button
                  class="inline-flex h-9 items-center gap-2 rounded-full border px-3 text-[11px] font-medium transition {filterButtonTone(activeToolFilter === tool)}"
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
                </button>
              {/each}
            </div>

            <div class="flex flex-wrap gap-2" data-testid="mesh-builder-filter-kinds">
              <button
                class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[11px] font-medium transition {filterButtonTone(activeKindFilter === 'all')}"
                type="button"
                onclick={() => {
                  activeKindFilter = 'all'
                }}
                data-testid="mesh-builder-filter-kind-all"
              >
                All Roles
                <span class="text-[10px] {t.textMuted}">{kindFilterCounts.all}</span>
              </button>
              <button
                class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[11px] font-medium transition {filterButtonTone(activeKindFilter === 'lead')}"
                type="button"
                onclick={() => toggleKindFilter('lead')}
                data-testid="mesh-builder-filter-kind-lead"
              >
                Leads
                <span class="text-[10px] {t.textMuted}">{kindFilterCounts.lead}</span>
              </button>
              <button
                class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[11px] font-medium transition {filterButtonTone(activeKindFilter === 'agent')}"
                type="button"
                onclick={() => toggleKindFilter('agent')}
                data-testid="mesh-builder-filter-kind-agent"
              >
                Agents
                <span class="text-[10px] {t.textMuted}">{kindFilterCounts.agent}</span>
              </button>
            </div>

            {#if presets.length > 0}
              <section class="space-y-2" data-testid="mesh-builder-preset-section">
                <div class="flex items-center justify-between gap-2">
                  <p class="text-[12px] font-medium {t.textSecondary}">Quick start</p>
                  <span class="text-[11px] {t.textMuted}">{presetCountLabel}</span>
                </div>
                <div class="flex flex-wrap gap-2">
                  {#each presets as preset (preset.presetId ?? preset.name)}
                    <button
                      class="inline-flex min-h-9 items-center gap-2 rounded-full border px-3 py-2 text-left text-[11px] font-medium transition {presetRowTone}"
                      type="button"
                      title={preset.description || 'No preset description available.'}
                      onclick={() => handlePresetApply(preset)}
                      data-testid={`mesh-template-preset-${preset.presetId ?? preset.name}`}
                    >
                      <span>{preset.name || 'Untitled preset'}</span>
                      <span class="text-[10px] {t.textMuted}" data-testid={`mesh-template-preset-summary-${preset.presetId ?? preset.name}`}>
                        {presetMemberSummary(preset)}
                      </span>
                      <span class="flex items-center gap-1">
                        {#each presetTools(preset) as tool}
                          <span
                            class="inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-full border {roleMedallionTone(tool)}"
                            title={getToolName(tool)}
                            data-testid={`mesh-template-preset-tool-${preset.presetId ?? preset.name}-${tool}`}
                          >
                            <svg class="h-2.5 w-2.5" viewBox={getToolIcon(tool, 'sidebarSmall').viewBox} fill="currentColor" aria-hidden="true">
                              <path d={getToolIcon(tool, 'sidebarSmall').path}></path>
                            </svg>
                          </span>
                        {/each}
                      </span>
                      {#if preset.builtIn ?? false}
                        <span
                          class="rounded-full border px-1.5 py-0.5 text-[9px] font-medium {presetBadgeTone}"
                          data-testid={`mesh-template-preset-built-in-${preset.presetId ?? preset.name}`}
                        >
                          Built-in
                        </span>
                      {/if}
                    </button>
                  {/each}
                </div>
              </section>
            {/if}

            <div class="flex items-center justify-between gap-2">
              <p class="text-[11px] {t.textMuted}">
                {showAllRoleVersions ? 'Showing all saved role versions.' : 'Showing the latest saved version for each role.'}
              </p>
              <button
                class="text-[11px] font-medium underline decoration-current/35 underline-offset-4 transition hover:decoration-current {dark ? 'text-zinc-200' : 'text-zinc-700'}"
                type="button"
                onclick={toggleRoleVersionVisibility}
                data-testid="mesh-builder-version-visibility-toggle"
              >
                {showAllRoleVersions ? 'Latest only' : 'Show all versions'}
              </button>
            </div>
          </section>

          {#if pinnedRoles.length > 0}
            <section class="space-y-2" data-testid="mesh-builder-pinned-strip">
              <div class="flex items-center justify-between gap-2">
                <p class="text-[13px] font-medium {t.textPrimary}">Favorites</p>
                <span class="text-[11px] {t.textMuted}">{pinnedRoles.length}</span>
              </div>
              <div class="space-y-1.5" data-testid="mesh-builder-pinned-list">
                {#each pinnedRoles as role (role.roleId)}
                  <button
                    class="flex w-full items-center gap-3 rounded-[18px] border px-3 py-2.5 text-left transition {surfaceTone}"
                    type="button"
                    onclick={() => assignRole(role)}
                    data-testid={`mesh-builder-pinned-chip-${role.roleId}`}
                  >
                    <span class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-xl border {roleMedallionTone(role.cliTool)}">
                      <svg class="h-3.5 w-3.5" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
                        <path d={getToolIcon(role.cliTool).path}></path>
                      </svg>
                    </span>
                    <span class="min-w-0 flex-1">
                      <span class="block truncate text-[12px] font-semibold {t.textPrimary}">{role.name}</span>
                      <span class="block truncate text-[11px] {t.textSecondary}">
                        {getToolName(role.cliTool)} · {role.model}
                      </span>
                    </span>
                  </button>
                {/each}
              </div>
            </section>
          {/if}

          <div class="min-h-0 flex-1 space-y-4 md:overflow-y-auto md:pr-1" data-testid="mesh-builder-role-scroll">
          {#if visibleRoleCount === 0}
            <div class="rounded-[18px] border px-4 py-5 text-center {surfaceTone}" data-testid="mesh-builder-empty-results">
              <p class="text-[13px] font-semibold {t.textPrimary}">No roles match these filters</p>
              <p class="mt-1 text-[11px] {t.textSecondary}">Clear a filter or widen the search.</p>
            </div>
          {/if}

          {#if activeKindFilter !== 'agent' && leadRoles.length > 0}
            <section class="space-y-2" data-testid="mesh-builder-role-section-leads">
              <div class="flex items-center justify-between gap-2">
                <p class="text-[13px] font-medium {t.textPrimary}">Lead roles</p>
                <span class="text-[11px] {t.textMuted}">{leadRoles.length}</span>
              </div>
              <div
                class={catalogDensityMode === 'compact' ? 'space-y-1.5' : 'space-y-2'}
                data-testid="mesh-builder-role-list-leads"
                data-density-mode={catalogDensityMode}
              >
                {#each leadRoles as role (role.roleId)}
                  <div class="flex h-10 items-center gap-2 rounded-[16px] border px-2.5 transition {surfaceTone} {roleCardTone(role)}">
                    <button
                      class="flex min-w-0 flex-1 items-center gap-2.5 overflow-hidden text-left"
                      type="button"
                      title={roleSummaryText(role)}
                      onclick={() => assignRole(role)}
                      data-testid={`mesh-builder-role-${role.roleId}`}
                    >
                      <span class="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-lg border {roleMedallionTone(role.cliTool)}">
                        <svg class="h-3 w-3" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
                          <path d={getToolIcon(role.cliTool).path}></path>
                        </svg>
                      </span>
                      <span class="min-w-0 flex-1 truncate text-[12px] leading-none">
                        <span class="font-semibold {t.textPrimary}">{role.name}</span>
                        <span class="mx-1 {t.textMuted}">—</span>
                        <span class="{t.textSecondary}">{roleSummaryText(role)}</span>
                      </span>
                    </button>
                    <div class="flex shrink-0 items-center gap-1.5">
                      <span class="rounded-full border px-1.5 py-0.5 text-[9px] font-medium leading-none {roleChipTone(role)}">
                        {roleKindLabel(role)}
                      </span>
                      <button
                        class="inline-flex h-7 w-7 items-center justify-center rounded-full border transition {pinButtonTone(isRolePinned(role.roleId))}"
                        type="button"
                        aria-label={isRolePinned(role.roleId) ? `Unpin ${role.name}` : `Pin ${role.name}`}
                        aria-pressed={isRolePinned(role.roleId)}
                        onclick={(event) => handlePinToggle(event, role.roleId)}
                        data-testid={`mesh-builder-pin-${role.roleId}`}
                      >
                        <svg class="h-4 w-4" viewBox="0 0 16 16" fill={isRolePinned(role.roleId) ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="1.4" aria-hidden="true">
                          <path d="M8 1.7l1.76 3.57 3.94.57-2.85 2.78.67 3.93L8 10.67 4.48 12.55l.67-3.93L2.3 5.84l3.94-.57L8 1.7Z" stroke-linejoin="round"></path>
                        </svg>
                      </button>
                      <button
                        class="inline-flex h-7 w-7 items-center justify-center rounded-full border transition {ghostTone}"
                        type="button"
                        aria-label={`Add ${role.name}`}
                        onclick={() => assignRole(role)}
                        data-testid={`mesh-builder-add-${role.roleId}`}
                      >
                        +
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            </section>
          {/if}

          {#if activeKindFilter !== 'lead' && agentRoles.length > 0}
            <section class="space-y-2" data-testid="mesh-builder-role-section-agents">
              <div class="flex items-center justify-between gap-2">
                <p class="text-[13px] font-medium {t.textPrimary}">Agent roles</p>
                <span class="text-[11px] {t.textMuted}">{agentRoles.length}</span>
              </div>
              <div
                class={catalogDensityMode === 'compact' ? 'space-y-1.5' : 'space-y-2'}
                data-testid="mesh-builder-role-list-agents"
                data-density-mode={catalogDensityMode}
              >
                {#each agentRoles as role (role.roleId)}
                  <div class="flex h-10 items-center gap-2 rounded-[16px] border px-2.5 transition {surfaceTone} {roleCardTone(role)}">
                    <button
                      class="flex min-w-0 flex-1 items-center gap-2.5 overflow-hidden text-left"
                      type="button"
                      title={roleSummaryText(role)}
                      onclick={() => assignRole(role)}
                      data-testid={`mesh-builder-role-${role.roleId}`}
                    >
                      <span class="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-lg border {roleMedallionTone(role.cliTool)}">
                        <svg class="h-3 w-3" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
                          <path d={getToolIcon(role.cliTool).path}></path>
                        </svg>
                      </span>
                      <span class="min-w-0 flex-1 truncate text-[12px] leading-none">
                        <span class="font-semibold {t.textPrimary}">{role.name}</span>
                        <span class="mx-1 {t.textMuted}">—</span>
                        <span class="{t.textSecondary}">{roleSummaryText(role)}</span>
                      </span>
                    </button>
                    <div class="flex shrink-0 items-center gap-1.5">
                      <span class="rounded-full border px-1.5 py-0.5 text-[9px] font-medium leading-none {roleChipTone(role)}">
                        {roleKindLabel(role)}
                      </span>
                      <button
                        class="inline-flex h-7 w-7 items-center justify-center rounded-full border transition {pinButtonTone(isRolePinned(role.roleId))}"
                        type="button"
                        aria-label={isRolePinned(role.roleId) ? `Unpin ${role.name}` : `Pin ${role.name}`}
                        aria-pressed={isRolePinned(role.roleId)}
                        onclick={(event) => handlePinToggle(event, role.roleId)}
                        data-testid={`mesh-builder-pin-${role.roleId}`}
                      >
                        <svg class="h-4 w-4" viewBox="0 0 16 16" fill={isRolePinned(role.roleId) ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="1.4" aria-hidden="true">
                          <path d="M8 1.7l1.76 3.57 3.94.57-2.85 2.78.67 3.93L8 10.67 4.48 12.55l.67-3.93L2.3 5.84l3.94-.57L8 1.7Z" stroke-linejoin="round"></path>
                        </svg>
                      </button>
                      <button
                        class="inline-flex h-7 w-7 items-center justify-center rounded-full border transition {ghostTone}"
                        type="button"
                        aria-label={`Add ${role.name}`}
                        onclick={() => assignRole(role)}
                        data-testid={`mesh-builder-add-${role.roleId}`}
                      >
                        +
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            </section>
          {/if}
          </div>
        </div>
      </section>

      <section
        class="flex min-h-0 flex-col gap-4 rounded-[28px] border p-4 shadow-sm backdrop-blur {highlightedRosterSection === 'all' ? leadDropTone : rosterSectionTone} md:sticky md:top-0 md:max-h-[calc(100vh-11rem)]"
        data-testid="mesh-builder-team-panel"
      >
        <div class="rounded-[24px] border p-4 {dark ? 'border-white/[0.08] bg-white/[0.055]' : 'border-brand-200/55 bg-brand-50/55'}">
          <div class="flex flex-wrap items-start justify-between gap-4">
            <div class="min-w-0 flex-1 space-y-3">
              <div class="flex items-center gap-2">
                <h2 class="text-[16px] font-semibold {t.textPrimary}">Your Team</h2>
                <span class="rounded-full border px-2 py-0.5 text-[10px] font-medium {presetBadgeTone}">
                  {memberCount} member{memberCount === 1 ? '' : 's'}
                </span>
              </div>

              <div class="space-y-1.5">
                {#if editingTeamName}
                  <input
                    bind:this={teamNameInput}
                    class="h-10 w-full rounded-[14px] border px-3 text-[20px] font-semibold outline-none {inputTone}"
                    value={teamName}
                    oninput={(event) => onTeamNameChange(event.currentTarget.value)}
                    onblur={finishEditingTeamName}
                    onkeydown={(event) => {
                      if (event.key === 'Enter') {
                        event.preventDefault()
                        finishEditingTeamName()
                      }
                    }}
                    data-testid="mesh-builder-team-name-input"
                  />
                {:else}
                  <button
                    class="flex min-w-0 items-center gap-2 text-left"
                    type="button"
                    onclick={startEditingTeamName}
                    data-testid="mesh-builder-team-name-display"
                  >
                    <span class="truncate text-[22px] font-semibold leading-tight {t.textPrimary}">
                      {teamNameDisplayLabel}
                    </span>
                    <svg class="h-4 w-4 shrink-0 {t.textMuted}" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                      <path d="m11.9 2.4 1.7 1.7M3 13l2.8-.6 7-7a1.2 1.2 0 0 0 0-1.7l-.5-.5a1.2 1.2 0 0 0-1.7 0l-7 7L3 13Z" stroke-linecap="round" stroke-linejoin="round"></path>
                    </svg>
                  </button>
                {/if}

                {#if editingDescription}
                  <input
                    bind:this={teamDescriptionInput}
                    class="h-9 w-full rounded-[14px] border px-3 text-[13px] outline-none {inputTone}"
                    value={normalizedTeam.description ?? ''}
                    oninput={(event) => onDescriptionChange(event.currentTarget.value)}
                    onblur={finishEditingDescription}
                    onkeydown={(event) => {
                      if (event.key === 'Enter') {
                        event.preventDefault()
                        finishEditingDescription()
                      }
                    }}
                    data-testid="mesh-builder-team-description-input"
                  />
                {:else}
                  <button
                    class="flex min-w-0 items-center gap-2 text-left"
                    type="button"
                    onclick={startEditingDescription}
                    data-testid="mesh-builder-team-description-display"
                  >
                    <span class="truncate text-[13px] {t.textSecondary}">
                      {teamDescriptionDisplayLabel}
                    </span>
                    <svg class="h-3.5 w-3.5 shrink-0 {t.textMuted}" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                      <path d="m11.9 2.4 1.7 1.7M3 13l2.8-.6 7-7a1.2 1.2 0 0 0 0-1.7l-.5-.5a1.2 1.2 0 0 0-1.7 0l-7 7L3 13Z" stroke-linecap="round" stroke-linejoin="round"></path>
                    </svg>
                  </button>
                {/if}
              </div>

              <p class="text-[11px] {t.textMuted}" data-testid="mesh-builder-team-meta">
                {memberCount === 0 ? 'Start with a lead, then add agents.' : 'Lead role first, then fill in the supporting team.'}
              </p>
            </div>

            <div class="rounded-[18px] border px-3 py-2 text-right {dark ? 'border-white/[0.08] bg-black/10' : 'border-white/70 bg-white/75'}" data-testid="mesh-builder-team-status">
              <p class="text-[10px] uppercase tracking-[0.16em] {t.textMuted}">Roster status</p>
              <p class="mt-1 text-[12px] font-medium {t.textPrimary}">
                {normalizedTeam.lead ? 'Lead ready' : 'Lead required'}
              </p>
              <p class="mt-0.5 text-[11px] {t.textMuted}">
                {agents.length} agent{agents.length === 1 ? '' : 's'} assigned
              </p>
            </div>
          </div>
        </div>

        <div class="min-h-0 flex-1 space-y-4 md:overflow-y-auto md:pr-1" data-testid="mesh-builder-team-scroll">
          <section class="space-y-3 rounded-[22px] border p-3 {dark ? 'border-white/[0.08] bg-black/15' : 'border-zinc-200/80 bg-white/80'}" data-testid="mesh-builder-team-lead-group">
            <div class="flex items-center justify-between gap-3">
              <div>
                <p class="text-[12px] font-semibold {t.textPrimary}">Lead role</p>
                <p class="text-[11px] {t.textMuted}">Choose the person coordinating the team.</p>
              </div>
              {#if normalizedTeam.lead}
                <span class="rounded-full border px-2 py-0.5 text-[10px] font-medium {presetBadgeTone}">
                  Assigned
                </span>
              {/if}
            </div>

            <section
              class="space-y-2 transition {highlightedRosterSection === 'lead' || highlightedRosterSection === 'all' ? leadDropTone : ''}"
              data-testid="mesh-builder-lead-section"
            >
              {#if normalizedTeam.lead}
                <article
                  class="rounded-[20px] border p-3 {teamCardTone(normalizedTeam.lead.tool)}"
                  data-testid="mesh-builder-lead-card"
                >
                  <div class="flex items-start gap-3" data-testid="mesh-builder-lead-summary">
                    <button
                      class="flex min-w-0 flex-1 items-center gap-3 text-left"
                      type="button"
                      onclick={toggleLeadDetails}
                      data-testid="mesh-builder-lead-edit-toggle"
                    >
                      <span class="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-[14px] border {roleMedallionTone(normalizeTool(normalizedTeam.lead.tool))}">
                        <svg class="h-4 w-4" viewBox={getToolIcon(normalizeTool(normalizedTeam.lead.tool)).viewBox} fill="currentColor" aria-hidden="true">
                          <path d={getToolIcon(normalizeTool(normalizedTeam.lead.tool)).path}></path>
                        </svg>
                      </span>
                      <span class="min-w-0 flex-1">
                        <span class="flex items-center gap-2">
                          <span class="rounded-full border px-2 py-0.5 text-[9px] font-medium {dark ? 'border-amber-400/25 text-amber-200' : 'border-amber-300/70 text-amber-800'}">
                            Lead
                          </span>
                        </span>
                        <span class="mt-1 block truncate text-[14px] font-semibold {t.textPrimary}">
                          {normalizedTeam.lead.roleName || normalizedTeam.lead.roleId || 'Lead'}
                        </span>
                        <span class="mt-0.5 block truncate text-[11px] {t.textSecondary}">
                          {getToolName(normalizeTool(normalizedTeam.lead.tool))} · {normalizedTeam.lead.model || defaultModelForTool(normalizedTeam.lead.tool)}
                        </span>
                      </span>
                    </button>
                    <div class="flex shrink-0 items-center gap-2">
                      <button
                        class="inline-flex h-8 w-8 items-center justify-center rounded-full border transition {ghostTone}"
                        type="button"
                        aria-label={leadDetailsExpanded ? 'Collapse lead details' : 'Edit lead details'}
                        onclick={toggleLeadDetails}
                      >
                        <svg class="h-4 w-4" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
                          <path d={leadDetailsExpanded ? 'M4 10.25 8 6.25l4 4' : 'M4 5.75 8 9.75l4-4'} stroke-linecap="round" stroke-linejoin="round"></path>
                        </svg>
                      </button>
                      <button
                        class="inline-flex h-8 w-8 items-center justify-center rounded-full border transition {ghostTone}"
                        type="button"
                        aria-label="Clear lead role"
                        onclick={onClearLead}
                        data-testid="mesh-builder-lead-clear"
                      >
                        ×
                      </button>
                    </div>
                  </div>

                  {#if leadDetailsExpanded}
                    <div class="mt-3 grid gap-2 lg:grid-cols-[minmax(0,1fr)_minmax(0,0.9fr)_minmax(0,1fr)]">
                      <label class="space-y-1">
                        <span class="text-[10px] {t.textMuted}">Lead name</span>
                        <input
                          class="h-10 w-full rounded-[14px] border px-3 text-sm outline-none {inputTone}"
                          value={normalizedTeam.lead.name ?? ''}
                          oninput={(event) => onUpdateLead({ name: event.currentTarget.value })}
                          data-testid="mesh-builder-lead-name-input"
                        />
                      </label>
                      <label class="space-y-1">
                        <span class="text-[10px] {t.textMuted}">Model</span>
                        <select
                          class="h-10 w-full rounded-[14px] border px-3 text-sm outline-none {inputTone}"
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
                        <span class="text-[10px] {t.textMuted}">Project</span>
                        <div class="relative">
                          <select
                            class="h-10 w-full appearance-none rounded-[14px] border px-3 pr-9 text-sm outline-none {inputTone}"
                            value={normalizedTeam.lead.projectId ?? ''}
                            onchange={(event) => onUpdateLead({ projectId: event.currentTarget.value })}
                            disabled={availableProjectOptions.length === 0}
                            title={projectSelectLabel(normalizedTeam.lead.projectId ?? '')}
                            data-testid="mesh-builder-lead-project-input"
                          >
                            <option value="">
                              {availableProjectOptions.length > 0 ? 'Choose project' : 'No registered projects'}
                            </option>
                            {#each availableProjectOptions as project}
                              <option value={project.id}>{project.label}</option>
                            {/each}
                          </select>
                          <div class="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 {t.textMuted}">
                            <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                              <path d="m6 9 6 6 6-6"></path>
                            </svg>
                          </div>
                        </div>
                      </label>
                    </div>
                  {/if}
                </article>
              {:else}
                <div class="rounded-[20px] border border-dashed px-4 py-4 {invalidDropTone}" data-testid="mesh-builder-lead-empty">
                  <div class="flex items-start justify-between gap-3">
                    <div>
                      <p class="text-[13px] font-medium {t.textPrimary}">Choose a lead role to anchor the team.</p>
                      <p class="mt-1 text-[11px] {t.textSecondary}">
                        Use the + button next to any lead on the left.
                      </p>
                    </div>
                    <span class="rounded-full border px-2 py-0.5 text-[10px] font-medium {dark ? 'border-danger-400/30 text-danger-200' : 'border-danger-300/70 text-danger-700'}">
                      Required
                    </span>
                  </div>
                </div>
              {/if}
            </section>
          </section>

          <section class="space-y-3 rounded-[22px] border p-3 {dark ? 'border-white/[0.08] bg-black/15' : 'border-zinc-200/80 bg-white/80'}" data-testid="mesh-builder-team-agents-group">
            <div class="flex items-center justify-between gap-3">
              <div>
                <p class="text-[12px] font-semibold {t.textPrimary}">Agent roles</p>
                <p class="text-[11px] {t.textMuted}">Add specialists from the catalog to round out the team.</p>
              </div>
              <span class="rounded-full border px-2 py-0.5 text-[10px] font-medium {presetBadgeTone}">
                {agents.length} assigned
              </span>
            </div>

            <section
              class="space-y-2 transition {highlightedRosterSection === 'agents' || highlightedRosterSection === 'all' ? leadDropTone : ''}"
              data-testid="mesh-builder-agents-section"
            >
              {#each agents as agent (agent.id)}
                <article
                  class="rounded-[20px] border p-3 {teamCardTone(agent.tool)}"
                  data-testid={`mesh-builder-agent-card-${agent.id}`}
                >
                  <div class="flex items-start gap-3" data-testid={`mesh-builder-agent-summary-${agent.id}`}>
                    <div class="flex min-w-0 flex-1 items-start gap-3" data-testid="mesh-node-agent">
                      <button
                        class="flex min-w-0 flex-1 items-center gap-3 text-left"
                        type="button"
                        onclick={() => toggleAgentDetails(agent.id)}
                        data-testid={`mesh-builder-agent-edit-toggle-${agent.id}`}
                      >
                        <span class="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-[14px] border {roleMedallionTone(normalizeTool(agent.tool))}">
                          <svg class="h-3.5 w-3.5" viewBox={getToolIcon(normalizeTool(agent.tool)).viewBox} fill="currentColor" aria-hidden="true">
                            <path d={getToolIcon(normalizeTool(agent.tool)).path}></path>
                          </svg>
                        </span>
                        <span class="min-w-0 flex-1">
                          <span class="block truncate text-[13px] font-semibold {t.textPrimary}">
                            {agent.roleName || agent.roleId || agent.name}
                          </span>
                          <span class="mt-0.5 block truncate text-[11px] {t.textSecondary}">
                            {getToolName(normalizeTool(agent.tool))} · {agent.model || defaultModelForTool(agent.tool)}
                          </span>
                        </span>
                      </button>
                      <div class="flex shrink-0 items-center gap-2">
                        <button
                          class="inline-flex h-8 w-8 items-center justify-center rounded-full border transition {ghostTone}"
                          type="button"
                          aria-label={isAgentExpanded(agent.id) ? `Collapse ${agent.name} details` : `Edit ${agent.name} details`}
                          onclick={() => toggleAgentDetails(agent.id)}
                        >
                          <svg class="h-4 w-4" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
                            <path d={isAgentExpanded(agent.id) ? 'M4 10.25 8 6.25l4 4' : 'M4 5.75 8 9.75l4-4'} stroke-linecap="round" stroke-linejoin="round"></path>
                          </svg>
                        </button>
                        <button
                          class="inline-flex h-8 w-8 items-center justify-center rounded-full border transition {ghostTone}"
                          type="button"
                          onclick={() => onRemoveAgent(agent.id)}
                          data-testid={`mesh-builder-agent-remove-${agent.id}`}
                        >
                          ×
                        </button>
                      </div>
                    </div>
                  </div>

                  {#if isAgentExpanded(agent.id)}
                    <div class="mt-3 grid gap-2 lg:grid-cols-[minmax(0,1fr)_minmax(0,0.9fr)_minmax(0,1fr)]">
                      <label class="space-y-1">
                        <span class="text-[10px] {t.textMuted}">Agent name</span>
                        <input
                          class="h-10 w-full rounded-[14px] border px-3 text-sm outline-none {inputTone}"
                          value={agent.name ?? ''}
                          oninput={(event) => onUpdateAgent(agent.id, { name: event.currentTarget.value })}
                          data-testid={`mesh-builder-agent-name-input-${agent.id}`}
                        />
                      </label>
                      <label class="space-y-1">
                        <span class="text-[10px] {t.textMuted}">Model</span>
                        <select
                          class="h-10 w-full rounded-[14px] border px-3 text-sm outline-none {inputTone}"
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
                        <span class="text-[10px] {t.textMuted}">Project</span>
                        <div class="relative">
                          <select
                            class="h-10 w-full appearance-none rounded-[14px] border px-3 pr-9 text-sm outline-none {inputTone}"
                            value={agent.projectId ?? ''}
                            onchange={(event) => onUpdateAgent(agent.id, { projectId: event.currentTarget.value })}
                            disabled={availableProjectOptions.length === 0}
                            title={projectSelectLabel(agent.projectId ?? '')}
                            data-testid={`mesh-builder-agent-project-input-${agent.id}`}
                          >
                            <option value="">
                              {availableProjectOptions.length > 0 ? 'Choose project' : 'No registered projects'}
                            </option>
                            {#each availableProjectOptions as project}
                              <option value={project.id}>{project.label}</option>
                            {/each}
                          </select>
                          <div class="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 {t.textMuted}">
                            <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                              <path d="m6 9 6 6 6-6"></path>
                            </svg>
                          </div>
                        </div>
                      </label>
                    </div>
                  {/if}
                </article>
              {/each}

              <div
                class="rounded-[18px] border border-dashed px-4 py-4 {surfaceTone}"
                data-testid="mesh-builder-agent-dropzone"
                data-dropzone-mode={agents.length > 0 ? 'compact' : 'empty'}
              >
                <p class="text-[12px] font-medium {t.textPrimary}">+ Add from catalog</p>
                <p class="mt-1 text-[11px] {t.textSecondary}">
                  {agents.length > 0
                    ? 'Keep building with developer, reviewer, and research roles from the left.'
                    : 'Start with a developer, researcher, or reviewer to flesh out the team.'}
                </p>
              </div>
            </section>
          </section>
        </div>

        <footer class="shrink-0 space-y-3 border-t pt-3 {dark ? 'border-white/[0.08]' : 'border-zinc-200/70'}" data-testid="mesh-action-bar">
          <div class="w-full" title={!canInitialize ? initializeButtonTitle : undefined} data-testid="mesh-action-initialize-hint">
            <button
              class="flex h-12 w-full items-center justify-center gap-2 rounded-[18px] bg-brand-600 px-4 text-[13px] font-semibold text-white transition hover:bg-brand-500 disabled:cursor-not-allowed disabled:opacity-50"
              type="button"
              onclick={onInitialize}
              disabled={!canInitialize}
              data-testid="mesh-action-initialize"
            >
              Initialize Team
              <span aria-hidden="true">→</span>
            </button>
          </div>

          <div class="flex items-center justify-between gap-3">
            <button
              class="text-[12px] font-medium underline decoration-current/30 underline-offset-4 transition hover:decoration-current {dark ? 'text-zinc-100' : 'text-zinc-800'}"
              type="button"
              onclick={onSavePreset}
              data-testid="mesh-builder-save-preset"
            >
              Save as Preset
            </button>
            <button
              class="text-[12px] font-medium underline decoration-current/30 underline-offset-4 transition hover:decoration-current {t.textMuted}"
              type="button"
              onclick={handleReset}
              data-testid="mesh-action-reset"
            >
              Reset
            </button>
          </div>
        </footer>
      </section>
    </div>
  </main>
</section>
