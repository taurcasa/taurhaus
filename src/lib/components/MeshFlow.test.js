import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  coordinationAddAgent: vi.fn(),
  coordinationDisbandTeam: vi.fn(),
  coordinationGetFeatureAvailability: vi.fn(),
  coordinationGetLiveTeamStatus: vi.fn(),
  coordinationInitializeTeam: vi.fn(),
  coordinationListTeams: vi.fn(),
  coordinationPreflightCheck: vi.fn(),
  onCoordinationStepProgress: vi.fn(),
}))

const {
  coordinationAddAgent,
  coordinationDisbandTeam,
  coordinationGetFeatureAvailability,
  coordinationGetLiveTeamStatus,
  coordinationInitializeTeam,
  coordinationListTeams,
  coordinationPreflightCheck,
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

describe('Mesh flow smoke', () => {
  let rosterMembers
  let progressHandler

  beforeEach(() => {
    vi.clearAllMocks()
    progressHandler = null
    globalThis.confirm = vi.fn(() => true)

    coordinationGetFeatureAvailability.mockResolvedValue({
      canInitialize: true,
      meshAvailable: true,
      tmuxAvailable: true,
      blockingErrors: [],
    })
    coordinationPreflightCheck.mockResolvedValue({
      blocking_errors: [],
      agent_warnings: [],
    })
    coordinationListTeams.mockResolvedValue([])

    rosterMembers = [
      {
        name: 'team-lead',
        role: 'lead',
        cli_tool: 'claude',
        model: 'opus',
        project_id: 'proj-core',
        session_status: 'active',
        pane_id: '%1',
      },
      {
        name: 'frontend-dev',
        role: 'member',
        cli_tool: 'codex',
        model: 'gpt-5.3',
        project_id: 'proj-web',
        session_status: 'idle',
        pane_id: '%2',
      },
    ]

    coordinationGetLiveTeamStatus.mockImplementation(async (teamName) => ({
      team_name: teamName,
      lead_name: 'team-lead',
      members: rosterMembers,
    }))

    coordinationAddAgent.mockImplementation(async ({ teamName, agent }) => {
      rosterMembers = [
        ...rosterMembers,
        {
          name: agent.name,
          role: 'member',
          cli_tool: agent.cliTool,
          model: agent.model,
          project_id: agent.projectId,
          session_status: 'offline',
          pane_id: '%9',
        },
      ]
      return {
        team_name: teamName,
        member_name: agent.name,
        failed_step: null,
        message: 'agent added',
        steps: [{ step: 'update_roster', status: 'succeeded', message: 'team roster updated' }],
      }
    })

    coordinationDisbandTeam.mockResolvedValue({
      team_name: 'taurhaus-team',
      disbanded: true,
      already_disbanded: false,
      message: 'team disbanded',
    })

    onCoordinationStepProgress.mockImplementation(async (callback) => {
      progressHandler = callback
      return () => {}
    })
  })

  it('setup -> initialize progress -> runtime roster -> hot-add -> disband', async () => {
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
      team_name: 'taurhaus-team',
      failed_step: null,
      retryable: false,
      message: 'team initialized',
      steps: [
        { step: 'validate_configuration', status: 'succeeded', message: 'ok' },
        { step: 'create_team', status: 'succeeded', message: 'ok' },
      ],
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('Team: taurhaus-team')
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

  it('unavailable mode shows blocking error', async () => {
    coordinationPreflightCheck.mockResolvedValue({
      blocking_errors: ['mesh binary not found'],
      agent_warnings: [],
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

    expect(screen.getByTestId('mesh-availability-error')).toHaveTextContent('mesh binary not found')
  })
})
