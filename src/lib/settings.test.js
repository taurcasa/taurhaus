/**
 * Settings component tests.
 *
 * Tests section structure (General, Display, Terminal & Sessions, Search),
 * terminal emulator/tmux preferences, code theme selection, threshold editing,
 * and index management.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

// Mock IPC module
vi.mock('./ipc.js', () => ({
  refreshAccountsUsage: vi.fn(() => Promise.resolve(true)),
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
  getIndexStatus: vi.fn(),
  rebuildIndex: vi.fn(),
  getPlatform: vi.fn(),
  listAccounts: vi.fn(() =>
    Promise.resolve({ accounts: [], source: 'native', degraded: false, error: null })
  ),
  setProjectAccount: vi.fn(() => Promise.resolve()),
  launchCliSession: vi.fn(() => Promise.resolve()),
  resolveLaunchAccount: vi.fn(() => Promise.resolve({ needsChoice: true })),
  resolveLaunchBases: vi.fn(() => Promise.resolve([])),
}))

const {
  refreshAccountsUsage,
  getSettings,
  updateSettings,
  getIndexStatus,
  rebuildIndex,
  getPlatform,
  listAccounts,
  launchCliSession,
  resolveLaunchBases,
} = await import('./ipc.js')
const { resetAccountsForTest } = await import('./accounts.svelte.js')

import Settings from './Settings.svelte'
import { TOOL_ICONS } from './toolLogos.js'

function mockCliCommandDefaults() {
  return {
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
}

function mockTerminalContract(platform = 'windows') {
  switch (platform) {
    case 'macos':
      return {
        platform: 'macos',
        default_emulator: 'iterm2',
        supported_emulators: ['iterm2', 'ghostty', 'terminal_app', 'custom'],
        cli_command_defaults: mockCliCommandDefaults(),
      }
    case 'linux':
      return {
        platform: 'linux',
        default_emulator: 'manual',
        supported_emulators: ['manual'],
        cli_command_defaults: mockCliCommandDefaults(),
      }
    default:
      return {
        platform: 'windows',
        default_emulator: 'windows_terminal',
        supported_emulators: ['windows_terminal', 'custom'],
        cli_command_defaults: mockCliCommandDefaults(),
      }
  }
}

/** Default mock settings matching the full schema. */
function mockSettings(overrides = {}) {
  const platform = overrides.terminal_contract?.platform ?? 'windows'
  const terminalContract = {
    ...mockTerminalContract(platform),
    ...(overrides.terminal_contract ?? {}),
  }
  const terminal = {
    emulator: terminalContract.default_emulator,
    custom_command: '',
    tmux_layout: 'new_window',
    cli_commands: mockCliCommandDefaults(),
    harness: { agy_hooks: true, grok_hooks: true },
    ...(overrides.terminal ?? {}),
  }

  return {
    scan_directories: ['~/projects'],
    thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
    ignore_patterns: ['node_modules', '.git', 'target', 'dist'],
    code_theme: { light: 'github-light', dark: 'github-dark-dimmed' },
    terminal,
    terminal_contract: terminalContract,
    ...overrides,
  }
}

/** Default props for Settings. */
function defaultProps(overrides = {}) {
  return {
    dark: false,
    onClose: vi.fn(),
    onSettingsChanged: vi.fn(),
    onCodeThemeChanged: vi.fn(),
    ...overrides,
  }
}

async function settledEffectiveDefault(tool = 'claude') {
  const line = await screen.findByTestId(`effective-default-${tool}`)
  await waitFor(() => expect(line).not.toHaveTextContent('resolving…'))
  return line
}

describe('Settings component', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getSettings.mockResolvedValue(mockSettings())
    getIndexStatus.mockResolvedValue({ doc_count: 42, is_empty: false })
    updateSettings.mockImplementation(async (s) => s)
    getPlatform.mockResolvedValue('windows')
    // The account store is module state shared by the whole app, detection
    // included: without this a test inherits the previous one's answer.
    resetAccountsForTest()
    listAccounts.mockResolvedValue(detected([]))
    launchCliSession.mockResolvedValue({ tmux_pane: '%1' })
    resolveLaunchBases.mockResolvedValue([])
  })

  /** What the backend answers when detection ran. */
  const detected = (accounts) => ({ accounts, source: 'native', degraded: false, error: null })

  const TWO_ACCOUNTS = [
    { id: 'account-1', email: 'a@example.com', display_name: 'A', logged_in: true, is_default: true },
    { id: 'account-2', email: 'b@example.com', display_name: 'B', logged_in: true, is_default: false },
  ]

  it('leaves accounts management to the Accounts home', async () => {
    // Accounts is the one place defaults, pins and usage are managed. Settings
    // keeps the launch commands and points at the home; it neither offers a
    // second default control nor polls usage for meters it no longer paints.
    listAccounts.mockResolvedValue(detected(TWO_ACCOUNTS))
    const onOpenAccounts = vi.fn()
    render(Settings, { props: defaultProps({ onOpenAccounts }) })

    await waitFor(() => expect(screen.getByTestId('settings-accounts')).toBeTruthy())
    expect(screen.queryByTestId('account-default-claude-account-2')).toBeNull()
    expect(refreshAccountsUsage).not.toHaveBeenCalled()

    await fireEvent.click(screen.getByTestId('settings-open-accounts'))
    expect(onOpenAccounts).toHaveBeenCalled()
  })

  it('names Codex among the tools whose launch command decides an account', async () => {
    // Regression: 08c3961 left Codex account selection and usage disabled in
    // the registry, hiding two CODEX_HOME accounts from generic settings. The
    // accounts and their meters live in the home now; what stays here is the
    // launch-command story, and Codex must still have one.
    listAccounts.mockImplementation((tool) =>
      Promise.resolve(
        detected(
          tool === 'codex'
            ? TWO_ACCOUNTS.map((account, index) => ({ ...account, id: `codex-${index + 1}` }))
            : []
        )
      )
    )

    render(Settings, { props: defaultProps() })

    await waitFor(() => expect(screen.getByTestId('settings-accounts-codex')).toBeInTheDocument())
    expect(screen.getByTestId('effective-default-codex')).toHaveTextContent('Effective default:')
  })

  it('gives every account section its registry mark, Grok in graphite', async () => {
    // Regression: commit c1005ec shipped the Grok mark and the graphite accent
    // in the registry but consumed neither in Settings, so the account section
    // the plan asks to carry Grok's identity was label text like any other.
    listAccounts.mockImplementation((tool) =>
      Promise.resolve(
        detected(TWO_ACCOUNTS.map((account, index) => ({ ...account, id: `${tool}-${index + 1}` })))
      )
    )

    render(Settings, { props: defaultProps() })

    const section = await screen.findByTestId('settings-accounts-grok')
    const mark = section.querySelector('[data-testid="tool-mark-grok"]')
    expect(mark).not.toBeNull()
    expect(mark.querySelector('path').getAttribute('d')).toBe(TOOL_ICONS.grok.path)
    expect(mark.getAttribute('class')).toContain('graphite')

    const codex = await screen.findByTestId('settings-accounts-codex')
    expect(codex.querySelector('[data-testid="tool-mark-codex"] path').getAttribute('d')).toBe(
      TOOL_ICONS.codex.path
    )
  })

  it('tints the Grok hook control with the graphite accent, not the app brand', async () => {
    // Regression: commit c1005ec gave the Antigravity toggle its google-blue
    // accent but left the Grok one on the generic brand colour, so the only
    // Grok-specific control in Settings carried no harness identity.
    render(Settings, { props: defaultProps() })

    const toggle = await screen.findByTestId('grok-hooks-toggle')
    expect(toggle.className).toContain('accent-graphite')
    expect(toggle.className).not.toContain('accent-brand')
  })

  /**
   * Detection as it really comes back: the account in the configured config
   * directory carries `is_default`, and it is the process default too.
   */
  const DETECTED_ACCOUNTS = [
    {
      ...TWO_ACCOUNTS[0],
      is_default: true,
      is_process_default: true,
      dir: '/home/mstie/.claude',
    },
    { ...TWO_ACCOUNTS[1], is_default: false, dir: '/home/mstie/.claude-account2' },
  ]

  const withResolvedBases = (bases, accounts = DETECTED_ACCOUNTS) => {
    resolveLaunchBases.mockImplementation((tool) =>
      Promise.resolve(tool === 'claude' ? bases : [])
    )
    return (tool) => Promise.resolve(tool === 'claude' ? detected(accounts) : detected([]))
  }

  // Regression: 0.8.4 / PR #75 made the account-list read path run shell
  // probes. The four 2026-08-30 stalls then timed out every project section at
  // 5 s. Settings is the only read surface that asks the dedicated resolver.
  it('resolves launch bases only for the visible accounts section', async () => {
    listAccounts.mockResolvedValue(detected(TWO_ACCOUNTS))
    let finishResolution
    resolveLaunchBases.mockReturnValue(
      new Promise((resolve) => {
        finishResolution = resolve
      })
    )

    render(Settings, { props: defaultProps() })

    await waitFor(() => expect(resolveLaunchBases).toHaveBeenCalledWith('claude'))
    expect(screen.getByTestId('effective-default-claude')).toHaveTextContent(
      'Effective default: resolving…'
    )

    finishResolution([])
    await waitFor(() =>
      expect(screen.getByTestId('effective-default-claude')).toHaveTextContent(
        'Effective default: A — default config directory'
      )
    )
  })

  // Regression: 0.8.3 derived this line from the literal command, so an alias
  // base like `claude2` showed no selector at all and the sentence claimed the
  // default config directory decided the account. 1c779eb then read the
  // detected `is_default` account first, which detection always sets on the
  // configured directory, so the base command was never reached at all.
  it('names the alias a launch command hides', async () => {
    listAccounts.mockImplementation(
      withResolvedBases([
        {
          command:
            "CLAUDE_CONFIG_DIR='/home/mstie/.claude-account2' claude --dangerously-skip-permissions",
          selectorValue: '/home/mstie/.claude-account2',
          expansions: [
            { name: 'claude2', body: 'CLAUDE_CONFIG_DIR=~/.claude-account2 claude' },
          ],
          opaqueHead: null,
        },
      ])
    )

    render(Settings, { props: defaultProps() })

    const line = await settledEffectiveDefault()
    expect(line).toHaveTextContent('B')
    expect(line).toHaveTextContent(
      'from your launch command "claude2" (alias for CLAUDE_CONFIG_DIR=~/.claude-account2 claude)'
    )
  })

  // Regression: commit 89e73bd left Settings tokenizing resolved command text
  // even though the backend is the authority on leading shell assignments.
  it('renders the launch account from resolved_bases metadata alone', async () => {
    listAccounts.mockImplementation(
      withResolvedBases([
        {
          // Deliberately carries no selector text: selectorValue is the only
          // account fact Settings receives from the resolved base.
          command: 'claude --dangerously-skip-permissions',
          selectorValue: '/home/mstie/.claude-account2',
          expansions: [
            { name: 'claude2', body: 'CLAUDE_CONFIG_DIR=~/.claude-account2 claude' },
          ],
          opaqueHead: null,
        },
      ])
    )

    render(Settings, { props: defaultProps() })

    const line = await settledEffectiveDefault()
    expect(line).toHaveTextContent(
      'Effective default: B — from your launch command "claude2" (alias for CLAUDE_CONFIG_DIR=~/.claude-account2 claude)'
    )
  })

  // Regression: 1c779eb compared a reported tilde value verbatim against
  // absolute account dirs, so an unresolved fallback matched no account.
  it('matches a tilde selector value the backend reports', async () => {
    listAccounts.mockImplementation(
      withResolvedBases([
        {
          command: 'CLAUDE_CONFIG_DIR=~/.claude-account2 claude --dangerously-skip-permissions',
          selectorValue: '~/.claude-account2',
          expansions: [
            { name: 'claude2', body: 'CLAUDE_CONFIG_DIR=~/.claude-account2 claude' },
          ],
          opaqueHead: null,
        },
      ])
    )

    render(Settings, { props: defaultProps() })

    const line = await settledEffectiveDefault()
    expect(line).toHaveTextContent('B')
    expect(line).toHaveTextContent('alias for CLAUDE_CONFIG_DIR=~/.claude-account2 claude')
  })

  // The backend reports the assignment the shared parser found in force; the
  // frontend consumes that fact without re-tokenizing the command.
  it('uses the selector value the backend reports in force', async () => {
    listAccounts.mockImplementation(
      withResolvedBases([
        {
          command:
            "CLAUDE_CONFIG_DIR='/home/mstie/.claude' CLAUDE_CONFIG_DIR='/home/mstie/.claude-account2' claude",
          selectorValue: '/home/mstie/.claude-account2',
          expansions: [
            { name: 'claude2', body: 'CLAUDE_CONFIG_DIR=~/.claude-account2 claude' },
          ],
          opaqueHead: null,
        },
      ])
    )

    render(Settings, { props: defaultProps() })

    const line = await settledEffectiveDefault()
    expect(line).toHaveTextContent('B')
  })

  // Regression: a010581 matched the selector anywhere in the launch command,
  // so an argument that only looks like an assignment — a word after the
  // executable, which a shell hands to the program — was reported as the
  // account the launch command selects. a3afcfe then delivered the tilde
  // spelling of that argument already expanded to an absolute path, which is
  // the spelling this line matches against a detected account.
  it.each([
    'claude --append-system-prompt CLAUDE_CONFIG_DIR=/home/mstie/.claude-account2',
    'claude --append-system-prompt CLAUDE_CONFIG_DIR=~/.claude-account2',
  ])('ignores the selector-shaped argument in %s', async (command) => {
    listAccounts.mockImplementation(
      withResolvedBases([{ command, expansions: [], opaqueHead: null }])
    )

    render(Settings, { props: defaultProps() })

    const line = await settledEffectiveDefault()
    expect(line).toHaveTextContent('Effective default: A — default config directory')
    expect(line).not.toHaveTextContent('from your launch command')
  })

  it('keeps a chosen global default above the launch command', async () => {
    getSettings.mockResolvedValue(
      mockSettings({ terminal: { default_account_ids: { claude: 'account-1' } } })
    )
    listAccounts.mockImplementation(
      withResolvedBases([
        {
          command: "CLAUDE_CONFIG_DIR='/home/mstie/.claude-account2' claude",
          expansions: [
            { name: 'claude2', body: 'CLAUDE_CONFIG_DIR=~/.claude-account2 claude' },
          ],
          opaqueHead: null,
        },
      ])
    )

    render(Settings, { props: defaultProps() })

    const line = await settledEffectiveDefault()
    expect(line).toHaveTextContent('A')
    expect(line).toHaveTextContent('default')
    expect(line).not.toHaveTextContent('alias for')
  })

  it('warns when the launch command does not run the CLI at all', async () => {
    listAccounts.mockImplementation(
      withResolvedBases([
        {
          command: 'my-claude-wrapper --dangerously-skip-permissions',
          expansions: [],
          opaqueHead: 'my-claude-wrapper',
        },
      ])
    )

    render(Settings, { props: defaultProps() })

    const line = await settledEffectiveDefault()
    expect(line).toHaveTextContent(
      'taurhaus could not select an account: your launch command runs "my-claude-wrapper", which is not the Claude CLI'
    )
  })

  // Regression: a chosen global default returned before the opaque-head check,
  // so an operator with both saw "default" where the launch command was in fact
  // a wrapper that decides the account itself. (Codex review, round 6.)
  it('warns about a wrapper even when a global default is chosen', async () => {
    getSettings.mockResolvedValue(
      mockSettings({ terminal: { default_account_ids: { claude: 'account-1' } } })
    )
    listAccounts.mockImplementation(
      withResolvedBases([
        {
          command: 'my-claude-wrapper --dangerously-skip-permissions',
          expansions: [],
          opaqueHead: 'my-claude-wrapper',
        },
      ])
    )

    render(Settings, { props: defaultProps() })

    const line = await settledEffectiveDefault()
    expect(line).toHaveTextContent(
      'taurhaus could not select an account: your launch command runs "my-claude-wrapper", which is not the Claude CLI'
    )
    expect(line).not.toHaveTextContent('— default')
  })

  // Regression: 1c779eb gave this line the backend's resolved bases and left
  // nothing to invalidate them. Saving a launch command only wrote settings, so
  // the line went on describing the command the operator had just replaced —
  // while the next launch resolved and ran the new one.
  it('re-resolves a launch command it has just saved', async () => {
    listAccounts.mockImplementation(
      withResolvedBases([
        {
          command:
            "CLAUDE_CONFIG_DIR='/home/mstie/.claude-account2' claude --dangerously-skip-permissions",
          selectorValue: '/home/mstie/.claude-account2',
          expansions: [
            { name: 'claude2', body: 'CLAUDE_CONFIG_DIR=~/.claude-account2 claude' },
          ],
          opaqueHead: null,
        },
      ])
    )

    render(Settings, { props: defaultProps() })
    const line = await settledEffectiveDefault()
    expect(line).toHaveTextContent(
      'Effective default: B — from your launch command "claude2" (alias for CLAUDE_CONFIG_DIR=~/.claude-account2 claude)'
    )

    // The operator drops the alias for the CLI itself.
    listAccounts.mockImplementation(
      withResolvedBases([
        { command: 'claude --dangerously-skip-permissions', expansions: [], opaqueHead: null },
      ])
    )
    const input = screen.getByTestId('cli-claude-fresh')
    input.value = 'claude --dangerously-skip-permissions'
    await fireEvent.blur(input)

    await waitFor(() =>
      expect(screen.getByTestId('effective-default-claude')).toHaveTextContent(
        'Effective default: A — default config directory'
      )
    )
  })

  it('hides the accounts card when no tool has multiple accounts', async () => {
    // Regression: c11770e exposed account controls to single-account users,
    // contradicting the chooser and overview visibility rule.
    listAccounts.mockResolvedValue(detected([TWO_ACCOUNTS[0]]))
    render(Settings, { props: defaultProps() })

    await waitFor(() => expect(listAccounts).toHaveBeenCalled())
    expect(screen.queryByTestId('settings-accounts')).toBeNull()
  })

  // --- IPC loading ---

  it('calls getSettings on mount', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(getSettings).toHaveBeenCalled()
    })
  })

  it('calls getIndexStatus on mount', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(getIndexStatus).toHaveBeenCalled()
    })
  })

  it('shows loading skeleton initially', () => {
    getSettings.mockReturnValue(new Promise(() => {})) // never resolves
    render(Settings, { props: defaultProps() })
    // Should not show settings sections yet
    expect(screen.queryByTestId('settings-scanning')).toBeNull()
  })

  it('shows error banner when getSettings fails', async () => {
    getSettings.mockRejectedValue(new Error('DB unavailable'))
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('settings-load-error')).toBeTruthy()
      expect(screen.getByTestId('settings-load-error').textContent).toContain('DB unavailable')
    })
  })

  it('uses the shared Windows fallback contract when settings loading fails', async () => {
    getSettings.mockRejectedValue(new Error('DB unavailable'))
    getPlatform.mockResolvedValue('windows')
    render(Settings, { props: defaultProps() })

    await waitFor(() => {
      const select = screen.getByTestId('terminal-emulator')
      const values = Array.from(select.querySelectorAll('option')).map((option) => option.value)
      expect(getPlatform).toHaveBeenCalled()
      expect(values).toEqual(['windows_terminal', 'custom'])
      expect(select).toHaveValue('windows_terminal')
      expect(screen.queryByTestId('terminal-linux-note')).toBeNull()
      expect(screen.queryByTestId('terminal-custom-cmd')).toBeNull()
      expect(screen.getByTestId('cli-claude-continue').placeholder)
        .toBe('claude --dangerously-skip-permissions --continue')
    })
  })

  it('uses the shared macOS fallback contract when settings loading fails', async () => {
    getSettings.mockRejectedValue(new Error('DB unavailable'))
    getPlatform.mockResolvedValue('macos')
    render(Settings, { props: defaultProps() })

    await waitFor(() => {
      const select = screen.getByTestId('terminal-emulator')
      const values = Array.from(select.querySelectorAll('option')).map((option) => option.value)
      expect(values).toEqual(['iterm2', 'ghostty', 'terminal_app', 'custom'])
      expect(select).toHaveValue('iterm2')
      expect(screen.queryByTestId('terminal-linux-note')).toBeNull()
      expect(screen.queryByTestId('terminal-custom-cmd')).toBeNull()
      expect(screen.getByTestId('cli-codex-resume').placeholder)
        .toBe('codex resume --last --yolo')
    })
  })

  it('uses the shared Linux fallback contract when settings loading fails', async () => {
    getSettings.mockRejectedValue(new Error('DB unavailable'))
    getPlatform.mockResolvedValue('linux')
    render(Settings, { props: defaultProps() })

    await waitFor(() => {
      const select = screen.getByTestId('terminal-emulator')
      const values = Array.from(select.querySelectorAll('option')).map((option) => option.value)
      expect(values).toEqual(['manual'])
      expect(select).toHaveValue('manual')
      expect(screen.getByTestId('terminal-linux-note')).toBeTruthy()
      expect(screen.queryByTestId('terminal-custom-cmd')).toBeNull()
      // Regression: 4cd067a changed the registry while the Settings fallback
      // still rendered commands for the retired Google CLI.
      expect(screen.getByTestId('cli-agy-fresh').placeholder).toBe(
        'agy --dangerously-skip-permissions'
      )
    })
  })

  // --- Section structure (P14) ---

  it('does not expose the retired Codex compaction source setting', async () => {
    // Regression: commit 6fe0aa3 exposed a transcript fallback after Codex
    // native hooks became the supported compaction path.
    render(Settings, { props: defaultProps() })
    await screen.findByTestId('settings-mesh')
    expect(screen.queryByTestId('codex-compaction-source')).toBeNull()
  })

  it('has Antigravity native activity hooks on by default and persists opting out', async () => {
    // Regression: 4e9e2c5 defaulted the hooks off while their trust-gated
    // loading was unverified; agy 1.1.22 was then observed firing them.
    render(Settings, { props: defaultProps() })

    const toggle = await screen.findByTestId('agy-hooks-toggle')
    expect(toggle).toBeChecked()
    await fireEvent.click(toggle)

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith(expect.objectContaining({
        terminal: expect.objectContaining({
          harness: expect.objectContaining({ agy_hooks: false }),
        }),
      }))
    })
  })

  it('General section has heading "General"', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const section = screen.getByTestId('settings-scanning')
      expect(section.textContent).toContain('General')
    })
  })

  it('Display section has heading "Display"', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const section = screen.getByTestId('settings-display')
      expect(section.textContent).toContain('Display')
    })
  })

  it('Terminal section has heading "Terminal & Sessions"', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const section = screen.getByTestId('settings-terminal')
      expect(section.textContent).toContain('Terminal & Sessions')
    })
  })

  it('Search section has heading "Search"', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const section = screen.getByTestId('settings-index')
      expect(section.textContent).toContain('Search')
    })
  })

  // --- General section: scan dirs ---

  it('shows scan directories in General section', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('settings-scanning').textContent).toContain('~/projects')
    })
  })

  it('shows scan directories as active and honest about runtime enforcement', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('scan-directories-status').textContent).toContain('Active')
      expect(screen.getByTestId('scan-directories-status').textContent)
        .toContain('Background scanning uses this list')
      expect(screen.getByTestId('settings-scanning').textContent).not.toContain('not yet active')
    })
  })

  it('shows ignore patterns as pills', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByText('node_modules')).toBeTruthy()
      expect(screen.getByText('.git')).toBeTruthy()
      expect(screen.getByText('target')).toBeTruthy()
    })
  })

  it('shows ignore patterns as active and applied to scanning and indexing', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('ignore-patterns-status').textContent).toContain('Active')
      expect(screen.getByTestId('ignore-patterns-status').textContent)
        .toContain('skipped during scanning and search indexing')
      expect(screen.getByTestId('settings-scanning').textContent).not.toContain('not yet wired')
    })
  })

  // --- General section: activity thresholds (moved from Display in P14) ---

  it('activity thresholds are in General section', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const section = screen.getByTestId('settings-scanning')
      expect(section.textContent).toContain('Activity state thresholds')
    })
  })

  it('renders threshold inputs with correct values', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('threshold-active')).toHaveValue(7)
      expect(screen.getByTestId('threshold-recent')).toHaveValue(30)
      expect(screen.getByTestId('threshold-stale')).toHaveValue(90)
    })
  })

  it('keeps visible focus styling on threshold inputs', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const active = screen.getByTestId('threshold-active')
      const recent = screen.getByTestId('threshold-recent')
      const stale = screen.getByTestId('threshold-stale')
      expect(active.className).toContain('focus-visible:ring-1')
      expect(active.className).toContain('focus-visible:ring-brand-500')
      expect(recent.className).toContain('focus-visible:ring-1')
      expect(stale.className).toContain('focus-visible:ring-brand-500')
    })
  })

  it('adds accessible labels to scan and ignore textareas while editing', async () => {
    render(Settings, { props: defaultProps() })

    const editButtons = await screen.findAllByRole('button', { name: 'Edit' })
    await fireEvent.click(editButtons[0])
    await fireEvent.click(editButtons[1])

    expect(screen.getByLabelText('Scan directories')).toBeInTheDocument()
    expect(screen.getByLabelText('Ignore patterns')).toBeInTheDocument()
  })

  it('threshold blur calls updateSettings', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => expect(screen.getByTestId('threshold-active')).toBeTruthy())

    const input = screen.getByTestId('threshold-active')
    await fireEvent.input(input, { target: { value: '5' } })
    await fireEvent.blur(input)

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalled()
    })
  })

  it('shows inline save error when updateSettings fails', async () => {
    updateSettings.mockRejectedValueOnce(new Error('permission denied'))
    render(Settings, { props: defaultProps() })
    await waitFor(() => expect(screen.getByTestId('threshold-active')).toBeTruthy())

    const input = screen.getByTestId('threshold-active')
    await fireEvent.input(input, { target: { value: '9' } })
    await fireEvent.blur(input)

    await waitFor(() => {
      expect(screen.getByTestId('settings-save-error')).toBeTruthy()
      expect(screen.getByTestId('settings-save-error').textContent).toContain('permission denied')
    })
  })

  // --- Display section: code themes ---

  it('renders code theme dropdowns in Display section', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('code-theme-light')).toBeTruthy()
      expect(screen.getByTestId('code-theme-dark')).toBeTruthy()
    })
  })

  it('Display section shows "Syntax highlighting" label', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('settings-display').textContent).toContain('Syntax highlighting')
    })
  })

  // --- Terminal & Sessions section (P12/P13) ---

  it('renders terminal emulator dropdown', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const select = screen.getByTestId('terminal-emulator')
      expect(select).toBeTruthy()
      expect(select.value).toBe('windows_terminal')
    })
  })

  it('keeps the CLI product names in the command settings headings', async () => {
    // Regression: 91f4d3f rendered short registry labels in place of the
    // existing Claude Code and Antigravity CLI product names.
    render(Settings, { props: defaultProps() })

    const section = await screen.findByTestId('settings-cli-tools')
    expect(section).toHaveTextContent('Claude Code')
    expect(section).toHaveTextContent('Codex')
    expect(section).toHaveTextContent('Antigravity CLI')
  })

  it('explains that the resume session token is substituted already quoted', async () => {
    // Regression: 987e0ac shell-escaped the `{session_id}` expansion without
    // saying so anywhere, so a user who quoted the token themselves got a
    // double-quoted id and an unresumable command with no explanation.
    render(Settings, { props: defaultProps() })

    const section = await screen.findByTestId('settings-cli-tools')
    expect(section).toHaveTextContent('{session_id}')
    expect(section).toHaveTextContent(/already quoted/i)
  })

  it('keeps Grok compaction hooks on by default and persists the toggle', async () => {
    // Regression: commit 358a7c9 registered grok without a compaction slice.
    // Its hook directory is always trusted, so the bridge is on by default —
    // and a user who does not want taurhaus in `~/.grok` must be able to say so.
    render(Settings, { props: defaultProps() })

    const toggle = await screen.findByTestId('grok-hooks-toggle')
    expect(toggle.checked).toBe(true)

    await fireEvent.click(toggle)
    await waitFor(() =>
      expect(updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          terminal: expect.objectContaining({
            harness: expect.objectContaining({ grok_hooks: false }),
          }),
        })
      )
    )
  })

  it('terminal emulator dropdown has Windows Terminal and Custom options', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const select = screen.getByTestId('terminal-emulator')
      const options = select.querySelectorAll('option')
      const values = Array.from(options).map(o => o.value)
      expect(values).toEqual(['windows_terminal', 'custom'])
    })
  })

  it('renders macOS terminal options from the backend contract', async () => {
    getSettings.mockResolvedValue(mockSettings({
      terminal_contract: mockTerminalContract('macos'),
      terminal: { emulator: 'iterm2', custom_command: '', tmux_layout: 'new_window', cli_commands: mockCliCommandDefaults() },
    }))
    render(Settings, { props: defaultProps() })

    await waitFor(() => {
      const select = screen.getByTestId('terminal-emulator')
      const values = Array.from(select.querySelectorAll('option')).map(o => o.value)
      expect(values).toEqual(['iterm2', 'ghostty', 'terminal_app', 'custom'])
      expect(select.value).toBe('iterm2')
    })
  })

  it('renders Linux terminal options from the backend contract and hides unsupported controls', async () => {
    getSettings.mockResolvedValue(mockSettings({
      terminal_contract: mockTerminalContract('linux'),
      terminal: { emulator: 'manual', custom_command: '', tmux_layout: 'new_window', cli_commands: mockCliCommandDefaults() },
    }))
    render(Settings, { props: defaultProps() })

    await waitFor(() => {
      const select = screen.getByTestId('terminal-emulator')
      const values = Array.from(select.querySelectorAll('option')).map(o => o.value)
      expect(values).toEqual(['manual'])
      expect(select.value).toBe('manual')
      expect(screen.getByTestId('terminal-linux-note')).toBeTruthy()
      expect(screen.queryByTestId('terminal-custom-cmd')).toBeNull()
    })
  })

  it('falls back to the contract default when a payload carries an invalid emulator for the platform', async () => {
    getSettings.mockResolvedValue(mockSettings({
      terminal_contract: mockTerminalContract('linux'),
      terminal: { emulator: 'windows_terminal', custom_command: '', tmux_layout: 'new_window', cli_commands: mockCliCommandDefaults() },
    }))
    render(Settings, { props: defaultProps() })

    await waitFor(() => {
      expect(screen.getByTestId('terminal-emulator')).toHaveValue('manual')
    })
  })

  it('renders tmux layout dropdown', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const select = screen.getByTestId('tmux-layout')
      expect(select).toBeTruthy()
      expect(select.value).toBe('new_window')
    })
  })

  it('tmux layout dropdown has three options', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const select = screen.getByTestId('tmux-layout')
      const options = select.querySelectorAll('option')
      const values = Array.from(options).map(o => o.value)
      expect(values).toContain('new_window')
      expect(values).toContain('split')
      expect(values).toContain('per_project')
    })
  })

  it('hides custom command input when emulator is windows_terminal', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.queryByTestId('terminal-custom-cmd')).toBeNull()
    })
  })

  it('shows custom command input when emulator is "custom"', async () => {
    getSettings.mockResolvedValue(mockSettings({
      terminal: { emulator: 'custom', custom_command: 'wezterm.exe', tmux_layout: 'new_window' },
    }))
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('terminal-custom-cmd')).toBeTruthy()
    })
  })

  it('custom command input has placeholder text with placeholders', async () => {
    getSettings.mockResolvedValue(mockSettings({
      terminal: { emulator: 'custom', custom_command: '', tmux_layout: 'new_window' },
    }))
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const input = screen.getByTestId('terminal-custom-cmd')
      expect(input.placeholder).toContain('{distro}')
      expect(input.placeholder).toContain('{tmux_session}')
    })
  })

  it('changing emulator dropdown calls updateSettings', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => expect(screen.getByTestId('terminal-emulator')).toBeTruthy())

    await fireEvent.change(screen.getByTestId('terminal-emulator'), { target: { value: 'custom' } })

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalled()
    })
  })

  it('uses backend contract defaults as CLI command placeholders', async () => {
    render(Settings, { props: defaultProps() })

    await waitFor(() => {
      expect(screen.getByTestId('cli-claude-continue').placeholder)
        .toBe('claude --dangerously-skip-permissions --continue')
      expect(screen.getByTestId('cli-codex-resume').placeholder)
        .toBe('codex resume --last --yolo')
    })
  })

  it('changing tmux layout dropdown calls updateSettings', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => expect(screen.getByTestId('tmux-layout')).toBeTruthy())

    await fireEvent.change(screen.getByTestId('tmux-layout'), { target: { value: 'split' } })

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalled()
    })
  })

  // --- Search section ---

  it('shows document count in Search section', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('settings-index').textContent).toContain('42 documents indexed')
    })
  })

  it('renders rebuild index button', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('rebuild-index-btn')).toBeTruthy()
      expect(screen.getByTestId('rebuild-index-btn').textContent).toContain('Rebuild index')
    })
  })

  it('clicking rebuild index calls rebuildIndex', async () => {
    rebuildIndex.mockResolvedValue(50)
    render(Settings, { props: defaultProps() })
    await waitFor(() => expect(screen.getByTestId('rebuild-index-btn')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('rebuild-index-btn'))

    await waitFor(() => {
      expect(rebuildIndex).toHaveBeenCalled()
    })
  })

  it('shows rebuild error with retry', async () => {
    rebuildIndex.mockRejectedValue(new Error('Index locked'))
    render(Settings, { props: defaultProps() })
    await waitFor(() => expect(screen.getByTestId('rebuild-index-btn')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('rebuild-index-btn'))

    await waitFor(() => {
      expect(screen.getByTestId('rebuild-error')).toBeTruthy()
      expect(screen.getByTestId('rebuild-error').textContent).toContain('Index locked')
    })
  })

  // --- Navigation ---

  it('back button calls onClose', async () => {
    const onClose = vi.fn()
    render(Settings, { props: defaultProps({ onClose }) })
    await waitFor(() => expect(screen.getByTestId('settings-back')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('settings-back'))
    expect(onClose).toHaveBeenCalled()
  })

  // --- Dark mode ---

  it('renders without errors in dark mode', async () => {
    render(Settings, { props: defaultProps({ dark: true }) })
    await waitFor(() => {
      expect(screen.getByTestId('settings-view')).toBeTruthy()
    })
  })

  // --- Empty states ---

  it('shows empty scan directories message', async () => {
    getSettings.mockResolvedValue(mockSettings({ scan_directories: [] }))
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByText('No directories configured')).toBeTruthy()
    })
  })

  it('shows empty ignore patterns message', async () => {
    getSettings.mockResolvedValue(mockSettings({ ignore_patterns: [] }))
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByText('No patterns configured')).toBeTruthy()
    })
  })
})
