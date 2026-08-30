import { buildMemberActionMessage } from './meshTabRuntime.svelte.js'

export function createMeshTabSetup({ state, refs, deps, gate }) {
  // A saved preset has to remember what the roster actually selected; without
  // overrides, reloading it restores the role defaults and drops the choice.
  function slotOverridesForMember(member) {
    const model = String(member?.model ?? '').trim()
    const reasoningEffort = String(member?.reasoningEffort ?? '').trim()
    if (!model && !reasoningEffort) return null
    return { model: model || null, reasoningEffort: reasoningEffort || null }
  }

  function detachPresetConfig(config) {
    if (!config || config.initializationMode !== 'preset') return config
    return {
      ...config,
      presetId: '',
      presetName: '',
      initializationMode: 'custom',
      composition: null,
    }
  }

  function closeSlideOver() {
    state.slideOver = null
    state.slideOverContext = null
  }

  function ensureBuilderConfig() {
    if (state.teamConfig) return state.teamConfig
    state.teamConfig = deps.emptyBuilderConfig()
    return state.teamConfig
  }

  function resolveBuilderRole(roleId) {
    return state.roleTemplates.find((entry) => entry.roleId === roleId) ?? null
  }

  async function handlePresetSelect(preset) {
    gate.invalidateDiscovery()
    const sequence = ++refs.presetSelectionSequence
    const presetId = preset?.presetId ?? ''
    let resolvedPreset = preset
    let compositionResult = null

    try {
      const hydratedPreset = presetId ? await deps.getTeamPreset(presetId) : null
      if (sequence !== refs.presetSelectionSequence) return
      if (hydratedPreset && typeof hydratedPreset === 'object') {
        resolvedPreset = { ...preset, ...hydratedPreset }
      }

      const leadRoleId = resolvedPreset?.leadRoleId ?? ''
      const agentSlots = Array.isArray(resolvedPreset?.agentSlots) ? resolvedPreset.agentSlots : []

      if (leadRoleId) {
        // A preset can pin its lead's model/effort (`TeamPreset::lead_overrides`);
        // composition only applies it when the request carries it as the lead
        // override, otherwise the launched lead falls back to the role defaults.
        const leadOverrides = resolvedPreset?.leadOverrides ?? resolvedPreset?.lead_overrides ?? null
        compositionResult = await deps.composeTeam({
          leadRoleId,
          agentSlots,
          overrides: {
            ...(leadOverrides ? { lead: leadOverrides } : {}),
            projectName: deps.inferTeamName(deps.getProjectPath()).replace(/-team$/, ''),
          },
        })
      }
    } catch (error) {
      console.error('Failed to hydrate quick preset details:', error)
    }

    if (sequence !== refs.presetSelectionSequence) return
    state.teamConfig = deps.buildTeamConfigFromPreset(
      resolvedPreset,
      compositionResult,
      deps.getProjectPath()
    )
    state.teamName = deps.inferTeamName(deps.getProjectPath())
    state.selectedNodeId = null
    state.mode = 'setup'
    closeSlideOver()
    state.runtimeMessage = ''
  }

  function handleTeamNameChange(value) {
    if (state.mode === 'empty') gate.invalidateDiscovery()
    state.teamName = String(value ?? '')
    if (state.mode === 'empty') state.mode = 'setup'
  }

  function handleTeamDescriptionChange(value) {
    if (state.mode === 'empty') gate.invalidateDiscovery()
    const next = ensureBuilderConfig()
    state.teamConfig = {
      ...next,
      description: String(value ?? ''),
    }
    if (state.mode === 'empty') state.mode = 'setup'
  }

  function handleAssignLeadRole(roleId) {
    if (state.mode === 'empty') gate.invalidateDiscovery()
    const role = resolveBuilderRole(roleId)
    if (!role || deps.normalizeRoleKind(role) !== 'lead') return
    const projectPath = deps.getProjectPath()
    const next = detachPresetConfig(ensureBuilderConfig())
    state.teamConfig = {
      ...next,
      lead: deps.createLeadFromRole(role, projectPath),
    }
    if (!state.teamName.trim()) state.teamName = deps.inferTeamName(projectPath)
    state.mode = 'setup'
    state.runtimeMessage = ''
  }

  function handleClearLead() {
    const next = detachPresetConfig(ensureBuilderConfig())
    state.teamConfig = {
      ...next,
      lead: null,
    }
  }

  function handleAppendAgentRole(roleId) {
    if (state.mode === 'empty') gate.invalidateDiscovery()
    const role = resolveBuilderRole(roleId)
    if (!role || deps.normalizeRoleKind(role) === 'lead') return
    const projectPath = deps.getProjectPath()
    const next = detachPresetConfig(ensureBuilderConfig())
    state.teamConfig = {
      ...next,
      agents: [...(next.agents ?? []), deps.createAgentFromRole(role, projectPath, next.agents ?? [])],
    }
    if (!state.teamName.trim()) state.teamName = deps.inferTeamName(projectPath)
    state.mode = 'setup'
  }

  function handleUpdateLead(payload) {
    const next = detachPresetConfig(ensureBuilderConfig())
    if (!next.lead) return
    state.teamConfig = {
      ...next,
      lead: {
        ...next.lead,
        ...payload,
      },
    }
  }

  function handleUpdateAgent(agentId, payload) {
    const next = detachPresetConfig(ensureBuilderConfig())
    state.teamConfig = {
      ...next,
      agents: (next.agents ?? []).map((agent) => (
        agent.id === agentId
          ? { ...agent, ...payload }
          : agent
      )),
    }
  }

  function handleRemoveBuilderAgent(agentId) {
    const next = detachPresetConfig(ensureBuilderConfig())
    state.teamConfig = {
      ...next,
      agents: (next.agents ?? []).filter((agent) => agent.id !== agentId),
    }
    if (state.selectedNodeId === agentId) state.selectedNodeId = null
  }

  function handleReorderBuilderAgent(sourceId, targetId) {
    if (!sourceId || !targetId || sourceId === targetId) return
    const next = detachPresetConfig(ensureBuilderConfig())
    const currentAgents = [...(next.agents ?? [])]
    const sourceIndex = currentAgents.findIndex((agent) => agent.id === sourceId)
    const targetIndex = currentAgents.findIndex((agent) => agent.id === targetId)
    if (sourceIndex < 0 || targetIndex < 0) return
    const [moved] = currentAgents.splice(sourceIndex, 1)
    currentAgents.splice(targetIndex, 0, moved)
    state.teamConfig = {
      ...next,
      agents: currentAgents,
    }
  }

  function handleMoveBuilderAgentToEnd(sourceId) {
    if (!sourceId) return
    const next = detachPresetConfig(ensureBuilderConfig())
    const currentAgents = [...(next.agents ?? [])]
    const sourceIndex = currentAgents.findIndex((agent) => agent.id === sourceId)
    if (sourceIndex < 0 || sourceIndex === currentAgents.length - 1) return
    const [moved] = currentAgents.splice(sourceIndex, 1)
    currentAgents.push(moved)
    state.teamConfig = {
      ...next,
      agents: currentAgents,
    }
  }

  async function handleSaveBuilderPreset() {
    const next = state.teamConfig
    const safeTeamName = String(state.teamName ?? '').trim()
    if (!next?.lead?.roleId || !safeTeamName) return

    try {
      await deps.upsertTeamPreset({
        schema: {
          kind: 'team_preset',
          version: 1,
        },
        presetId: safeTeamName
          .toLowerCase()
          .replace(/[^a-z0-9\s_-]+/g, '')
          .replace(/[\s_]+/g, '-')
          .replace(/-+/g, '-')
          .replace(/^-|-$/g, '') || 'custom-preset',
        name: safeTeamName,
        description: String(next.description ?? '').trim() || 'Custom team preset',
        version: '1.0.0',
        leadRoleId: next.lead.roleId,
        agentSlots: (next.agents ?? []).map((agent) => ({
          roleId: agent.roleId || null,
          count: 1,
          projectBinding: 'lead_project',
          projectId: null,
          overrides: slotOverridesForMember(agent),
        })),
        defaults: {
          teamNamePattern: '{project}-team',
          tmuxLayout: 'tiled',
        },
      })
      state.runtimeMessage = 'Preset saved to catalog.'
      state.errorMessage = ''
      state.presetsLoaded = false
      void loadTeamPresets()
    } catch (error) {
      state.errorMessage = error?.message || 'Failed to save preset.'
    }
  }

  function openAddAgentPanel() {
    if (state.isResumingTeam) return
    const projectPath = deps.getProjectPath()
    const projectOptions = (deps.getAvailableProjects() ?? [])
      .map((project) => deps.normalizeProjectOption(project, {
        stringLabel: 'raw',
        objectFallbackLabel: 'raw',
      }))
      .filter((project) => project.id)
    const defaultProject = projectOptions[0]?.id || projectPath || ''

    state.roleTemplates = []
    state.slideOver = 'addAgent'
    state.slideOverContext = {
      roleId: '',
      roleName: '',
      name: '',
      tool: 'codex',
      model: deps.defaultModelFor('codex'),
      reasoningEffort: deps.defaultEffortFor('codex', deps.defaultModelFor('codex')),
      projectId: defaultProject,
      description: '',
      instructions: '',
      focusArea: '',
      contextSummary: '',
      behaviorSummary: '',
      submitting: false,
      error: '',
      isLocked: false,
    }
    void loadRoleTemplates()
  }

  async function loadRoleTemplates() {
    state.loadingRoles = true
    try {
      const loaded = await deps.listRoleTemplates()
      state.roleTemplates = Array.isArray(loaded) ? loaded.filter(Boolean) : []
    } catch (error) {
      console.error('Failed to load role templates:', error)
    } finally {
      state.loadingRoles = false
    }
  }

  async function loadTeamPresets() {
    state.loadingPresets = true
    try {
      const fetchedPresets = await deps.listTeamPresets()
      state.availablePresets = deps.mergePresetCatalog(
        deps.quickPresets,
        Array.isArray(fetchedPresets) ? fetchedPresets : []
      )
    } catch (error) {
      console.error('Failed to load team presets:', error)
      state.availablePresets = deps.quickPresets
    } finally {
      state.presetsLoaded = true
      state.loadingPresets = false
    }
  }

  function handleRoleChange(selectedRoleId) {
    const draft = state.addAgentDraft
    if (!draft) return
    if (!selectedRoleId) {
      state.slideOverContext = {
        ...draft,
        roleId: '',
        roleName: '',
        focusArea: '',
        contextSummary: '',
        behaviorSummary: '',
        instructions: '',
        isLocked: false,
      }
      return
    }

    const role = state.roleTemplates.find((entry) => entry.roleId === selectedRoleId)
    if (!role) return
    const tool = deps.normalizeTool(role.cliTool || 'codex')
    // The role response keeps the effort under `defaults` unless it was lifted;
    // read both. A role that names a model but no effort inherits the CLI's
    // global setting, so only a catalog-supplied model brings a catalog effort.
    const roleModel = deps.resolveRoleModel(role)
    const model = roleModel || deps.defaultModelFor(tool)
    const reasoningEffort =
      deps.resolveRoleReasoningEffort(role)
      ?? (roleModel ? null : deps.defaultEffortFor(tool, model))
    const instructions = role.instructions || ''
    state.slideOverContext = {
      ...draft,
      roleId: selectedRoleId,
      roleName: role.name || '',
      name: deps.buildRuntimeAgentName(role, draft.projectId, state.teamConfig, deps.getProjectPath()),
      tool,
      model,
      reasoningEffort,
      description: instructions,
      instructions,
      focusArea: role.focusArea || '',
      contextSummary: role.contextSummary || '',
      behaviorSummary: role.behaviorSummary || '',
      isLocked: true,
    }
  }

  function toggleAddAgentLock() {
    const draft = state.addAgentDraft
    if (!draft) return
    state.slideOverContext = { ...draft, isLocked: !draft.isLocked }
  }

  function updateAddAgentField(field, value) {
    const draft = state.addAgentDraft
    if (!draft) return
    const next = { ...draft, [field]: value }
    if (field === 'tool') {
      next.model = deps.defaultModelFor(value)
      next.reasoningEffort = deps.defaultEffortFor(value, next.model)
    }
    state.slideOverContext = next
  }

  async function submitAddAgent() {
    if (state.isResumingTeam) return
    const draft = state.addAgentDraft
    if (!draft || !state.canSubmitAddAgent) return
    state.slideOverContext = { ...draft, submitting: true, error: '' }

    try {
      const report = await deps.coordinationAddAgent({
        teamName: state.teamName,
        agent: {
          name: String(draft.name || '').trim(),
          cliTool: deps.normalizeTool(draft.tool),
          model: String(draft.model || '').trim(),
          reasoningEffort: draft.reasoningEffort ?? null,
          projectId: String(draft.projectId || '').trim(),
          description: String(draft.description || '').trim() || null,
          roleId: String(draft.roleId || '').trim() || null,
          roleName: String(draft.roleName || '').trim() || null,
          focusArea: String(draft.focusArea || '').trim() || null,
          contextSummary: String(draft.contextSummary || '').trim() || null,
          behaviorSummary: String(draft.behaviorSummary || '').trim() || null,
          instructions: String(draft.instructions || '').trim() || null,
        },
      })

      deps.onAddAgent(report)
      state.runtimeMessage = buildMemberActionMessage(
        `Agent '${report?.memberName ?? String(draft.name || '').trim()}' added.`,
        report?.warnings
      )
      closeSlideOver()
      const sequence = ++refs.discoverySequence
      await gate.refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
    } catch (error) {
      const latest = state.addAgentDraft
      if (!latest) return
      state.slideOverContext = {
        ...latest,
        submitting: false,
        error: error?.message || 'Failed to add agent.',
      }
    }
  }

  function openCaptureRoleDialog() {
    if (state.isResumingTeam) return
    if (!state.selectedNode || state.mode !== 'runtime') return
    const roleName = String(state.selectedNode.name || '').trim() || 'captured-role'
    const normalizedContract = deps.normalizeBehavioralContract(state.selectedNode.behavioralContract)
    const description = String(state.selectedNode.description || '').trim()

    state.captureRoleDialog = {
      roleKind: state.selectedNode.role === 'lead' ? 'lead' : 'agent',
      name: roleName,
      roleId: deps.slugifyRoleId(roleName),
      manualRoleId: false,
      tool: deps.normalizeTool(state.selectedNode.tool),
      model: String(state.selectedNode.model || '').trim(),
      reasoningEffort: state.selectedNode.reasoningEffort ?? null,
      description,
      includeInstructions: description.length > 0,
      includeBehavioralContract: deps.contractHasRules(normalizedContract),
      behavioralContract: normalizedContract,
      capabilities: Array.isArray(state.selectedNode.capabilities) ? state.selectedNode.capabilities : [],
      submitting: false,
      error: '',
    }
  }

  function closeCaptureRoleDialog() {
    state.captureRoleDialog = null
  }

  function updateCaptureRoleName(value) {
    const draft = state.captureRoleDraft
    if (!draft) return
    const name = String(value || '')
    state.captureRoleDialog = {
      ...draft,
      name,
      roleId: draft.manualRoleId ? draft.roleId : deps.slugifyRoleId(name),
    }
  }

  function updateCaptureRoleId(value) {
    const draft = state.captureRoleDraft
    if (!draft) return
    state.captureRoleDialog = { ...draft, roleId: String(value || ''), manualRoleId: true }
  }

  function toggleCaptureRoleFlag(field) {
    const draft = state.captureRoleDraft
    if (!draft) return
    state.captureRoleDialog = { ...draft, [field]: !draft[field] }
  }

  async function submitCaptureRole() {
    const draft = state.captureRoleDraft
    if (!draft || !state.canSaveCapturedRole) return

    state.captureRoleDialog = { ...draft, submitting: true, error: '' }
    try {
      await deps.upsertRoleTemplate(deps.buildCapturedRoleTemplate(draft, deps.getModelCatalog()))
      state.runtimeMessage = 'Role saved to catalog'
      closeCaptureRoleDialog()
      void loadRoleTemplates()
    } catch (error) {
      const latest = state.captureRoleDraft
      if (!latest) return
      state.captureRoleDialog = {
        ...latest,
        submitting: false,
        error: error?.message || 'Failed to save role to catalog.',
      }
    }
  }

  function handleReset() {
    state.teamConfig = null
    state.teamName = deps.inferTeamName(deps.getProjectPath())
    state.selectedNodeId = null
    state.initProgress = null
    state.mode = 'empty'
    state.runtimeMessage = ''
    state.errorMessage = ''
    closeSlideOver()
  }

  function openTemplates() {
    if (state.roleTemplates.length === 0 && !state.loadingRoles) {
      void loadRoleTemplates()
    }
    if (!state.presetsLoaded && !state.loadingPresets) {
      void loadTeamPresets()
    }
  }

  function openTemplateBrowser() {
    openTemplates()
    state.slideOver = 'templates'
  }

  return {
    closeSlideOver,
    handleAppendAgentRole,
    handleAssignLeadRole,
    handleCaptureRoleFlag: toggleCaptureRoleFlag,
    handleClearLead,
    handleMoveBuilderAgentToEnd,
    handlePresetSelect,
    handleRemoveBuilderAgent,
    handleReorderBuilderAgent,
    handleReset,
    handleRoleChange,
    handleSaveBuilderPreset,
    handleTeamDescriptionChange,
    handleTeamNameChange,
    handleUpdateAgent,
    handleUpdateLead,
    loadRoleTemplates,
    loadTeamPresets,
    openAddAgentPanel,
    openCaptureRoleDialog,
    openTemplateBrowser,
    openTemplates,
    submitAddAgent,
    submitCaptureRole,
    toggleAddAgentLock,
    toggleCaptureRoleFlag,
    updateAddAgentField,
    updateCaptureRoleId,
    updateCaptureRoleName,
    closeCaptureRoleDialog,
  }
}
