function normalizeBehavioralContractPayload(value) {
  if (!value || typeof value !== 'object') return null

  return {
    communication: Array.isArray(value.communication) ? value.communication : [],
    execution: Array.isArray(value.execution) ? value.execution : [],
    escalation: Array.isArray(value.escalation) ? value.escalation : [],
  }
}

function normalizeAgentSetupPayload(value) {
  if (!value || typeof value !== 'object') return value

  return {
    ...value,
    roleId: value.roleId ?? null,
    instructions: value.instructions ?? null,
    behavioralContract: normalizeBehavioralContractPayload(value.behavioralContract),
    capabilities: Array.isArray(value.capabilities) ? value.capabilities : null,
  }
}

export function normalizeInitializeTeamPayload(request) {
  if (!request || typeof request !== 'object') return request

  return {
    ...request,
    lead: normalizeAgentSetupPayload(request.lead),
    agents: Array.isArray(request.agents)
      ? request.agents.map((agent) => normalizeAgentSetupPayload(agent))
      : [],
  }
}
