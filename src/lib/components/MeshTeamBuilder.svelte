<script>
  import { onDestroy, onMount, tick } from 'svelte'
  import {
    deleteRoleTemplate,
    exportRoleToFile,
    getRoleTemplate,
    importRoleFromFile,
    isTauri,
    upsertRoleTemplate,
  } from '../ipc.js'
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
  import ConfirmDialog from './ConfirmDialog.svelte'
  import MeshNodeDetail from './MeshNodeDetail.svelte'
  import MeshRoleEditorDialog from './MeshRoleEditorDialog.svelte'
  import {
    latestRoleVersions,
    ROLE_VERSION_VISIBILITY_STORAGE_KEY,
  } from './roleVersioning.js'

  const CATALOG_DENSITY_STORAGE_KEY = 'taurhaus.mesh.roleCatalogDensity'
  const PINNED_ROLE_IDS_STORAGE_KEY = 'taurhaus.mesh.pinnedRoleIds'
  const ROLE_SOURCE_FEEDBACK_MS = 400
  const ROSTER_FEEDBACK_MS = 500
  const ROSTER_ENTRY_FEEDBACK_MS = 600
  const REMOVE_AGENT_FEEDBACK_MS = 120
  const PIN_BOUNCE_FEEDBACK_MS = 200

  let {
    dark = false,
    mode = 'empty',
    teamName = '',
    teamConfig = null,
    roleTemplates = [],
    presets = [],
    availableProjects = [],
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
    onRefreshRoleTemplates = async () => {},
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
  const rosterPanelHighlightTone = $derived(
    dark ? 'bg-white/[0.02]' : 'bg-brand-50/55'
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
      : 'border-brand-200/80 bg-brand-50/55 shadow-[inset_0_1px_0_rgba(255,255,255,0.8)]'
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
  const teamSummaryTone = $derived(
    dark
      ? 'border-white/[0.08] bg-black/10'
      : 'border-brand-200/80 bg-brand-50/65 shadow-[inset_0_1px_0_rgba(255,255,255,0.78)]'
  )
  const agentDropzoneTone = $derived(
    dark
      ? 'border-white/[0.08] bg-black/15'
      : 'border-brand-200/75 bg-white/82 shadow-[inset_0_1px_0_rgba(255,255,255,0.78)]'
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
  let flashedRoleId = $state('')
  let enteringMemberIds = $state([])
  let removingAgentIds = $state([])
  let bouncingPinRoleIds = $state([])
  let draggingCatalogRoleId = $state('')
  let draggingRosterAgentId = $state('')
  let leadDropState = $state('idle')
  let agentDropState = $state('idle')
  let reorderTargetAgentId = $state('')
  let catalogSearchInput = $state(null)
  let catalogDensityPreference = $state(null)
  let teamNameInput = $state(null)
  let teamDescriptionInput = $state(null)
  let selectedRoleDetailId = $state('')
  let roleEditorOpen = $state(false)
  let roleEditorRole = $state(null)
  let roleEditorError = $state('')
  let roleEditorSaving = $state(false)
  let roleStatusMessage = $state('')
  let roleStatusKind = $state('info')
  let deleteRoleContext = $state(null)
  let importConflict = $state(null)
  let exportingRoleId = $state('')
  let rosterFeedbackTimer = null
  let roleFeedbackTimer = null
  let roleStatusTimer = null
  let previousRosterMemberIds = []
  let hasObservedRosterMembers = false
  const memberEntryTimers = new Map()
  const memberRemovalTimers = new Map()
  const pinBounceTimers = new Map()

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
  const visiblePinnedRoles = $derived.by(() => {
    const visibleRoleIds = new Set(filteredRoles.map((role) => role.roleId))
    return pinnedRoles.filter((role) => visibleRoleIds.has(role.roleId))
  })
  const catalogListRoles = $derived(
    filteredRoles.filter((role) => !pinnedRoleIds.includes(role.roleId))
  )
  const leadRoles = $derived(catalogListRoles.filter((role) => role.kind === 'lead'))
  const agentRoles = $derived(catalogListRoles.filter((role) => role.kind !== 'lead'))
  const selectedRoleDetail = $derived.by(() =>
    normalizedRoles.find((role) => role.roleId === selectedRoleDetailId) ?? null
  )
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
    if (roleFeedbackTimer) {
      clearTimeout(roleFeedbackTimer)
    }
    if (roleStatusTimer) {
      clearTimeout(roleStatusTimer)
    }
    for (const timer of memberEntryTimers.values()) {
      clearTimeout(timer)
    }
    for (const timer of memberRemovalTimers.values()) {
      clearTimeout(timer)
    }
    for (const timer of pinBounceTimers.values()) {
      clearTimeout(timer)
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

  $effect(() => {
    const currentRosterMemberIds = [
      normalizedTeam?.lead?.id ? `lead:${normalizedTeam.lead.id}` : null,
      ...agents.map((agent) => `agent:${agent.id}`),
    ].filter(Boolean)

    const nextEnteringMemberIds = enteringMemberIds.filter((memberId) =>
      currentRosterMemberIds.includes(memberId)
    )
    if (!sameItems(nextEnteringMemberIds, enteringMemberIds)) {
      enteringMemberIds = nextEnteringMemberIds
    }

    const nextRemovingAgentIds = removingAgentIds.filter((agentId) =>
      agents.some((agent) => agent.id === agentId)
    )
    if (!sameItems(nextRemovingAgentIds, removingAgentIds)) {
      removingAgentIds = nextRemovingAgentIds
    }

    if (!hasObservedRosterMembers) {
      previousRosterMemberIds = currentRosterMemberIds
      hasObservedRosterMembers = true
      return
    }

    const previousIds = new Set(previousRosterMemberIds)
    for (const memberId of currentRosterMemberIds) {
      if (!previousIds.has(memberId)) {
        markRosterMemberEntry(memberId)
      }
    }
    previousRosterMemberIds = currentRosterMemberIds
  })

  function sameItems(left, right) {
    if (left.length !== right.length) return false
    return left.every((item, index) => item === right[index])
  }

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
    }, ROSTER_FEEDBACK_MS)
  }

  function triggerRoleSourceFeedback(roleId) {
    flashedRoleId = roleId
    if (roleFeedbackTimer) {
      clearTimeout(roleFeedbackTimer)
    }
    roleFeedbackTimer = setTimeout(() => {
      flashedRoleId = ''
      roleFeedbackTimer = null
    }, ROLE_SOURCE_FEEDBACK_MS)
  }

  function markRosterMemberEntry(memberId) {
    if (!memberId || enteringMemberIds.includes(memberId)) return
    enteringMemberIds = [...enteringMemberIds, memberId]
    const existingTimer = memberEntryTimers.get(memberId)
    if (existingTimer) {
      clearTimeout(existingTimer)
    }
    const timer = setTimeout(() => {
      enteringMemberIds = enteringMemberIds.filter((entry) => entry !== memberId)
      memberEntryTimers.delete(memberId)
    }, ROSTER_ENTRY_FEEDBACK_MS)
    memberEntryTimers.set(memberId, timer)
  }

  function isRosterMemberEntering(memberId) {
    return enteringMemberIds.includes(memberId)
  }

  function markPinBounce(roleId) {
    if (!roleId) return
    if (!bouncingPinRoleIds.includes(roleId)) {
      bouncingPinRoleIds = [...bouncingPinRoleIds, roleId]
    }
    const existingTimer = pinBounceTimers.get(roleId)
    if (existingTimer) {
      clearTimeout(existingTimer)
    }
    const timer = setTimeout(() => {
      bouncingPinRoleIds = bouncingPinRoleIds.filter((entry) => entry !== roleId)
      pinBounceTimers.delete(roleId)
    }, PIN_BOUNCE_FEEDBACK_MS)
    pinBounceTimers.set(roleId, timer)
  }

  function isPinBouncing(roleId) {
    return bouncingPinRoleIds.includes(roleId)
  }

  function isRoleFlashing(roleId) {
    return flashedRoleId === roleId
  }

  function isAgentRemoving(agentId) {
    return removingAgentIds.includes(agentId)
  }

  function roleCardTone(role) {
    if (draggingCatalogRoleId === role.roleId) return 'opacity-50'
    return ''
  }

  function teamCardTone(tool, kind = 'agent') {
    if (kind === 'lead') {
      return dark
        ? 'border-amber-400/35 bg-amber-500/10'
        : 'border-amber-300/90 bg-amber-50 shadow-[inset_0_1px_0_rgba(255,255,255,0.82)]'
    }

    const normalizedTool = normalizeTool(tool)
    if (normalizedTool === 'codex') {
      return dark
        ? 'border-sky-400/28 bg-sky-500/10'
        : 'border-sky-300/85 bg-sky-50 shadow-[inset_0_1px_0_rgba(255,255,255,0.82)]'
    }
    if (normalizedTool === 'gemini') {
      return dark
        ? 'border-violet-400/28 bg-violet-500/10'
        : 'border-violet-300/85 bg-violet-50 shadow-[inset_0_1px_0_rgba(255,255,255,0.82)]'
    }
    return dark
      ? 'border-emerald-400/28 bg-emerald-500/10'
      : 'border-emerald-300/85 bg-emerald-50 shadow-[inset_0_1px_0_rgba(255,255,255,0.82)]'
  }

  function memberAccentTone(tool, kind = 'agent') {
    if (kind === 'lead') return dark ? 'bg-amber-300' : 'bg-amber-500'
    switch (normalizeTool(tool)) {
      case 'codex':
        return dark ? 'bg-sky-300' : 'bg-sky-500'
      case 'gemini':
        return dark ? 'bg-violet-300' : 'bg-violet-500'
      default:
        return dark ? 'bg-emerald-300' : 'bg-emerald-500'
    }
  }

  function memberMetaTone(tool, kind = 'agent') {
    if (kind === 'lead') {
      return dark
        ? 'border-amber-400/30 bg-amber-500/14 text-amber-100'
        : 'border-amber-300/70 bg-amber-50 text-amber-900'
    }
    switch (normalizeTool(tool)) {
      case 'codex':
        return dark
          ? 'border-sky-400/30 bg-sky-500/14 text-sky-100'
          : 'border-sky-300/70 bg-sky-50 text-sky-900'
      case 'gemini':
        return dark
          ? 'border-violet-400/30 bg-violet-500/14 text-violet-100'
          : 'border-violet-300/70 bg-violet-50 text-violet-900'
      default:
        return dark
          ? 'border-emerald-400/30 bg-emerald-500/14 text-emerald-100'
          : 'border-emerald-300/70 bg-emerald-50 text-emerald-900'
    }
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
    const shouldBounce = !isRolePinned(roleId)
    togglePinnedRole(roleId)
    if (shouldBounce) {
      markPinBounce(roleId)
    }
  }

  function assignRole(role) {
    if (!role?.roleId) return
    triggerRoleSourceFeedback(role.roleId)
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

  function roleDetailNode(role) {
    if (!role) return null
    return {
      name: role.name,
      roleName: role.name,
      role: role.kind,
      tool: role.cliTool,
      model: role.model,
      status: 'available',
      roleId: role.roleId,
      focusArea: role.focusArea ?? role.focus_area ?? '',
      contextSummary: role.contextSummary ?? role.context_summary ?? '',
      behaviorSummary: role.behaviorSummary ?? role.behavior_summary ?? '',
      instructions: role.instructions ?? role.description ?? '',
      behavioralContract: role.behavioralContract ?? role.behavioral_contract ?? null,
      capabilities: Array.isArray(role.capabilities) ? role.capabilities : [],
    }
  }

  function openRoleDetail(role) {
    selectedRoleDetailId = String(role?.roleId ?? '').trim()
  }

  function closeRoleDetail() {
    selectedRoleDetailId = ''
  }

  function handleRoleDetailAdd() {
    if (!selectedRoleDetail) return
    assignRole(selectedRoleDetail)
    closeRoleDetail()
  }

  function handleRemoveAgent(agentId) {
    if (!agentId || isAgentRemoving(agentId)) return
    removingAgentIds = [...removingAgentIds, agentId]
    const existingTimer = memberRemovalTimers.get(agentId)
    if (existingTimer) {
      clearTimeout(existingTimer)
    }
    const timer = setTimeout(() => {
      memberRemovalTimers.delete(agentId)
      onRemoveAgent(agentId)
      removingAgentIds = removingAgentIds.filter((entry) => entry !== agentId)
    }, REMOVE_AGENT_FEEDBACK_MS)
    memberRemovalTimers.set(agentId, timer)
  }

  async function handleInitializeClick() {
    if (memberRemovalTimers.size > 0) {
      const pendingAgentIds = [...memberRemovalTimers.keys()]
      for (const agentId of pendingAgentIds) {
        const timer = memberRemovalTimers.get(agentId)
        if (timer) {
          clearTimeout(timer)
        }
        memberRemovalTimers.delete(agentId)
        onRemoveAgent(agentId)
      }
      removingAgentIds = []
      await tick()
    }

    onInitialize()
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
          ? 'border-emerald-400/35 bg-emerald-500/12 text-emerald-200'
          : 'border-emerald-300/70 bg-emerald-50 text-emerald-800'
      case 'codex':
        return dark
          ? 'border-sky-400/35 bg-sky-500/12 text-sky-200'
          : 'border-sky-300/70 bg-sky-50 text-sky-800'
      case 'gemini':
        return dark
          ? 'border-violet-400/35 bg-violet-500/12 text-violet-200'
          : 'border-violet-300/70 bg-violet-50 text-violet-800'
      default:
        return dark
          ? 'border-white/[0.14] bg-white/[0.05] text-zinc-200'
          : 'border-zinc-200 bg-zinc-50 text-zinc-700'
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

  function showRoleStatus(message, kind = 'info') {
    roleStatusMessage = String(message ?? '').trim()
    roleStatusKind = kind === 'error' ? 'error' : 'info'
    if (roleStatusTimer) {
      clearTimeout(roleStatusTimer)
    }
    if (!roleStatusMessage) {
      roleStatusTimer = null
      return
    }
    roleStatusTimer = setTimeout(() => {
      roleStatusMessage = ''
      roleStatusTimer = null
    }, 3200)
  }

  function clearRoleStatus() {
    if (roleStatusTimer) {
      clearTimeout(roleStatusTimer)
      roleStatusTimer = null
    }
    roleStatusMessage = ''
  }

  function closeRoleEditor() {
    roleEditorOpen = false
    roleEditorRole = null
    roleEditorError = ''
    roleEditorSaving = false
  }

  function exportFilenameForRole(role) {
    const base = String(role?.name ?? role?.roleId ?? 'role')
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'role'
    return `${base}.yaml`
  }

  async function saveExportedRoleFile(filename, fileContent) {
    if (isTauri()) {
      const [{ save }, { writeTextFile }] = await Promise.all([
        import('@tauri-apps/plugin-dialog'),
        import('@tauri-apps/plugin-fs'),
      ])

      const path = await save({
        defaultPath: filename,
        filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }],
      })

      if (!path) return false
      await writeTextFile(path, fileContent)
      return true
    }

    const blob = new Blob([fileContent], { type: 'application/yaml;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = filename
    link.click()
    URL.revokeObjectURL(url)
    return true
  }

  async function pickRoleImportFile() {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selection = await open({
      multiple: false,
      filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }],
    })

    if (Array.isArray(selection)) {
      return typeof selection[0] === 'string' ? selection[0] : null
    }

    return typeof selection === 'string' ? selection : null
  }

  function createDefaultBehavioralContract() {
    return {
      communication: [],
      execution: [],
      escalation: [],
    }
  }

  function buildRoleSavePayload(draft) {
    const source = roleEditorRole && typeof roleEditorRole === 'object' ? roleEditorRole : null
    const roleId = String(draft?.roleId ?? '').trim()
    const kind = String(draft?.kind ?? source?.kind ?? 'agent').trim().toLowerCase() === 'lead'
      ? 'lead'
      : 'agent'
    const tool = normalizeTool(draft?.tool ?? source?.tool ?? source?.cliTool ?? 'codex')
    const model = String(draft?.model ?? '').trim() || defaultModelForTool(tool)
    const defaultNamePattern = source?.defaults?.defaultNamePattern
      ?? source?.defaults?.default_name_pattern
      ?? (kind === 'lead' ? 'team-lead' : `${roleId || 'agent'}-{n}`)
    const sourceConstraints = source?.constraints && typeof source.constraints === 'object'
      ? source.constraints
      : {}

    return {
      schema: {
        kind: 'role_template',
        version: Number(source?.schema?.version ?? 1) || 1,
      },
      roleId,
      name: String(draft?.name ?? '').trim(),
      version: String(source?.version ?? '1.0.0'),
      kind,
      defaults: {
        cliTool: tool,
        model,
        defaultNamePattern,
      },
      instructions: String(draft?.instructions ?? '').trim(),
      focusArea: draft?.focusArea ?? null,
      contextSummary: draft?.contextSummary ?? null,
      behaviorSummary: draft?.behaviorSummary ?? null,
      behavioralContract:
        source?.behavioralContract
        ?? source?.behavioral_contract
        ?? createDefaultBehavioralContract(),
      capabilities: Array.isArray(source?.capabilities) ? source.capabilities : [],
      provenance: source?.provenance ?? null,
      constraints: {
        minInstances:
          kind === 'lead'
            ? 1
            : Math.max(0, Number(sourceConstraints.minInstances ?? 0) || 0),
        maxInstances:
          kind === 'lead'
            ? 1
            : Math.max(1, Number(sourceConstraints.maxInstances ?? 8) || 8),
        requiresLeadTool: sourceConstraints.requiresLeadTool ?? null,
        allowedProjectBinding: sourceConstraints.allowedProjectBinding ?? 'lead_project',
      },
    }
  }

  function openCreateRoleEditor() {
    clearRoleStatus()
    selectedRoleDetailId = ''
    roleEditorRole = null
    roleEditorError = ''
    roleEditorSaving = false
    roleEditorOpen = true
  }

  async function openRoleEditor(role = selectedRoleDetail) {
    if (!role?.roleId || role.readOnly) return
    clearRoleStatus()
    roleEditorError = ''
    roleEditorSaving = false

    try {
      const detail = await getRoleTemplate(role.roleId)
      roleEditorRole = { ...role, ...detail }
    } catch {
      roleEditorRole = { ...role }
    }

    selectedRoleDetailId = ''
    roleEditorOpen = true
  }

  async function handleRoleEditorSave(draft) {
    roleEditorSaving = true
    roleEditorError = ''

    try {
      const payload = buildRoleSavePayload(draft)
      await upsertRoleTemplate(payload)
      await onRefreshRoleTemplates?.()
      closeRoleEditor()
      selectedRoleDetailId = payload.roleId
      showRoleStatus(`Saved '${payload.name}'.`)
    } catch (error) {
      roleEditorError = error?.message || 'Failed to save role template.'
    } finally {
      roleEditorSaving = false
    }
  }

  async function handleImportYaml() {
    clearRoleStatus()
    let filePath = null

    try {
      filePath = await pickRoleImportFile()
    } catch (error) {
      showRoleStatus(error?.message || 'Failed to open the YAML import dialog.', 'error')
      return
    }

    if (!filePath) return

    try {
      const result = await importRoleFromFile(filePath)
      if (result?.conflict) {
        importConflict = {
          rawRole: result.role,
          importedRole: result.role,
          existingRole: result.conflict,
        }
        return
      }

      await onRefreshRoleTemplates?.()
      selectedRoleDetailId = String(result?.role?.roleId ?? '').trim()
      showRoleStatus(`Imported '${result?.role?.name ?? result?.role?.roleId ?? 'role'}'.`)
    } catch (error) {
      showRoleStatus(error?.message || 'Failed to import role YAML.', 'error')
    }
  }

  function cancelImportConflict() {
    importConflict = null
  }

  async function confirmImportConflictReplace() {
    if (!importConflict?.rawRole) return
    const pendingImport = importConflict
    importConflict = null

    try {
      await upsertRoleTemplate(pendingImport.rawRole)
      await onRefreshRoleTemplates?.()
      selectedRoleDetailId = String(pendingImport.rawRole?.roleId ?? '').trim()
      showRoleStatus(
        `Replaced '${pendingImport.rawRole?.name ?? pendingImport.rawRole?.roleId ?? 'role'}'.`
      )
    } catch (error) {
      showRoleStatus(error?.message || 'Failed to replace the existing role.', 'error')
    }
  }

  async function handleRoleDetailExport() {
    if (!selectedRoleDetail?.roleId || exportingRoleId) return
    clearRoleStatus()
    exportingRoleId = selectedRoleDetail.roleId

    try {
      const exported = await exportRoleToFile(selectedRoleDetail.roleId, 'yaml')
      const saved = await saveExportedRoleFile(
        exportFilenameForRole(selectedRoleDetail),
        exported?.fileContent ?? ''
      )
      if (!saved) return
      showRoleStatus(`Exported '${selectedRoleDetail.name}'.`)
    } catch (error) {
      showRoleStatus(error?.message || 'Failed to export role YAML.', 'error')
    } finally {
      exportingRoleId = ''
    }
  }

  function requestRoleDelete() {
    if (!selectedRoleDetail?.roleId || selectedRoleDetail.readOnly) return
    deleteRoleContext = {
      roleId: selectedRoleDetail.roleId,
      name: selectedRoleDetail.name,
    }
  }

  function cancelRoleDelete() {
    deleteRoleContext = null
  }

  async function confirmRoleDelete() {
    if (!deleteRoleContext?.roleId) return
    const target = deleteRoleContext
    deleteRoleContext = null

    try {
      await deleteRoleTemplate(target.roleId)
      await onRefreshRoleTemplates?.()
      if (selectedRoleDetailId === target.roleId) {
        closeRoleDetail()
      }
      showRoleStatus(`Deleted '${target.name ?? target.roleId}'.`)
    } catch (error) {
      showRoleStatus(error?.message || 'Failed to delete role template.', 'error')
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

  <main class="min-h-0" data-testid="mesh-builder-roster">
    <div
      class="grid min-h-0 overflow-hidden rounded-[30px] border shadow-sm backdrop-blur {panelTone} md:h-[calc(100vh-10.75rem)] md:min-h-[640px] md:grid-cols-[minmax(0,1.22fr)_minmax(340px,0.94fr)] md:items-stretch"
      data-testid="mesh-builder-shell"
    >
      <section
        class="flex min-h-0 flex-col overflow-hidden border-b p-4 md:border-b-0 md:border-r md:pr-5 {dark ? 'border-white/[0.08]' : 'border-brand-200/55'}"
        data-testid="mesh-builder-catalog"
        data-collapsed="false"
      >
        <div class="flex flex-wrap items-start justify-between gap-3 pb-4">
          <div class="space-y-1">
            <h2 class="text-[16px] font-semibold {t.textPrimary}">Available Roles</h2>
            <p class="text-[12px] {t.textSecondary}">
              Search roles, pin favorites, and build the lineup from left to right.
            </p>
          </div>

          <div class="flex flex-wrap items-center gap-2">
            <button
              class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[11px] font-medium transition {ghostTone}"
              type="button"
              onclick={handleImportYaml}
              data-testid="mesh-builder-import-yaml"
            >
              Import YAML
            </button>
            <button
              class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[11px] font-medium transition {ghostTone}"
              type="button"
              onclick={openCreateRoleEditor}
              data-testid="mesh-builder-create-role"
            >
              New role
            </button>
            <button
              class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[11px] font-medium transition {ghostTone}"
              type="button"
              onclick={focusCatalogSearch}
              data-testid="mesh-template-browse-catalog"
            >
              Focus search
            </button>
          </div>
        </div>

        {#if roleStatusMessage}
          <div
            class="mb-3 rounded-[16px] border px-3 py-2 text-[12px] {roleStatusKind === 'error'
              ? (dark ? 'border-danger-400/30 bg-danger-500/10 text-danger-100' : 'border-danger-300 bg-danger-50 text-danger-700')
              : (dark ? 'border-brand-400/30 bg-brand-500/10 text-zinc-100' : 'border-brand-200 bg-brand-50 text-brand-900')}"
            data-testid="mesh-builder-role-status"
          >
            {roleStatusMessage}
          </div>
        {/if}

        <div class="flex min-h-0 flex-1 flex-col space-y-3 overflow-hidden" data-testid="mesh-builder-catalog-content">
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
                      class="mesh-builder-pressable inline-flex min-h-9 items-center gap-2 rounded-full border px-3 py-2 text-left text-[11px] font-medium transition active:scale-[0.98] {presetRowTone}"
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

          {#if visiblePinnedRoles.length > 0}
            <section
              class="space-y-2 border-b pb-3 {dark ? 'border-white/[0.08]' : 'border-zinc-200/80'}"
              data-testid="mesh-builder-pinned-strip"
            >
              <div class="flex items-center justify-between gap-2">
                <p class="text-[13px] font-medium {t.textPrimary}">Favorites</p>
                <span class="text-[11px] {t.textMuted}">{visiblePinnedRoles.length}</span>
              </div>
              <div
                class={catalogDensityMode === 'compact' ? 'space-y-1.5' : 'space-y-2'}
                data-testid="mesh-builder-pinned-list"
                data-density-mode={catalogDensityMode}
              >
                {#each visiblePinnedRoles as role (role.roleId)}
                  <div
                    class="mesh-builder-role-row group flex h-10 cursor-pointer items-center gap-2 rounded-[16px] border px-2.5 transition {surfaceTone} {roleCardTone(role)} {isRoleFlashing(role.roleId) ? 'mesh-builder-role-row-active' : ''}"
                    data-testid={`mesh-builder-pinned-row-${role.roleId}`}
                  >
                    <button
                      class="mesh-builder-pressable flex min-w-0 flex-1 items-center gap-2.5 overflow-hidden text-left"
                      type="button"
                      onclick={() => assignRole(role)}
                      data-testid={`mesh-builder-pinned-chip-${role.roleId}`}
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
                        class="mesh-builder-row-control inline-flex h-7 w-7 items-center justify-center rounded-full border transition {ghostTone}"
                        type="button"
                        aria-label={`View details for ${role.name}`}
                        onclick={() => openRoleDetail(role)}
                        data-testid={`mesh-builder-pinned-info-${role.roleId}`}
                      >
                        i
                      </button>
                      <button
                        class="mesh-builder-row-control inline-flex h-7 w-7 items-center justify-center rounded-full border transition {pinButtonTone(isRolePinned(role.roleId))} {isPinBouncing(role.roleId) ? 'mesh-builder-pin-bounce' : ''}"
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
                        class="mesh-builder-row-control mesh-builder-add-button inline-flex h-7 w-7 items-center justify-center rounded-full border transition active:scale-[0.98] {ghostTone} {isRoleFlashing(role.roleId) ? 'mesh-builder-add-button-added' : ''}"
                        type="button"
                        aria-label={`Add ${role.name}`}
                        onclick={() => assignRole(role)}
                        data-testid={`mesh-builder-pinned-add-${role.roleId}`}
                      >
                        {isRoleFlashing(role.roleId) ? '✓' : '+'}
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            </section>
          {/if}

          <div
            class="min-h-0 flex-1 space-y-4 md:overflow-y-auto md:overscroll-contain md:pr-1"
            data-testid="mesh-builder-role-scroll"
          >
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
                  <div
                    class="mesh-builder-role-row group flex h-10 cursor-pointer items-center gap-2 rounded-[16px] border px-2.5 transition {surfaceTone} {roleCardTone(role)} {isRoleFlashing(role.roleId) ? 'mesh-builder-role-row-active' : ''}"
                    data-testid={`mesh-builder-role-row-${role.roleId}`}
                  >
                    <button
                      class="mesh-builder-pressable flex min-w-0 flex-1 items-center gap-2.5 overflow-hidden text-left"
                      type="button"
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
                        class="mesh-builder-row-control inline-flex h-7 w-7 items-center justify-center rounded-full border transition {ghostTone}"
                        type="button"
                        aria-label={`View details for ${role.name}`}
                        onclick={() => openRoleDetail(role)}
                        data-testid={`mesh-builder-role-info-${role.roleId}`}
                      >
                        i
                      </button>
                      <button
                        class="mesh-builder-row-control inline-flex h-7 w-7 items-center justify-center rounded-full border transition {pinButtonTone(isRolePinned(role.roleId))} {isPinBouncing(role.roleId) ? 'mesh-builder-pin-bounce' : ''}"
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
                        class="mesh-builder-row-control mesh-builder-add-button inline-flex h-7 w-7 items-center justify-center rounded-full border transition active:scale-[0.98] {ghostTone} {isRoleFlashing(role.roleId) ? 'mesh-builder-add-button-added' : ''}"
                        type="button"
                        aria-label={`Add ${role.name}`}
                        onclick={() => assignRole(role)}
                        data-testid={`mesh-builder-add-${role.roleId}`}
                      >
                        {isRoleFlashing(role.roleId) ? '✓' : '+'}
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
                  <div
                    class="mesh-builder-role-row group flex h-10 cursor-pointer items-center gap-2 rounded-[16px] border px-2.5 transition {surfaceTone} {roleCardTone(role)} {isRoleFlashing(role.roleId) ? 'mesh-builder-role-row-active' : ''}"
                    data-testid={`mesh-builder-role-row-${role.roleId}`}
                  >
                    <button
                      class="mesh-builder-pressable flex min-w-0 flex-1 items-center gap-2.5 overflow-hidden text-left"
                      type="button"
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
                        class="mesh-builder-row-control inline-flex h-7 w-7 items-center justify-center rounded-full border transition {ghostTone}"
                        type="button"
                        aria-label={`View details for ${role.name}`}
                        onclick={() => openRoleDetail(role)}
                        data-testid={`mesh-builder-role-info-${role.roleId}`}
                      >
                        i
                      </button>
                      <button
                        class="mesh-builder-row-control inline-flex h-7 w-7 items-center justify-center rounded-full border transition {pinButtonTone(isRolePinned(role.roleId))} {isPinBouncing(role.roleId) ? 'mesh-builder-pin-bounce' : ''}"
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
                        class="mesh-builder-row-control mesh-builder-add-button inline-flex h-7 w-7 items-center justify-center rounded-full border transition active:scale-[0.98] {ghostTone} {isRoleFlashing(role.roleId) ? 'mesh-builder-add-button-added' : ''}"
                        type="button"
                        aria-label={`Add ${role.name}`}
                        onclick={() => assignRole(role)}
                        data-testid={`mesh-builder-add-${role.roleId}`}
                      >
                        {isRoleFlashing(role.roleId) ? '✓' : '+'}
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
        class="flex min-h-0 flex-col gap-4 overflow-hidden p-4 md:pl-5 {highlightedRosterSection === 'all' ? rosterPanelHighlightTone : ''}"
        data-testid="mesh-builder-team-panel"
      >
        <div class="rounded-[22px] border px-4 py-4 {teamSummaryTone}" data-testid="mesh-builder-team-summary-card">
          <div class="space-y-3">
            <div class="min-w-0 space-y-3">
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
            </div>

            <p class="text-[12px] {t.textSecondary}" data-testid="mesh-builder-team-meta">
              {memberCount === 0
                ? 'Pick roles from the left to build your lineup.'
                : `${agents.length} agent${agents.length === 1 ? '' : 's'} supporting the lead.`}
            </p>
          </div>
        </div>

        <div
          class="min-h-0 flex-1 space-y-4 md:overflow-y-auto md:overscroll-contain md:pr-1"
          data-testid="mesh-builder-team-scroll"
        >
          <section class="space-y-2" data-testid="mesh-builder-team-lead-group">
            <div class="flex items-center justify-between gap-3">
              <p class="text-[11px] font-semibold uppercase tracking-[0.14em] {t.textMuted}">
                Lead
              </p>
              {#if normalizedTeam.lead}
                <span class="rounded-full border px-2 py-0.5 text-[10px] font-medium {presetBadgeTone}">
                  Assigned
                </span>
              {/if}
            </div>

            <section
              class="space-y-2 rounded-[22px] border p-2.5 transition {highlightedRosterSection === 'lead' || highlightedRosterSection === 'all' ? leadDropTone : rosterSectionTone}"
              data-testid="mesh-builder-lead-section"
            >
              {#if normalizedTeam.lead}
                <article
                  class="relative overflow-hidden rounded-[20px] border px-3 py-3 shadow-sm {teamCardTone(normalizedTeam.lead.tool, 'lead')} {isRosterMemberEntering(`lead:${normalizedTeam.lead.id}`) ? 'content-enter mesh-builder-roster-entry' : ''}"
                  data-testid="mesh-builder-lead-card"
                >
                  <span
                    class="absolute inset-y-0 left-0 w-1.5 {memberAccentTone(normalizedTeam.lead.tool, 'lead')}"
                    aria-hidden="true"
                  ></span>

                  <div class="flex items-start gap-3 pl-2" data-testid="mesh-builder-lead-summary">
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
                          <span class="rounded-full border px-2 py-0.5 text-[9px] font-medium {memberMetaTone(normalizedTeam.lead.tool, 'lead')}">
                            Team Lead
                          </span>
                        </span>
                        <span class="mt-1 block truncate text-[14px] font-semibold {t.textPrimary}">
                          {normalizedTeam.lead.roleName || normalizedTeam.lead.roleId || 'Lead'}
                        </span>
                        <span class="mt-0.5 flex flex-wrap items-center gap-1.5 text-[11px] {t.textSecondary}">
                          <span class="truncate">{normalizedTeam.lead.name || 'team-lead'}</span>
                          <span class="{t.textMuted}">•</span>
                          <span class="truncate">
                            {getToolName(normalizeTool(normalizedTeam.lead.tool))} · {normalizedTeam.lead.model || defaultModelForTool(normalizedTeam.lead.tool)}
                          </span>
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
                    <div class="mt-3 grid gap-2 border-t pt-3 {dark ? 'border-white/[0.08]' : 'border-zinc-200/80'} lg:grid-cols-[minmax(0,1fr)_minmax(0,0.9fr)_minmax(0,1fr)]">
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
                  <p class="text-[13px] font-medium {t.textPrimary}">Choose a lead role to anchor the team.</p>
                  <p class="mt-1 text-[11px] {t.textSecondary}">
                    Use the + button next to any lead on the left.
                  </p>
                </div>
              {/if}
            </section>
          </section>

          <section class="space-y-2" data-testid="mesh-builder-team-agents-group">
            <div class="flex items-center justify-between gap-3">
              <p class="text-[11px] font-semibold uppercase tracking-[0.14em] {t.textMuted}">
                Agents
              </p>
              <span class="rounded-full border px-2 py-0.5 text-[10px] font-medium {presetBadgeTone}">
                {agents.length} assigned
              </span>
            </div>

            <section
              class="space-y-2 rounded-[22px] border p-2.5 transition {highlightedRosterSection === 'agents' || highlightedRosterSection === 'all' ? leadDropTone : rosterSectionTone}"
              data-testid="mesh-builder-agents-section"
            >
              {#each agents as agent (agent.id)}
                <article
                  class="relative overflow-hidden rounded-[20px] border px-3 py-3 shadow-sm {teamCardTone(agent.tool)} {isRosterMemberEntering(`agent:${agent.id}`) ? 'content-enter mesh-builder-roster-entry' : ''} {isAgentRemoving(agent.id) ? 'mesh-builder-roster-exit' : ''}"
                  data-testid={`mesh-builder-agent-card-${agent.id}`}
                >
                  <span
                    class="absolute inset-y-0 left-0 w-1.5 {memberAccentTone(agent.tool)}"
                    aria-hidden="true"
                  ></span>

                  <div class="flex items-start gap-3 pl-2" data-testid={`mesh-builder-agent-summary-${agent.id}`}>
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
                          <span class="mt-0.5 flex flex-wrap items-center gap-1.5 text-[11px] {t.textSecondary}">
                            <span class="truncate">{agent.name}</span>
                            <span class="{t.textMuted}">•</span>
                            <span class="truncate">
                              {getToolName(normalizeTool(agent.tool))} · {agent.model || defaultModelForTool(agent.tool)}
                            </span>
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
                          aria-label={`Remove ${agent.name}`}
                          onclick={() => handleRemoveAgent(agent.id)}
                          data-testid={`mesh-builder-agent-remove-${agent.id}`}
                        >
                          ×
                        </button>
                      </div>
                    </div>
                  </div>

                  {#if isAgentExpanded(agent.id)}
                    <div class="mt-3 grid gap-2 border-t pt-3 {dark ? 'border-white/[0.08]' : 'border-zinc-200/80'} lg:grid-cols-[minmax(0,1fr)_minmax(0,0.9fr)_minmax(0,1fr)]">
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
                class="rounded-[18px] border border-dashed px-4 py-3 text-center {agentDropzoneTone}"
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
              onclick={handleInitializeClick}
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

  {#if selectedRoleDetail}
    <MeshNodeDetail
      node={roleDetailNode(selectedRoleDetail)}
      mode="builder"
      {dark}
      actions={{
        onAdd: handleRoleDetailAdd,
        onEdit: selectedRoleDetail.readOnly ? undefined : () => openRoleEditor(selectedRoleDetail),
        onExport: handleRoleDetailExport,
        exportDisabled: exportingRoleId === selectedRoleDetail.roleId,
        onDelete: selectedRoleDetail.readOnly ? undefined : requestRoleDelete,
        deleteDisabled: Boolean(selectedRoleDetail.readOnly),
        onClose: closeRoleDetail,
      }}
    />
  {/if}

  <MeshRoleEditorDialog
    open={roleEditorOpen}
    role={roleEditorRole}
    {dark}
    saving={roleEditorSaving}
    errorMessage={roleEditorError}
    onSave={handleRoleEditorSave}
    onCancel={closeRoleEditor}
  />

  <ConfirmDialog
    {dark}
    open={Boolean(deleteRoleContext)}
    title="Delete role?"
    message={deleteRoleContext
      ? `Delete '${deleteRoleContext.name ?? deleteRoleContext.roleId}' from the local catalog?`
      : ''}
    confirmLabel="Delete"
    variant="danger"
    onConfirm={confirmRoleDelete}
    onCancel={cancelRoleDelete}
  />

  <ConfirmDialog
    {dark}
    open={Boolean(importConflict)}
    title="Replace existing role?"
    message={importConflict
      ? `A role with id '${importConflict.rawRole?.roleId ?? ''}' already exists. Replace it with the imported YAML version?`
      : ''}
    confirmLabel="Replace"
    variant="danger"
    onConfirm={confirmImportConflictReplace}
    onCancel={cancelImportConflict}
  />
</section>

<style>
  .mesh-builder-pressable {
    transition:
      transform 140ms ease,
      box-shadow 180ms ease;
  }

  .mesh-builder-role-row {
    position: relative;
    isolation: isolate;
    transition:
      transform 160ms ease,
      box-shadow 180ms ease,
      border-color 180ms ease,
      background-color 180ms ease;
  }

  .mesh-builder-role-row:hover,
  .mesh-builder-role-row:focus-within {
    z-index: 2;
    transform: translateY(-1px);
    box-shadow:
      0 0 0 1px rgba(45, 212, 191, 0.14),
      0 10px 24px rgba(15, 118, 110, 0.12);
  }

  .mesh-builder-role-row-active {
    animation: mesh-builder-source-flash 400ms ease-out;
  }

  .mesh-builder-add-button {
    transition:
      transform 160ms ease,
      box-shadow 160ms ease,
      border-color 160ms ease,
      background-color 160ms ease;
  }

  .mesh-builder-row-control {
    transition:
      transform 160ms ease,
      box-shadow 160ms ease,
      border-color 160ms ease,
      background-color 160ms ease,
      color 160ms ease;
  }

  .mesh-builder-role-row:hover .mesh-builder-row-control,
  .mesh-builder-role-row:focus-within .mesh-builder-row-control {
    box-shadow: 0 6px 16px rgba(15, 118, 110, 0.08);
  }

  .mesh-builder-role-row:hover .mesh-builder-add-button,
  .mesh-builder-role-row:focus-within .mesh-builder-add-button {
    transform: scale(1.08);
    box-shadow: 0 8px 20px rgba(15, 118, 110, 0.16);
  }

  .mesh-builder-add-button-added {
    animation: mesh-builder-add-check 400ms ease-out;
  }

  .mesh-builder-roster-entry.content-enter {
    animation:
      content-enter 120ms ease-out,
      mesh-builder-roster-glow 600ms ease-out;
  }

  .mesh-builder-roster-exit {
    pointer-events: none;
    animation: mesh-builder-card-exit 120ms ease-in forwards;
  }

  .mesh-builder-pin-bounce {
    animation: mesh-builder-pin-bounce 200ms ease-out;
    transform-origin: center;
  }

  @keyframes mesh-builder-source-flash {
    0% {
      box-shadow:
        0 0 0 0 rgba(20, 184, 166, 0.28),
        inset 0 0 0 999px rgba(20, 184, 166, 0.14);
    }

    65% {
      box-shadow:
        0 0 0 10px rgba(20, 184, 166, 0),
        inset 0 0 0 999px rgba(20, 184, 166, 0.08);
    }

    100% {
      box-shadow:
        0 0 0 14px rgba(20, 184, 166, 0),
        inset 0 0 0 999px rgba(20, 184, 166, 0);
    }
  }

  @keyframes mesh-builder-add-check {
    0% {
      transform: scale(0.92);
    }

    45% {
      transform: scale(1.12);
    }

    100% {
      transform: scale(1);
    }
  }

  @keyframes mesh-builder-roster-glow {
    0% {
      box-shadow:
        0 0 0 0 rgba(20, 184, 166, 0.2),
        inset 0 0 0 1px rgba(20, 184, 166, 0.28);
    }

    100% {
      box-shadow:
        0 0 0 12px rgba(20, 184, 166, 0),
        inset 0 0 0 1px rgba(20, 184, 166, 0);
    }
  }

  @keyframes mesh-builder-card-exit {
    from {
      opacity: 1;
      transform: scale(1) translateY(0);
    }

    to {
      opacity: 0;
      transform: scale(0.96) translateY(-4px);
    }
  }

  @keyframes mesh-builder-pin-bounce {
    0% {
      transform: scale(1);
    }

    38% {
      transform: scale(1.18);
    }

    70% {
      transform: scale(0.94);
    }

    100% {
      transform: scale(1);
    }
  }
</style>
