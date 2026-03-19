export function createMeshTabInit({ state, refs, deps, gate, setup }) {
  function handleInitialize() {
    if (!state.canInitialize) return
    state.initProgress = deps.buildInitializationRequest(
      state.teamConfig,
      state.teamName,
      deps.getProjectPath()
    )
    state.mode = 'initializing'
    state.selectedNodeId = null
    state.runtimeMessage = ''
  }

  async function handleInitializeSuccess(result) {
    const projectPath = deps.getProjectPath()
    const completedRequest = state.initProgress
    const nextTeamName = result?.teamName
      || completedRequest?.teamName
      || state.teamName
      || deps.inferTeamName(projectPath)

    state.teamName = nextTeamName
    state.initProgress = null
    state.runtimeMessage = result?.openedExisting
      ? 'Opened existing team.'
      : 'Team initialized successfully.'
    state.mode = 'runtime'
    state.selectedNodeId = null
    setup.closeSlideOver()

    const sequence = ++refs.discoverySequence
    try {
      await gate.refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
    } catch (error) {
      state.errorMessage = error?.message || 'Failed to load runtime team status.'
      state.teamConfig = {
        lead: deps.createLead(
          { id: 'lead', name: 'team-lead', tool: 'claude', status: 'active' },
          projectPath
        ),
        agents: [],
        presetId: '',
        presetName: '',
        composition: null,
      }
    }
  }

  function setInitializingBack() {
    state.initProgress = null
    state.mode = 'setup'
  }

  return {
    handleInitialize,
    handleInitializeSuccess,
    setInitializingBack,
  }
}
