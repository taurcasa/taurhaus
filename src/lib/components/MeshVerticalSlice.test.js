import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  checkMeshInstallStatus: vi.fn(),
  coordinationAddAgent: vi.fn(),
  coordinationDisbandTeam: vi.fn(),
  coordinationGetFeatureAvailability: vi.fn(),
  coordinationGetLiveTeamStatus: vi.fn(),
  coordinationInitializeTeam: vi.fn(),
  coordinationListTeams: vi.fn(),
  coordinationPreflightCheck: vi.fn(),
  installMesh: vi.fn(),
  onCoordinationStepProgress: vi.fn(),
}))

const {
  checkMeshInstallStatus,
  coordinationAddAgent,
  coordinationDisbandTeam,
  coordinationGetFeatureAvailability,
  coordinationGetLiveTeamStatus,
  coordinationInitializeTeam,
  coordinationListTeams,
  coordinationPreflightCheck,
  installMesh,
  onCoordinationStepProgress,
} = await import('../ipc.js')

import MeshTab from './MeshTab.svelte'

function deferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('Mesh vertical slice smoke', () => {
  let rosterMembers
  let progressHandler

  beforeEach(() => {
    vi.clearAllMocks()
    progressHandler = null
    globalThis.confirm = vi.fn(() => true)
    checkMeshInstallStatus.mockResolvedValue({
      installed: true,
      version: '0.1.0',
      bundled_version: '0.1.0',
      needs_update: false,
      environment_available: true,
      error: null,
    })
    installMesh.mockResolvedValue('Mesh installed successfully: mesh 0.1.0')

    coordinationGetFeatureAvailability.mockResolvedValue({
      canInitialize: true,
      meshAvailable: true,
      tmuxAvailable: true,
      blockingErrors: [],
    })
    coordinationPreflightCheck.mockResolvedValue({
      blockingErrors: [],
      agentWarnings: [],
    })
    coordinationListTeams.mockResolvedValue([])

    rosterMembers = [
      {
        name: 'team-lead',
        role: 'lead',
        cliTool: 'Claude',
        model: 'opus',
        projectId: 'proj-core',
        sessionStatus: 'active',
        paneId: '%1',
      },
      {
        name: 'frontend-dev',
        role: 'member',
        cliTool: 'Codex',
        model: 'gpt-5.3',
        projectId: 'proj-web',
        sessionStatus: 'idle',
        paneId: '%2',
      },
    ]

    coordinationGetLiveTeamStatus.mockImplementation(async (teamName) => ({
      teamName,
      leadName: 'team-lead',
      members: rosterMembers,
    }))
    coordinationAddAgent.mockImplementation(async ({ teamName, agent }) => {
      rosterMembers = [
        ...rosterMembers,
        {
          name: agent.name,
          role: 'member',
          cliTool: agent.cliTool,
          model: agent.model,
          projectId: agent.projectId,
          sessionStatus: 'offline',
          paneId: '%9',
        },
      ]
      return {
        teamName,
        memberName: agent.name,
        failedStep: null,
        message: 'agent added',
        steps: [{ step: 'update_roster', status: 'succeeded', message: 'team roster updated' }],
      }
    })
    coordinationDisbandTeam.mockResolvedValue({
      teamName: 'taurhaus-team',
      disbanded: true,
      alreadyDisbanded: false,
      message: 'team disbanded',
    })
    onCoordinationStepProgress.mockImplementation(async (callback) => {
      progressHandler = callback
      return () => {}
    })
  })

  it('full flow: setup -> init progress -> runtime roster -> hot-add -> disband', async () => {
    const init = deferred()
    coordinationInitializeTeam.mockReturnValueOnce(init.promise)

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects: [
          { id: 'proj-web', name: 'Web' },
          { id: 'proj-api', name: 'API' },
        ],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-setup-title')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-advanced-toggle'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-team-basics')).toBeInTheDocument()
    })

    await fireEvent.input(screen.getByTestId('mesh-agent-name-input-0'), {
      target: { value: 'frontend-dev' },
    })
    await fireEvent.change(screen.getByTestId('mesh-agent-project-select-0'), {
      target: { value: 'proj-web' },
    })
    await fireEvent.click(screen.getByTestId('mesh-create-team-button'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-progress')).toBeInTheDocument()
    })
    await waitFor(() => {
      expect(typeof progressHandler).toBe('function')
    })

    progressHandler({
      payload: {
        teamName: 'taurhaus-team',
        operation: 'initialize_team',
        progress: {
          step: 'validate_configuration',
          status: 'running',
          message: null,
        },
      },
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-icon-validate_configuration')).toHaveTextContent('●')
    })

    init.resolve({
      teamName: 'taurhaus-team',
      failedStep: null,
      retryable: false,
      message: 'team initialized',
      steps: [
        { step: 'validate_configuration', status: 'succeeded', message: 'ok' },
        { step: 'create_team', status: 'succeeded', message: 'ok' },
      ],
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('taurhaus-team')
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-roster-card-frontend-dev')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-form')).toBeInTheDocument()
    })
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
          teamName: 'taurhaus-team',
          agent: expect.objectContaining({
            name: 'backend-dev',
            projectId: 'proj-api',
          }),
        })
      )
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-roster-card-backend-dev')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-overflow-menu-button'))
    await fireEvent.click(screen.getByTestId('mesh-disband-button'))
    await waitFor(() => {
      expect(globalThis.confirm).toHaveBeenCalled()
    })
    await waitFor(() => {
      expect(coordinationDisbandTeam).toHaveBeenCalledWith('taurhaus-team')
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-setup-title')).toBeInTheDocument()
    })
  })

  it('error branch: hot-add failure shows inline error and keeps runtime view', async () => {
    coordinationListTeams.mockResolvedValueOnce([
      { teamName: 'taurhaus-team', leadProjectPath: '/projects/taurhaus' },
    ])
    coordinationAddAgent.mockRejectedValueOnce(new Error('hot-add failed'))

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects: [{ id: 'proj-api', name: 'API' }],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('taurhaus-team')
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
      expect(screen.getByTestId('mesh-add-agent-error')).toHaveTextContent('hot-add failed')
    })
    expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('taurhaus-team')
  })
})
