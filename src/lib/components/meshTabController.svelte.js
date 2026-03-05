import {
  coordinationAddAgent,
  coordinationDisbandTeam,
  coordinationGetLiveTeamStatus,
  coordinationListTeams,
  coordinationRemoveMember,
  coordinationResumeMember,
  getTeamPreset,
  listRoleTemplates,
  upsertRoleTemplate,
} from '../ipc.js'
import { defaultModelForTool, normalizeTool } from '../meshDefaults.js'
import { normalizeProjectOption } from '../projectOptions.js'
import {
  buildCapturedRoleTemplate,
  buildInitializationRequest,
  buildTeamConfigFromPreset,
  buildTeamConfigFromRuntimeStatus,
  coerceTeams,
  composeConfigFromPayload,
  contractHasRules,
  createAgent,
  createLead,
  inferTeamName,
  normalizeBehavioralContract,
  normalizeTeamName,
  slugifyRoleId,
  teamMatchesProject,
} from './meshTabUtils.js'

export function createMeshTabController({
  getProjectPath,
  getAvailableProjects,
  onAddAgent,
  onDisband,
  onRemoveAgent,
  onFocusPane,
}) {
  const quickPresets = [
    {
      presetId: 'standard-team',
      name: 'Standard Dev Team',
      description: 'Orchestrator, architect, two full-stack developers, and a UI specialist',
      leadCount: 1,
      agentCount: 4,
      tools: ['claude', 'codex', 'gemini'],
      builtIn: true,
    },
    {
      presetId: 'fullstack-dev',
      name: 'Full Stack Dev Team',
      description: 'Lead with implementation and review agents',
      leadCount: 1,
      agentCount: 3,
      tools: ['claude', 'codex', 'gemini'],
      builtIn: true,
    },
    {
      presetId: 'research-dev',
      name: 'Research + Development Team',
      description: 'Lead with research and implementation collaboration',
      leadCount: 1,
      agentCount: 3,
      tools: ['claude', 'gemini', 'codex'],
      builtIn: true,
    },
    {
      presetId: 'review-team',
      name: 'Review Team',
      description: 'Lead with focused implementation and QA reviewers',
      leadCount: 1,
      agentCount: 2,
      tools: ['claude', 'codex'],
      builtIn: true,
    },
  ]

  let mode = $state('gate')
  let teamName = $state('')
  let teamConfig = $state(null)
  let slideOver = $state(null)
  let slideOverContext = $state(null)
  let selectedNodeId = $state(null)
  let initProgress = $state(null)
  let errorMessage = $state('')
  let runtimeMessage = $state('')
  let confirmContext = $state(null)
  let roleTemplates = $state([])
  let loadingRoles = $state(false)
  let captureRoleDialog = $state(null)

  let gateBootstrapping = false
  let discoverySequence = 0
  let presetSelectionSequence = 0
  let runtimeMessageTimer = null
  let errorMessageTimer = null

  const selectedNode = $derived.by(() => {
    const config = teamConfig
    if (!config || !selectedNodeId) return null
    if (String(config.lead?.id ?? 'lead') === String(selectedNodeId)) {
      return { ...config.lead, id: String(config.lead?.id ?? 'lead'), role: 'lead' }
    }
    const agent = (config.agents ?? []).find((entry) => String(entry.id) === String(selectedNodeId))
    return agent ? { ...agent, role: 'agent' } : null
  })

  const canInitialize = $derived.by(() => {
    const config = teamConfig
    return Boolean(config?.lead) && Array.isArray(config?.agents)
  })

  const addAgentDraft = $derived(
    slideOver === 'addAgent' && slideOverContext && typeof slideOverContext === 'object'
      ? slideOverContext
      : null
  )

  const canSubmitAddAgent = $derived.by(() => {
    const draft = addAgentDraft
    if (!draft || draft.submitting) return false
    return (
      String(draft.name || '').trim().length > 0 &&
      String(draft.tool || '').trim().length > 0 &&
      String(draft.model || '').trim().length > 0 &&
      String(draft.projectId || '').trim().length > 0
    )
  })

  const captureRoleDraft = $derived(
    captureRoleDialog && typeof captureRoleDialog === 'object' ? captureRoleDialog : null
  )

  const canSaveCapturedRole = $derived.by(() => {
    const draft = captureRoleDraft
    if (!draft || draft.submitting) return false
    return String(draft.name || '').trim().length > 0 && String(draft.roleId || '').trim().length > 0
  })

  async function refreshRuntimeTeamConfig(nextTeamName, sequence) {
    const report = await coordinationGetLiveTeamStatus(nextTeamName)
    if (sequence !== discoverySequence) return
    teamConfig = buildTeamConfigFromRuntimeStatus(report, getProjectPath())
  }

  async function bootstrapFromGate() {
    const sequence = ++discoverySequence
    errorMessage = ''
    runtimeMessage = ''
    try {
      const response = await coordinationListTeams()
      if (sequence !== discoverySequence) return
      const matchingTeam = coerceTeams(response).find((team) => teamMatchesProject(team, getProjectPath()))
      if (matchingTeam) {
        const matchedTeamName = normalizeTeamName(matchingTeam)
        teamName = matchedTeamName
        mode = 'runtime'
        selectedNodeId = null
        initProgress = null
        await refreshRuntimeTeamConfig(matchedTeamName, sequence)
        return
      }
      teamName = inferTeamName(getProjectPath())
      teamConfig = null
      selectedNodeId = null
      initProgress = null
      mode = 'empty'
    } catch (error) {
      if (sequence !== discoverySequence) return
      errorMessage = error?.message || 'Failed to load Mesh team state.'
      teamName = inferTeamName(getProjectPath())
      mode = 'empty'
    }
  }

  function ensureGateReady() {
    if (mode !== 'gate' || gateBootstrapping) return
    gateBootstrapping = true
    void bootstrapFromGate().finally(() => {
      gateBootstrapping = false
    })
  }

  function closeSlideOver() {
    slideOver = null
    slideOverContext = null
  }

  async function handlePresetSelect(preset) {
    const sequence = ++presetSelectionSequence
    const presetId = preset?.presetId ?? preset?.preset_id ?? ''
    let resolvedPreset = preset
    let roleCatalog = []

    try {
      const [hydratedPreset, hydratedRoles] = await Promise.all([
        presetId ? getTeamPreset(presetId) : Promise.resolve(null),
        listRoleTemplates(),
      ])
      if (sequence !== presetSelectionSequence) return
      if (hydratedPreset && typeof hydratedPreset === 'object') {
        resolvedPreset = { ...preset, ...hydratedPreset }
      }
      roleCatalog = Array.isArray(hydratedRoles) ? hydratedRoles : []
    } catch (error) {
      console.error('Failed to hydrate quick preset details:', error)
    }

    if (sequence !== presetSelectionSequence) return
    teamConfig = buildTeamConfigFromPreset(resolvedPreset, roleCatalog, getProjectPath())
    teamName = inferTeamName(getProjectPath())
    selectedNodeId = null
    mode = 'setup'
    closeSlideOver()
    runtimeMessage = ''
  }

  function handleStartCustom() {
    const projectPath = getProjectPath()
    teamConfig = {
      lead: createLead({ id: 'lead', name: 'team-lead', tool: 'claude', status: 'offline' }, projectPath),
      agents: [
        createAgent(0, { id: 'agent-1', name: 'agent-1', tool: 'codex', status: 'offline' }, projectPath),
      ],
      presetId: '',
      presetName: '',
      composition: null,
    }
    teamName = inferTeamName(projectPath)
    selectedNodeId = null
    mode = 'setup'
    closeSlideOver()
    runtimeMessage = ''
  }

  function handleInitialize() {
    if (!canInitialize) return
    initProgress = buildInitializationRequest(teamConfig, teamName, getProjectPath())
    mode = 'initializing'
    selectedNodeId = null
    runtimeMessage = ''
  }

  async function handleInitializeSuccess(result) {
    const projectPath = getProjectPath()
    const nextTeamName =
      (result?.teamName ?? result?.team_name ?? initProgress?.teamName ?? initProgress?.team_name ?? teamName) ||
      inferTeamName(projectPath)
    teamName = nextTeamName
    runtimeMessage = result?.openedExisting ? 'Opened existing team.' : 'Team initialized successfully.'
    mode = 'runtime'
    selectedNodeId = null
    closeSlideOver()

    const sequence = ++discoverySequence
    try {
      await refreshRuntimeTeamConfig(nextTeamName, sequence)
    } catch (error) {
      errorMessage = error?.message || 'Failed to load runtime team status.'
      teamConfig = {
        lead: createLead({ id: 'lead', name: 'team-lead', tool: 'claude', status: 'active' }, projectPath),
        agents: [],
        presetId: '',
        presetName: '',
        composition: null,
      }
    }
  }

  function handleTeamSave(payload) {
    teamConfig = composeConfigFromPayload(payload, getProjectPath())
    if (!teamName.trim()) teamName = inferTeamName(getProjectPath())
    selectedNodeId = null
    mode = 'setup'
    closeSlideOver()
  }

  function openAddAgentPanel() {
    const projectPath = getProjectPath()
    const projectOptions = (getAvailableProjects() ?? [])
      .map((project) => normalizeProjectOption(project, { stringLabel: 'raw', objectFallbackLabel: 'raw' }))
      .filter((project) => project.id)
    const defaultProject = projectOptions[0]?.id || projectPath || ''

    roleTemplates = []
    slideOver = 'addAgent'
    slideOverContext = {
      roleId: '',
      name: '',
      tool: 'codex',
      model: defaultModelForTool('codex'),
      projectId: defaultProject,
      description: '',
      submitting: false,
      error: '',
      isLocked: false,
    }
    void loadRoleTemplates()
  }

  async function loadRoleTemplates() {
    loadingRoles = true
    try {
      roleTemplates = await listRoleTemplates()
    } catch (error) {
      console.error('Failed to load role templates:', error)
    } finally {
      loadingRoles = false
    }
  }

  function handleRoleChange(selectedRoleId) {
    const draft = addAgentDraft
    if (!draft) return
    if (!selectedRoleId) {
      slideOverContext = { ...draft, roleId: '', isLocked: false }
      return
    }

    const role = roleTemplates.find((entry) => entry.roleId === selectedRoleId)
    if (!role) return
    slideOverContext = {
      ...draft,
      roleId: selectedRoleId,
      tool: normalizeTool(role.cliTool),
      model: role.model || defaultModelForTool(role.cliTool),
      description: role.instructions || '',
      isLocked: true,
    }
  }

  function toggleAddAgentLock() {
    const draft = addAgentDraft
    if (!draft) return
    slideOverContext = { ...draft, isLocked: !draft.isLocked }
  }

  function updateAddAgentField(field, value) {
    const draft = addAgentDraft
    if (!draft) return
    const next = { ...draft, [field]: value }
    if (field === 'tool') next.model = defaultModelForTool(value)
    slideOverContext = next
  }

  async function submitAddAgent() {
    const draft = addAgentDraft
    if (!draft || !canSubmitAddAgent) return
    slideOverContext = { ...draft, submitting: true, error: '' }

    try {
      const report = await coordinationAddAgent({
        teamName,
        agent: {
          name: String(draft.name || '').trim(),
          cliTool: normalizeTool(draft.tool),
          model: String(draft.model || '').trim(),
          projectId: String(draft.projectId || '').trim(),
          description: String(draft.description || '').trim() || null,
        },
      })

      onAddAgent(report)
      runtimeMessage = `Agent '${report?.memberName ?? String(draft.name || '').trim()}' added.`
      closeSlideOver()
      const sequence = ++discoverySequence
      await refreshRuntimeTeamConfig(teamName, sequence)
    } catch (error) {
      const latest = addAgentDraft
      if (!latest) return
      slideOverContext = { ...latest, submitting: false, error: error?.message || 'Failed to add agent.' }
    }
  }

  function openCaptureRoleDialog() {
    if (!selectedNode || mode !== 'runtime') return
    const roleName = String(selectedNode.name || '').trim() || 'captured-role'
    const normalizedContract = normalizeBehavioralContract(selectedNode.behavioralContract)
    const description = String(selectedNode.description || '').trim()

    captureRoleDialog = {
      roleKind: selectedNode.role === 'lead' ? 'lead' : 'agent',
      name: roleName,
      roleId: slugifyRoleId(roleName),
      manualRoleId: false,
      tool: normalizeTool(selectedNode.tool),
      model: String(selectedNode.model || '').trim() || defaultModelForTool(selectedNode.tool),
      description,
      includeInstructions: description.length > 0,
      includeBehavioralContract: contractHasRules(normalizedContract),
      behavioralContract: normalizedContract,
      capabilities: Array.isArray(selectedNode.capabilities) ? selectedNode.capabilities : [],
      submitting: false,
      error: '',
    }
  }

  function closeCaptureRoleDialog() {
    captureRoleDialog = null
  }

  function updateCaptureRoleName(value) {
    const draft = captureRoleDraft
    if (!draft) return
    const name = String(value || '')
    captureRoleDialog = { ...draft, name, roleId: draft.manualRoleId ? draft.roleId : slugifyRoleId(name) }
  }

  function updateCaptureRoleId(value) {
    const draft = captureRoleDraft
    if (!draft) return
    captureRoleDialog = { ...draft, roleId: String(value || ''), manualRoleId: true }
  }

  function toggleCaptureRoleFlag(field) {
    const draft = captureRoleDraft
    if (!draft) return
    captureRoleDialog = { ...draft, [field]: !draft[field] }
  }

  async function submitCaptureRole() {
    const draft = captureRoleDraft
    if (!draft || !canSaveCapturedRole) return

    captureRoleDialog = { ...draft, submitting: true, error: '' }
    try {
      await upsertRoleTemplate(buildCapturedRoleTemplate(draft))
      runtimeMessage = 'Role saved to catalog'
      closeCaptureRoleDialog()
      void loadRoleTemplates()
    } catch (error) {
      const latest = captureRoleDraft
      if (!latest) return
      captureRoleDialog = { ...latest, submitting: false, error: error?.message || 'Failed to save role to catalog.' }
    }
  }

  async function handleConfirmAction() {
    if (!confirmContext) return
    const action = confirmContext
    confirmContext = null

    if (action.kind === 'disband') {
      try {
        const result = await coordinationDisbandTeam(teamName)
        onDisband(result)
        runtimeMessage = result?.alreadyDisbanded
          ? 'Team was already disbanded.'
          : 'Team disbanded and active sessions were stopped.'
        mode = 'empty'
        selectedNodeId = null
        teamConfig = null
      } catch (error) {
        errorMessage = error?.message || 'Failed to disband team.'
      }
      return
    }

    if (action.kind === 'remove' && action.memberName) {
      try {
        const report = await coordinationRemoveMember(teamName, action.memberName)
        onRemoveAgent(report)
        runtimeMessage = `Removed '${action.memberName}'.`
        selectedNodeId = null
        const sequence = ++discoverySequence
        await refreshRuntimeTeamConfig(teamName, sequence)
      } catch (error) {
        errorMessage = error?.message || `Failed to remove member '${action.memberName}'.`
      }
    }
  }

  async function resumeSelected(contextMode = 'continue') {
    if (!selectedNode || selectedNode.role !== 'agent') return
    try {
      const report = await coordinationResumeMember(teamName, selectedNode.name, contextMode === 'fresh' ? 'fresh' : 'continue')
      if (!report?.resumed) {
        errorMessage = report?.message || `Failed to resume member '${selectedNode.name}'.`
        return
      }
      runtimeMessage = `Resumed '${selectedNode.name}'.`
      const sequence = ++discoverySequence
      await refreshRuntimeTeamConfig(teamName, sequence)
    } catch (error) {
      errorMessage = error?.message || `Failed to resume member '${selectedNode.name}'.`
    }
  }

  function stopSelected() {
    if (!selectedNode) return
    if (selectedNode.role === 'lead') {
      confirmContext = { kind: 'disband' }
      return
    }
    confirmContext = { kind: 'remove', memberName: selectedNode.name }
  }

  function focusSelectedPane() {
    if (selectedNode?.paneId) onFocusPane(selectedNode.paneId)
  }

  function handleReset() {
    teamConfig = null
    selectedNodeId = null
    initProgress = null
    mode = 'empty'
    runtimeMessage = ''
    errorMessage = ''
    closeSlideOver()
  }

  function openCustomizer() {
    if (!teamConfig) return
    slideOver = 'customizer'
    slideOverContext = { ...slideOverContext }
  }

  function toggleNode(nodeId) {
    selectedNodeId = String(selectedNodeId) === String(nodeId) ? null : String(nodeId)
  }

  function clearSelectedNode() {
    selectedNodeId = null
  }

  function removeSelectedSetupNode() {
    if (selectedNode?.role !== 'agent') return
    teamConfig = {
      ...teamConfig,
      agents: (teamConfig?.agents ?? []).filter((entry) => entry.id !== selectedNode.id),
    }
    selectedNodeId = null
  }

  function openTemplates() {
    slideOver = 'templates'
    slideOverContext = null
  }

  function openRoleFromBrowser(role) {
    slideOver = 'customizer'
    slideOverContext = { selectedRole: role }
  }

  function requestDisband() {
    if (teamName) confirmContext = { kind: 'disband' }
  }

  function cancelConfirm() {
    confirmContext = null
  }

  $effect(() => {
    void getProjectPath()
    mode = 'gate'
    teamName = ''
    teamConfig = null
    slideOver = null
    slideOverContext = null
    captureRoleDialog = null
    selectedNodeId = null
    initProgress = null
    errorMessage = ''
    runtimeMessage = ''
    confirmContext = null
    gateBootstrapping = false
  })

  $effect(() => {
    if (!selectedNodeId) {
      captureRoleDialog = null
      return
    }
    if (!selectedNode) {
      selectedNodeId = null
      captureRoleDialog = null
    }
  })

  $effect(() => {
    if (runtimeMessageTimer) clearTimeout(runtimeMessageTimer)
    if (!runtimeMessage) return
    runtimeMessageTimer = setTimeout(() => {
      runtimeMessage = ''
    }, 5000)
    return () => {
      if (runtimeMessageTimer) clearTimeout(runtimeMessageTimer)
    }
  })

  $effect(() => {
    if (errorMessageTimer) clearTimeout(errorMessageTimer)
    if (!errorMessage) return
    errorMessageTimer = setTimeout(() => {
      errorMessage = ''
    }, 8000)
    return () => {
      if (errorMessageTimer) clearTimeout(errorMessageTimer)
    }
  })

  return {
    quickPresets,
    get mode() {
      return mode
    },
    get teamName() {
      return teamName
    },
    get teamConfig() {
      return teamConfig
    },
    get slideOver() {
      return slideOver
    },
    get slideOverContext() {
      return slideOverContext
    },
    get selectedNodeId() {
      return selectedNodeId
    },
    get initProgress() {
      return initProgress
    },
    get errorMessage() {
      return errorMessage
    },
    get runtimeMessage() {
      return runtimeMessage
    },
    get confirmContext() {
      return confirmContext
    },
    get roleTemplates() {
      return roleTemplates
    },
    get loadingRoles() {
      return loadingRoles
    },
    get selectedNode() {
      return selectedNode
    },
    get addAgentDraft() {
      return addAgentDraft
    },
    get canInitialize() {
      return canInitialize
    },
    get canSubmitAddAgent() {
      return canSubmitAddAgent
    },
    get captureRoleDraft() {
      return captureRoleDraft
    },
    get canSaveCapturedRole() {
      return canSaveCapturedRole
    },
    ensureGateReady,
    closeSlideOver,
    handlePresetSelect,
    handleStartCustom,
    handleInitialize,
    handleInitializeSuccess,
    handleTeamSave,
    openAddAgentPanel,
    handleRoleChange,
    toggleAddAgentLock,
    updateAddAgentField,
    submitAddAgent,
    openCaptureRoleDialog,
    closeCaptureRoleDialog,
    updateCaptureRoleName,
    updateCaptureRoleId,
    toggleCaptureRoleFlag,
    submitCaptureRole,
    handleConfirmAction,
    resumeSelected,
    stopSelected,
    focusSelectedPane,
    handleReset,
    openCustomizer,
    toggleNode,
    clearSelectedNode,
    removeSelectedSetupNode,
    openTemplates,
    openRoleFromBrowser,
    requestDisband,
    cancelConfirm,
    dismissError: () => {
      errorMessage = ''
    },
    dismissRuntimeMessage: () => {
      runtimeMessage = ''
    },
    setInitializingBack: () => {
      mode = 'setup'
    },
  }
}
