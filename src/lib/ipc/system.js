import {
  MOCK_DETAIL,
  MOCK_PROJECTS,
  MOCK_SEARCH_RESULTS,
  MOCK_SETTINGS,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'

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

const DEFAULT_TERMINAL_CONTRACTS = {
  linux: {
    platform: 'linux',
    default_emulator: 'manual',
    supported_emulators: ['manual'],
    cli_command_defaults: DEFAULT_CLI_COMMANDS,
  },
  macos: {
    platform: 'macos',
    default_emulator: 'iterm2',
    supported_emulators: ['iterm2', 'ghostty', 'terminal_app', 'custom'],
    cli_command_defaults: DEFAULT_CLI_COMMANDS,
  },
  windows: {
    platform: 'windows',
    default_emulator: 'windows_terminal',
    supported_emulators: ['windows_terminal', 'custom'],
    cli_command_defaults: DEFAULT_CLI_COMMANDS,
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

function getDefaultTerminalContract(platform = 'linux') {
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
  }
}

function normalizeTerminalContract(raw) {
  const contract = raw && typeof raw === 'object' ? raw : {}
  const platform = contract.platform ?? 'linux'
  const defaults = getDefaultTerminalContract(platform)
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

  return {
    platform: defaults.platform,
    default_emulator: contract.default_emulator ?? contract.defaultEmulator ?? defaults.default_emulator,
    supported_emulators: [...supportedEmulators],
    cli_command_defaults: {
      claude: normalizeToolCommands(cliCommandDefaults.claude, defaults.cli_command_defaults.claude),
      codex: normalizeToolCommands(cliCommandDefaults.codex, defaults.cli_command_defaults.codex),
      gemini: normalizeToolCommands(cliCommandDefaults.gemini, defaults.cli_command_defaults.gemini),
    },
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
