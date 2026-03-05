import { BEHAVIORAL_CONTRACT_MODES, normalizeBehavioralContract } from './normalize.js'

function normalizeAgentSetupPayload(value) {
  if (!value || typeof value !== 'object') return value

  return {
    ...value,
    roleId: value.roleId ?? null,
    instructions: value.instructions ?? null,
    behavioralContract: normalizeBehavioralContract(value.behavioralContract, {
      mode: BEHAVIORAL_CONTRACT_MODES.OPTIONAL_OBJECT,
    }),
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
