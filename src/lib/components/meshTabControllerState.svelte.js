const WRITABLE_STATE_KEYS = [
  'mode',
  'teamName',
  'teamConfig',
  'slideOver',
  'slideOverContext',
  'selectedNodeId',
  'initProgress',
  'errorMessage',
  'runtimeMessage',
  'confirmContext',
  'availabilityMessage',
  'roleTemplates',
  'loadingRoles',
  'availablePresets',
  'loadingPresets',
  'presetsLoaded',
  'captureRoleDialog',
  'teamRuntimeState',
  'teamResumeProgress',
]

export const QUICK_PRESETS = [
  {
    presetId: 'pair',
    name: 'Pair',
    description: 'Lead plus one developer',
    leadCount: 1,
    agentCount: 1,
    tools: ['claude', 'codex'],
    builtIn: true,
  },
  {
    presetId: 'dev-team',
    name: 'Dev Team',
    description: 'Lead plus two developers',
    leadCount: 1,
    agentCount: 2,
    tools: ['claude', 'codex'],
    builtIn: true,
  },
  {
    presetId: 'full-team',
    name: 'Full Team',
    description: 'Lead, architect, and two developers',
    leadCount: 1,
    agentCount: 3,
    tools: ['claude', 'codex'],
    builtIn: true,
  },
  {
    presetId: 'research-team',
    name: 'Research Team',
    description: 'Lead, researcher, and developer',
    leadCount: 1,
    agentCount: 2,
    tools: ['claude', 'codex'],
    builtIn: true,
  },
]

function defineWritableAccessors(target, source, keys) {
  for (const key of keys) {
    Object.defineProperty(target, key, {
      enumerable: true,
      get() {
        return source[key]
      },
      set(value) {
        source[key] = value
      },
    })
  }
}

function defineReadonlyAccessors(target, accessors) {
  for (const [key, read] of Object.entries(accessors)) {
    Object.defineProperty(target, key, {
      enumerable: true,
      get: read,
    })
  }
}

export function createMeshTabState(quickPresets = QUICK_PRESETS) {
  const rawState = $state({
    mode: 'empty',
    teamName: '',
    teamConfig: null,
    slideOver: null,
    slideOverContext: null,
    selectedNodeId: null,
    initProgress: null,
    errorMessage: '',
    runtimeMessage: '',
    confirmContext: null,
    availabilityMessage: '',
    roleTemplates: [],
    loadingRoles: false,
    availablePresets: quickPresets,
    loadingPresets: false,
    presetsLoaded: false,
    captureRoleDialog: null,
    teamRuntimeState: 'none',
    teamResumeProgress: null,
  })

  const selectedNode = $derived.by(() => {
    const config = rawState.teamConfig
    if (!config || !rawState.selectedNodeId) return null
    if (String(config.lead?.id ?? 'lead') === String(rawState.selectedNodeId)) {
      return { ...config.lead, id: String(config.lead?.id ?? 'lead'), role: 'lead' }
    }
    const agent = (config.agents ?? []).find(
      (entry) => String(entry.id) === String(rawState.selectedNodeId)
    )
    return agent ? { ...agent, role: 'agent' } : null
  })

  const canInitialize = $derived.by(
    () => Boolean(rawState.teamConfig?.lead) && Array.isArray(rawState.teamConfig?.agents)
  )

  const addAgentDraft = $derived(
    rawState.slideOver === 'addAgent' &&
      rawState.slideOverContext &&
      typeof rawState.slideOverContext === 'object'
      ? rawState.slideOverContext
      : null
  )

  const canSubmitAddAgent = $derived.by(() => {
    const draft = addAgentDraft
    if (!draft || draft.submitting) return false
    return (
      String(draft.roleId || '').trim().length > 0 &&
      String(draft.name || '').trim().length > 0 &&
      String(draft.tool || '').trim().length > 0 &&
      String(draft.model || '').trim().length > 0 &&
      String(draft.projectId || '').trim().length > 0
    )
  })

  const captureRoleDraft = $derived(
    rawState.captureRoleDialog && typeof rawState.captureRoleDialog === 'object'
      ? rawState.captureRoleDialog
      : null
  )

  const isResumingTeam = $derived.by(() => Boolean(rawState.teamResumeProgress?.inFlight))
  const canResumeTeam = $derived.by(
    () => rawState.teamRuntimeState === 'coldResume' || rawState.teamRuntimeState === 'degraded'
  )

  const canSaveCapturedRole = $derived.by(() => {
    const draft = captureRoleDraft
    if (!draft || draft.submitting) return false
    return String(draft.name || '').trim().length > 0 && String(draft.roleId || '').trim().length > 0
  })

  const state = {}
  defineWritableAccessors(state, rawState, WRITABLE_STATE_KEYS)
  defineReadonlyAccessors(state, {
    selectedNode: () => selectedNode,
    canInitialize: () => canInitialize,
    addAgentDraft: () => addAgentDraft,
    canSubmitAddAgent: () => canSubmitAddAgent,
    captureRoleDraft: () => captureRoleDraft,
    isResumingTeam: () => isResumingTeam,
    canResumeTeam: () => canResumeTeam,
    canSaveCapturedRole: () => canSaveCapturedRole,
  })

  return state
}

export function createMeshTabRefs() {
  return {
    discoverySequence: 0,
    presetSelectionSequence: 0,
    runtimeMessageTimer: null,
    errorMessageTimer: null,
    runtimePollTimer: null,
    runtimeRefreshTimer: null,
    runtimeRefreshMeta: null,
    runtimeStatusRequest: null,
    runtimeStatusRequestMeta: null,
    queuedRuntimeStatusRequest: null,
    projectSnapshotRefreshTimer: null,
    teamResumeProgressTimer: null,
    hydrationPerf: null,
    pendingProjectSnapshot: null,
  }
}

export function createMeshTabPublicApi({ state, gate, setup, init, runtime }) {
  return {
    get quickPresets() {
      return state.availablePresets
    },
    get mode() {
      return state.mode
    },
    get teamName() {
      return state.teamName
    },
    get teamConfig() {
      return state.teamConfig
    },
    get slideOver() {
      return state.slideOver
    },
    get slideOverContext() {
      return state.slideOverContext
    },
    get selectedNodeId() {
      return state.selectedNodeId
    },
    get initProgress() {
      return state.initProgress
    },
    get errorMessage() {
      return state.errorMessage
    },
    get runtimeMessage() {
      return state.runtimeMessage
    },
    get availabilityMessage() {
      return state.availabilityMessage
    },
    get confirmContext() {
      return state.confirmContext
    },
    get roleTemplates() {
      return state.roleTemplates
    },
    get teamRuntimeState() {
      return state.teamRuntimeState
    },
    get isResumingTeam() {
      return state.isResumingTeam
    },
    get teamResumeProgress() {
      return state.teamResumeProgress
    },
    get loadingRoles() {
      return state.loadingRoles
    },
    get selectedNode() {
      return state.selectedNode
    },
    get addAgentDraft() {
      return state.addAgentDraft
    },
    get canInitialize() {
      return state.canInitialize
    },
    get canSubmitAddAgent() {
      return state.canSubmitAddAgent
    },
    get captureRoleDraft() {
      return state.captureRoleDraft
    },
    get canSaveCapturedRole() {
      return state.canSaveCapturedRole
    },
    ensureGateReady: gate.ensureGateReady,
    closeSlideOver: setup.closeSlideOver,
    handlePresetSelect: setup.handlePresetSelect,
    handleTeamNameChange: setup.handleTeamNameChange,
    handleTeamDescriptionChange: setup.handleTeamDescriptionChange,
    handleAssignLeadRole: setup.handleAssignLeadRole,
    handleClearLead: setup.handleClearLead,
    handleAppendAgentRole: setup.handleAppendAgentRole,
    handleUpdateLead: setup.handleUpdateLead,
    handleUpdateAgent: setup.handleUpdateAgent,
    handleRemoveBuilderAgent: setup.handleRemoveBuilderAgent,
    handleReorderBuilderAgent: setup.handleReorderBuilderAgent,
    handleMoveBuilderAgentToEnd: setup.handleMoveBuilderAgentToEnd,
    handleSaveBuilderPreset: setup.handleSaveBuilderPreset,
    handleInitialize: init.handleInitialize,
    handleInitializeSuccess: init.handleInitializeSuccess,
    openAddAgentPanel: setup.openAddAgentPanel,
    handleRoleChange: setup.handleRoleChange,
    toggleAddAgentLock: setup.toggleAddAgentLock,
    updateAddAgentField: setup.updateAddAgentField,
    submitAddAgent: setup.submitAddAgent,
    openCaptureRoleDialog: setup.openCaptureRoleDialog,
    closeCaptureRoleDialog: setup.closeCaptureRoleDialog,
    updateCaptureRoleName: setup.updateCaptureRoleName,
    updateCaptureRoleId: setup.updateCaptureRoleId,
    toggleCaptureRoleFlag: setup.toggleCaptureRoleFlag,
    submitCaptureRole: setup.submitCaptureRole,
    handleConfirmAction: runtime.handleConfirmAction,
    resumeTeam: runtime.resumeTeam,
    resumeSelected: runtime.resumeSelected,
    stopSelected: runtime.stopSelected,
    focusSelectedPane: runtime.focusSelectedPane,
    handleReset: setup.handleReset,
    toggleNode: runtime.toggleNode,
    clearSelectedNode: runtime.clearSelectedNode,
    openTemplates: setup.openTemplates,
    requestDisband: runtime.requestDisband,
    cancelConfirm: runtime.cancelConfirm,
    dismissError: () => {
      state.errorMessage = ''
    },
    dismissRuntimeMessage: () => {
      state.runtimeMessage = ''
    },
    setInitializingBack: init.setInitializingBack,
  }
}
