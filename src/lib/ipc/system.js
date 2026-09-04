import {
  MOCK_DETAIL,
  MOCK_PROJECTS,
  MOCK_SEARCH_RESULTS,
  MOCK_SETTINGS,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'
import { FALLBACK_TOOLS, normalizeToolDescriptors } from '../toolRegistry.js'

const ALLOWED_EXTERNAL_PROTOCOLS = new Set(['https:', 'mailto:'])

const DEFAULT_CLI_COMMANDS = {
  claude: {
    continue_cmd: 'claude --dangerously-skip-permissions --continue',
    fresh: 'claude --dangerously-skip-permissions',
    resume: 'claude --dangerously-skip-permissions --resume',
  },
  codex: {
    continue_cmd: 'codex --yolo',
    fresh: 'codex --yolo',
    resume: 'codex resume --last --yolo',
  },
  agy: {
    continue_cmd: 'agy --dangerously-skip-permissions --continue',
    fresh: 'agy --dangerously-skip-permissions',
    resume: 'agy --dangerously-skip-permissions --conversation {session_id}',
  },
  grok: {
    continue_cmd: 'grok --always-approve --continue',
    fresh: 'grok --always-approve',
    resume: 'grok --always-approve --resume {session_id}',
  },
}

const EMPTY_MODEL_CATALOG = {
  claude: [],
  codex: [],
  agy: [],
  grok: [],
}

const EMPTY_CLI_VERSIONS = {
  codex: null,
  claude: null,
  agy: null,
  codex_compaction_hooks_supported: false,
  codex_notify_supported: false,
  codex_queue_wake_supported: false,
  agy_hooks_supported: false,
}

const DEFAULT_TERMINAL_CONTRACTS = {
  linux: {
    platform: 'linux',
    default_emulator: 'manual',
    supported_emulators: ['manual'],
    cli_command_defaults: DEFAULT_CLI_COMMANDS,
    model_catalog: EMPTY_MODEL_CATALOG,
    cli_versions: EMPTY_CLI_VERSIONS,
    tools: FALLBACK_TOOLS,
  },
  macos: {
    platform: 'macos',
    default_emulator: 'iterm2',
    supported_emulators: ['iterm2', 'ghostty', 'terminal_app', 'custom'],
    cli_command_defaults: DEFAULT_CLI_COMMANDS,
    model_catalog: EMPTY_MODEL_CATALOG,
    cli_versions: EMPTY_CLI_VERSIONS,
    tools: FALLBACK_TOOLS,
  },
  windows: {
    platform: 'windows',
    default_emulator: 'windows_terminal',
    supported_emulators: ['windows_terminal', 'custom'],
    cli_command_defaults: DEFAULT_CLI_COMMANDS,
    model_catalog: EMPTY_MODEL_CATALOG,
    cli_versions: EMPTY_CLI_VERSIONS,
    tools: FALLBACK_TOOLS,
  },
}

function normalizeToolCommands(raw, defaults = {}) {
  const commands = raw && typeof raw === 'object' ? raw : {}
  const normalized = {
    ...commands,
    continue_cmd: commands.continue_cmd ?? commands.continueCmd ?? defaults.continue_cmd ?? '',
    fresh: commands.fresh ?? defaults.fresh ?? '',
    resume: commands.resume ?? defaults.resume ?? '',
  }
  delete normalized.continueCmd
  return normalized
}

function normalizeModelCatalogEntry(raw) {
  if (!raw || typeof raw !== 'object') return null
  const id = String(raw.id ?? '').trim()
  if (!id) return null
  const efforts = Array.isArray(raw.efforts)
    ? raw.efforts.map((effort) => String(effort ?? '').trim()).filter(Boolean)
    : []
  const defaultEffort = raw.defaultEffort ?? raw.default_effort ?? null
  const replacement = raw.replacement ?? null
  const capabilityTier = raw.capabilityTier ?? raw.capability_tier ?? null
  const tierRank = raw.tierRank ?? raw.tier_rank ?? null
  const normalized = {
    ...raw,
    id,
    label: String(raw.label ?? id).trim() || id,
    efforts,
    defaultEffort: defaultEffort == null ? null : String(defaultEffort).trim() || null,
    deprecated: Boolean(raw.deprecated),
    replacement: replacement == null ? null : String(replacement).trim() || null,
    capabilityTier: capabilityTier == null ? null : String(capabilityTier).trim() || null,
    tierRank: Number.isInteger(tierRank) && tierRank >= 0 ? tierRank : null,
  }
  delete normalized.default_effort
  delete normalized.capability_tier
  delete normalized.tier_rank
  return normalized
}

function normalizeModelCatalogEntries(raw, defaults = []) {
  const entries = Array.isArray(raw) ? raw : defaults
  return entries.map((entry) => normalizeModelCatalogEntry(entry)).filter(Boolean)
}

function normalizeModelCatalog(raw, defaults = EMPTY_MODEL_CATALOG) {
  const catalog = raw && typeof raw === 'object' ? raw : defaults
  return {
    ...catalog,
    claude: normalizeModelCatalogEntries(catalog.claude, defaults.claude),
    codex: normalizeModelCatalogEntries(catalog.codex, defaults.codex),
    agy: normalizeModelCatalogEntries(catalog.agy, defaults.agy),
    grok: normalizeModelCatalogEntries(catalog.grok, defaults.grok),
  }
}

function normalizeCliVersions(raw, defaults = EMPTY_CLI_VERSIONS) {
  const versions = raw && typeof raw === 'object' ? raw : defaults
  const normalized = {
    ...versions,
    codex: versions.codex == null ? null : String(versions.codex),
    claude: versions.claude == null ? null : String(versions.claude),
    agy: versions.agy == null ? null : String(versions.agy),
    codex_compaction_hooks_supported: Boolean(
      versions.codex_compaction_hooks_supported ??
        versions.codexCompactionHooksSupported ??
        defaults.codex_compaction_hooks_supported,
    ),
    codex_notify_supported: Boolean(
      versions.codex_notify_supported ??
        versions.codexNotifySupported ??
        defaults.codex_notify_supported,
    ),
    codex_queue_wake_supported: Boolean(
      versions.codex_queue_wake_supported ??
        versions.codexQueueWakeSupported ??
        defaults.codex_queue_wake_supported,
    ),
    agy_hooks_supported: Boolean(
      versions.agy_hooks_supported ?? versions.agyHooksSupported ?? defaults.agy_hooks_supported,
    ),
  }
  delete normalized.codexCompactionHooksSupported
  delete normalized.codexNotifySupported
  delete normalized.codexQueueWakeSupported
  delete normalized.agyHooksSupported
  return normalized
}

export function buildFrontendFallbackTerminalContract(platform = 'linux') {
  const fallback = DEFAULT_TERMINAL_CONTRACTS[platform] ?? DEFAULT_TERMINAL_CONTRACTS.linux
  return {
    platform: fallback.platform,
    default_emulator: fallback.default_emulator,
    supported_emulators: [...fallback.supported_emulators],
    cli_command_defaults: {
      claude: { ...fallback.cli_command_defaults.claude },
      codex: { ...fallback.cli_command_defaults.codex },
      agy: { ...fallback.cli_command_defaults.agy },
      grok: { ...fallback.cli_command_defaults.grok },
    },
    model_catalog: normalizeModelCatalog(fallback.model_catalog),
    cli_versions: normalizeCliVersions(fallback.cli_versions),
    tools: normalizeToolDescriptors(fallback.tools),
  }
}

function normalizeTerminalContract(raw) {
  const contract = raw && typeof raw === 'object' ? raw : {}
  const platform = contract.platform ?? 'linux'
  const defaults = buildFrontendFallbackTerminalContract(platform)
  const supportedEmulators = Array.isArray(contract.supported_emulators)
    ? contract.supported_emulators
    : Array.isArray(contract.supportedEmulators)
      ? contract.supportedEmulators
      : defaults.supported_emulators
  const cliCommandDefaults =
    contract.cli_command_defaults && typeof contract.cli_command_defaults === 'object'
      ? contract.cli_command_defaults
      : contract.cliCommandDefaults && typeof contract.cliCommandDefaults === 'object'
        ? contract.cliCommandDefaults
        : defaults.cli_command_defaults
  const modelCatalog =
    contract.model_catalog && typeof contract.model_catalog === 'object'
      ? contract.model_catalog
      : contract.modelCatalog && typeof contract.modelCatalog === 'object'
        ? contract.modelCatalog
        : defaults.model_catalog
  const cliVersions =
    contract.cli_versions && typeof contract.cli_versions === 'object'
      ? contract.cli_versions
      : contract.cliVersions && typeof contract.cliVersions === 'object'
        ? contract.cliVersions
        : defaults.cli_versions
  const tools = Array.isArray(contract.tools) ? contract.tools : defaults.tools

  const normalized = {
    ...contract,
    platform: defaults.platform,
    default_emulator: contract.default_emulator ?? contract.defaultEmulator ?? defaults.default_emulator,
    supported_emulators: [...supportedEmulators],
    cli_command_defaults: {
      ...cliCommandDefaults,
      claude: normalizeToolCommands(cliCommandDefaults.claude, defaults.cli_command_defaults.claude),
      codex: normalizeToolCommands(cliCommandDefaults.codex, defaults.cli_command_defaults.codex),
      agy: normalizeToolCommands(cliCommandDefaults.agy, defaults.cli_command_defaults.agy),
      grok: normalizeToolCommands(cliCommandDefaults.grok, defaults.cli_command_defaults.grok),
    },
    model_catalog: normalizeModelCatalog(modelCatalog, defaults.model_catalog),
    cli_versions: normalizeCliVersions(cliVersions, defaults.cli_versions),
    tools: normalizeToolDescriptors(tools, defaults.tools),
  }
  delete normalized.defaultEmulator
  delete normalized.supportedEmulators
  delete normalized.cliCommandDefaults
  delete normalized.modelCatalog
  delete normalized.cliVersions
  return normalized
}

function normalizeSettings(raw) {
  const settings = raw && typeof raw === 'object' ? raw : {}
  const thresholds = settings.thresholds && typeof settings.thresholds === 'object' ? settings.thresholds : {}
  const daemon = settings.daemon && typeof settings.daemon === 'object' ? settings.daemon : {}
  const terminal = settings.terminal && typeof settings.terminal === 'object' ? settings.terminal : {}
  const cliCommands =
    terminal.cli_commands && typeof terminal.cli_commands === 'object'
      ? terminal.cli_commands
      : terminal.cliCommands && typeof terminal.cliCommands === 'object'
        ? terminal.cliCommands
        : {}
  const harness = terminal.harness && typeof terminal.harness === 'object'
    ? terminal.harness
    : {}
  const agyHooks = harness.agy_hooks ?? harness.agyHooks
  const grokHooks = harness.grok_hooks ?? harness.grokHooks
  const terminalContract = normalizeTerminalContract(
    settings.terminal_contract && typeof settings.terminal_contract === 'object'
      ? settings.terminal_contract
      : settings.terminalContract
  )
  const codeTheme =
    settings.code_theme && typeof settings.code_theme === 'object'
      ? settings.code_theme
      : settings.codeTheme && typeof settings.codeTheme === 'object'
        ? settings.codeTheme
        : {}
  const requestedEmulator = terminal.emulator ?? terminalContract.default_emulator
  const emulator = terminalContract.supported_emulators.includes(requestedEmulator)
    ? requestedEmulator
    : terminalContract.default_emulator
  const legacyClaudeDefault =
    terminal.claude_default_account_id ?? terminal.claudeDefaultAccountId ?? null
  const defaultAccountIds =
    terminal.default_account_ids && typeof terminal.default_account_ids === 'object'
      ? terminal.default_account_ids
      : terminal.defaultAccountIds && typeof terminal.defaultAccountIds === 'object'
        ? terminal.defaultAccountIds
        : legacyClaudeDefault
          ? { claude: legacyClaudeDefault }
          : {}

  const normalized = {
    ...settings,
    scan_directories: settings.scan_directories ?? settings.scanDirectories ?? [],
    thresholds: {
      ...thresholds,
      active_days: thresholds.active_days ?? thresholds.activeDays ?? 7,
      recent_days: thresholds.recent_days ?? thresholds.recentDays ?? 30,
      stale_days: thresholds.stale_days ?? thresholds.staleDays ?? 90,
    },
    ignore_patterns: settings.ignore_patterns ?? settings.ignorePatterns ?? [],
    dark_mode: settings.dark_mode ?? settings.darkMode ?? false,
    project_dialog_last_path:
      settings.project_dialog_last_path ?? settings.projectDialogLastPath ?? '',
    code_theme: {
      ...codeTheme,
      light: codeTheme.light ?? 'github-light',
      dark: codeTheme.dark ?? 'github-dark-dimmed',
    },
    daemon: {
      ...daemon,
      port: daemon.port ?? 17233,
      path: daemon.path ?? '',
      auto_start: daemon.auto_start ?? daemon.autoStart ?? true,
    },
    terminal: {
      ...terminal,
      emulator,
      custom_command: terminal.custom_command ?? terminal.customCommand ?? '',
      tmux_layout: terminal.tmux_layout ?? terminal.tmuxLayout ?? 'new_window',
      cli_commands: {
        ...cliCommands,
        claude: normalizeToolCommands(cliCommands.claude, terminalContract.cli_command_defaults.claude),
        codex: normalizeToolCommands(cliCommands.codex, terminalContract.cli_command_defaults.codex),
        agy: normalizeToolCommands(cliCommands.agy, terminalContract.cli_command_defaults.agy),
        grok: normalizeToolCommands(cliCommands.grok, terminalContract.cli_command_defaults.grok),
      },
      harness: {
        ...harness,
        agy_hooks: agyHooks == null ? true : Boolean(agyHooks),
        grok_hooks: grokHooks == null ? true : Boolean(grokHooks),
      },
      default_account_ids: { ...defaultAccountIds },
    },
    terminal_contract: terminalContract,
  }
  delete normalized.scanDirectories
  delete normalized.ignorePatterns
  delete normalized.darkMode
  delete normalized.projectDialogLastPath
  delete normalized.codeTheme
  delete normalized.terminalContract
  delete normalized.thresholds.activeDays
  delete normalized.thresholds.recentDays
  delete normalized.thresholds.staleDays
  delete normalized.daemon.autoStart
  delete normalized.terminal.customCommand
  delete normalized.terminal.tmuxLayout
  delete normalized.terminal.cliCommands
  delete normalized.terminal.defaultAccountIds
  delete normalized.terminal.claudeDefaultAccountId
  delete normalized.terminal.claude_default_account_id
  delete normalized.terminal.harness.agyHooks
  delete normalized.terminal.harness.grokHooks
  return normalized
}

function normalizeDaemonStatus(raw) {
  const status = raw && typeof raw === 'object' ? raw : {}
  const normalized = {
    ...status,
    status: status.status ?? 'disconnected',
    version: status.version ?? null,
    protocol_version: status.protocol_version ?? status.protocolVersion ?? 0,
    expected_protocol_version:
      status.expected_protocol_version ?? status.expectedProtocolVersion ?? 0,
    uptime_secs: status.uptime_secs ?? status.uptimeSecs ?? null,
    port: status.port ?? 17233,
    wsl_distro: status.wsl_distro ?? status.wslDistro ?? null,
  }
  delete normalized.protocolVersion
  delete normalized.expectedProtocolVersion
  delete normalized.uptimeSecs
  delete normalized.wslDistro
  return normalized
}

function normalizeDaemonInstallStatus(raw) {
  const status = raw && typeof raw === 'object' ? raw : {}
  const normalized = {
    ...status,
    installed: Boolean(status.installed),
    version: status.version ?? null,
    bundled_version: status.bundled_version ?? status.bundledVersion ?? '',
    needs_update: status.needs_update ?? status.needsUpdate ?? false,
    wsl_available: status.wsl_available ?? status.wslAvailable ?? true,
    error: status.error ?? null,
  }
  delete normalized.bundledVersion
  delete normalized.needsUpdate
  delete normalized.wslAvailable
  return normalized
}

function normalizeMeshInstallStatus(raw) {
  const status = raw && typeof raw === 'object' ? raw : {}
  const bundledContract =
    status.bundled_contract && typeof status.bundled_contract === 'object'
      ? status.bundled_contract
      : status.bundledContract && typeof status.bundledContract === 'object'
        ? status.bundledContract
        : {}
  const installedContract =
    status.installed_contract && typeof status.installed_contract === 'object'
      ? status.installed_contract
      : status.installedContract && typeof status.installedContract === 'object'
        ? status.installedContract
        : null
  const compatibilityIssues = Array.isArray(status.compatibility_issues)
    ? status.compatibility_issues
    : Array.isArray(status.compatibilityIssues)
      ? status.compatibilityIssues
      : []

  const normalized = {
    ...status,
    installed: Boolean(status.installed),
    version: status.version ?? null,
    bundled_version: status.bundled_version ?? status.bundledVersion ?? '',
    needs_update: status.needs_update ?? status.needsUpdate ?? false,
    bundled_contract: {
      ...bundledContract,
      version: bundledContract.version ?? '',
      protocol_version: bundledContract.protocol_version ?? bundledContract.protocolVersion ?? 0,
      schema_version: bundledContract.schema_version ?? bundledContract.schemaVersion ?? 0,
      git_commit: bundledContract.git_commit ?? bundledContract.gitCommit ?? null,
    },
    installed_contract: installedContract
      ? {
          ...installedContract,
          version: installedContract.version ?? '',
          protocol_version:
            installedContract.protocol_version ?? installedContract.protocolVersion ?? 0,
          schema_version:
            installedContract.schema_version ?? installedContract.schemaVersion ?? 0,
          git_commit: installedContract.git_commit ?? installedContract.gitCommit ?? null,
        }
      : null,
    compatibility_issues: compatibilityIssues.map((issue) => ({
      ...(issue && typeof issue === 'object' ? issue : {}),
      code: issue?.code ?? '',
      message: issue?.message ?? '',
      expected: issue?.expected ?? null,
      actual: issue?.actual ?? null,
    })),
    environment_available:
      status.environment_available ?? status.environmentAvailable ?? true,
    error: status.error ?? null,
  }
  delete normalized.bundledVersion
  delete normalized.needsUpdate
  delete normalized.bundledContract
  delete normalized.installedContract
  delete normalized.compatibilityIssues
  delete normalized.environmentAvailable
  delete normalized.bundled_contract.protocolVersion
  delete normalized.bundled_contract.schemaVersion
  delete normalized.bundled_contract.gitCommit
  if (normalized.installed_contract) {
    delete normalized.installed_contract.protocolVersion
    delete normalized.installed_contract.schemaVersion
    delete normalized.installed_contract.gitCommit
  }
  return normalized
}

function normalizeUsageWindow(raw) {
  const window = raw && typeof raw === 'object' ? raw : null
  if (!window) return null
  const used = Number(window.used_percentage ?? window.usedPercentage)
  if (!Number.isFinite(used)) return null
  const resetsAt = Number(window.resets_at ?? window.resetsAt)
  const normalized = {
    ...window,
    key: String(window.key ?? ''),
    title: String(window.title ?? ''),
    used_percentage: used,
    resets_at: Number.isFinite(resetsAt) ? resetsAt : null,
    severity: String(window.severity ?? 'normal'),
    is_active: Boolean(window.is_active ?? window.isActive ?? true),
    ...(window.compact == null ? {} : { compact: Boolean(window.compact) }),
  }
  delete normalized.usedPercentage
  delete normalized.resetsAt
  delete normalized.isActive
  return normalized
}

function normalizeAccountUsage(raw) {
  const usage = raw && typeof raw === 'object' ? raw : null
  if (!usage) return null
  const windows = Array.isArray(usage.windows)
    ? usage.windows.map(normalizeUsageWindow).filter(Boolean)
    : []
  const observedAt = usage.observed_at ?? usage.observedAt ?? null
  const normalized = {
    ...usage,
    observed_at: observedAt == null ? null : String(observedAt),
    status: String(usage.status ?? 'ok'),
    windows,
    note: usage.note == null ? null : String(usage.note),
  }
  delete normalized.observedAt
  return normalized
}

function normalizeAccount(raw) {
  const account = raw && typeof raw === 'object' ? raw : {}
  const id = String(account.id ?? '').trim()
  if (!id) return null
  const identity = account.identity && typeof account.identity === 'object' ? account.identity : {}
  const displayName = identity.display_name ?? identity.displayName ?? null
  const plan = identity.plan ?? null
  const usageCapable = identity.usage_capable ?? identity.usageCapable ?? true
  const normalized = {
    ...account,
    tool: String(account.tool ?? ''),
    id,
    dir: String(account.dir ?? ''),
    identity: {
      ...identity,
      id: String(identity.id ?? id),
      label: String(identity.label ?? '').trim(),
      display_name: displayName == null ? null : String(displayName).trim() || null,
      organization:
        identity.organization == null ? null : String(identity.organization).trim() || null,
      plan: plan == null ? null : String(plan).trim() || null,
      logged_in: Boolean(identity.logged_in ?? identity.loggedIn),
      usage_capable: Boolean(usageCapable),
      credential_expires_at:
        identity.credential_expires_at ?? identity.credentialExpiresAt ?? null,
    },
    // Flat aliases keep generic rendering terse and ease the 0.6.8 UI migration.
    label: String(identity.label ?? '').trim(),
    display_name: displayName == null ? null : String(displayName).trim() || null,
    organization:
      identity.organization == null ? null : String(identity.organization).trim() || null,
    plan: plan == null ? null : String(plan).trim() || null,
    logged_in: Boolean(identity.logged_in ?? identity.loggedIn),
    usage_capable: Boolean(usageCapable),
    is_default: Boolean(account.is_default ?? account.isDefault),
    is_process_default: Boolean(account.is_process_default ?? account.isProcessDefault),
    usage: normalizeAccountUsage(account.usage),
  }
  delete normalized.isDefault
  delete normalized.isProcessDefault
  delete normalized.identity.displayName
  delete normalized.identity.loggedIn
  delete normalized.identity.usageCapable
  delete normalized.identity.credentialExpiresAt
  return normalized
}

/**
 * One configured launch command as the pane's own shell reads it: an alias
 * expanded, and a head that is not the CLI named rather than guessed at.
 */
function normalizeResolvedBase(raw) {
  const base = raw && typeof raw === 'object' ? raw : {}
  const command = String(base.command ?? '')
  if (!command) return null
  const expansions = Array.isArray(base.expansions) ? base.expansions : []
  const opaqueHead = base.opaqueHead ?? base.opaque_head ?? null
  const selectorValue = base.selectorValue ?? base.selector_value ?? null
  const normalized = {
    ...base,
    command,
    expansions: expansions
      .map((expansion) => ({
        ...(expansion && typeof expansion === 'object' ? expansion : {}),
        name: String(expansion?.name ?? ''),
        body: String(expansion?.body ?? ''),
      }))
      .filter((expansion) => expansion.name),
    opaqueHead: opaqueHead == null ? null : String(opaqueHead),
    selectorValue: selectorValue == null ? null : String(selectorValue),
    modes: Array.isArray(base.modes) ? base.modes.map(String) : [],
  }
  delete normalized.opaque_head
  delete normalized.selector_value
  return normalized
}

function normalizeAccountsResult(raw) {
  const result = raw && typeof raw === 'object' && !Array.isArray(raw) ? raw : {}
  const accounts = Array.isArray(result.accounts) ? result.accounts : []
  const resolvedBases = result.resolvedBases ?? result.resolved_bases
  const normalized = {
    ...result,
    accounts: accounts.map(normalizeAccount).filter(Boolean),
    source: String(result.source ?? 'native'),
    degraded: Boolean(result.degraded),
    error: result.error == null ? null : String(result.error),
    // A backend that reports none — an older one, or a caller that did not
    // ask — contributes nothing to the effective-default line; Settings shows
    // "resolving…" while a resolution is in flight.
    resolvedBases: (Array.isArray(resolvedBases) ? resolvedBases : [])
      .map(normalizeResolvedBase)
      .filter(Boolean),
  }
  delete normalized.resolved_bases
  return normalized
}

/**
 * Claude subscriptions detected on this host (in-process on Linux/macOS, via
 * the WSL daemon on Windows), and whether detection ran at all.
 *
 * An empty list with `degraded: false` is an answer — no accounts, or a daemon
 * too old to know about them, and every launch behaves as it did before
 * per-project accounts existed. An empty list with `degraded: true` is silence:
 * nothing answered, and callers keep whatever they last knew.
 */
export function listAccounts(tool) {
  return invokeOrMock('list_accounts', { tool }, () => ({
    accounts: [],
    source: 'native',
    degraded: false,
    error: null,
    resolvedBases: [],
  })).then(normalizeAccountsResult)
}

/** Resolve configured launch commands where the pane shell runs. */
export function resolveLaunchBases(tool, force = false) {
  return invokeOrMock('resolve_launch_bases', { tool, force }, () => []).then((bases) =>
    (Array.isArray(bases) ? bases : []).map(normalizeResolvedBase).filter(Boolean)
  )
}

export function refreshAccountsUsage(tool) {
  return invokeOrMock('refresh_accounts_usage', { tool }, () => false)
}

function normalizeAccountProjectRelationship(raw) {
  const relationship = raw && typeof raw === 'object' ? raw : {}
  return {
    ...relationship,
    id: String(relationship.id ?? ''),
    name: String(relationship.name ?? ''),
    path: String(relationship.path ?? ''),
    updatedAt: relationship.updatedAt ?? relationship.updated_at ?? null,
  }
}

function normalizeAccountRelationships(raw) {
  const relationships = raw && typeof raw === 'object' ? raw : {}
  const projects = (camel, snake) =>
    (Array.isArray(relationships[camel] ?? relationships[snake])
      ? relationships[camel] ?? relationships[snake]
      : []
    ).map(normalizeAccountProjectRelationship)
  const teams = Array.isArray(relationships.teams) ? relationships.teams : []
  return {
    pinnedProjects: projects('pinnedProjects', 'pinned_projects'),
    lastUsedProjects: projects('lastUsedProjects', 'last_used_projects'),
    teams: teams.map((team) => ({
      ...(team && typeof team === 'object' ? team : {}),
      name: String(team?.name ?? ''),
      projectId: team?.projectId ?? team?.project_id ?? null,
      projectName: team?.projectName ?? team?.project_name ?? null,
      projectPath: team?.projectPath ?? team?.project_path ?? null,
    })),
  }
}

export function listAccountRelationships(tool) {
  return invokeOrMock('list_account_relationships', { tool }, () => ({ byAccount: {} })).then(
    (raw) => {
      const source = raw?.byAccount ?? raw?.by_account ?? {}
      return {
        byAccount: Object.fromEntries(
          Object.entries(source).map(([accountId, relationships]) => [
            accountId,
            normalizeAccountRelationships(relationships),
          ])
        ),
      }
    }
  )
}

export function setGlobalDefaultAccount(tool, accountId) {
  return invokeOrMock(
    'set_global_default_account',
    { tool, accountId: accountId ?? null },
    () => undefined
  )
}

export function prepareAccountDirectory(tool, label) {
  return invokeOrMock('prepare_account_directory', { tool, label }, () => `/tmp/${tool}-${label}`)
}

export function launchAccountLogin(projectId, tool, configDir) {
  return invokeOrMock(
    'launch_account_login',
    { projectId, tool, configDir },
    () => ({ tmux_session: 'taurhaus', tmux_window: 'accounts', tmux_pane: '%98' })
  )
}

export function revealDirectory(path) {
  return invokeOrMock('account_directory_host_path', { path }, () => path).then((revealPath) => {
    return invokeOrMock('plugin:opener|reveal_item_in_dir', { path: revealPath }, () => undefined)
  })
}

export function search(query, limit = 20) {
  return invokeOrMock('search', { query, limit }, () => {
    if (!query || !query.trim()) {
      return []
    }

    const needle = query.toLowerCase()
    return MOCK_SEARCH_RESULTS.filter((result) =>
      result.title.toLowerCase().includes(needle) || result.snippet.toLowerCase().includes(needle)
    )
  })
}

export function getIndexStatus() {
  return invokeOrMock('get_index_status', undefined, () => ({
    doc_count: 42,
    is_empty: false,
  }))
}

export function rebuildIndex() {
  return invokeOrMock('rebuild_index', undefined, () => 42)
}

export function getSettings() {
  return invokeOrMock('get_settings', undefined, () => MOCK_SETTINGS).then(normalizeSettings)
}

// The terminal contract is backend-owned runtime state: attached fresh on every
// read, replaced wholesale on save, and the normalized frontend copy no longer
// matches the backend's wire spelling — sending it back can only break the save.
export function settingsUpdatePayload(settings) {
  if (!settings || typeof settings !== 'object') return settings
  const { terminal_contract: _omitted, ...payload } = settings
  return payload
}

export function updateSettings(settings) {
  const payload = settingsUpdatePayload(settings)
  return invokeOrMock('update_settings', { settings: payload }, () => ({
    ...MOCK_SETTINGS,
    ...payload,
  })).then(normalizeSettings)
}

export function openExternalUrl(url) {
  const trimmedUrl = String(url ?? '').trim()
  let parsedUrl

  try {
    parsedUrl = new URL(trimmedUrl)
  } catch {
    return Promise.reject(new Error('Invalid external URL'))
  }

  if (!ALLOWED_EXTERNAL_PROTOCOLS.has(parsedUrl.protocol)) {
    return Promise.reject(new Error('Only HTTPS and mailto links can be opened externally.'))
  }

  return invokeOrMock('plugin:opener|open_url', { url: trimmedUrl }, () => {
    window.open(trimmedUrl, '_blank')
  })
}

export function isFirstRun() {
  return invokeOrMock('is_first_run', undefined, () => MOCK_PROJECTS.length === 0)
}

export function registerProjectsBatch(paths) {
  return invokeOrMock('register_projects_batch', { paths }, () =>
    paths.map((path, index) => ({
      path,
      success: true,
      project: {
        ...MOCK_DETAIL,
        id: `mock-batch-${index}`,
        path,
        name: path.split('/').pop(),
      },
      error: null,
    }))
  )
}

export function getDaemonStatus() {
  return invokeOrMock(
    'get_daemon_status',
    undefined,
    () => ({
      status: 'connected',
      version: null,
      protocol_version: 0,
      expected_protocol_version: 0,
      uptime_secs: null,
      port: 17233,
      wsl_distro: null,
    })
  ).then(normalizeDaemonStatus)
}

export function getPlatform() {
  return invokeOrMock('get_platform', undefined, () => 'linux')
}

export function startDaemon() {
  return invokeOrMock('start_daemon', undefined, () => 'Daemon started')
}

export function checkDaemonInstallStatus() {
  return invokeOrMock(
    'check_daemon_install_status',
    undefined,
    () => ({
      installed: true,
      version: '0.3.1',
      bundled_version: '0.3.1',
      needs_update: false,
      wsl_available: true,
      error: null,
    })
  ).then(normalizeDaemonInstallStatus)
}

export function installDaemon() {
  return invokeOrMock('install_daemon', undefined, () => ({
    success: true,
    message: 'Daemon installed successfully: taurhaus-daemon 0.3.1',
  }))
}

export function checkMeshInstallStatus() {
  return invokeOrMock(
    'check_mesh_install_status',
    undefined,
    () => ({
      installed: true,
      version: '0.1.0',
      bundled_version: '0.1.0',
      needs_update: false,
      bundled_contract: {
        version: '0.1.0',
        protocol_version: 1,
        schema_version: 1,
        git_commit: 'mock-mesh-commit',
      },
      installed_contract: {
        version: '0.1.0',
        protocol_version: 1,
        schema_version: 1,
        git_commit: 'mock-mesh-commit',
      },
      compatibility_issues: [],
      environment_available: true,
      error: null,
    })
  ).then(normalizeMeshInstallStatus)
}

export function installMesh() {
  return invokeOrMock('install_mesh', undefined, () => ({
    success: true,
    message: 'Mesh installed successfully: mesh 0.1.0',
  }))
}
