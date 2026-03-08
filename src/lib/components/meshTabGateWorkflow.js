export async function refreshRuntimeTeamConfigWorkflow({
  nextTeamName,
  sequence,
  getDiscoverySequence,
  coordinationGetLiveTeamStatus,
  coordinationGetCompactionAudit = null,
  buildTeamConfigFromRuntimeStatus,
  getProjectPath,
  onTeamConfig,
  onCompactionAudit = () => {},
}) {
  const [report, audit] = await Promise.all([
    coordinationGetLiveTeamStatus(nextTeamName),
    coordinationGetCompactionAudit
      ? coordinationGetCompactionAudit(nextTeamName).catch(() => null)
      : Promise.resolve(null),
  ])
  if (sequence !== getDiscoverySequence()) return false
  onTeamConfig(buildTeamConfigFromRuntimeStatus(report, getProjectPath()))
  onCompactionAudit(audit)
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
