export const BEHAVIORAL_CONTRACT_MODES = {
  OPTIONAL_OBJECT: 'optional_object',
  TEMPLATE_INPUT: 'template_input',
}

const DEFAULT_EXECUTION_RULE = 'Execute assigned tasks and report status clearly.'

function isObjectRecord(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function normalizeStringList(value) {
  if (!Array.isArray(value)) return []
  return value.map((line) => String(line ?? '').trim()).filter(Boolean)
}

function normalizeTemplateExecutionRules(value) {
  return value
    .map((entry) => {
      if (typeof entry === 'string') return entry.trim()
      if (!isObjectRecord(entry)) return ''
      if (entry.enabled === false) return ''
      return String(entry.rule ?? entry.text ?? '').trim()
    })
    .filter(Boolean)
}

export function normalizeBehavioralContract(value, options = {}) {
  const mode = options.mode ?? BEHAVIORAL_CONTRACT_MODES.OPTIONAL_OBJECT

  if (mode === BEHAVIORAL_CONTRACT_MODES.OPTIONAL_OBJECT) {
    if (!isObjectRecord(value)) return null
    return {
      ...value,
      communication: Array.isArray(value.communication) ? value.communication : [],
      execution: Array.isArray(value.execution) ? value.execution : [],
      escalation: Array.isArray(value.escalation) ? value.escalation : [],
    }
  }

  if (mode === BEHAVIORAL_CONTRACT_MODES.TEMPLATE_INPUT) {
    if (Array.isArray(value)) {
      const execution = normalizeTemplateExecutionRules(value)
      return {
        communication: [],
        execution: execution.length > 0 ? execution : [DEFAULT_EXECUTION_RULE],
        escalation: [],
      }
    }

    const communication = normalizeStringList(value?.communication)
    const execution = normalizeStringList(value?.execution)
    const escalation = normalizeStringList(value?.escalation)

    if (communication.length || execution.length || escalation.length) {
      return { ...(isObjectRecord(value) ? value : {}), communication, execution, escalation }
    }

    return {
      ...(isObjectRecord(value) ? value : {}),
      communication: [],
      execution: [DEFAULT_EXECUTION_RULE],
      escalation: [],
    }
  }

  throw new Error(`Unsupported behavioral contract normalization mode: ${mode}`)
}
