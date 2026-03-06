export const TOOL_OPTIONS = ['claude', 'codex', 'gemini']

export const MODEL_OPTIONS_BY_TOOL = {
  claude: ['opus', 'sonnet', 'haiku'],
  codex: ['gpt-5.4-high', 'gpt-5.3-codex', 'gpt-5-mini'],
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
  return String(roleTemplate?.model ?? roleTemplate?.defaults?.model ?? defaultModelForTool(tool))
}

export function uniquifyMemberName(name, seenNames) {
  const baseName = String(name || '').trim()
  if (!baseName) return ''
  const seen = seenNames.get(baseName) ?? 0
  seenNames.set(baseName, seen + 1)
  return seen === 0 ? baseName : `${baseName}-${seen}`
}
