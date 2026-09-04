import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor, fireEvent, within } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import '../app.css'

vi.mock('./ipc.js', () => ({
  scanDirectory: vi.fn(),
  registerProjectsBatch: vi.fn(),
  createProject: vi.fn(),
  listProjects: vi.fn(),
  removeProject: vi.fn(),
  validateProjectPath: vi.fn(),
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
}))

vi.mock('./DirectoryBrowser.svelte', () => ({
  default: function MockDirectoryBrowser(target, props) {
    const root = document.createElement('div')
    root.setAttribute('data-testid', 'mock-directory-browser')

    const pickButton = document.createElement('button')
    pickButton.textContent = 'Pick path'
    pickButton.setAttribute('data-testid', 'mock-directory-select')
    pickButton.addEventListener('click', () => {
      props?.onSelect?.('/manual/selected')
    })
    root.appendChild(pickButton)

    if (target.nodeType === Node.ELEMENT_NODE) {
      target.appendChild(root)
    } else {
      target.parentNode.insertBefore(root, target)
    }

    return {
      $set(nextProps) {
        props = nextProps
      },
      $destroy() {
        root.remove()
      },
    }
  },
}))

const {
  scanDirectory,
  registerProjectsBatch,
  createProject,
  listProjects,
  removeProject,
  validateProjectPath,
  getSettings,
  updateSettings,
} = await import('./ipc.js')
import ProjectsTakeover from './ProjectsTakeover.svelte'

describe('ProjectsTakeover', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    listProjects.mockResolvedValue([
      {
        id: 'p1',
        name: 'Project One',
        path: '/projects/one',
        activityState: 'active',
      },
    ])
    scanDirectory.mockResolvedValue([])
    registerProjectsBatch.mockResolvedValue([{ success: true }])
    createProject.mockResolvedValue({ id: 'p-new', name: 'new-project', path: '/projects/new-project' })
    removeProject.mockResolvedValue(undefined)
    getSettings.mockResolvedValue({
      scan_directories: ['~/projects'],
      thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
      ignore_patterns: [],
      daemon: { port: 17233, path: '', auto_start: true },
      code_theme: { light: 'github-light', dark: 'github-dark-dimmed' },
      terminal: { emulator: 'manual', custom_command: '', tmux_layout: 'new_window', cli_commands: {} },
      terminal_contract: { platform: 'linux', default_emulator: 'manual', supported_emulators: ['manual'], cli_command_defaults: {} },
      dark_mode: false,
      project_dialog_last_path: '',
    })
    updateSettings.mockImplementation(async (settings) => settings)
    validateProjectPath.mockResolvedValue({
      exists: true,
      isGitRepo: true,
      isRegistered: false,
    })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('shows empty registered state when listProjects load fails', async () => {
    listProjects.mockRejectedValueOnce(new Error('offline'))

    render(ProjectsTakeover, { props: { dark: false } })

    await waitFor(() => {
      expect(screen.getByTestId('no-projects')).toBeInTheDocument()
    })
  })

  it('uses two-click remove confirmation and removes project', async () => {
    const onProjectsChanged = vi.fn()

    render(ProjectsTakeover, {
      props: {
        dark: false,
        onProjectsChanged,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('remove-p1')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('remove-p1'))
    await fireEvent.click(screen.getByTestId('confirm-remove-p1'))

    await waitFor(() => {
      expect(removeProject).toHaveBeenCalledWith('p1')
      expect(onProjectsChanged).toHaveBeenCalled()
    })
    expect(screen.queryByTestId('remove-p1')).not.toBeInTheDocument()
  })

  it('handles removeProject failure without crashing', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
    removeProject.mockRejectedValueOnce(new Error('permission denied'))

    render(ProjectsTakeover, {
      props: {
        dark: false,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('remove-p1')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('remove-p1'))
    await fireEvent.click(screen.getByTestId('confirm-remove-p1'))

    await waitFor(() => {
      expect(removeProject).toHaveBeenCalledWith('p1')
    })
    expect(consoleError).toHaveBeenCalled()

    consoleError.mockRestore()
  })

  it('scans and preselects only unregistered git projects', async () => {
    const onProjectsChanged = vi.fn()
    scanDirectory.mockResolvedValueOnce([
      { name: 'one', path: '/projects/one', has_git: true }, // already registered
      { name: 'two', path: '/projects/two', has_git: true },
      { name: 'three', path: '/projects/three', has_git: false },
    ])
    registerProjectsBatch.mockResolvedValueOnce([{ success: true }])

    render(ProjectsTakeover, {
      props: {
        dark: false,
        onProjectsChanged,
      },
    })

    await fireEvent.click(screen.getByTestId('show-add-section'))

    await waitFor(() => {
      expect(screen.getByTestId('discovered-list')).toBeInTheDocument()
      expect(screen.getByText('2 new projects')).toBeInTheDocument()
    })

    expect(screen.getByTestId('register-button')).toHaveTextContent('Register 1')

    await fireEvent.click(screen.getByTestId('register-button'))

    await waitFor(() => {
      expect(registerProjectsBatch).toHaveBeenCalledWith(['/projects/two'])
      expect(onProjectsChanged).toHaveBeenCalled()
      expect(screen.getByTestId('add-success')).toHaveTextContent('1 project added')
    })
  })

  it('shows scan error and allows switching to manual mode', async () => {
    scanDirectory.mockRejectedValueOnce(new Error('scan failed'))

    render(ProjectsTakeover, { props: { dark: false } })

    await fireEvent.click(screen.getByTestId('show-add-section'))

    await waitFor(() => {
      expect(screen.getByTestId('scan-error')).toBeInTheDocument()
      expect(screen.getByText('Could not scan that folder. Try again, choose another folder, or enter a path manually.')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getAllByTestId('enter-manual-mode')[0])
    expect(screen.getByTestId('manual-path-input')).toBeInTheDocument()
    expect(screen.getByTestId('mock-directory-browser')).toBeInTheDocument()
  })

  it('shows partial batch registration failures with an expandable failed-path list', async () => {
    scanDirectory.mockResolvedValueOnce([
      { name: 'two', path: '/projects/two', has_git: true },
      { name: 'three', path: '/projects/three', has_git: true },
    ])
    registerProjectsBatch.mockResolvedValueOnce([
      { path: '/projects/two', success: true },
      { path: '/projects/three', success: false, error: 'already tracked elsewhere' },
    ])
    listProjects
      .mockResolvedValueOnce([{ id: 'p1', name: 'Project One', path: '/projects/one', activityState: 'active' }])
      .mockResolvedValueOnce([
        { id: 'p1', name: 'Project One', path: '/projects/one', activityState: 'active' },
        { id: 'p2', name: 'Project Two', path: '/projects/two', activityState: 'recent' },
      ])

    render(ProjectsTakeover, { props: { dark: false } })

    await fireEvent.click(screen.getByTestId('show-add-section'))
    await waitFor(() => {
      expect(screen.getByTestId('register-button')).toHaveTextContent('Register 2')
    })

    await fireEvent.click(screen.getByTestId('register-button'))

    await waitFor(() => {
      expect(screen.getByTestId('add-success')).toHaveTextContent('1 project added')
      expect(screen.getByTestId('add-failure-summary')).toHaveTextContent('1 project could not be added.')
      expect(screen.getByTestId('register-button')).toHaveTextContent('Register 1')
    })

    await fireEvent.click(screen.getByText('Show failed paths'))

    expect(screen.getByTestId('add-failure-details')).toHaveTextContent('/projects/three')
    expect(screen.getByTestId('add-failure-details')).toHaveTextContent('already tracked elsewhere')
  })

  it('shows all-registered state when scan returns only registered projects', async () => {
    scanDirectory.mockResolvedValueOnce([
      { name: 'one', path: '/projects/one', has_git: true },
    ])

    render(ProjectsTakeover, { props: { dark: false } })

    await fireEvent.click(screen.getByTestId('show-add-section'))

    await waitFor(() => {
      expect(screen.getByTestId('all-registered')).toBeInTheDocument()
    })
  })

  it('validates manual path and blocks invalid registration', async () => {
    scanDirectory.mockResolvedValueOnce([])
    validateProjectPath.mockResolvedValueOnce({
      exists: false,
      isGitRepo: false,
      isRegistered: false,
    })

    render(ProjectsTakeover, { props: { dark: false } })

    await fireEvent.click(screen.getByTestId('show-add-section'))
    await waitFor(() => {
      expect(screen.getByTestId('empty-scan')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getAllByTestId('enter-manual-mode')[0])

    const input = screen.getByTestId('manual-path-input')
    await fireEvent.input(input, { target: { value: '/missing/project' } })
    await fireEvent.blur(input)

    await waitFor(() => {
      expect(screen.getByTestId('validation-message')).toHaveTextContent('Directory not found')
    })

    expect(screen.getByTestId('manual-add-button')).toBeDisabled()
    expect(registerProjectsBatch).not.toHaveBeenCalled()
  })

  it('adds manual project when validation succeeds', async () => {
    const onProjectsChanged = vi.fn()
    scanDirectory.mockResolvedValueOnce([])
    validateProjectPath.mockResolvedValue({
      exists: true,
      isGitRepo: true,
      isRegistered: false,
    })
    registerProjectsBatch.mockResolvedValueOnce([{ success: true }])
    listProjects
      .mockResolvedValueOnce([{ id: 'p1', name: 'Project One', path: '/projects/one', activityState: 'active' }])
      .mockResolvedValueOnce([
        { id: 'p1', name: 'Project One', path: '/projects/one', activityState: 'active' },
        { id: 'p2', name: 'Project Two', path: '/manual/selected', activityState: 'recent' },
      ])

    render(ProjectsTakeover, {
      props: {
        dark: false,
        onProjectsChanged,
      },
    })

    await fireEvent.click(screen.getByTestId('show-add-section'))
    await waitFor(() => {
      expect(screen.getByTestId('empty-scan')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getAllByTestId('enter-manual-mode')[0])
    await fireEvent.click(screen.getByTestId('mock-directory-select'))

    await waitFor(() => {
      expect(screen.getByTestId('manual-path-input')).toHaveValue('/manual/selected')
      expect(screen.getByTestId('manual-add-button')).not.toBeDisabled()
    })

    await fireEvent.click(screen.getByTestId('manual-add-button'))

    await waitFor(() => {
      expect(registerProjectsBatch).toHaveBeenCalledWith(['/manual/selected'])
      expect(updateSettings).toHaveBeenCalledWith(expect.objectContaining({
        project_dialog_last_path: '/manual/selected',
      }))
      expect(onProjectsChanged).toHaveBeenCalled()
      expect(screen.getByTestId('add-success')).toHaveTextContent('1 project added')
    })
  })

  it('restores remembered project path into manual and create workflows', async () => {
    getSettings.mockResolvedValueOnce({
      scan_directories: ['~/projects'],
      thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
      ignore_patterns: [],
      daemon: { port: 17233, path: '', auto_start: true },
      code_theme: { light: 'github-light', dark: 'github-dark-dimmed' },
      terminal: { emulator: 'manual', custom_command: '', tmux_layout: 'new_window', cli_commands: {} },
      terminal_contract: { platform: 'linux', default_emulator: 'manual', supported_emulators: ['manual'], cli_command_defaults: {} },
      dark_mode: false,
      project_dialog_last_path: '/remembered/path',
    })

    render(ProjectsTakeover, { props: { dark: false } })

    await fireEvent.click(screen.getByTestId('show-add-section'))
    await waitFor(() => {
      expect(screen.getByTestId('empty-scan')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getAllByTestId('enter-manual-mode')[0])
    await waitFor(() => {
      expect(screen.getByTestId('manual-path-input')).toHaveValue('/remembered/path')
    })

    await fireEvent.click(screen.getByTestId('mode-create'))
    expect(screen.getByTestId('create-parent-input')).toHaveValue('/remembered/path')
  })

  it('persists selected create parent path', async () => {
    render(ProjectsTakeover, { props: { dark: false } })

    await fireEvent.click(screen.getByTestId('show-add-section'))
    await fireEvent.click(screen.getByTestId('mode-create'))
    await fireEvent.click(screen.getByTestId('mock-directory-select'))

    await waitFor(() => {
      expect(updateSettings).toHaveBeenCalledWith(expect.objectContaining({
        project_dialog_last_path: '/manual/selected',
      }))
      expect(screen.getByTestId('create-parent-input')).toHaveValue('/manual/selected')
    })
  })

  it('shows manual error from failed registration result', async () => {
    scanDirectory.mockResolvedValueOnce([])
    validateProjectPath.mockResolvedValue({
      exists: true,
      isGitRepo: true,
      isRegistered: false,
    })
    registerProjectsBatch.mockResolvedValueOnce([{ success: false, error: 'already tracked' }])

    render(ProjectsTakeover, { props: { dark: false } })

    await fireEvent.click(screen.getByTestId('show-add-section'))
    await waitFor(() => {
      expect(screen.getByTestId('empty-scan')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getAllByTestId('enter-manual-mode')[0])
    const input = screen.getByTestId('manual-path-input')
    await fireEvent.input(input, { target: { value: '/manual/dup' } })
    await fireEvent.blur(input)
    await waitFor(() => {
      expect(screen.getByTestId('manual-add-button')).not.toBeDisabled()
    })

    await fireEvent.click(screen.getByTestId('manual-add-button'))

    await waitFor(() => {
      expect(screen.getByTestId('manual-error')).toHaveTextContent('already tracked')
    })
  })

  it('opens behind the shared doorway with the Projects key echo and count', async () => {
    render(ProjectsTakeover, { props: { dark: false } })

    await waitFor(() => {
      expect(screen.getByTestId('projects-takeover')).toBeInTheDocument()
    })

    const doorway = screen.getByTestId('surface-doorway')
    expect(within(doorway).getByRole('heading', { name: 'Projects' })).toBeInTheDocument()
    expect(within(doorway).getByTestId('projects-back')).toHaveTextContent('Back')
    expect(doorway.querySelector('kbd')).toHaveTextContent('Esc')

    await waitFor(() => {
      expect(screen.getByTestId('projects-registered-count')).toHaveTextContent('1 registered')
    })
  })

  it('closes on Escape and on the doorway back button', async () => {
    const onClose = vi.fn()

    render(ProjectsTakeover, {
      props: {
        dark: false,
        onClose,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('projects-takeover')).toBeInTheDocument()
    })

    await fireEvent.keyDown(document, { key: 'Escape' })
    expect(onClose).toHaveBeenCalledTimes(1)

    await fireEvent.click(screen.getByTestId('projects-back'))
    expect(onClose).toHaveBeenCalledTimes(2)
  })

  it('clears pending remove-confirm timer on unmount', async () => {
    vi.useFakeTimers()

    const { unmount } = render(ProjectsTakeover, {
      props: {
        dark: false,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('remove-p1')).toBeInTheDocument()
    })

    const baselineTimers = vi.getTimerCount()
    await fireEvent.click(screen.getByTestId('remove-p1'))
    expect(vi.getTimerCount()).toBeGreaterThan(baselineTimers)

    unmount()
    expect(vi.getTimerCount()).toBe(baselineTimers)
  })

  it('create mode validates project name before submit', async () => {
    render(ProjectsTakeover, { props: { dark: false } })

    await fireEvent.click(screen.getByTestId('show-add-section'))
    await fireEvent.click(screen.getByTestId('mode-create'))

    const createButton = screen.getByTestId('create-project-button')
    expect(createButton).toBeDisabled()

    await fireEvent.input(screen.getByTestId('create-name-input'), { target: { value: 'bad/name' } })
    await fireEvent.input(screen.getByTestId('create-parent-input'), { target: { value: '/projects' } })
    await fireEvent.click(createButton)

    expect(screen.getByTestId('create-error')).toHaveTextContent('Enter a valid project name')
    expect(createProject).not.toHaveBeenCalled()
  })

  it('creates a new project, closes the takeover, and emits callbacks', async () => {
    const onClose = vi.fn()
    const onProjectsChanged = vi.fn()
    const onProjectCreated = vi.fn()
    validateProjectPath
      .mockResolvedValueOnce({ exists: true, isGitRepo: false, isRegistered: false }) // parent exists
      .mockResolvedValueOnce({ exists: false, isGitRepo: false, isRegistered: false }) // target missing
    createProject.mockResolvedValueOnce({
      id: 'p-new',
      name: 'new-project',
      path: '/projects/new-project',
    })

    render(ProjectsTakeover, {
      props: { dark: false, onClose, onProjectsChanged, onProjectCreated },
    })

    await fireEvent.click(screen.getByTestId('show-add-section'))
    await fireEvent.click(screen.getByTestId('mode-create'))
    await fireEvent.input(screen.getByTestId('create-name-input'), { target: { value: 'new-project' } })
    await fireEvent.input(screen.getByTestId('create-parent-input'), { target: { value: '/projects' } })
    await fireEvent.click(screen.getByTestId('create-project-button'))

    await waitFor(() => {
      expect(createProject).toHaveBeenCalledWith('new-project', '/projects')
      expect(onProjectsChanged).toHaveBeenCalled()
      expect(onProjectCreated).toHaveBeenCalledWith({
        id: 'p-new',
        name: 'new-project',
        path: '/projects/new-project',
      })
      expect(onClose).toHaveBeenCalled()
    })
  })
})
