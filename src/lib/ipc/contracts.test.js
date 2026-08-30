import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import accountsFixture from './__fixtures__/accounts-result.json'
import liveTeamStatusFixture from './__fixtures__/live-team-status.json'
import settingsFixture from './__fixtures__/settings.json'
import { normalizeLiveTeamStatus } from './coordinationResponses.js'
import { getSettings, listAccounts } from './system.js'

const SETTINGS_RENAMES = {
  scanDirectories: 'scan_directories',
  'thresholds.activeDays': 'active_days',
  'thresholds.recentDays': 'recent_days',
  'thresholds.staleDays': 'stale_days',
  ignorePatterns: 'ignore_patterns',
  darkMode: 'dark_mode',
  projectDialogLastPath: 'project_dialog_last_path',
  codeTheme: 'code_theme',
  'daemon.autoStart': 'auto_start',
  'terminal.customCommand': 'custom_command',
  'terminal.tmuxLayout': 'tmux_layout',
  'terminal.cliCommands': 'cli_commands',
  'terminal.cliCommands.claude.continueCmd': 'continue_cmd',
  'terminal.cliCommands.codex.continueCmd': 'continue_cmd',
  'terminal.cliCommands.agy.continueCmd': 'continue_cmd',
  'terminal.cliCommands.grok.continueCmd': 'continue_cmd',
  'terminal.harness.agyHooks': 'agy_hooks',
  'terminal.harness.grokHooks': 'grok_hooks',
  'terminal.defaultAccountIds': 'default_account_ids',
  terminalContract: 'terminal_contract',
  'terminalContract.defaultEmulator': 'default_emulator',
  'terminalContract.supportedEmulators': 'supported_emulators',
  'terminalContract.cliCommandDefaults': 'cli_command_defaults',
  'terminalContract.cliCommandDefaults.claude.continueCmd': 'continue_cmd',
  'terminalContract.cliCommandDefaults.codex.continueCmd': 'continue_cmd',
  'terminalContract.cliCommandDefaults.agy.continueCmd': 'continue_cmd',
  'terminalContract.cliCommandDefaults.grok.continueCmd': 'continue_cmd',
  'terminalContract.modelCatalog': 'model_catalog',
  'terminalContract.cliVersions': 'cli_versions',
  'terminalContract.cliVersions.codexCompactionHooksSupported':
    'codex_compaction_hooks_supported',
  'terminalContract.cliVersions.codexNotifySupported': 'codex_notify_supported',
  'terminalContract.cliVersions.codexQueueWakeSupported': 'codex_queue_wake_supported',
  'terminalContract.cliVersions.agyHooksSupported': 'agy_hooks_supported',
}

const ACCOUNTS_RENAMES = {
  'accounts[].identity.displayName': 'display_name',
  'accounts[].identity.loggedIn': 'logged_in',
  'accounts[].identity.usageCapable': 'usage_capable',
  'accounts[].identity.credentialExpiresAt': 'credential_expires_at',
  'accounts[].isDefault': 'is_default',
  'accounts[].isProcessDefault': 'is_process_default',
  'accounts[].usage.observedAt': 'observed_at',
  'accounts[].usage.windows[].usedPercentage': 'used_percentage',
  'accounts[].usage.windows[].resetsAt': 'resets_at',
  'accounts[].usage.windows[].isActive': 'is_active',
}

function assertEveryFixtureKeySurvives(source, normalized, renames, path = '') {
  if (Array.isArray(source)) {
    expect(normalized, `${path || '<root>'} must remain an array`).toBeInstanceOf(Array)
    expect(normalized, `${path || '<root>'} array length`).toHaveLength(source.length)
    source.forEach((entry, index) => {
      assertEveryFixtureKeySurvives(entry, normalized[index], renames, `${path}[]`)
    })
    return
  }

  if (source && typeof source === 'object') {
    expect(normalized, `${path || '<root>'} must remain an object`).toBeTypeOf('object')
    for (const [key, value] of Object.entries(source)) {
      const sourcePath = path ? `${path}.${key}` : key
      const normalizedKey = renames[sourcePath] ?? key
      expect(
        Object.hasOwn(normalized, normalizedKey),
        `${sourcePath} must survive as ${normalizedKey}`,
      ).toBe(true)
      assertEveryFixtureKeySurvives(
        value,
        normalized[normalizedKey],
        renames,
        sourcePath,
      )
    }
    return
  }

  expect(normalized, `${path} value`).toEqual(source)
}

describe('Rust IPC fixture contracts', () => {
  beforeEach(() => {
    window.__TAURI_INTERNALS__ = {}
    vi.mocked(invoke).mockReset()
  })

  afterEach(() => {
    delete window.__TAURI_INTERNALS__
  })

  it('preserves every Settings and TerminalSettings fixture key', async () => {
    vi.mocked(invoke).mockResolvedValue(settingsFixture)
    assertEveryFixtureKeySurvives(
      settingsFixture,
      await getSettings(),
      SETTINGS_RENAMES,
    )
  })

  it('preserves every live-team member fixture key', () => {
    assertEveryFixtureKeySurvives(
      liveTeamStatusFixture,
      normalizeLiveTeamStatus(liveTeamStatusFixture),
      {},
    )
  })

  it('preserves every accounts-result fixture key', async () => {
    vi.mocked(invoke).mockResolvedValue(accountsFixture)
    assertEveryFixtureKeySurvives(
      accountsFixture,
      await listAccounts('codex'),
      ACCOUNTS_RENAMES,
    )
  })
})
