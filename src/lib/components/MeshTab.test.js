import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  coordinationAddAgent: vi.fn(),
  coordinationDisbandTeam: vi.fn(),
  coordinationListTeams: vi.fn(),
  coordinationInitializeTeam: vi.fn(),
  coordinationPreflightCheck: vi.fn(),
  onCoordinationStepProgress: vi.fn(),
}))

const {
  coordinationAddAgent,
  coordinationDisbandTeam,
  coordinationListTeams,
  coordinationInitializeTeam,
  coordinationPreflightCheck,
  onCoordinationStepProgress,
} = await import('../ipc.js')

import MeshTab from './MeshTab.svelte'

function createDeferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('MeshTab', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    globalThis.confirm = vi.fn(() => true)
    coordinationAddAgent.mockResolvedValue({
      teamName: 'architecture-final',
      memberName: 'backend-dev',
      failedStep: null,
      message: 'agent added',
      steps: [],
    })
    coordinationPreflightCheck.mockResolvedValue({
      blockingErrors: [],
      agentWarnings: [],
    })
    coordinationListTeams.mockResolvedValue([])
    coordinationDisbandTeam.mockResolvedValue({
      teamName: 'architecture-final',
      disbanded: true,
      alreadyDisbanded: false,
      message: 'team disbanded',
    })
    coordinationInitializeTeam.mockResolvedValue({
      teamName: 'architecture-final',
      failedStep: null,
      retryable: false,
      message: 'team initialized',
      steps: [{ step: 'validate_configuration', status: 'succeeded', message: 'ok' }],
    })
    onCoordinationStepProgress.mockResolvedValue(() => {})
  })

  it('renders setup mode when no teams exist', async () => {
    coordinationListTeams.mockResolvedValueOnce([])

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-setup-title')).toBeInTheDocument()
    })

    expect(screen.getByTestId('mesh-setup-description')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-create-team-button')).toBeInTheDocument()
  })

  it('renders missing-binary setup prompt and disables initialize CTA', async () => {
    coordinationPreflightCheck.mockResolvedValue({
      blockingErrors: ['Mesh CLI not found. Install it to enable multi-agent collaboration.'],
      agentWarnings: [],
    })

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-availability-title')).toBeInTheDocument()
    })

    expect(screen.getByTestId('mesh-availability-error')).toHaveTextContent(
      'Mesh CLI not found. Install it to enable multi-agent collaboration.'
    )
    expect(screen.queryByTestId('mesh-create-team-button')).not.toBeInTheDocument()
  })

  it('renders runtime mode when team exists for current project', async () => {
    coordinationListTeams.mockResolvedValueOnce([
      { teamName: 'architecture-final', leadProjectPath: '/projects/taurhaus' },
      { teamName: 'ops', leadProjectPath: '/projects/ops' },
    ])

    render(MeshTab, {
      props: {
        dark: true,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    })
    expect(screen.getByTestId('mesh-runtime-placeholder')).toBeInTheDocument()
  })

  it('switches mode when project changes', async () => {
    coordinationListTeams
      .mockResolvedValueOnce([
        { teamName: 'architecture-final', leadProjectPath: '/projects/taurhaus' },
      ])
      .mockResolvedValueOnce([
        { teamName: 'architecture-final', leadProjectPath: '/projects/taurhaus' },
      ])

    const view = render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    })

    await view.rerender({
      dark: false,
      projectPath: '/projects/different-project',
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-setup-title')).toBeInTheDocument()
    })

    expect(coordinationListTeams).toHaveBeenCalledTimes(2)
  })

  it('add agent shows form and submitting calls coordinationAddAgent', async () => {
    coordinationListTeams.mockResolvedValueOnce([
      { teamName: 'architecture-final', leadProjectPath: '/projects/taurhaus' },
    ])

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects: [{ id: 'proj-api', name: 'API' }],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    expect(screen.getByTestId('mesh-add-agent-form')).toBeInTheDocument()
    const toolLabels = Array.from(
      screen.getByTestId('mesh-add-agent-tool-select').querySelectorAll('option')
    ).map((option) => option.textContent)
    expect(toolLabels).toEqual(['Claude', 'Codex', 'Gemini'])
    await fireEvent.input(screen.getByTestId('mesh-add-agent-name-input'), {
      target: { value: 'backend-dev' },
    })
    await fireEvent.change(screen.getByTestId('mesh-add-agent-project-select'), {
      target: { value: 'proj-api' },
    })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-submit'))

    await waitFor(() => {
      expect(coordinationAddAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          teamName: 'architecture-final',
          agent: expect.objectContaining({
            name: 'backend-dev',
            projectId: 'proj-api',
          }),
        })
      )
    })
  })

  it('shows add-agent loading text while submission is in progress', async () => {
    const deferred = createDeferred()
    coordinationAddAgent.mockReturnValueOnce(deferred.promise)
    coordinationListTeams.mockResolvedValueOnce([
      { teamName: 'architecture-final', leadProjectPath: '/projects/taurhaus' },
    ])

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects: [{ id: 'proj-api', name: 'API' }],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    await fireEvent.input(screen.getByTestId('mesh-add-agent-name-input'), {
      target: { value: 'backend-dev' },
    })
    await fireEvent.change(screen.getByTestId('mesh-add-agent-project-select'), {
      target: { value: 'proj-api' },
    })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-submit'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-submit')).toHaveTextContent('Adding...')
      expect(screen.getByTestId('mesh-add-agent-submit')).toBeDisabled()
    })

    deferred.resolve({
      teamName: 'architecture-final',
      memberName: 'backend-dev',
      failedStep: null,
      message: 'agent added',
      steps: [],
    })

    await waitFor(() => {
      expect(screen.queryByTestId('mesh-add-agent-form')).not.toBeInTheDocument()
    })
  })

  it('disband confirms, calls coordinationDisbandTeam, and returns to setup view', async () => {
    coordinationListTeams.mockResolvedValueOnce([
      { teamName: 'architecture-final', leadProjectPath: '/projects/taurhaus' },
    ])

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    })

    await fireEvent.click(screen.getByTestId('mesh-overflow-menu-button'))
    await fireEvent.click(screen.getByTestId('mesh-disband-button'))
    expect(globalThis.confirm).toHaveBeenCalled()

    await waitFor(() => {
      expect(coordinationDisbandTeam).toHaveBeenCalledWith('architecture-final')
      expect(screen.getByTestId('mesh-setup-title')).toBeInTheDocument()
    })
  })

  it('shows disband loading state while disband call is running', async () => {
    const deferred = createDeferred()
    coordinationDisbandTeam.mockReturnValueOnce(deferred.promise)
    coordinationListTeams.mockResolvedValueOnce([
      { teamName: 'architecture-final', leadProjectPath: '/projects/taurhaus' },
    ])

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    })

    await fireEvent.click(screen.getByTestId('mesh-overflow-menu-button'))
    await fireEvent.click(screen.getByTestId('mesh-disband-button'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-disband-button')).toHaveTextContent('Disbanding...')
      expect(screen.getByTestId('mesh-disband-button')).toBeDisabled()
    })

    deferred.resolve({
      teamName: 'architecture-final',
      disbanded: true,
      alreadyDisbanded: false,
      message: 'team disbanded',
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-setup-title')).toBeInTheDocument()
    })
  })

  it('disband cancelled does nothing', async () => {
    globalThis.confirm = vi.fn(() => false)
    coordinationListTeams.mockResolvedValueOnce([
      { teamName: 'architecture-final', leadProjectPath: '/projects/taurhaus' },
    ])

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    })

    await fireEvent.click(screen.getByTestId('mesh-overflow-menu-button'))
    await fireEvent.click(screen.getByTestId('mesh-disband-button'))

    expect(coordinationDisbandTeam).not.toHaveBeenCalled()
    expect(screen.getByTestId('mesh-runtime-title')).toBeInTheDocument()
  })

  it('dismisses runtime success message when close button is clicked', async () => {
    coordinationListTeams.mockResolvedValueOnce([
      { teamName: 'architecture-final', leadProjectPath: '/projects/taurhaus' },
    ])

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects: [{ id: 'proj-api', name: 'API' }],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    await fireEvent.input(screen.getByTestId('mesh-add-agent-name-input'), {
      target: { value: 'backend-dev' },
    })
    await fireEvent.change(screen.getByTestId('mesh-add-agent-project-select'), {
      target: { value: 'proj-api' },
    })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-submit'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-message')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-dismiss-runtime-message'))

    await waitFor(() => {
      expect(screen.queryByTestId('mesh-runtime-message')).not.toBeInTheDocument()
    })
  })
})
