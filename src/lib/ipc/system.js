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
  gemini: {
    continue_cmd: 'gemini --yolo --resume',
    fresh: 'gemini --yolo',
    resume: 'gemini --yolo --resume',
  },
}

const EMPTY_MODEL_CATALOG = {
  claude: [],
  codex: [],
  gemini: [],
}

const EMPTY_CLI_VERSIONS = {
  codex: null,
  claude: null,
  codex_compaction_hooks_supported: false,
  codex_notify_supported: false,
  codex_queue_wake_supported: false,
  claude_statusline_usage_supported: false,
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
  return {
    continue_cmd: commands.continue_cmd ?? commands.continueCmd ?? defaults.continue_cmd ?? '',
    fresh: commands.fresh ?? defaults.fresh ?? '',
    resume: commands.resume ?? defaults.resume ?? '',
  }
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
  return {
    id,
    label: String(raw.label ?? id).trim() || id,
    efforts,
    defaultEffort: defaultEffort == null ? null : String(defaultEffort).trim() || null,
    deprecated: Boolean(raw.deprecated),
    replacement: replacement == null ? null : String(replacement).trim() || null,
  }
}

function normalizeModelCatalogEntries(raw, defaults = []) {
  const entries = Array.isArray(raw) ? raw : defaults
  return entries.map((entry) => normalizeModelCatalogEntry(entry)).filter(Boolean)
}

function normalizeModelCatalog(raw, defaults = EMPTY_MODEL_CATALOG) {
  const catalog = raw && typeof raw === 'object' ? raw : defaults
  return {
    claude: normalizeModelCatalogEntries(catalog.claude, defaults.claude),
    codex: normalizeModelCatalogEntries(catalog.codex, defaults.codex),
    gemini: normalizeModelCatalogEntries(catalog.gemini, defaults.gemini),
  }
}

function normalizeCliVersions(raw, defaults = EMPTY_CLI_VERSIONS) {
  const versions = raw && typeof raw === 'object' ? raw : defaults
  return {
    codex: versions.codex == null ? null : String(versions.codex),
    claude: versions.claude == null ? null : String(versions.claude),
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
    claude_statusline_usage_supported: Boolean(
      versions.claude_statusline_usage_supported ??
        versions.claudeStatuslineUsageSupported ??
        defaults.claude_statusline_usage_supported,
    ),
  }
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
      gemini: { ...fallback.cli_command_defaults.gemini },
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

  return {
    platform: defaults.platform,
    default_emulator: contract.default_emulator ?? contract.defaultEmulator ?? defaults.default_emulator,
    supported_emulators: [...supportedEmulators],
    cli_command_defaults: {
      claude: normalizeToolCommands(cliCommandDefaults.claude, defaults.cli_command_defaults.claude),
      codex: normalizeToolCommands(cliCommandDefaults.codex, defaults.cli_command_defaults.codex),
      gemini: normalizeToolCommands(cliCommandDefaults.gemini, defaults.cli_command_defaults.gemini),
    },
    model_catalog: normalizeModelCatalog(modelCatalog, defaults.model_catalog),
    cli_versions: normalizeCliVersions(cliVersions, defaults.cli_versions),
    tools: normalizeToolDescriptors(tools, defaults.tools),
  }
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
  const codexCompaction = harness.codex_compaction ?? harness.codexCompaction
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

  return {
    scan_directories: settings.scan_directories ?? settings.scanDirectories ?? [],
    thresholds: {
      active_days: thresholds.active_days ?? thresholds.activeDays ?? 7,
      recent_days: thresholds.recent_days ?? thresholds.recentDays ?? 30,
      stale_days: thresholds.stale_days ?? thresholds.staleDays ?? 90,
    },
    ignore_patterns: settings.ignore_patterns ?? settings.ignorePatterns ?? [],
    dark_mode: settings.dark_mode ?? settings.darkMode ?? false,
    project_dialog_last_path:
      settings.project_dialog_last_path ?? settings.projectDialogLastPath ?? '',
    code_theme: {
      light: codeTheme.light ?? 'github-light',
      dark: codeTheme.dark ?? 'github-dark-dimmed',
    },
    daemon: {
      port: daemon.port ?? 17233,
      path: daemon.path ?? '',
      auto_start: daemon.auto_start ?? daemon.autoStart ?? true,
    },
    terminal: {
      emulator,
      custom_command: terminal.custom_command ?? terminal.customCommand ?? '',
      tmux_layout: terminal.tmux_layout ?? terminal.tmuxLayout ?? 'new_window',
      cli_commands: {
        claude: normalizeToolCommands(cliCommands.claude, terminalContract.cli_command_defaults.claude),
        codex: normalizeToolCommands(cliCommands.codex, terminalContract.cli_command_defaults.codex),
        gemini: normalizeToolCommands(cliCommands.gemini, terminalContract.cli_command_defaults.gemini),
      },
      harness: {
        codex_compaction: codexCompaction === 'hooks' ? 'hooks' : 'transcript',
      },
      claude_default_account_id:
        terminal.claude_default_account_id ?? terminal.claudeDefaultAccountId ?? null,
    },
    terminal_contract: terminalContract,
  }
}

function normalizeDaemonStatus(raw) {
  const status = raw && typeof raw === 'object' ? raw : {}
  return {
    status: status.status ?? 'disconnected',
    version: status.version ?? null,
    protocol_version: status.protocol_version ?? status.protocolVersion ?? 0,
    expected_protocol_version:
      status.expected_protocol_version ?? status.expectedProtocolVersion ?? 0,
    uptime_secs: status.uptime_secs ?? status.uptimeSecs ?? null,
    port: status.port ?? 17233,
    wsl_distro: status.wsl_distro ?? status.wslDistro ?? null,
  }
}

function normalizeDaemonInstallStatus(raw) {
  const status = raw && typeof raw === 'object' ? raw : {}
  return {
    installed: Boolean(status.installed),
    version: status.version ?? null,
    bundled_version: status.bundled_version ?? status.bundledVersion ?? '',
    needs_update: status.needs_update ?? status.needsUpdate ?? false,
    wsl_available: status.wsl_available ?? status.wslAvailable ?? true,
    error: status.error ?? null,
  }
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

  return {
    installed: Boolean(status.installed),
    version: status.version ?? null,
    bundled_version: status.bundled_version ?? status.bundledVersion ?? '',
    needs_update: status.needs_update ?? status.needsUpdate ?? false,
    bundled_contract: {
      version: bundledContract.version ?? '',
      protocol_version: bundledContract.protocol_version ?? bundledContract.protocolVersion ?? 0,
      schema_version: bundledContract.schema_version ?? bundledContract.schemaVersion ?? 0,
      git_commit: bundledContract.git_commit ?? bundledContract.gitCommit ?? null,
    },
    installed_contract: installedContract
      ? {
          version: installedContract.version ?? '',
          protocol_version:
            installedContract.protocol_version ?? installedContract.protocolVersion ?? 0,
          schema_version:
            installedContract.schema_version ?? installedContract.schemaVersion ?? 0,
          git_commit: installedContract.git_commit ?? installedContract.gitCommit ?? null,
        }
      : null,
    compatibility_issues: compatibilityIssues.map((issue) => ({
      code: issue?.code ?? '',
      message: issue?.message ?? '',
      expected: issue?.expected ?? null,
      actual: issue?.actual ?? null,
    })),
    environment_available:
      status.environment_available ?? status.environmentAvailable ?? true,
    error: status.error ?? null,
  }
}

function normalizeUsageWindow(raw) {
  const window = raw && typeof raw === 'object' ? raw : null
  if (!window) return null
  const used = Number(window.used_percentage ?? window.usedPercentage)
  if (!Number.isFinite(used)) return null
  const resetsAt = Number(window.resets_at ?? window.resetsAt)
  return {
    key: String(window.key ?? ''),
    title: String(window.title ?? ''),
    used_percentage: used,
    resets_at: Number.isFinite(resetsAt) ? resetsAt : null,
    severity: String(window.severity ?? 'normal'),
    is_active: Boolean(window.is_active ?? window.isActive ?? true),
  }
}

function normalizeAccountUsage(raw) {
  const usage = raw && typeof raw === 'object' ? raw : null
  if (!usage) return null
  const windows = Array.isArray(usage.windows)
    ? usage.windows.map(normalizeUsageWindow).filter(Boolean)
    : []
  const observedAt = usage.observed_at ?? usage.observedAt ?? null
  return {
    observed_at: observedAt == null ? null : String(observedAt),
    status: String(usage.status ?? 'ok'),
    windows,
    note: usage.note == null ? null : String(usage.note),
  }
}

function normalizeAccount(raw) {
  const account = raw && typeof raw === 'object' ? raw : {}
  const id = String(account.id ?? '').trim()
  if (!id) return null
  const identity = account.identity && typeof account.identity === 'object' ? account.identity : {}
  const displayName = identity.display_name ?? identity.displayName ?? null
  const plan = identity.plan ?? null
  return {
    tool: String(account.tool ?? ''),
    id,
    dir: String(account.dir ?? ''),
    identity: {
      id: String(identity.id ?? id),
      label: String(identity.label ?? '').trim(),
      display_name: displayName == null ? null : String(displayName).trim() || null,
      organization:
        identity.organization == null ? null : String(identity.organization).trim() || null,
      plan: plan == null ? null : String(plan).trim() || null,
      logged_in: Boolean(identity.logged_in ?? identity.loggedIn),
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
    is_default: Boolean(account.is_default ?? account.isDefault),
    is_process_default: Boolean(account.is_process_default ?? account.isProcessDefault),
    usage: normalizeAccountUsage(account.usage),
  }
}

function normalizeAccountsResult(raw) {
  const result = raw && typeof raw === 'object' && !Array.isArray(raw) ? raw : {}
  const accounts = Array.isArray(result.accounts) ? result.accounts : []
  return {
    accounts: accounts.map(normalizeAccount).filter(Boolean),
    source: String(result.source ?? 'native'),
    degraded: Boolean(result.degraded),
    error: result.error == null ? null : String(result.error),
  }
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
  })).then(normalizeAccountsResult)
}

export function refreshAccountsUsage(tool) {
  return invokeOrMock('refresh_accounts_usage', { tool }, () => false)
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

export function updateSettings(settings) {
  return invokeOrMock('update_settings', { settings }, () => ({
    ...MOCK_SETTINGS,
    ...settings,
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
