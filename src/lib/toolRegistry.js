import { createToolRegistryState } from './toolRegistryState.svelte.js'

const CAPABILITY_DEFAULTS = Object.freeze({
  modelFlag: null,
  effortFlag: null,
  autoApproveFlag: null,
  displayNameFlag: null,
  teamFlags: false,
  nativeInboxPoller: false,
  sessionSource: false,
  runtimeSessionCapture: false,
  authoritativeIdle: false,
  compactionHook: false,
  compactionHookCompatImport: false,
  transcriptParser: false,
  catalog: false,
  sessionRoot: 'toolHome',
  accountSelector: null,
  accountSelection: false,
  teamConfigNamespace: false,
  usage: false,
  usageNote: null,
  notifySink: false,
  hookTrust: false,
  managedHome: false,
})

export const FALLBACK_TOOLS = Object.freeze([
  {
    id: 'claude',
    label: 'Claude',
    displayName: 'Claude Code',
    accent: 'emerald',
    medallionAccent: 'amber',
    defaultAgentRoleId: 'v4-developer-claude',
    aliases: ['claude', 'claude_native'],
    capabilities: {
      modelFlag: '--model',
      effortFlag: { kind: 'argument', flag: '--effort' },
      autoApproveFlag: '--dangerously-skip-permissions',
      displayNameFlag: '-n',
      teamFlags: true,
      nativeInboxPoller: true,
      sessionSource: true,
      runtimeSessionCapture: true,
      authoritativeIdle: true,
      compactionHook: true,
      compactionHookCompatImport: false,
      transcriptParser: true,
      catalog: true,
      sessionRoot: 'appManagedClaudeDir',
      accountSelector: 'CLAUDE_CONFIG_DIR',
      accountSelection: true,
      teamConfigNamespace: true,
      usage: true,
      usageNote: null,
      notifySink: false,
      hookTrust: false,
      managedHome: false,
    },
  },
  {
    id: 'codex',
    label: 'Codex',
    displayName: 'Codex',
    accent: 'sky',
    medallionAccent: 'emerald',
    defaultAgentRoleId: 'v4-developer-codex',
    aliases: ['codex', 'mesh', 'mesh_bridged'],
    capabilities: {
      modelFlag: '-m',
      effortFlag: { kind: 'config', flag: '-c', key: 'model_reasoning_effort' },
      autoApproveFlag: '--yolo',
      displayNameFlag: null,
      teamFlags: false,
      nativeInboxPoller: false,
      sessionSource: true,
      runtimeSessionCapture: true,
      authoritativeIdle: true,
      compactionHook: true,
      compactionHookCompatImport: false,
      transcriptParser: true,
      catalog: true,
      sessionRoot: 'toolHome',
      accountSelector: 'CODEX_HOME',
      accountSelection: true,
      teamConfigNamespace: false,
      usage: true,
      usageNote: null,
      notifySink: true,
      hookTrust: true,
      managedHome: true,
    },
  },
  {
    id: 'agy',
    label: 'Antigravity',
    displayName: 'Antigravity CLI',
    accent: 'google-blue',
    medallionAccent: 'google-blue',
    defaultAgentRoleId: 'v4-developer-agy',
    aliases: ['agy', 'antigravity'],
    capabilities: {
      modelFlag: '--model',
      effortFlag: { kind: 'argument', flag: '--effort' },
      autoApproveFlag: '--dangerously-skip-permissions',
      displayNameFlag: null,
      teamFlags: false,
      nativeInboxPoller: false,
      sessionSource: true,
      runtimeSessionCapture: false,
      authoritativeIdle: false,
      compactionHook: false,
      compactionHookCompatImport: false,
      transcriptParser: false,
      catalog: true,
      sessionRoot: 'toolHome',
      accountSelector: null,
      accountSelection: false,
      teamConfigNamespace: false,
      usage: true,
      usageNote: null,
      notifySink: false,
      hookTrust: false,
      managedHome: false,
    },
  },
  {
    id: 'grok',
    label: 'Grok',
    displayName: 'Grok CLI',
    accent: 'graphite',
    medallionAccent: 'graphite',
    defaultAgentRoleId: 'v4-developer-grok',
    aliases: ['grok'],
    capabilities: {
      modelFlag: '--model',
      effortFlag: { kind: 'argument', flag: '--effort' },
      autoApproveFlag: '--always-approve',
      displayNameFlag: null,
      teamFlags: false,
      nativeInboxPoller: false,
      sessionSource: true,
      runtimeSessionCapture: true,
      authoritativeIdle: true,
      compactionHook: true,
      compactionHookCompatImport: true,
      transcriptParser: false,
      catalog: true,
      sessionRoot: 'toolHome',
      accountSelector: 'GROK_HOME',
      accountSelection: true,
      teamConfigNamespace: false,
      usage: false,
      usageNote: 'Grok shows credits in its own /usage',
      notifySink: false,
      hookTrust: false,
      managedHome: false,
    },
  },
])

const registryState = createToolRegistryState(FALLBACK_TOOLS)

function stringOrNull(value) {
  if (value == null) return null
  return String(value).trim() || null
}

function capability(raw, camel, snake, fallback = false) {
  return raw?.[camel] ?? raw?.[snake] ?? fallback
}

// The snake_case spellings `capability()` reads; the backend contract is
// camelCase, so a consumed alias must not survive next to the key written here.
const CAPABILITY_ALIASES = [
  'model_flag', 'effort_flag', 'auto_approve_flag', 'display_name_flag', 'team_flags',
  'native_inbox_poller', 'session_source', 'runtime_session_capture', 'authoritative_idle',
  'compaction_hook', 'compaction_hook_compat_import', 'transcript_parser', 'session_root',
  'account_selector', 'account_selection', 'team_config_namespace', 'usage_note',
  'notify_sink', 'hook_trust', 'managed_home',
]
const DESCRIPTOR_ALIASES = ['display_name', 'medallion_accent', 'default_agent_role_id']

function withoutKeys(value, keys) {
  for (const key of keys) delete value[key]
  return value
}

function normalizeCapabilities(raw) {
  const source = raw && typeof raw === 'object' ? raw : {}
  const effortFlag = capability(source, 'effortFlag', 'effort_flag', null)
  return withoutKeys({
    ...CAPABILITY_DEFAULTS,
    ...source,
    modelFlag: stringOrNull(capability(source, 'modelFlag', 'model_flag', null)),
    effortFlag: effortFlag && typeof effortFlag === 'object' ? { ...effortFlag } : null,
    autoApproveFlag: stringOrNull(
      capability(source, 'autoApproveFlag', 'auto_approve_flag', null)
    ),
    displayNameFlag: stringOrNull(capability(source, 'displayNameFlag', 'display_name_flag', null)),
    teamFlags: Boolean(capability(source, 'teamFlags', 'team_flags')),
    nativeInboxPoller: Boolean(capability(source, 'nativeInboxPoller', 'native_inbox_poller')),
    sessionSource: Boolean(capability(source, 'sessionSource', 'session_source')),
    runtimeSessionCapture: Boolean(
      capability(source, 'runtimeSessionCapture', 'runtime_session_capture')
    ),
    authoritativeIdle: Boolean(capability(source, 'authoritativeIdle', 'authoritative_idle')),
    compactionHook: Boolean(capability(source, 'compactionHook', 'compaction_hook')),
    compactionHookCompatImport: Boolean(
      capability(source, 'compactionHookCompatImport', 'compaction_hook_compat_import')
    ),
    transcriptParser: Boolean(capability(source, 'transcriptParser', 'transcript_parser')),
    catalog: Boolean(capability(source, 'catalog', 'catalog')),
    sessionRoot:
      stringOrNull(capability(source, 'sessionRoot', 'session_root', 'toolHome')) ?? 'toolHome',
    accountSelector: stringOrNull(
      capability(source, 'accountSelector', 'account_selector', null)
    ),
    accountSelection: Boolean(capability(source, 'accountSelection', 'account_selection')),
    teamConfigNamespace: Boolean(
      capability(source, 'teamConfigNamespace', 'team_config_namespace')
    ),
    usage: Boolean(capability(source, 'usage', 'usage')),
    usageNote: stringOrNull(capability(source, 'usageNote', 'usage_note', null)),
    notifySink: Boolean(capability(source, 'notifySink', 'notify_sink')),
    hookTrust: Boolean(capability(source, 'hookTrust', 'hook_trust')),
    managedHome: Boolean(capability(source, 'managedHome', 'managed_home')),
  }, CAPABILITY_ALIASES)
}

function normalizeDescriptor(raw) {
  if (!raw || typeof raw !== 'object') return null
  const id = String(raw.id ?? '').trim().toLowerCase()
  if (!id) return null
  const aliases = Array.isArray(raw.aliases)
    ? raw.aliases.map((alias) => String(alias ?? '').trim().toLowerCase()).filter(Boolean)
    : []
  if (!aliases.includes(id)) aliases.unshift(id)
  return withoutKeys({
    ...raw,
    id,
    label: String(raw.label ?? id).trim() || id,
    displayName:
      String(raw.displayName ?? raw.display_name ?? raw.label ?? id).trim() || id,
    accent: String(raw.accent ?? 'brand').trim() || 'brand',
    medallionAccent:
      String(raw.medallionAccent ?? raw.medallion_accent ?? raw.accent ?? 'brand').trim() ||
      'brand',
    defaultAgentRoleId: stringOrNull(raw.defaultAgentRoleId ?? raw.default_agent_role_id),
    aliases,
    capabilities: normalizeCapabilities(raw.capabilities),
  }, DESCRIPTOR_ALIASES)
}

export function normalizeToolDescriptors(raw, fallback = FALLBACK_TOOLS) {
  if (!Array.isArray(raw)) return fallback.map((entry) => normalizeDescriptor(entry))
  const normalized = raw.map(normalizeDescriptor).filter(Boolean)
  return normalized.length > 0 ? normalized : fallback.map((entry) => normalizeDescriptor(entry))
}

export function configureToolRegistry(raw) {
  registryState.tools = normalizeToolDescriptors(raw)
  return registryState.tools
}

export function resetToolRegistry() {
  registryState.tools = FALLBACK_TOOLS
}

export function tools() {
  return registryState.tools
}

export function toolDescriptor(value) {
  const normalized = String(value ?? '').trim().toLowerCase()
  return tools().find((entry) => entry.aliases.includes(normalized)) ?? null
}

export function toolOptions() {
  return tools().map((entry) => entry.id)
}

export function defaultToolForRole(kind) {
  const wantsNativeInbox = String(kind ?? '').trim().toLowerCase() === 'lead'
  return (
    tools().find(
      (entry) => entry.capabilities.nativeInboxPoller === wantsNativeInbox
    )?.id ?? fallbackDescriptor()?.id ?? ''
  )
}

function fallbackDescriptor(value = null) {
  return (
    toolDescriptor(value) ??
    tools().find((entry) => entry.capabilities.nativeInboxPoller) ??
    tools()[0] ??
    null
  )
}

export function normalizeTool(value, fallback = null) {
  return toolDescriptor(value)?.id ?? fallbackDescriptor(fallback)?.id ?? ''
}

export function toolLabel(value, fallback = 'Unknown') {
  return toolDescriptor(value)?.label ?? fallback
}

export function toolDisplayName(value, fallback = 'Unknown') {
  return toolDescriptor(value)?.displayName ?? fallback
}

export function toolAccent(value, fallback = 'brand') {
  return toolDescriptor(value)?.accent ?? fallback
}

export function toolMedallionAccent(value, fallback = 'brand') {
  return toolDescriptor(value)?.medallionAccent ?? fallback
}

export function toolCounts(items, readTool) {
  const values = Array.isArray(items) ? items : []
  const counts = Object.fromEntries(tools().map((entry) => [entry.id, 0]))
  for (const item of values) {
    const id = toolDescriptor(readTool(item))?.id
    if (id && Object.hasOwn(counts, id)) counts[id] += 1
  }
  return { all: values.length, ...counts }
}
