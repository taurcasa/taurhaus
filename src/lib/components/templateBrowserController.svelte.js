import {
  deleteRoleTemplate,
  deleteTeamPreset,
  getRoleTemplate,
  getTeamPreset,
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
  normalizeTeamPreset,
  presetDraftToTeamConfig,
} from './templateBrowserUtils.js'

export function createTemplateBrowserController({ getOpen }) {
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
  let catalogLoadSequence = 0

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

    errorMessage = ''
    try {
      await upsertTeamPreset({
        schema: { kind: 'team_preset', version: 1 },
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
    setSearchQuery,
    setTab,
    resetDetail,
    resetRoleEditor,
    openCreateRoleEditor,
    openEditRoleEditor,
    handleRoleSave,
    requestRoleDelete,
    cancelRoleDelete,
    confirmRoleDelete,
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
