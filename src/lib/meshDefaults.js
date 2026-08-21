export const TOOL_OPTIONS = ['claude', 'codex', 'gemini']

export const MODEL_OPTIONS_BY_TOOL = {
  claude: ['opus', 'sonnet', 'haiku'],
  codex: ['gpt-5.4', 'gpt-5-mini'],
  gemini: ['gemini-3.1-pro', 'gemini-2.5-pro', 'gemini-2.0-flash'],
}

export function normalizeTool(tool) {
  const value = String(tool || '').trim().toLowerCase()
  return TOOL_OPTIONS.includes(value) ? value : 'claude'
}

export function modelsForTool(tool) {
  return MODEL_OPTIONS_BY_TOOL[normalizeTool(tool)] ?? MODEL_OPTIONS_BY_TOOL.claude
}

export function defaultModelForTool(tool) {
  return modelsForTool(tool)[0] ?? 'default'
}

export function applyNamePattern(pattern, n, projectName) {
  return String(pattern || '')
    .replace(/\{n\}/g, String(n))
    .replace(/\{project\}/g, projectName)
}

function seenNameMapHas(seenNames, name) {
  return seenNames instanceof Set
    ? seenNames.has(name)
    : seenNames.has(name)
}

function seenNameMapAdd(seenNames, name) {
  if (seenNames instanceof Set) {
    seenNames.add(name)
    return
  }
  seenNames.set(name, true)
}

function splitNumericSuffix(name) {
  const match = String(name).match(/^(.*?)-(\d+)$/)
  if (!match) {
    return { stem: name, numericSuffix: null }
  }
  return {
    stem: match[1] || name,
    numericSuffix: Number(match[2]),
  }
}

export function resolveDefaultNamePattern(roleTemplate) {
  return (
    roleTemplate?.defaults?.defaultNamePattern ??
    roleTemplate?.defaults?.default_name_pattern ??
    roleTemplate?.defaultNamePattern ??
    roleTemplate?.default_name_pattern ??
    null
  )
}

export function resolveSlotNamePattern(slot, roleTemplate) {
  const overridePattern = slot?.overrides?.namePattern ?? slot?.overrides?.name_pattern
  return overridePattern ?? resolveDefaultNamePattern(roleTemplate) ?? 'agent-{n}'
}

export function resolveRoleTool(roleTemplate, fallbackTool = 'codex') {
  return normalizeTool(
    roleTemplate?.cliTool ??
    roleTemplate?.cli_tool ??
    roleTemplate?.defaults?.cliTool ??
    roleTemplate?.defaults?.cli_tool ??
    fallbackTool
  )
}

export function resolveRoleModel(roleTemplate, tool) {
  const model = String(roleTemplate?.model ?? roleTemplate?.defaults?.model ?? '').trim()
  return model || defaultModelForTool(tool)
}

export function uniquifyMemberName(name, seenNames) {
  const baseName = String(name || '').trim()
  if (!baseName) return ''
  const normalizedBaseName = baseName.toLowerCase()
  if (!seenNameMapHas(seenNames, normalizedBaseName)) {
    seenNameMapAdd(seenNames, normalizedBaseName)
    return baseName
  }

  const { stem, numericSuffix } = splitNumericSuffix(baseName)
  const normalizedStem = String(stem || baseName).trim() || baseName
  let nextSuffix = numericSuffix === null ? 1 : numericSuffix + 1

  while (nextSuffix < 10_000) {
    const candidate = `${normalizedStem}-${nextSuffix}`
    const normalizedCandidate = candidate.toLowerCase()
    if (!seenNameMapHas(seenNames, normalizedCandidate)) {
      seenNameMapAdd(seenNames, normalizedCandidate)
      return candidate
    }
    nextSuffix += 1
  }

  return ''
}
