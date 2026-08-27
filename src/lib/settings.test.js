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
}))

const {
  getSettings,
  updateSettings,
  getIndexStatus,
  rebuildIndex,
  getPlatform,
  listAccounts,
  launchCliSession,
} = await import('./ipc.js')
const { accountState, requestLaunch, resetAccountsForTest } = await import('./accounts.svelte.js')
const claudeAccounts = accountState('claude')

import Settings from './Settings.svelte'

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
    gemini: {
      continue_cmd: 'gemini --yolo --resume',
      fresh: 'gemini --yolo',
      resume: 'gemini --yolo --resume',
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
    harness: { codex_compaction: 'hooks' },
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
  })

  /** What the backend answers when detection ran. */
  const detected = (accounts) => ({ accounts, source: 'native', degraded: false, error: null })

  const TWO_ACCOUNTS = [
    { id: 'account-1', email: 'a@example.com', display_name: 'A', logged_in: true, is_default: true },
    { id: 'account-2', email: 'b@example.com', display_name: 'B', logged_in: true, is_default: false },
  ]

  // Regression: c982822 pushed the newly chosen default into the shared account
  // store before the write landed, and restored neither the store nor the form
  // when it failed. requestClaudeLaunch reads that store as an established
  // default and launches without naming an account, while the backend still
  // reads the old persisted one — so a failed save left the UI claiming one
  // subscription while every launch used another.
  it('keeps the shared default untouched when saving it fails', async () => {
    listAccounts.mockResolvedValue(detected(TWO_ACCOUNTS))
    updateSettings.mockRejectedValueOnce(new Error('disk full'))
    render(Settings, { props: defaultProps() })
    await waitFor(() => expect(screen.getByTestId('settings-accounts')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('account-default-claude-account-2'))

    await waitFor(() => expect(screen.getByTestId('settings-save-error')).toBeTruthy())
    expect(claudeAccounts.defaultAccountId).toBe(null)

    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })
    expect(launchCliSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  it('shares the chosen default once it is persisted', async () => {
    listAccounts.mockResolvedValue(detected(TWO_ACCOUNTS))
    render(Settings, { props: defaultProps() })
    await waitFor(() => expect(screen.getByTestId('settings-accounts')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('account-default-claude-account-2'))

    await waitFor(() => expect(claudeAccounts.defaultAccountId).toBe('account-2'))
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
      expect(screen.getByTestId('cli-gemini-fresh').placeholder).toBe('gemini --yolo')
    })
  })

  // --- Section structure (P14) ---

  it('renders the Mesh section with the Codex compaction source', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      // Regression: 0b87699 had no UI control to select the Codex hook bridge
      // or restore the transcript fallback.
      expect(screen.getByTestId('settings-mesh')).toBeTruthy()
      expect(screen.getByTestId('codex-compaction-source')).toHaveValue('hooks')
    })
  })

  it('saves the Codex compaction source from the Mesh section', async () => {
    render(Settings, { props: defaultProps() })
    const select = await screen.findByTestId('codex-compaction-source')

    await fireEvent.change(select, { target: { value: 'transcript' } })

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith(expect.objectContaining({
        terminal: expect.objectContaining({
          harness: { codex_compaction: 'transcript' },
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
    // existing Claude Code and Gemini CLI product names.
    render(Settings, { props: defaultProps() })

    const section = await screen.findByTestId('settings-cli-tools')
    expect(section).toHaveTextContent('Claude Code')
    expect(section).toHaveTextContent('Codex')
    expect(section).toHaveTextContent('Gemini CLI')
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
