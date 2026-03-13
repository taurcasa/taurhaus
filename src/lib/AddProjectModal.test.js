import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
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
import AddProjectModal from './AddProjectModal.svelte'

const appCss = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8')

describe('AddProjectModal', () => {
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
      terminal: { emulator: 'default', custom_command: '', tmux_layout: 'new_window', cli_commands: {} },
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

    render(AddProjectModal, { props: { dark: false } })

    await waitFor(() => {
      expect(screen.getByTestId('no-projects')).toBeInTheDocument()
    })
  })

  it('uses two-click remove confirmation and removes project', async () => {
    const onProjectsChanged = vi.fn()

    render(AddProjectModal, {
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

    render(AddProjectModal, {
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

    render(AddProjectModal, {
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

    render(AddProjectModal, { props: { dark: false } })

    await fireEvent.click(screen.getByTestId('show-add-section'))

    await waitFor(() => {
      expect(screen.getByTestId('scan-error')).toBeInTheDocument()
      expect(screen.getByText('scan failed')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getAllByTestId('enter-manual-mode')[0])
    expect(screen.getByTestId('manual-path-input')).toBeInTheDocument()
    expect(screen.getByTestId('mock-directory-browser')).toBeInTheDocument()
  })

  it('shows all-registered state when scan returns only registered projects', async () => {
    scanDirectory.mockResolvedValueOnce([
      { name: 'one', path: '/projects/one', has_git: true },
    ])

    render(AddProjectModal, { props: { dark: false } })

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

    render(AddProjectModal, { props: { dark: false } })

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

    render(AddProjectModal, {
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
      terminal: { emulator: 'default', custom_command: '', tmux_layout: 'new_window', cli_commands: {} },
      dark_mode: false,
      project_dialog_last_path: '/remembered/path',
    })

    render(AddProjectModal, { props: { dark: false } })

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
    render(AddProjectModal, { props: { dark: false } })

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

    render(AddProjectModal, { props: { dark: false } })

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

  it('closes on Escape, done button, and backdrop click only', async () => {
    const onClose = vi.fn()

    const { container } = render(AddProjectModal, {
      props: {
        dark: false,
        onClose,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('manage-projects-modal')).toBeInTheDocument()
    })

    const dialog = screen.getByTestId('manage-projects-modal')
    const backdrop = container.firstElementChild

    await fireEvent.mouseDown(dialog)
    expect(onClose).not.toHaveBeenCalled()

    await fireEvent.mouseDown(backdrop)
    expect(onClose).toHaveBeenCalledTimes(1)

    await fireEvent.click(screen.getByTestId('done-button'))
    expect(onClose).toHaveBeenCalledTimes(2)

    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(onClose).toHaveBeenCalledTimes(3)
  })

  it('keeps the manage projects overlay out of the shell frame flow', async () => {
    const shellFrame = document.createElement('div')
    shellFrame.className = 'shell-frame'
    document.body.appendChild(shellFrame)

    const mainContent = document.createElement('div')
    mainContent.setAttribute('data-testid', 'shell-main-content')
    shellFrame.appendChild(mainContent)

    render(AddProjectModal, {
      target: shellFrame,
      props: {
        dark: false,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('manage-projects-modal')).toBeInTheDocument()
    })

    // Regression: commit 188211f reintroduced `.shell-frame > * { position: relative }`,
    // which overrode Tailwind's `.fixed` on direct-child overlays and pushed the app
    // content upward instead of floating Manage Projects above it.
    const backdrop = screen.getByTestId('manage-projects-backdrop')
    expect(backdrop).toHaveAttribute('data-shell-overlay')
    expect(appCss).toContain('.shell-frame > :not([data-shell-overlay])')
    expect(appCss).not.toContain('.shell-frame > * {\n  position: relative;')
    expect(shellFrame.firstElementChild).toBe(mainContent)
    expect(shellFrame.lastElementChild).toBe(backdrop)

    shellFrame.remove()
  })

  it('traps Tab focus inside modal and restores trigger focus on close', async () => {
    const trigger = document.createElement('button')
    trigger.textContent = 'Open projects modal'
    document.body.appendChild(trigger)
    trigger.focus()

    const onClose = vi.fn()
    const { unmount } = render(AddProjectModal, {
      props: {
        dark: false,
        onClose,
      },
    })

    const firstFocusable = screen.getByTestId('modal-close')
    const lastFocusable = screen.getByTestId('done-button')

    await waitFor(() => {
      expect(firstFocusable).toHaveFocus()
    })

    lastFocusable.focus()
    expect(lastFocusable).toHaveFocus()

    await fireEvent.keyDown(window, { key: 'Tab' })
    expect(firstFocusable).toHaveFocus()

    await fireEvent.keyDown(window, { key: 'Tab', shiftKey: true })
    expect(lastFocusable).toHaveFocus()

    await fireEvent.click(lastFocusable)
    expect(onClose).toHaveBeenCalledTimes(1)

    unmount()
    expect(trigger).toHaveFocus()
    trigger.remove()
  })

  it('clears pending remove-confirm timer on unmount', async () => {
    vi.useFakeTimers()

    const { unmount } = render(AddProjectModal, {
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
    render(AddProjectModal, { props: { dark: false } })

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

  it('creates a new project, closes modal, and emits callbacks', async () => {
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

    render(AddProjectModal, {
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
