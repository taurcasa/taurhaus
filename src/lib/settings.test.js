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
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
  getIndexStatus: vi.fn(),
  rebuildIndex: vi.fn(),
}))

const { getSettings, updateSettings, getIndexStatus, rebuildIndex } = await import('./ipc.js')

import Settings from './Settings.svelte'

/** Default mock settings matching the full schema. */
function mockSettings(overrides = {}) {
  return {
    scan_directories: ['~/projects'],
    thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
    ignore_patterns: ['node_modules', '.git', 'target', 'dist'],
    code_theme: { light: 'github-light', dark: 'github-dark-dimmed' },
    terminal: { emulator: 'windows_terminal', custom_command: '', tmux_layout: 'new_window' },
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

  // --- Section structure (P14) ---

  it('renders all four sections: General, Display, Terminal & Sessions, Search', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByTestId('settings-scanning')).toBeTruthy()
      expect(screen.getByTestId('settings-display')).toBeTruthy()
      expect(screen.getByTestId('settings-terminal')).toBeTruthy()
      expect(screen.getByTestId('settings-index')).toBeTruthy()
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

  it('shows ignore patterns as pills', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      expect(screen.getByText('node_modules')).toBeTruthy()
      expect(screen.getByText('.git')).toBeTruthy()
      expect(screen.getByText('target')).toBeTruthy()
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

  it('terminal emulator dropdown has Windows Terminal and Custom options', async () => {
    render(Settings, { props: defaultProps() })
    await waitFor(() => {
      const select = screen.getByTestId('terminal-emulator')
      const options = select.querySelectorAll('option')
      const values = Array.from(options).map(o => o.value)
      expect(values).toContain('windows_terminal')
      expect(values).toContain('custom')
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
