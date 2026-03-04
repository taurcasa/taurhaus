import { describe, it, expect, vi, beforeEach, beforeAll, afterAll } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  checkMeshInstallStatus: vi.fn(),
  composeTeam: vi.fn(),
  coordinationAddAgent: vi.fn(),
  coordinationDisbandTeam: vi.fn(),
  coordinationGetLiveTeamStatus: vi.fn(),
  coordinationInitializeTeam: vi.fn(),
  coordinationListTeams: vi.fn(),
  coordinationPreflightCheck: vi.fn(),
  coordinationRemoveMember: vi.fn(),
  coordinationResumeMember: vi.fn(),
  getRoleTemplate: vi.fn(),
  getTeamPreset: vi.fn(),
  installMesh: vi.fn(),
  listRoleTemplates: vi.fn(),
  listTeamPresets: vi.fn(),
  onCoordinationStepProgress: vi.fn(),
}))

const {
  checkMeshInstallStatus,
  composeTeam,
  coordinationAddAgent,
  coordinationDisbandTeam,
  coordinationGetLiveTeamStatus,
  coordinationInitializeTeam,
  coordinationListTeams,
  coordinationPreflightCheck,
  coordinationRemoveMember,
  coordinationResumeMember,
  getRoleTemplate,
  getTeamPreset,
  installMesh,
  listRoleTemplates,
  listTeamPresets,
  onCoordinationStepProgress,
} = await import('../ipc.js')

import MeshTab from './MeshTab.svelte'

let previousResizeObserver

beforeAll(() => {
  previousResizeObserver = globalThis.ResizeObserver
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
})

afterAll(() => {
  if (previousResizeObserver) {
    globalThis.ResizeObserver = previousResizeObserver
    return
  }
  delete globalThis.ResizeObserver
})

describe('Mesh vertical slice smoke', () => {
  let rosterMembers

  beforeEach(() => {
    vi.clearAllMocks()

    checkMeshInstallStatus.mockResolvedValue({
      installed: true,
      version: '0.1.0',
      bundled_version: '0.1.0',
      needs_update: false,
      environment_available: true,
      error: null,
    })
    installMesh.mockResolvedValue('Mesh installed successfully: mesh 0.1.0')

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
        model: 'gpt-5.3-codex',
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

    coordinationInitializeTeam.mockResolvedValue({
      team_name: 'taurhaus-team',
      failed_step: null,
      retryable: false,
      message: 'team initialized',
      steps: [
        { step: 'validate_configuration', status: 'succeeded', message: 'ok' },
        { step: 'create_team', status: 'succeeded', message: 'ok' },
      ],
    })

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

    coordinationRemoveMember.mockResolvedValue({
      team_name: 'taurhaus-team',
      member_name: 'frontend-dev',
      removed: true,
      message: 'member removed',
      steps: [],
      warnings: [],
    })

    coordinationResumeMember.mockResolvedValue({
      team_name: 'taurhaus-team',
      member_name: 'frontend-dev',
      resumed: true,
      failed_step: null,
      message: 'member resumed',
      steps: [],
      warnings: [],
    })

    listRoleTemplates.mockResolvedValue([
      { role_id: 'lead-default', name: 'Lead', kind: 'lead', cli_tool: 'claude', model: 'opus' },
      { role_id: 'agent-default', name: 'Agent', kind: 'agent', cli_tool: 'codex', model: 'gpt-5.3-codex' },
    ])
    listTeamPresets.mockResolvedValue([])
    getRoleTemplate.mockResolvedValue({ role_id: 'lead-default', instructions: 'Lead the team.' })
    getTeamPreset.mockResolvedValue({ preset_id: 'preset-a', agent_slots: [] })

    composeTeam.mockResolvedValue({
      roster: [
        {
          name: 'team-lead',
          role_id: 'lead-default',
          role_kind: 'lead',
          cli_tool: 'claude',
          model: 'opus',
          instructions: '',
          capabilities: [],
          project_binding: 'lead_project',
          project_id: '/projects/taurhaus',
        },
      ],
      warnings: [],
      validation_errors: [],
    })

    onCoordinationStepProgress.mockResolvedValue(() => {})
  })

  it('full flow: empty -> setup -> initializing -> runtime -> add -> disband', async () => {
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
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-build-custom'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-add-agent'))
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
          agent: expect.objectContaining({
            name: 'backend-dev',
            projectId: 'proj-api',
          }),
        })
      )
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-disband'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))

    await waitFor(() => {
      expect(coordinationDisbandTeam).toHaveBeenCalledWith('taurhaus-team')
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })
  })

  it('hot-add failure shows inline error and keeps runtime view', async () => {
    coordinationListTeams.mockResolvedValueOnce([
      { team_name: 'taurhaus-team', lead_project_path: '/projects/taurhaus' },
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
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-add-agent'))
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
    expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
  })
})
