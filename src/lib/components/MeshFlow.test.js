import { describe, it, expect, vi, beforeEach, beforeAll, afterAll } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  checkMeshInstallStatus: vi.fn(),
  composeTeam: vi.fn(),
  coordinationAddAgent: vi.fn(),
  coordinationGetCompactionAudit: vi.fn(),
  coordinationDisbandTeam: vi.fn(),
  coordinationGetProjectMeshSnapshot: vi.fn(),
  coordinationGetLiveTeamStatus: vi.fn(),
  coordinationInitializeTeam: vi.fn(),
  coordinationListTeams: vi.fn(),
  coordinationPreflightCheck: vi.fn(),
  coordinationRemoveMember: vi.fn(),
  coordinationResumeTeam: vi.fn(),
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
  coordinationGetCompactionAudit,
  coordinationDisbandTeam,
  coordinationGetProjectMeshSnapshot,
  coordinationGetLiveTeamStatus,
  coordinationInitializeTeam,
  coordinationListTeams,
  coordinationPreflightCheck,
  coordinationRemoveMember,
  coordinationResumeTeam,
  coordinationResumeMember,
  getRoleTemplate,
  getTeamPreset,
  installMesh,
  listRoleTemplates,
  listTeamPresets,
  onCoordinationStepProgress,
} = await import('../ipc.js')

import MeshTab from './MeshTab.svelte'
import { resetMeshCache } from '../meshCache.svelte.js'

function deferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

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

describe('Mesh flow smoke', () => {
  let rosterMembers

  function buildProjectMeshSnapshot(overrides = {}) {
    return {
      meshAvailable: overrides.meshAvailable ?? true,
      tmuxAvailable: overrides.tmuxAvailable ?? true,
      teamName: overrides.teamName ?? null,
      teamRuntimeState: overrides.teamRuntimeState ?? 'none',
      teamStatus: overrides.teamStatus ?? null,
      warnings: overrides.warnings ?? [],
    }
  }

  beforeEach(() => {
    vi.clearAllMocks()
    resetMeshCache()

    checkMeshInstallStatus.mockResolvedValue({
      installed: true,
      version: '0.1.0',
      bundled_version: '0.1.0',
      needs_update: false,
      environment_available: true,
      error: null,
    })
    coordinationPreflightCheck.mockResolvedValue({
      blockingErrors: [],
      agentWarnings: [],
    })
    installMesh.mockResolvedValue({
      success: true,
      message: 'Mesh installed successfully: mesh 0.1.0',
    })

    coordinationListTeams.mockResolvedValue([])
    coordinationGetProjectMeshSnapshot.mockResolvedValue(buildProjectMeshSnapshot())

    rosterMembers = [
      {
        name: 'team-lead',
        role: 'lead',
        cliTool: 'claude',
        model: 'opus',
        projectId: 'proj-core',
        sessionStatus: 'active',
        paneId: '%1',
      },
      {
        name: 'frontend-dev',
        role: 'member',
        cliTool: 'codex',
        model: 'gpt-5.4 high',
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
    coordinationGetCompactionAudit.mockResolvedValue({
      teamName: 'taurhaus-team',
      entries: [],
    })

    coordinationInitializeTeam.mockResolvedValue({
      teamName: 'taurhaus-team',
      failedStep: null,
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

    coordinationRemoveMember.mockResolvedValue({
      teamName: 'taurhaus-team',
      memberName: 'frontend-dev',
      removed: true,
      message: 'member removed',
      steps: [],
      warnings: [],
    })

    coordinationResumeTeam.mockResolvedValue({
      teamName: 'taurhaus-team',
      resumed: true,
      totalMembers: 2,
      resumedMembers: ['team-lead', 'frontend-dev'],
      failedMembers: [],
      warnings: [],
      startedTeamDaemon: false,
      teamDaemonWarning: null,
    })

    coordinationResumeMember.mockResolvedValue({
      teamName: 'taurhaus-team',
      memberName: 'frontend-dev',
      resumed: true,
      failedStep: null,
      message: 'member resumed',
      steps: [],
      warnings: [],
    })

    listRoleTemplates.mockResolvedValue([
      { roleId: 'lead-default', name: 'Lead', kind: 'lead', cliTool: 'claude', model: 'opus' },
      { roleId: 'agent-default', name: 'Agent', kind: 'agent', cliTool: 'codex', model: 'gpt-5.4 high' },
    ])
    listTeamPresets.mockResolvedValue([])
    getRoleTemplate.mockResolvedValue({ roleId: 'lead-default', instructions: 'Lead the team.' })
    getTeamPreset.mockResolvedValue({ presetId: 'preset-a', agentSlots: [] })

    composeTeam.mockResolvedValue({
      roster: [
        {
          name: 'team-lead',
          roleId: 'lead-default',
          roleKind: 'lead',
          cliTool: 'claude',
          model: 'opus',
          instructions: '',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: '/projects/taurhaus',
        },
      ],
      warnings: [],
      validationErrors: [],
    })

    onCoordinationStepProgress.mockResolvedValue(() => {})
  })

  it('setup -> initialize progress -> runtime canvas -> hot-add -> disband', async () => {
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
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-build-custom'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-initializing')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-init-progress')).toBeInTheDocument()
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
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('taurhaus-team')
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
          teamName: 'taurhaus-team',
          agent: expect.objectContaining({
            name: 'backend-dev',
            projectId: 'proj-api',
          }),
        })
      )
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-more-toggle'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-disband')).toBeInTheDocument()
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

  it('unavailable mode shows blocking error', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildProjectMeshSnapshot({
      meshAvailable: false,
      warnings: ['mesh binary not found'],
    }))

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-availability-inline')).toBeInTheDocument()
    })

    expect(screen.getByTestId('mesh-availability-inline')).toHaveTextContent('mesh binary not found')
  })
})
