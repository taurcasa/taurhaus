const CAPABILITY_DEFAULTS = Object.freeze({
  modelFlag: null,
  effortFlag: null,
  displayNameFlag: null,
  teamFlags: false,
  nativeInboxPoller: false,
  sessionSource: false,
  authoritativeIdle: false,
  compactionHook: false,
  transcriptParser: false,
  transcriptCompactionSignals: false,
  catalog: false,
  configDirEnv: null,
  usageBridge: false,
  notifySink: false,
  hookTrust: false,
})

export const FALLBACK_TOOLS = Object.freeze([
  {
    id: 'claude',
    label: 'Claude',
    accent: 'emerald',
    aliases: ['claude', 'claude_native'],
    capabilities: {
      modelFlag: '--model',
      effortFlag: { kind: 'argument', flag: '--effort' },
      displayNameFlag: '-n',
      teamFlags: true,
      nativeInboxPoller: true,
      sessionSource: true,
      authoritativeIdle: true,
      compactionHook: true,
      transcriptParser: true,
      transcriptCompactionSignals: false,
      catalog: true,
      configDirEnv: 'CLAUDE_CONFIG_DIR',
      usageBridge: true,
      notifySink: false,
      hookTrust: false,
    },
  },
  {
    id: 'codex',
    label: 'Codex',
    accent: 'sky',
    aliases: ['codex', 'mesh', 'mesh_bridged'],
    capabilities: {
      modelFlag: '-m',
      effortFlag: { kind: 'config', flag: '-c', key: 'model_reasoning_effort' },
      displayNameFlag: null,
      teamFlags: false,
      nativeInboxPoller: false,
      sessionSource: true,
      authoritativeIdle: true,
      compactionHook: true,
      transcriptParser: true,
      transcriptCompactionSignals: true,
      catalog: true,
      configDirEnv: null,
      usageBridge: false,
      notifySink: true,
      hookTrust: true,
    },
  },
  {
    id: 'gemini',
    label: 'Gemini',
    accent: 'violet',
    aliases: ['gemini'],
    capabilities: {
      modelFlag: '-m',
      effortFlag: null,
      displayNameFlag: null,
      teamFlags: false,
      nativeInboxPoller: false,
      sessionSource: false,
      authoritativeIdle: false,
      compactionHook: false,
      transcriptParser: false,
      transcriptCompactionSignals: false,
      catalog: true,
      configDirEnv: null,
      usageBridge: false,
      notifySink: false,
      hookTrust: false,
    },
  },
])

let currentTools = FALLBACK_TOOLS

function stringOrNull(value) {
  if (value == null) return null
  return String(value).trim() || null
}

function capability(raw, camel, snake, fallback = false) {
  return raw?.[camel] ?? raw?.[snake] ?? fallback
}

function normalizeCapabilities(raw) {
  const source = raw && typeof raw === 'object' ? raw : {}
  const effortFlag = capability(source, 'effortFlag', 'effort_flag', null)
  return {
    ...CAPABILITY_DEFAULTS,
    modelFlag: stringOrNull(capability(source, 'modelFlag', 'model_flag', null)),
    effortFlag: effortFlag && typeof effortFlag === 'object' ? { ...effortFlag } : null,
    displayNameFlag: stringOrNull(capability(source, 'displayNameFlag', 'display_name_flag', null)),
    teamFlags: Boolean(capability(source, 'teamFlags', 'team_flags')),
    nativeInboxPoller: Boolean(capability(source, 'nativeInboxPoller', 'native_inbox_poller')),
    sessionSource: Boolean(capability(source, 'sessionSource', 'session_source')),
    authoritativeIdle: Boolean(capability(source, 'authoritativeIdle', 'authoritative_idle')),
    compactionHook: Boolean(capability(source, 'compactionHook', 'compaction_hook')),
    transcriptParser: Boolean(capability(source, 'transcriptParser', 'transcript_parser')),
    transcriptCompactionSignals: Boolean(
      capability(source, 'transcriptCompactionSignals', 'transcript_compaction_signals')
    ),
    catalog: Boolean(capability(source, 'catalog', 'catalog')),
    configDirEnv: stringOrNull(capability(source, 'configDirEnv', 'config_dir_env', null)),
    usageBridge: Boolean(capability(source, 'usageBridge', 'usage_bridge')),
    notifySink: Boolean(capability(source, 'notifySink', 'notify_sink')),
    hookTrust: Boolean(capability(source, 'hookTrust', 'hook_trust')),
  }
}

function normalizeDescriptor(raw) {
  if (!raw || typeof raw !== 'object') return null
  const id = String(raw.id ?? '').trim().toLowerCase()
  if (!id) return null
  const aliases = Array.isArray(raw.aliases)
    ? raw.aliases.map((alias) => String(alias ?? '').trim().toLowerCase()).filter(Boolean)
    : []
  if (!aliases.includes(id)) aliases.unshift(id)
  return {
    id,
    label: String(raw.label ?? id).trim() || id,
    accent: String(raw.accent ?? 'brand').trim() || 'brand',
    aliases,
    capabilities: normalizeCapabilities(raw.capabilities),
  }
}

export function normalizeToolDescriptors(raw, fallback = FALLBACK_TOOLS) {
  if (!Array.isArray(raw)) return fallback.map((entry) => normalizeDescriptor(entry))
  const normalized = raw.map(normalizeDescriptor).filter(Boolean)
  return normalized.length > 0 ? normalized : fallback.map((entry) => normalizeDescriptor(entry))
}

export function configureToolRegistry(raw) {
  currentTools = normalizeToolDescriptors(raw)
  return currentTools
}

export function resetToolRegistry() {
  currentTools = FALLBACK_TOOLS
}

export function tools() {
  return currentTools
}

export function toolDescriptor(value) {
  const normalized = String(value ?? '').trim().toLowerCase()
  return currentTools.find((entry) => entry.aliases.includes(normalized)) ?? null
}

export const TOOL_OPTIONS = Object.freeze(FALLBACK_TOOLS.map((entry) => entry.id))

function fallbackDescriptor(value = null) {
  return (
    toolDescriptor(value) ??
    currentTools.find((entry) => entry.capabilities.nativeInboxPoller) ??
    currentTools[0] ??
    null
  )
}

export function normalizeTool(value, fallback = null) {
  return toolDescriptor(value)?.id ?? fallbackDescriptor(fallback)?.id ?? ''
}

export function toolLabel(value, fallback = 'Unknown') {
  return toolDescriptor(value)?.label ?? fallback
}

export function toolAccent(value, fallback = 'brand') {
  return toolDescriptor(value)?.accent ?? fallback
}

export function toolCounts(items, readTool) {
  const values = Array.isArray(items) ? items : []
  const counts = Object.fromEntries(currentTools.map((entry) => [entry.id, 0]))
  for (const item of values) {
    const id = toolDescriptor(readTool(item))?.id
    if (id && Object.hasOwn(counts, id)) counts[id] += 1
  }
  return { all: values.length, ...counts }
}
