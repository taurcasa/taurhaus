export async function refreshRuntimeTeamConfigWorkflow({
  nextTeamName,
  sequence,
  getDiscoverySequence,
  coordinationGetLiveTeamStatus,
  buildTeamConfigFromRuntimeStatus,
  getProjectPath,
  onTeamConfig,
}) {
  const report = await coordinationGetLiveTeamStatus(nextTeamName)
  if (sequence !== getDiscoverySequence()) return false
  onTeamConfig(buildTeamConfigFromRuntimeStatus(report, getProjectPath()))
  return true
}

async function bootstrapFromGateWorkflow({
  sequence,
  getDiscoverySequence,
  coordinationListTeams,
  coerceTeams,
  teamMatchesProject,
  getProjectPath,
  normalizeTeamName,
  inferTeamName,
  onRuntimeTeamMatched,
  onEmptyTeamState,
  onEmptyTeamStateWithError,
}) {
  try {
    const response = await coordinationListTeams()
    if (sequence !== getDiscoverySequence()) return
    const matchingTeam = coerceTeams(response).find((team) => teamMatchesProject(team, getProjectPath()))
    if (matchingTeam) {
      const matchedTeamName = normalizeTeamName(matchingTeam)
      await onRuntimeTeamMatched(matchedTeamName, sequence)
      return
    }
    onEmptyTeamState(inferTeamName(getProjectPath()))
  } catch (error) {
    if (sequence !== getDiscoverySequence()) return
    onEmptyTeamStateWithError(error?.message || 'Failed to load Mesh team state.', inferTeamName(getProjectPath()))
  }
}
