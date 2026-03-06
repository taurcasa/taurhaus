import { describe, it, expect, vi, beforeEach, beforeAll, afterAll } from 'vitest'
import { render, screen, waitFor, fireEvent, within } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

vi.mock('../ipc.js', () => ({
  checkMeshInstallStatus: vi.fn(),
  composeTeam: vi.fn(),
  coordinationAddAgent: vi.fn(),
  coordinationDisbandTeam: vi.fn(),
  coordinationGetProjectMeshSnapshot: vi.fn(),
  coordinationGetLiveTeamStatus: vi.fn(),
  coordinationInitializeTeam: vi.fn(),
  coordinationPreflightCheck: vi.fn(),
  coordinationRemoveMember: vi.fn(),
  coordinationResumeMember: vi.fn(),
  getRoleTemplate: vi.fn(),
  getTeamPreset: vi.fn(),
  installMesh: vi.fn(),
  listRoleTemplates: vi.fn(),
  listTeamPresets: vi.fn(),
  onCoordinationStepProgress: vi.fn(),
  upsertRoleTemplate: vi.fn(),
}))

const {
  checkMeshInstallStatus,
  composeTeam,
  coordinationAddAgent,
  coordinationDisbandTeam,
  coordinationGetProjectMeshSnapshot,
  coordinationGetLiveTeamStatus,
  coordinationInitializeTeam,
  coordinationPreflightCheck,
  coordinationRemoveMember,
  coordinationResumeMember,
  getRoleTemplate,
  getTeamPreset,
  installMesh,
  listRoleTemplates,
  listTeamPresets,
  onCoordinationStepProgress,
  upsertRoleTemplate,
} = await import('../ipc.js')

import MeshTab from './MeshTab.svelte'
import { clearMeshCache, getMeshCache, resetMeshCache, setMeshCache } from '../meshCache.svelte.js'

const appCss = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8')

function deferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function buildLiveTeamStatus(overrides = {}) {
  return {
    teamName: overrides.teamName ?? 'architecture-final',
    leadName: overrides.leadName ?? 'team-lead',
    members: overrides.members ?? [
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
        description: 'Implements UI surface details for the mesh canvas.',
        sessionStatus: 'idle',
        paneId: '%2',
      },
    ],
  }
}

function buildProjectMeshSnapshot(overrides = {}) {
  return {
    meshAvailable: overrides.meshAvailable ?? true,
    tmuxAvailable: overrides.tmuxAvailable ?? true,
    teamName: overrides.teamName ?? null,
    teamStatus: overrides.teamStatus ?? null,
    warnings: overrides.warnings ?? [],
  }
}

function buildRuntimeSnapshot(overrides = {}) {
  const liveStatus = buildLiveTeamStatus(overrides)
  return buildProjectMeshSnapshot({
    meshAvailable: overrides.meshAvailable ?? true,
    tmuxAvailable: overrides.tmuxAvailable ?? true,
    teamName: liveStatus.teamName,
    warnings: overrides.warnings ?? [],
    teamStatus: {
      leadName: liveStatus.leadName,
      members: liveStatus.members.map(({ model, ...member }) => member),
    },
  })
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

    coordinationGetProjectMeshSnapshot.mockResolvedValue(buildProjectMeshSnapshot())
    coordinationGetLiveTeamStatus.mockResolvedValue(buildLiveTeamStatus())

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
      { roleId: 'agent-default', name: 'Agent', kind: 'agent', cliTool: 'codex', model: 'gpt-5.4 high' },
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
    upsertRoleTemplate.mockResolvedValue({
      roleId: 'frontend-dev',
      name: 'frontend-dev',
      kind: 'agent',
      builtIn: false,
      readOnly: false,
    })
  })

  async function renderRuntime(overrides = {}) {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        ...overrides,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })
  }

  it('renders from cached snapshot immediately on revisit without snapshot IPC', () => {
    setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())
    const liveRefresh = deferred()
    coordinationGetLiveTeamStatus.mockReturnValueOnce(liveRefresh.promise)

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    expect(coordinationGetProjectMeshSnapshot).not.toHaveBeenCalled()

    liveRefresh.resolve(buildLiveTeamStatus())
  })

  it('cache miss triggers snapshot IPC and updates the cache', async () => {
    const snapshot = buildProjectMeshSnapshot({
      meshAvailable: true,
      tmuxAvailable: true,
      teamName: null,
      teamStatus: null,
      warnings: [],
    })
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(snapshot)

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
    expect(coordinationGetProjectMeshSnapshot).toHaveBeenCalledWith('/projects/taurhaus')
    expect(getMeshCache('/projects/taurhaus')).toEqual(snapshot)
  })

  it('background live refresh patches member status after cached render', async () => {
    setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())
    const liveRefresh = deferred()
    coordinationGetLiveTeamStatus.mockReturnValueOnce(liveRefresh.promise)

    render(MeshTab, {
      props: {
        dark: true,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Idle')
    })

    liveRefresh.resolve(buildLiveTeamStatus({
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
          model: 'gpt-5.4 high',
          projectId: 'proj-web',
          description: 'Implements UI surface details for the mesh canvas.',
          sessionStatus: 'active',
          paneId: '%2',
        },
      ],
    }))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Active')
    })
  })

  it('shows availability inline and skips preflight gating on mount', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildProjectMeshSnapshot({
      meshAvailable: false,
      tmuxAvailable: true,
      warnings: ['Mesh CLI is unavailable for this environment.'],
    }))

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    expect(screen.queryByTestId('mesh-mode-gate')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-availability-inline')).toHaveTextContent('Mesh CLI is unavailable')
    expect(coordinationPreflightCheck).not.toHaveBeenCalled()
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

  it('initialization failure exposes recovery controls and back returns to setup', async () => {
    coordinationInitializeTeam.mockResolvedValueOnce({
      teamName: 'architecture-final',
      failedStep: 'create_team',
      retryable: true,
      message: 'team already exists',
      steps: [
        { step: 'validate_configuration', status: 'succeeded', message: 'ok' },
        { step: 'create_team', status: 'failed', message: 'conflict' },
      ],
    })

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
    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-failure')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-init-retry-button')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-init-back-button')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-init-back-button'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })
  })

  it('opens and closes template, customizer, and add-agent slideovers', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    const runtimeView = render(MeshTab, {
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
    expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()

    runtimeView.unmount()
    clearMeshCache('/projects/taurhaus')

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

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
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

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

  it('shows loading state for role picker while role templates load', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())
    const rolesLoad = deferred()
    listRoleTemplates.mockReturnValueOnce(rolesLoad.promise)

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

    const roleSelect = screen.getByTestId('mesh-add-agent-role-select')
    expect(roleSelect).toBeDisabled()
    expect(screen.getByRole('option', { name: 'Loading roles...' })).toBeInTheDocument()

    rolesLoad.resolve([
      { roleId: 'lead-default', name: 'Lead', kind: 'lead', cliTool: 'claude', model: 'opus' },
    ])

    await waitFor(() => {
      expect(roleSelect).not.toBeDisabled()
    })
    expect(screen.getByRole('option', { name: 'Manual configuration' })).toBeInTheDocument()
  })

  it('captures runtime node as role and saves through upsertRoleTemplate', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-capture')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-capture'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-capture-role-form')).toBeInTheDocument()
    })

    expect(screen.getByTestId('mesh-capture-role-name-input')).toHaveValue('frontend-dev')
    expect(screen.getByTestId('mesh-capture-role-id-input')).toHaveValue('frontend-dev')
    expect(screen.getByTestId('mesh-capture-role-tool-input')).toHaveValue('codex')
    expect(screen.getByTestId('mesh-capture-role-model-input')).toHaveValue('gpt-5.4 high')
    expect(screen.getByTestId('mesh-capture-role-description-input')).toHaveValue(
      'Implements UI surface details for the mesh canvas.'
    )

    await fireEvent.input(screen.getByTestId('mesh-capture-role-name-input'), {
      target: { value: 'Frontend Specialist' },
    })
    expect(screen.getByTestId('mesh-capture-role-id-input')).toHaveValue('frontend-specialist')

    await fireEvent.input(screen.getByTestId('mesh-capture-role-id-input'), {
      target: { value: 'custom-frontend-role' },
    })

    await fireEvent.click(screen.getByTestId('mesh-capture-role-save'))

    await waitFor(() => {
      expect(upsertRoleTemplate).toHaveBeenCalledWith(
        expect.objectContaining({
          roleId: 'custom-frontend-role',
          name: 'Frontend Specialist',
          kind: 'agent',
          defaults: expect.objectContaining({
            cliTool: 'codex',
            model: 'gpt-5.4 high',
          }),
        })
      )
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-message')).toHaveTextContent('Role saved to catalog')
    })
    await waitFor(() => {
      const dialog = screen.queryByRole('dialog', { name: 'Capture as Role' })
      if (!dialog) {
        expect(dialog).toBeNull()
        return
      }
      expect(within(dialog).getByRole('button', { name: 'Save to Catalog' })).toBeDisabled()
    })
  })

  it('builds setup from quick preset and initializes with inferred team name', async () => {
    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/my-app',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-fullstack-dev'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalledWith(
        expect.objectContaining({
          teamName: 'my-app-team',
          lead: expect.objectContaining({
            cliTool: 'claude',
          }),
          agents: expect.arrayContaining([
            expect.objectContaining({ cliTool: 'codex' }),
          ]),
        })
      )
    })
  })

  it('resolves standard-team agent names from slot and role name patterns', async () => {
    listRoleTemplates.mockResolvedValueOnce([
      {
        roleId: 'claude-orchestrator',
        name: 'Claude Orchestrator',
        kind: 'lead',
        cliTool: 'claude',
        model: 'opus',
        defaults: { defaultNamePattern: 'lead-{project}' },
      },
      {
        roleId: 'codex-architect',
        name: 'Codex Architect',
        kind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.4 high',
        defaults: { defaultNamePattern: 'architect-{n}' },
      },
      {
        roleId: 'codex-developer',
        name: 'Codex Developer',
        kind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.4 high',
        defaults: { defaultNamePattern: 'dev-{n}' },
      },
      {
        roleId: 'gemini-ui-specialist',
        name: 'Gemini UI Specialist',
        kind: 'agent',
        cliTool: 'gemini',
        model: 'gemini-3.1-pro',
        defaults: { defaultNamePattern: 'ui-specialist-{n}' },
      },
    ])
    getTeamPreset.mockResolvedValueOnce({
      presetId: 'standard-team',
      name: 'Standard Dev Team',
      leadRoleId: 'claude-orchestrator',
      agentSlots: [
        {
          roleId: 'codex-architect',
          count: 1,
          overrides: { namePattern: 'architect' },
        },
        {
          roleId: 'codex-developer',
          count: 2,
          overrides: { namePattern: 'developer{n}' },
        },
        {
          roleId: 'gemini-ui-specialist',
          count: 1,
          overrides: { namePattern: 'ui-specialist' },
        },
      ],
    })

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/my-app',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-standard-team'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalled()
    })

    const request = coordinationInitializeTeam.mock.calls.at(-1)?.[0]
    expect(request?.lead?.name).toBe('team-lead')
    expect(request?.agents?.map((agent) => agent.name)).toEqual([
      'architect',
      'developer1',
      'developer2',
      'ui-specialist',
    ])
  })

  it('matches teams when lead project path uses windows drive notation', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/mnt/c/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })
  })

  it('matches WSL UNC distro-root path to linux root', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })
  })

  it('shows discovery error and allows dismissing the error banner', async () => {
    coordinationGetProjectMeshSnapshot.mockRejectedValueOnce(new Error('discovery failed'))

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-error')).toHaveTextContent('discovery failed')
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-dismiss-error-message'))

    await waitFor(() => {
      expect(screen.queryByTestId('mesh-error')).not.toBeInTheDocument()
    })
  })

  it('shows error when disband action fails', async () => {
    coordinationDisbandTeam.mockRejectedValueOnce(new Error('cannot disband'))
    await renderRuntime()

    await fireEvent.click(screen.getByTestId('mesh-runtime-disband'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-error')).toHaveTextContent('cannot disband')
    })
  })

  it('removes selected runtime agent after confirm', async () => {
    await renderRuntime()

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-stop')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-stop'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))

    await waitFor(() => {
      expect(coordinationRemoveMember).toHaveBeenCalledWith('architecture-final', 'frontend-dev')
    })
  })

  it('passes runtime diagnostics into detail and focuses pane for selected agent', async () => {
    const onFocusPane = vi.fn()
    await renderRuntime({ onFocusPane })

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-pane')).toHaveTextContent('%2')
      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Idle')
      expect(screen.getByTestId('mesh-node-detail-focus')).toBeEnabled()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-focus'))
    expect(onFocusPane).toHaveBeenCalledWith('%2')
  })

  it('keeps runtime actions visible for offline agents and disables focus when pane is missing', async () => {
    coordinationGetLiveTeamStatus.mockResolvedValueOnce({
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
          model: 'gpt-5.4 high',
          projectId: 'proj-web',
          description: 'Offline agent',
          sessionStatus: 'offline',
          paneId: null,
        },
      ],
    })
    const onFocusPane = vi.fn()
    await renderRuntime({ onFocusPane })

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Offline')
      expect(screen.getByTestId('mesh-node-detail-resume')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-node-detail-stop')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-node-detail-capture')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-node-detail-focus')).toBeDisabled()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-focus'))
    expect(onFocusPane).not.toHaveBeenCalled()
  })

  it('wires setup detail actions to customizer open and member removal', async () => {
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

    expect(screen.getAllByTestId('mesh-node-agent')).toHaveLength(1)
    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-edit')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-node-detail-remove')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-edit'))
    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-panel')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getAllByTestId('slideover-close').at(-1))
    await waitFor(() => {
      expect(screen.queryByTestId('team-customizer-panel')).not.toBeInTheDocument()
    })

    if (!screen.queryByTestId('mesh-node-detail-remove')) {
      await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    }
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-remove')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-node-detail-remove'))
    await waitFor(() => {
      expect(screen.queryAllByTestId('mesh-node-agent')).toHaveLength(0)
    })
  })

  it('shows error when remove member action fails', async () => {
    coordinationRemoveMember.mockRejectedValueOnce(new Error('remove failed'))
    await renderRuntime()

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-stop'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-error')).toHaveTextContent('remove failed')
    })
  })

  it('resume action shows error when backend reports not resumed', async () => {
    coordinationResumeMember.mockResolvedValueOnce({
      teamName: 'architecture-final',
      memberName: 'frontend-dev',
      resumed: false,
      message: 'resume blocked',
    })
    await renderRuntime()

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-resume')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-node-detail-resume'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-error')).toHaveTextContent('resume blocked')
    })
  })

  it('stop action from selected lead opens disband confirmation', async () => {
    await renderRuntime()

    await fireEvent.click(screen.getByTestId('mesh-node-lead'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-stop')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-stop'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
      expect(screen.getByText('Disband team?')).toBeInTheDocument()
    })
  })

  it('supports add-agent role picker locking/unlocking and shows submit errors', async () => {
    listRoleTemplates.mockResolvedValueOnce([
      {
        roleId: 'runtime-agent',
        name: 'Runtime Agent',
        kind: 'agent',
        cliTool: 'gemini',
        model: '',
        instructions: '',
      },
    ])
    coordinationAddAgent.mockRejectedValueOnce(new Error('add failed'))

    await renderRuntime({
      availableProjects: [{ id: 'proj-core', name: 'Core' }],
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-add-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-form')).toBeInTheDocument()
    })

    await fireEvent.change(screen.getByTestId('mesh-add-agent-role-select'), {
      target: { value: 'runtime-agent' },
    })
    expect(screen.getByTestId('mesh-add-agent-tool-select')).toHaveValue('gemini')
    expect(screen.getByTestId('mesh-add-agent-model-select')).toHaveValue('gemini-3.1-pro')

    await fireEvent.click(screen.getByTestId('mesh-add-agent-unlock-toggle'))
    await fireEvent.change(screen.getByTestId('mesh-add-agent-tool-select'), {
      target: { value: 'claude' },
    })
    expect(screen.getByTestId('mesh-add-agent-model-select')).toHaveValue('opus')

    await fireEvent.change(screen.getByTestId('mesh-add-agent-role-select'), {
      target: { value: '' },
    })

    await fireEvent.input(screen.getByTestId('mesh-add-agent-name-input'), {
      target: { value: 'runtime-dev' },
    })
    await fireEvent.change(screen.getByTestId('mesh-add-agent-project-select'), {
      target: { value: 'proj-core' },
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-submit'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-error')).toHaveTextContent('add failed')
    })
  })

  it('shows capture-role save error for runtime node with array behavioral contract', async () => {
    coordinationGetLiveTeamStatus.mockResolvedValueOnce({
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
          model: '',
          projectId: 'proj-web',
          description: '',
          sessionStatus: 'idle',
          paneId: '%2',
          behavioralContract: [
            null,
            { text: 'report progress', enabled: true },
            { rule: 'skip disabled', enabled: false },
          ],
          capabilities: [],
        },
      ],
    })
    upsertRoleTemplate.mockRejectedValueOnce(new Error('save failed'))
    await renderRuntime()

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-capture'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-capture-role-form')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-capture-role-include-instructions'))
    await fireEvent.click(screen.getByTestId('mesh-capture-role-save'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-capture-role-error')).toHaveTextContent('save failed')
    })
  })

  it('cancels capture dialog without saving', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    render(MeshTab, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-capture'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-capture-role-form')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-capture-role-cancel'))

    expect(screen.getByTestId('slideover-panel').className).toContain('slideover-panel-exit')
    expect(upsertRoleTemplate).not.toHaveBeenCalled()
  })
})

describe('MeshTab animation CSS', () => {
  it('uses transform-based shrink animation with left origin', () => {
    expect(appCss).toContain('@keyframes mesh-shrink-transform')
    expect(appCss).toContain('transform: scaleX(1)')
    expect(appCss).toContain('transform: scaleX(0)')
    expect(appCss).toContain('transform-origin: left center')
    expect(appCss).toContain('[data-testid="mesh-error"] > .animate-\\[shrink_8s_linear_forwards\\]')
    expect(appCss).toContain('[data-testid="mesh-runtime-message"] > .animate-\\[shrink_5s_linear_forwards\\]')
  })
})
