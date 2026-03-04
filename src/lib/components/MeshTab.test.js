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

describe('MeshTab', () => {
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
    coordinationPreflightCheck.mockResolvedValue({
      blockingErrors: [],
      agentWarnings: [],
    })
    installMesh.mockResolvedValue('Mesh installed successfully: mesh 0.1.0')

    coordinationListTeams.mockResolvedValue([])
    coordinationGetLiveTeamStatus.mockResolvedValue({
      teamName: 'architecture-final',
      leadName: 'team-lead',
      members: [
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
          model: 'gpt-5.3-codex',
          projectId: 'proj-web',
          sessionStatus: 'idle',
          paneId: '%2',
        },
      ],
    })

    coordinationInitializeTeam.mockResolvedValue({
      teamName: 'architecture-final',
      failedStep: null,
      retryable: false,
      message: 'team initialized',
      steps: [{ step: 'validate_configuration', status: 'succeeded', message: 'ok' }],
    })

    coordinationAddAgent.mockResolvedValue({
      teamName: 'architecture-final',
      memberName: 'backend-dev',
      failedStep: null,
      message: 'agent added',
      steps: [],
    })

    coordinationDisbandTeam.mockResolvedValue({
      teamName: 'architecture-final',
      disbanded: true,
      alreadyDisbanded: false,
      message: 'team disbanded',
    })

    coordinationRemoveMember.mockResolvedValue({
      teamName: 'architecture-final',
      memberName: 'frontend-dev',
      removed: true,
      message: 'member removed',
      steps: [],
      warnings: [],
    })

    coordinationResumeMember.mockResolvedValue({
      teamName: 'architecture-final',
      memberName: 'frontend-dev',
      resumed: true,
      failedStep: null,
      message: 'member resumed',
      steps: [],
      warnings: [],
    })

    listRoleTemplates.mockResolvedValue([
      { roleId: 'lead-default', name: 'Lead', kind: 'lead', cliTool: 'claude', model: 'opus' },
      { roleId: 'agent-default', name: 'Agent', kind: 'agent', cliTool: 'codex', model: 'gpt-5.3-codex' },
    ])
    listTeamPresets.mockResolvedValue([
      {
        presetId: 'fullstack-dev',
        name: 'Full Stack Dev Team',
        description: 'Lead + agents',
        leadRoleId: 'lead-default',
        roleCount: 3,
        agentCount: 2,
        tools: ['claude', 'codex'],
      },
    ])
    getRoleTemplate.mockResolvedValue({ roleId: 'lead-default', instructions: 'Lead the team.' })
    getTeamPreset.mockResolvedValue({
      presetId: 'fullstack-dev',
      leadRoleId: 'lead-default',
      agentSlots: [{ roleId: 'agent-default', count: 2 }],
    })

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

  it('renders availability gate in gate mode before resolving project team state', () => {
    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    expect(screen.getByTestId('mesh-mode-gate')).toBeInTheDocument()
  })

  it('transitions to empty mode when no existing team matches project', async () => {
    coordinationListTeams.mockResolvedValueOnce([])

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-empty-state')).toBeInTheDocument()
  })

  it('transitions to runtime mode when matching project team exists', async () => {
    coordinationListTeams.mockResolvedValueOnce([
      { teamName: 'architecture-final', leadProjectPath: '/projects/taurhaus' },
      { teamName: 'ops-team', leadProjectPath: '/projects/ops' },
    ])

    render(MeshTab, {
      props: {
        dark: true,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
  })

  it('empty -> setup transition via start custom', async () => {
    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-build-custom'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-canvas')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-action-bar')).toBeInTheDocument()
  })

  it('setup -> initializing -> runtime transition works through initialize flow', async () => {
    const init = deferred()
    coordinationInitializeTeam.mockReturnValueOnce(init.promise)

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
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
      teamName: 'architecture-final',
      failedStep: null,
      retryable: false,
      message: 'team initialized',
      steps: [{ step: 'validate_configuration', status: 'succeeded', message: 'ok' }],
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
  })

  it('opens and closes template, customizer, and add-agent slideovers', async () => {
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
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-add-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-form')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-cancel'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-disband'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('confirm-dialog-cancel'))

    await fireEvent.click(screen.getByTestId('mesh-runtime-disband'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-browse-catalog'))
    await waitFor(() => {
      expect(screen.getByTestId('template-browser-panel')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getAllByTestId('slideover-close').at(-1))
    await waitFor(() => {
      expect(screen.queryByTestId('template-browser-panel')).not.toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-build-custom'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-action-customize'))
    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-panel')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getAllByTestId('slideover-close').at(-1))
    await waitFor(() => {
      expect(screen.queryByTestId('team-customizer-panel')).not.toBeInTheDocument()
    })
  })

  it('reset returns setup state back to empty mode', async () => {
    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-build-custom'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-action-reset'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })
  })

  it('submits add-agent flow in runtime mode', async () => {
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
          teamName: 'architecture-final',
          agent: expect.objectContaining({
            name: 'backend-dev',
            projectId: 'proj-api',
          }),
        })
      )
    })
  })
})
