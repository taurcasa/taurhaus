import {
  deleteRoleTemplate,
  deleteTeamPreset,
  exportAgentDefinitions,
  exportRoleToFile,
  getRoleTemplate,
  getTeamPreset,
  importRoleFromFile,
  isTauri,
  listRoleTemplates,
  listTeamPresets,
  upsertRoleTemplate,
  upsertTeamPreset,
} from '../ipc.js'
import {
  defaultAgentRoleId,
  defaultLeadRoleId,
  ensureUniquePresetId,
  normalizePresetDraft,
  normalizeRoleTemplate,
  normalizeSlotOverridesForDraft,
  normalizeTeamPreset,
  presetDraftToTeamConfig,
} from './templateBrowserUtils.js'

function importSourceLabel(role) {
  switch (String(role?.provenance?.sourceFormat ?? '').toLowerCase()) {
    case 'claude_agent':
      return 'Claude Code'
    case 'copilot_agent':
      return 'Copilot'
    case 'agents_md':
      return 'AGENTS.md'
    case 'gemini_md':
      return 'GEMINI.md'
    default:
      return 'external source'
  }
}

/**
 * What a saved row pins for model and effort. An override is user intent, never a
 * comparison against the role defaults: a field the user changed in this editing
 * session is written as the editor showed it (clearing the effort removes the pin),
 * and a field nobody touched keeps exactly what was loaded - including a pin that
 * happens to equal the role default today.
 */
function modelOverridesFor(row, loadedOverrides) {
  const touched = row?.touched ?? {}
  const model = String(row?.model ?? '').trim()
  const reasoningEffort = String(row?.reasoningEffort ?? '').trim()

  return {
    model: touched.model ? model || null : (loadedOverrides?.model ?? null),
    reasoningEffort: touched.reasoningEffort
      ? reasoningEffort || null
      : (loadedOverrides?.reasoningEffort ?? null),
  }
}

/**
 * Turns the customizer roster back into preset slots. Each row keeps the other
 * override fields of the slot it came from and contributes its own model/effort;
 * consecutive rows that agree on role and overrides collapse back into one slot,
 * and a row that diverges splits the slot instead of losing the difference.
 */
function agentSlotsFromCustomizer(agents, draft, fallbackRoleId) {
  const slots = []

  for (const agent of agents) {
    const roleId = String(agent?.roleId ?? '').trim() || fallbackRoleId
    if (!roleId) continue

    const baseSlot = Number.isInteger(agent?.slotIndex) ? draft.agentSlots[agent.slotIndex] : null
    const overrides = normalizeSlotOverridesForDraft({
      ...(baseSlot?.overrides ?? {}),
      ...modelOverridesFor(agent, baseSlot?.overrides),
    })

    const previous = slots[slots.length - 1]
    const projectBinding = baseSlot?.projectBinding ?? 'lead_project'
    const projectId = baseSlot?.projectId ?? null
    if (
      previous &&
      previous.roleId === roleId &&
      previous.projectBinding === projectBinding &&
      previous.projectId === projectId &&
      JSON.stringify(previous.overrides) === JSON.stringify(overrides)
    ) {
      previous.count += 1
      continue
    }

    slots.push({ roleId, count: 1, projectBinding, projectId, overrides })
  }

  return slots
}

function formatUiError(error, fallback) {
  if (error instanceof Error && error.message) {
    return error.message
  }

  const message = String(error ?? '').trim()
  return message || fallback
}

async function pickRoleImportFile() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selection = await open({
    multiple: false,
    filters: [{ name: 'Markdown', extensions: ['md'] }],
  })

  if (Array.isArray(selection)) {
    return typeof selection[0] === 'string' ? selection[0] : null
  }

  return typeof selection === 'string' ? selection : null
}

export function createTemplateBrowserController({ getOpen, getProjectId = () => '' }) {
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
  let importConflict = $state(null)
  let presetEditorOpen = $state(false)
  let presetEditorMode = $state('create')
  let presetEditorDraft = $state(null)
  let presetEditorTeamConfig = $state(null)
  let deletePresetId = $state('')
  let deletePresetName = $state('')
  let exportingRoleId = $state('')
  let exportingAgentDefinitions = $state(false)
  let exportNotice = $state('')
  let catalogLoadSequence = 0
  let exportNoticeTimer = null

  const filteredRoleTemplates = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase()
    return roleTemplates.filter((role) => {
      if (!query) return true
      return (
        String(role.name ?? '').toLowerCase().includes(query)
        || String(role.roleId ?? '').toLowerCase().includes(query)
        || String(role.model ?? '').toLowerCase().includes(query)
      )
    })
  })

  const hasCustomRoles = $derived.by(() => roleTemplates.some((role) => !role.builtIn))

  const filteredTeamPresets = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase()
    return teamPresets.filter((preset) => {
      if (!query) return true
      return (
        String(preset.name ?? '').toLowerCase().includes(query)
        || String(preset.presetId ?? '').toLowerCase().includes(query)
        || String(preset.description ?? '').toLowerCase().includes(query)
      )
    })
  })

  const buildPresetDraft = (source = {}) => normalizePresetDraft(source, roleTemplates, teamPresets)
  const buildTeamConfig = (draft) => presetDraftToTeamConfig(draft, roleTemplates)

  function exportFilenameForRole(role) {
    const base = String(role?.name ?? role?.roleId ?? 'role')
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'role'
    return `${base}.md`
  }

  function showExportNotice(message) {
    exportNotice = message
    if (exportNoticeTimer) {
      clearTimeout(exportNoticeTimer)
    }
    exportNoticeTimer = setTimeout(() => {
      exportNotice = ''
      exportNoticeTimer = null
    }, 2500)
  }

  async function saveExportedRoleFile(filename, fileContent) {
    if (isTauri()) {
      const [{ save }, { writeTextFile }] = await Promise.all([
        import('@tauri-apps/plugin-dialog'),
        import('@tauri-apps/plugin-fs'),
      ])

      const path = await save({
        defaultPath: filename,
        filters: [{ name: 'Markdown', extensions: ['md'] }],
      })

      if (!path) return false
      await writeTextFile(path, fileContent)
      return true
    }

    const blob = new Blob([fileContent], { type: 'text/markdown;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = filename
    link.click()
    URL.revokeObjectURL(url)
    return true
  }

  function setSearchQuery(value) {
    searchQuery = value
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

  async function fetchRoles() {
    const roles = await listRoleTemplates()
    return (roles ?? []).map(normalizeRoleTemplate)
  }

  async function fetchPresets() {
    const presets = await listTeamPresets()
    return (presets ?? []).map(normalizeTeamPreset)
  }

  async function refreshRoles() {
    roleTemplates = await fetchRoles()
  }

  async function refreshPresets() {
    teamPresets = await fetchPresets()
  }

  async function loadCatalog() {
    const sequence = ++catalogLoadSequence
    loading = true
    errorMessage = ''
    try {
      const [roles, presets] = await Promise.all([fetchRoles(), fetchPresets()])
      if (sequence !== catalogLoadSequence) return
      roleTemplates = roles
      teamPresets = presets
    } catch (error) {
      if (sequence !== catalogLoadSequence) return
      roleTemplates = []
      teamPresets = []
      errorMessage = error?.message || 'Failed to load template catalog.'
    } finally {
      if (sequence === catalogLoadSequence) {
        loading = false
      }
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
      roleEditorRole = { ...merged, tool: merged.cliTool }
    } catch {
      roleEditorRole = { ...role, tool: role.cliTool }
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

  function clearImportConflict() {
    importConflict = null
  }

  async function importRole() {
    errorMessage = ''

    let filePath
    try {
      filePath = await pickRoleImportFile()
    } catch (error) {
      errorMessage = formatUiError(error, 'Failed to open the role import dialog.')
      return
    }

    if (!filePath) return

    try {
      const result = await importRoleFromFile(filePath)
      const importedRole = normalizeRoleTemplate(result?.role ?? {})

      if (result?.conflict) {
        importConflict = {
          rawRole: result.role,
          importedRole,
          existingRole: normalizeRoleTemplate(result.conflict),
        }
        return
      }

      clearImportConflict()
      await refreshRoles()
      showExportNotice(`Imported '${importedRole.name}' from ${importSourceLabel(importedRole)}`)
    } catch (error) {
      errorMessage = formatUiError(error, 'Failed to import role template.')
    }
  }

  function skipImportConflict() {
    clearImportConflict()
  }

  async function replaceImportedRole() {
    if (!importConflict?.rawRole) return

    const pendingImport = importConflict
    clearImportConflict()
    errorMessage = ''
    try {
      await upsertRoleTemplate(pendingImport.rawRole)
      await refreshRoles()
      showExportNotice(
        `Imported '${pendingImport.importedRole.name}' from ${importSourceLabel(pendingImport.importedRole)}`
      )
    } catch (error) {
      errorMessage = formatUiError(error, 'Failed to replace the existing role template.')
    }
  }

  async function handleRoleExport(role, targetFormat) {
    if (!role?.roleId || !targetFormat) return

    exportingRoleId = role.roleId
    errorMessage = ''

    try {
      const exported = await exportRoleToFile(role.roleId, targetFormat)
      const saved = await saveExportedRoleFile(
        exportFilenameForRole(role),
        exported?.fileContent ?? ''
      )
      if (!saved) return

      const approximatedFieldCount = Array.isArray(exported?.lossyFields)
        ? exported.lossyFields.length
        : 0
      if (approximatedFieldCount > 0) {
        showExportNotice(`Exported (${approximatedFieldCount} fields approximated)`)
      }
    } catch (error) {
      errorMessage = error?.message || 'Failed to export role.'
    } finally {
      exportingRoleId = ''
    }
  }

  async function exportAgentDefinitionsForProject() {
    const projectId = getProjectId()
    if (!projectId || exportingAgentDefinitions) return

    exportingAgentDefinitions = true
    errorMessage = ''

    try {
      const result = await exportAgentDefinitions(projectId)
      const written = result?.written?.length ?? 0
      // Definitions of roles that left the catalog, deleted so Claude Code
      // stops resolving them.
      const removed = result?.removed?.length ?? 0
      const skipped = result?.skipped ?? []
      // Two different reasons to skip: a file someone wrote by hand, and a role
      // id Claude Code would never register as an agent name.
      const handWritten = skipped.filter((entry) => entry?.reason === 'user_authored').length
      const unusableId = skipped.length - handWritten
      const parts = [
        `Exported ${written} agent ${written === 1 ? 'definition' : 'definitions'} to .claude/agents`,
      ]
      if (removed > 0) {
        parts.push(`removed ${removed} obsolete`)
      }
      if (handWritten > 0) {
        parts.push(
          `${handWritten} hand-written ${handWritten === 1 ? 'agent' : 'agents'} left untouched`
        )
      }
      if (unusableId > 0) {
        parts.push(
          `${unusableId} ${unusableId === 1 ? 'role' : 'roles'} skipped for an id Claude cannot register`
        )
      }
      showExportNotice(parts.join(' \u00b7 '))
    } catch (error) {
      errorMessage = formatUiError(error, 'Failed to export the agent definitions.')
    } finally {
      exportingAgentDefinitions = false
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
    const draft = buildPresetDraft({
      presetId: ensureUniquePresetId('custom-preset', teamPresets),
      name: 'New Preset',
      description: 'Custom team preset',
      leadRoleId: defaultLeadRoleId(roleTemplates),
      agentSlots: [{
        roleId: defaultAgentRoleId(roleTemplates),
        count: 1,
        projectBinding: 'lead_project',
        projectId: null,
      }],
    })
    presetEditorMode = 'create'
    presetEditorDraft = draft
    presetEditorTeamConfig = buildTeamConfig(draft)
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

    const merged = buildPresetDraft({ ...preset, ...(detail ?? {}) })
    if (mode === 'duplicate') {
      merged.name = `Copy of ${merged.name}`
      merged.presetId = ensureUniquePresetId(`copy-of-${merged.presetId || merged.name}`, teamPresets)
    }
    if (mode === 'create') {
      merged.presetId = ensureUniquePresetId(merged.presetId || merged.name, teamPresets)
    }

    presetEditorMode = mode
    presetEditorDraft = merged
    presetEditorTeamConfig = buildTeamConfig(merged)
    presetEditorOpen = true
  }

  async function savePresetFromCustomizer(payload) {
    if (!presetEditorDraft) return
    const name = String(payload?.teamName ?? presetEditorDraft.name ?? '').trim() || 'New Preset'
    const description = String(payload?.description ?? presetEditorDraft.description ?? '').trim() || 'Custom team preset'
    const desiredId = presetEditorMode === 'edit'
      ? (presetEditorDraft.presetId || ensureUniquePresetId(name, teamPresets))
      : ensureUniquePresetId(name, teamPresets)
    const draft = buildPresetDraft({ ...presetEditorDraft, presetId: desiredId, name, description })
    const leadRoleId = String(payload?.lead?.roleId ?? '').trim() || draft.leadRoleId
    // The lead card is editable in this editor, so its model/effort is read the
    // same way an agent row's is and stored as the preset's lead pin.
    const leadOverrides = normalizeSlotOverridesForDraft({
      ...(draft.leadOverrides ?? {}),
      ...modelOverridesFor(payload?.lead, draft.leadOverrides),
    })
    const editedSlots = agentSlotsFromCustomizer(
      Array.isArray(payload?.agents) ? payload.agents : [],
      draft,
      defaultAgentRoleId(roleTemplates, leadRoleId)
    )

    errorMessage = ''
    try {
      await upsertTeamPreset({
        schema: { kind: 'team_preset', version: 1 },
        presetId: draft.presetId,
        name: draft.name,
        description: draft.description,
        version: draft.version,
        leadRoleId,
        leadOverrides,
        agentSlots: editedSlots.length > 0 ? editedSlots : draft.agentSlots,
        defaults: draft.defaults,
      })
      closePresetEditor()
      await refreshPresets()
    } catch (error) {
      errorMessage = error?.message || 'Failed to save team preset.'
    }
  }

  function requestPresetDelete(preset) {
    if (!preset?.presetId || (preset?.builtIn || preset?.readOnly)) return
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
      selectedRole = normalizeRoleTemplate({ ...role, ...detail })
      selectedPreset = null
      historyTemplateId = role.roleId
      historyTemplateKind = 'role'
    } catch (error) {
      selectedRole = normalizeRoleTemplate({ ...role })
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

  $effect(() => {
    if (!getOpen()) {
      catalogLoadSequence += 1
      loading = false
      return
    }
    void loadCatalog()
  })

  return {
    get loading() {
      return loading
    },
    get errorMessage() {
      return errorMessage
    },
    get filteredRoleTemplates() {
      return filteredRoleTemplates
    },
    get hasCustomRoles() {
      return hasCustomRoles
    },
    get filteredTeamPresets() {
      return filteredTeamPresets
    },
    get activeTab() {
      return activeTab
    },
    get detailKind() {
      return detailKind
    },
    get detailLoading() {
      return detailLoading
    },
    get selectedRole() {
      return selectedRole
    },
    get selectedPreset() {
      return selectedPreset
    },
    get historyTemplateId() {
      return historyTemplateId
    },
    get historyTemplateKind() {
      return historyTemplateKind
    },
    get roleEditorOpen() {
      return roleEditorOpen
    },
    get roleEditorRole() {
      return roleEditorRole
    },
    get roleTemplates() {
      return roleTemplates
    },
    get importConflict() {
      return importConflict
    },
    get presetEditorOpen() {
      return presetEditorOpen
    },
    get presetEditorTeamConfig() {
      return presetEditorTeamConfig
    },
    get deleteRoleId() {
      return deleteRoleId
    },
    get deleteRoleName() {
      return deleteRoleName
    },
    get deletePresetId() {
      return deletePresetId
    },
    get deletePresetName() {
      return deletePresetName
    },
    get exportingRoleId() {
      return exportingRoleId
    },
    get exportingAgentDefinitions() {
      return exportingAgentDefinitions
    },
    get canExportAgentDefinitions() {
      return Boolean(getProjectId())
    },
    get exportNotice() {
      return exportNotice
    },
    setSearchQuery,
    setTab,
    resetDetail,
    resetRoleEditor,
    clearImportConflict,
    openCreateRoleEditor,
    openEditRoleEditor,
    handleRoleSave,
    importRole,
    skipImportConflict,
    replaceImportedRole,
    requestRoleDelete,
    cancelRoleDelete,
    confirmRoleDelete,
    handleRoleExport,
    exportAgentDefinitionsForProject,
    closePresetEditor,
    openCreatePresetEditor,
    openPresetEditorForMutation,
    savePresetFromCustomizer,
    requestPresetDelete,
    cancelPresetDelete,
    confirmPresetDelete,
    inspectRole,
    inspectPreset,
  }
}
