import { describe, it, expect, vi, beforeEach, beforeAll, afterAll, afterEach } from 'vitest'
import { tick } from 'svelte'
import { cleanup, render, screen, waitFor, fireEvent, within } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

vi.mock('../ipc.js', () => ({
  checkMeshInstallStatus: vi.fn(),
  composeTeam: vi.fn(),
  deleteRoleTemplate: vi.fn(),
  deleteTeamPreset: vi.fn(),
  exportAgentDefinitions: vi.fn(),
  exportRoleToFile: vi.fn(),
  getTemplateDiff: vi.fn(),
  getTemplateHistory: vi.fn(),
  getTemplateStorageStatus: vi.fn(),
  importRoleFromFile: vi.fn(),
  isTauri: vi.fn(),
  revertTemplateVersion: vi.fn(),
  coordinationAddAgent: vi.fn(),
  coordinationDisbandTeam: vi.fn(),
  coordinationGetProjectMeshSnapshot: vi.fn(),
  coordinationGetLiveTeamStatus: vi.fn(),
  coordinationInitializeTeam: vi.fn(),
  coordinationPreflightCheck: vi.fn(),
  coordinationRemoveMember: vi.fn(),
  coordinationResumeTeam: vi.fn(),
  coordinationResumeMember: vi.fn(),
  getRoleTemplate: vi.fn(),
  getTeamPreset: vi.fn(),
  installMesh: vi.fn(),
  listRoleTemplates: vi.fn(),
  listTeamPresets: vi.fn(),
  onCoordinationResumeTeamProgress: vi.fn(),
  onCoordinationStepProgress: vi.fn(),
  upsertRoleTemplate: vi.fn(),
  upsertTeamPreset: vi.fn(),
}))

const {
  checkMeshInstallStatus,
  composeTeam,
  exportAgentDefinitions,
  coordinationAddAgent,
  coordinationDisbandTeam,
  coordinationGetProjectMeshSnapshot,
  coordinationGetLiveTeamStatus,
  coordinationInitializeTeam,
  coordinationPreflightCheck,
  coordinationRemoveMember,
  coordinationResumeTeam,
  coordinationResumeMember,
  getRoleTemplate,
  getTeamPreset,
  installMesh,
  listRoleTemplates,
  listTeamPresets,
  onCoordinationResumeTeamProgress,
  onCoordinationStepProgress,
  upsertRoleTemplate,
  upsertTeamPreset,
} = await import('../ipc.js')

import MeshTab from './MeshTab.svelte'
import { TEST_MODEL_CATALOG } from '../../test/fixtures/modelCatalog.js'
import { clearMeshCache, getMeshCache, resetMeshCache, setMeshCache } from '../meshCache.svelte.js'

const appCss = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8')
const INITIAL_RUNTIME_REFRESH_DELAY_MS = 120

function deferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

async function flushUi() {
  await Promise.resolve()
  await Promise.resolve()
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
        roleId: 'claude-orchestrator',
        roleName: 'Claude Orchestrator',
        focusArea: 'Team sequencing and escalation',
        contextSummary: 'Keeps the full delivery plan and blocker state in view.',
        behaviorSummary: 'Coordinates specialists and escalates blockers.',
        projectId: 'proj-core',
        sessionStatus: 'active',
        paneId: '%1',
      },
      {
        name: 'frontend-dev',
        role: 'member',
        cliTool: 'codex',
        model: 'gpt-5.4 high',
        roleId: 'codex-architect',
        roleName: 'Codex Architect',
        focusArea: 'Architecture decisions and structural review',
        contextSummary: 'Carries long-lived context around module boundaries and reviews.',
        behaviorSummary: 'Handles pattern choices and escalates direction changes.',
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
    teamRuntimeState: overrides.teamRuntimeState ?? 'none',
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
    teamRuntimeState: overrides.teamRuntimeState ?? 'active',
    warnings: overrides.warnings ?? [],
    teamStatus: {
      // FastAgentSnapshot carries model and reasoning effort since PR 5a/5b, so the
      // fast snapshot is no longer model-less on cold load.
      leadName: liveStatus.leadName,
      members: liveStatus.members,
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
    vi.resetAllMocks()
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

    coordinationResumeTeam.mockResolvedValue({
      teamName: 'architecture-final',
      resumed: true,
      totalMembers: 2,
      resumedMembers: ['team-lead', 'frontend-dev'],
      failedMembers: [],
      warnings: [],
      startedTeamDaemon: false,
      teamDaemonWarning: null,
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
        presetId: 'dev-team',
        name: 'Dev Team',
        description: 'Lead + agents',
        leadRoleId: 'lead-default',
        roleCount: 3,
        agentCount: 2,
        tools: ['claude', 'codex'],
      },
    ])
    getRoleTemplate.mockResolvedValue({ roleId: 'lead-default', instructions: 'Lead the team.' })
    getTeamPreset.mockResolvedValue({
      presetId: 'dev-team',
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
    onCoordinationResumeTeamProgress.mockResolvedValue(() => {})
    upsertRoleTemplate.mockResolvedValue({
      roleId: 'frontend-dev',
      name: 'frontend-dev',
      kind: 'agent',
      builtIn: false,
      readOnly: false,
    })
  })

  afterEach(() => {
    cleanup()
    vi.clearAllTimers()
    vi.useRealTimers()
  })

  async function renderRuntime(overrides = {}) {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot(overrides))

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
        ...overrides,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })
  }

  it('shows a lifecycle header for cold-resume teams', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(
      buildRuntimeSnapshot({
        teamRuntimeState: 'coldResume',
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'claude-orchestrator',
            roleName: 'Claude Orchestrator',
            focusArea: 'Team sequencing and escalation',
            contextSummary: 'Keeps the full delivery plan and blocker state in view.',
            behaviorSummary: 'Coordinates specialists and escalates blockers.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%1',
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%2',
          },
        ],
      })
    )

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.queryByTestId('mesh-runtime-banner')).not.toBeInTheDocument()
      expect(screen.getByTestId('mesh-runtime-summary-line')).toHaveTextContent('2 members • 0 active • 2 stopped')
      expect(screen.getByTestId('mesh-runtime-state-copy')).toHaveTextContent('All members stopped')
      expect(screen.getByTestId('mesh-runtime-primary-action')).toHaveTextContent('Resume Team')
      expect(screen.queryByTestId('mesh-runtime-add-agent')).not.toBeInTheDocument()
    })
  })

  it('shows Add Agent as the primary action for active teams', async () => {
    await renderRuntime({ teamRuntimeState: 'active' })

    expect(screen.queryByTestId('mesh-runtime-banner')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-runtime-summary-line')).toHaveTextContent('2 members • 1 active • 1 idle')
    expect(screen.getByTestId('mesh-runtime-primary-action')).toHaveTextContent('Add Agent')
    expect(screen.queryByTestId('mesh-runtime-add-agent')).not.toBeInTheDocument()
  })

  it('shows Resume Stopped count for degraded teams', async () => {
    coordinationGetLiveTeamStatus.mockResolvedValueOnce(
      buildLiveTeamStatus({
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'claude-orchestrator',
            roleName: 'Claude Orchestrator',
            focusArea: 'Team sequencing and escalation',
            contextSummary: 'Keeps the full delivery plan and blocker state in view.',
            behaviorSummary: 'Coordinates specialists and escalates blockers.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'active',
            paneId: '%1',
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%2',
          },
        ],
      })
    )

    await renderRuntime({
      teamRuntimeState: 'degraded',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          model: 'opus',
          roleId: 'claude-orchestrator',
          roleName: 'Claude Orchestrator',
          focusArea: 'Team sequencing and escalation',
          contextSummary: 'Keeps the full delivery plan and blocker state in view.',
          behaviorSummary: 'Coordinates specialists and escalates blockers.',
          projectId: '/projects/taurhaus',
          sessionStatus: 'active',
          paneId: '%1',
        },
        {
          name: 'frontend-dev',
          role: 'member',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          roleId: 'codex-architect',
          roleName: 'Codex Architect',
          focusArea: 'Architecture decisions and structural review',
          contextSummary: 'Carries long-lived context around module boundaries and reviews.',
          behaviorSummary: 'Handles pattern choices and escalates direction changes.',
          projectId: '/projects/taurhaus',
          sessionStatus: 'offline',
          paneId: '%2',
        },
      ],
    })

    expect(screen.getByTestId('mesh-runtime-summary-line')).toHaveTextContent('2 members • 1 active • 1 stopped')
    expect(screen.getByTestId('mesh-runtime-state-copy')).toHaveTextContent('1 member stopped')
    expect(screen.getByTestId('mesh-runtime-primary-action')).toHaveTextContent('Resume Stopped (1)')
    expect(screen.queryByTestId('mesh-runtime-add-agent')).not.toBeInTheDocument()
  })

  it('resume team CTA calls coordinationResumeTeam', async () => {
    const resumeRequest = deferred()
    coordinationResumeTeam.mockReturnValueOnce(resumeRequest.promise)
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(
      buildRuntimeSnapshot({
        teamRuntimeState: 'coldResume',
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'claude-orchestrator',
            roleName: 'Claude Orchestrator',
            focusArea: 'Team sequencing and escalation',
            contextSummary: 'Keeps the full delivery plan and blocker state in view.',
            behaviorSummary: 'Coordinates specialists and escalates blockers.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%1',
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%2',
          },
        ],
      })
    )

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-primary-action')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))

    expect(coordinationResumeTeam).toHaveBeenCalledWith('architecture-final')

    resumeRequest.resolve({
      teamName: 'architecture-final',
      resumed: true,
      totalMembers: 2,
      resumedMembers: ['team-lead', 'frontend-dev'],
      failedMembers: [],
      warnings: [],
      startedTeamDaemon: false,
      teamDaemonWarning: null,
    })
  })

  it('shows resumed and failed member lists when team resume partially fails', async () => {
    coordinationGetProjectMeshSnapshot
      .mockResolvedValueOnce(
        buildRuntimeSnapshot({
          teamRuntimeState: 'coldResume',
          members: [
            {
              name: 'team-lead',
              role: 'lead',
              cliTool: 'claude',
              model: 'opus',
              roleId: 'claude-orchestrator',
              roleName: 'Claude Orchestrator',
              focusArea: 'Team sequencing and escalation',
              contextSummary: 'Keeps the full delivery plan and blocker state in view.',
              behaviorSummary: 'Coordinates specialists and escalates blockers.',
              projectId: '/projects/taurhaus',
              sessionStatus: 'offline',
              paneId: '%1',
            },
            {
              name: 'frontend-dev',
              role: 'member',
              cliTool: 'codex',
              model: 'gpt-5.4 high',
              roleId: 'codex-architect',
              roleName: 'Codex Architect',
              focusArea: 'Architecture decisions and structural review',
              contextSummary: 'Carries long-lived context around module boundaries and reviews.',
              behaviorSummary: 'Handles pattern choices and escalates direction changes.',
              projectId: '/projects/taurhaus',
              sessionStatus: 'offline',
              paneId: '%2',
            },
          ],
        })
      )
      .mockResolvedValueOnce(
        buildRuntimeSnapshot({
          teamRuntimeState: 'degraded',
          members: [
            {
              name: 'team-lead',
              role: 'lead',
              cliTool: 'claude',
              model: 'opus',
              roleId: 'claude-orchestrator',
              roleName: 'Claude Orchestrator',
              focusArea: 'Team sequencing and escalation',
              contextSummary: 'Keeps the full delivery plan and blocker state in view.',
              behaviorSummary: 'Coordinates specialists and escalates blockers.',
              projectId: '/projects/taurhaus',
              sessionStatus: 'active',
              paneId: '%11',
            },
            {
              name: 'frontend-dev',
              role: 'member',
              cliTool: 'codex',
              model: 'gpt-5.4 high',
              roleId: 'codex-architect',
              roleName: 'Codex Architect',
              focusArea: 'Architecture decisions and structural review',
              contextSummary: 'Carries long-lived context around module boundaries and reviews.',
              behaviorSummary: 'Handles pattern choices and escalates direction changes.',
              projectId: '/projects/taurhaus',
              sessionStatus: 'offline',
              paneId: '%2',
            },
          ],
        })
      )
    coordinationGetLiveTeamStatus.mockResolvedValueOnce(
      buildLiveTeamStatus({
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'claude-orchestrator',
            roleName: 'Claude Orchestrator',
            focusArea: 'Team sequencing and escalation',
            contextSummary: 'Keeps the full delivery plan and blocker state in view.',
            behaviorSummary: 'Coordinates specialists and escalates blockers.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'active',
            paneId: '%11',
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%2',
          },
        ],
      })
    )
    coordinationResumeTeam.mockResolvedValueOnce({
      teamName: 'architecture-final',
      resumed: true,
      totalMembers: 2,
      resumedMembers: ['team-lead'],
      failedMembers: [{ memberName: 'frontend-dev', message: 'launch failed', retryable: true }],
      warnings: [],
      startedTeamDaemon: false,
      teamDaemonWarning: null,
    })

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-primary-action')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-resume-progress')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-runtime-resume-subtitle')).toHaveTextContent(
        'Resume completed with failures. Resumed: team-lead. Failed: frontend-dev (launch failed).'
      )
      expect(screen.getByTestId('mesh-runtime-resume-progress')).toHaveTextContent('team-lead')
      expect(screen.getByTestId('mesh-runtime-resume-progress')).toHaveTextContent('frontend-dev')
      expect(screen.getByTestId('mesh-runtime-resume-progress')).toHaveTextContent('launch failed')
      expect(screen.getByTestId('mesh-runtime-resume-footer')).toHaveTextContent(
        'Team background service was already running.'
      )
    })
  })

  it('shows daemon warnings and backend notes in the final resume summary', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(
      buildRuntimeSnapshot({
        teamRuntimeState: 'coldResume',
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'claude-orchestrator',
            roleName: 'Claude Orchestrator',
            focusArea: 'Team sequencing and escalation',
            contextSummary: 'Keeps the full delivery plan and blocker state in view.',
            behaviorSummary: 'Coordinates specialists and escalates blockers.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%1',
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%2',
          },
        ],
      })
    )
    coordinationResumeTeam.mockResolvedValueOnce({
      teamName: 'architecture-final',
      resumed: true,
      totalMembers: 2,
      resumedMembers: ['team-lead', 'frontend-dev'],
      failedMembers: [],
      warnings: ['frontend-dev: created a replacement pane'],
      startedTeamDaemon: false,
      teamDaemonWarning: 'could not verify the background service state',
    })

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-primary-action')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-resume-subtitle')).toHaveTextContent(
        'Resume completed with a background service warning. Resumed: team-lead, frontend-dev.'
      )
      expect(screen.getByTestId('mesh-runtime-resume-footer')).toHaveTextContent(
        'Team background service warning: could not verify the background service state'
      )
      expect(screen.getByTestId('mesh-runtime-resume-footer')).toHaveTextContent(
        'Additional notes: frontend-dev: started a replacement terminal session.'
      )
    })
  })

  it('shows clean success copy without a footer when only backend notes remain', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(
      buildRuntimeSnapshot({
        teamRuntimeState: 'coldResume',
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'claude-orchestrator',
            roleName: 'Claude Orchestrator',
            focusArea: 'Team sequencing and escalation',
            contextSummary: 'Keeps the full delivery plan and blocker state in view.',
            behaviorSummary: 'Coordinates specialists and escalates blockers.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%1',
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%2',
          },
        ],
      })
    )
    coordinationResumeTeam.mockResolvedValueOnce({
      teamName: 'architecture-final',
      resumed: true,
      totalMembers: 2,
      resumedMembers: ['team-lead', 'frontend-dev'],
      failedMembers: [],
      warnings: ['frontend-dev: created a replacement pane'],
      startedTeamDaemon: false,
      teamDaemonWarning: null,
    })

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-primary-action')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-resume-subtitle')).toHaveTextContent(
        'Resume complete. Resumed: team-lead, frontend-dev.'
      )
      expect(screen.getByTestId('mesh-runtime-resume-footer')).toHaveTextContent(
        'Notes: frontend-dev: started a replacement terminal session.'
      )
    })
  })

  it('states clearly when all members were already running', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(
      buildRuntimeSnapshot({
        teamRuntimeState: 'coldResume',
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'claude-orchestrator',
            roleName: 'Claude Orchestrator',
            focusArea: 'Team sequencing and escalation',
            contextSummary: 'Keeps the full delivery plan and blocker state in view.',
            behaviorSummary: 'Coordinates specialists and escalates blockers.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%1',
          },
        ],
      })
    )
    coordinationResumeTeam.mockResolvedValueOnce({
      teamName: 'architecture-final',
      resumed: true,
      totalMembers: 1,
      resumedMembers: [],
      failedMembers: [],
      warnings: [],
      startedTeamDaemon: false,
      teamDaemonWarning: null,
    })

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-primary-action')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-resume-subtitle')).toHaveTextContent(
        'All members were already running.'
      )
      expect(screen.queryByTestId('mesh-runtime-resume-footer')).not.toBeInTheDocument()
    })
  })

  it('updates the resume panel live from streamed member progress events', async () => {
    const resumeRequest = deferred()
    let resumeProgressHandler = null
    onCoordinationResumeTeamProgress.mockImplementationOnce(async (callback) => {
      resumeProgressHandler = callback
      return () => {}
    })
    coordinationResumeTeam.mockReturnValueOnce(resumeRequest.promise)
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(
      buildRuntimeSnapshot({
        teamRuntimeState: 'coldResume',
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'claude-orchestrator',
            roleName: 'Claude Orchestrator',
            focusArea: 'Team sequencing and escalation',
            contextSummary: 'Keeps the full delivery plan and blocker state in view.',
            behaviorSummary: 'Coordinates specialists and escalates blockers.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%1',
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%2',
          },
        ],
      })
    )

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-primary-action')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))

    expect(resumeProgressHandler).toBeTypeOf('function')

    resumeProgressHandler({
      payload: {
        operation: 'resume_team',
        teamName: 'architecture-final',
        memberName: 'team-lead',
        memberIndex: 1,
        memberCount: 2,
        stage: 'launch_session',
        status: 'running',
        message: 'Launching session...',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-resume-progress')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-runtime-resume-header')).toHaveTextContent('Resuming 1 of 2 members')
      expect(screen.getByTestId('mesh-runtime-resume-subtitle')).toHaveTextContent('team-lead')
      expect(screen.getByTestId('mesh-runtime-resume-subtitle')).toHaveTextContent('Launching session')
      expect(screen.getByTestId('mesh-runtime-resume-progress')).toHaveTextContent('Launching session...')
    })

    resumeRequest.resolve({
      teamName: 'architecture-final',
      resumed: true,
      totalMembers: 2,
      resumedMembers: ['team-lead', 'frontend-dev'],
      failedMembers: [],
      warnings: [],
      startedTeamDaemon: true,
      teamDaemonWarning: null,
    })

    await waitFor(() => {
      expect(screen.queryByTestId('mesh-runtime-resume-footer')).not.toBeInTheDocument()
    })
  })

  it('disables add, disband, and resume actions while team resume is in flight', async () => {
    const resumeRequest = deferred()
    coordinationResumeTeam.mockReturnValueOnce(resumeRequest.promise)
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(
      buildRuntimeSnapshot({
        teamRuntimeState: 'coldResume',
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'claude-orchestrator',
            roleName: 'Claude Orchestrator',
            focusArea: 'Team sequencing and escalation',
            contextSummary: 'Keeps the full delivery plan and blocker state in view.',
            behaviorSummary: 'Coordinates specialists and escalates blockers.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%1',
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'offline',
            paneId: '%2',
          },
        ],
      })
    )

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-primary-action')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-resume')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-more-toggle'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-disband')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-primary-action')).toBeDisabled()
      expect(screen.queryByTestId('mesh-runtime-add-agent')).not.toBeInTheDocument()
      expect(screen.getByTestId('mesh-runtime-more-toggle')).toBeDisabled()
      expect(screen.getByTestId('mesh-runtime-disband')).toBeDisabled()
      expect(screen.getByTestId('mesh-node-detail-resume')).toBeDisabled()
    })

    resumeRequest.resolve({
      teamName: 'architecture-final',
      resumed: true,
      totalMembers: 2,
      resumedMembers: ['team-lead', 'frontend-dev'],
      failedMembers: [],
      warnings: [],
      startedTeamDaemon: false,
      teamDaemonWarning: null,
    })
  })

  it('auto-collapses the resume progress tray after completion', async () => {
    coordinationGetProjectMeshSnapshot
      .mockResolvedValueOnce(
        buildRuntimeSnapshot({
          teamRuntimeState: 'coldResume',
          members: [
            {
              name: 'team-lead',
              role: 'lead',
              cliTool: 'claude',
              model: 'opus',
              roleId: 'claude-orchestrator',
              roleName: 'Claude Orchestrator',
              focusArea: 'Team sequencing and escalation',
              contextSummary: 'Keeps the full delivery plan and blocker state in view.',
              behaviorSummary: 'Coordinates specialists and escalates blockers.',
              projectId: '/projects/taurhaus',
              sessionStatus: 'offline',
              paneId: '%1',
            },
            {
              name: 'frontend-dev',
              role: 'member',
              cliTool: 'codex',
              model: 'gpt-5.4 high',
              roleId: 'codex-architect',
              roleName: 'Codex Architect',
              focusArea: 'Architecture decisions and structural review',
              contextSummary: 'Carries long-lived context around module boundaries and reviews.',
              behaviorSummary: 'Handles pattern choices and escalates direction changes.',
              projectId: '/projects/taurhaus',
              sessionStatus: 'offline',
              paneId: '%2',
            },
          ],
        })
      )
      .mockResolvedValueOnce(
        buildRuntimeSnapshot({
          teamRuntimeState: 'active',
          members: [
            {
              name: 'team-lead',
              role: 'lead',
              cliTool: 'claude',
              model: 'opus',
              roleId: 'claude-orchestrator',
              roleName: 'Claude Orchestrator',
              focusArea: 'Team sequencing and escalation',
              contextSummary: 'Keeps the full delivery plan and blocker state in view.',
              behaviorSummary: 'Coordinates specialists and escalates blockers.',
              projectId: '/projects/taurhaus',
              sessionStatus: 'active',
              paneId: '%11',
            },
            {
              name: 'frontend-dev',
              role: 'member',
              cliTool: 'codex',
              model: 'gpt-5.4 high',
              roleId: 'codex-architect',
              roleName: 'Codex Architect',
              focusArea: 'Architecture decisions and structural review',
              contextSummary: 'Carries long-lived context around module boundaries and reviews.',
              behaviorSummary: 'Handles pattern choices and escalates direction changes.',
              projectId: '/projects/taurhaus',
              sessionStatus: 'active',
              paneId: '%12',
            },
          ],
        })
      )
    coordinationGetLiveTeamStatus.mockResolvedValueOnce(
      buildLiveTeamStatus({
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'claude-orchestrator',
            roleName: 'Claude Orchestrator',
            focusArea: 'Team sequencing and escalation',
            contextSummary: 'Keeps the full delivery plan and blocker state in view.',
            behaviorSummary: 'Coordinates specialists and escalates blockers.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'active',
            paneId: '%11',
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            projectId: '/projects/taurhaus',
            sessionStatus: 'active',
            paneId: '%12',
          },
        ],
      })
    )

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-primary-action')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-resume-progress')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-runtime-resume-progress')).toHaveTextContent('Completed')
    })

    await new Promise((resolve) => setTimeout(resolve, 5200))

    await waitFor(() => {
      expect(screen.queryByTestId('mesh-runtime-resume-progress')).not.toBeInTheDocument()
    })
  }, 8000)

  it('renders from cached snapshot immediately on revisit without snapshot IPC', async () => {
    vi.useFakeTimers()
    setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())
    const liveRefresh = deferred()
    coordinationGetLiveTeamStatus.mockReturnValueOnce(liveRefresh.promise)

    try {
      render(MeshTab, {
        props: {
          dark: false,
          modelCatalog: TEST_MODEL_CATALOG,
          projectPath: '/projects/taurhaus',
        },
      })

      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
      expect(coordinationGetProjectMeshSnapshot).not.toHaveBeenCalled()
      expect(coordinationGetLiveTeamStatus).not.toHaveBeenCalled()

      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS - 1)
      expect(coordinationGetLiveTeamStatus).not.toHaveBeenCalled()

      await vi.advanceTimersByTimeAsync(1)
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(1)

      liveRefresh.resolve(buildLiveTeamStatus())
    } finally {
      vi.useRealTimers()
    }
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
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })
    await waitFor(() => {
      expect(getMeshCache('/projects/taurhaus')).toEqual(snapshot)
    })
    expect(screen.getByTestId('mesh-empty-state')).toBeInTheDocument()
    expect(coordinationGetProjectMeshSnapshot).toHaveBeenCalledWith('/projects/taurhaus')
  })

  it('keeps the gate visible during uncached discovery so cold resume can replace setup on first load', async () => {
    const snapshotLoad = deferred()
    const liveRefresh = deferred()
    coordinationGetProjectMeshSnapshot.mockReturnValueOnce(snapshotLoad.promise)
    coordinationGetLiveTeamStatus.mockReturnValueOnce(liveRefresh.promise)

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    expect(screen.getByTestId('mesh-mode-gate')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-mode-empty')).not.toBeInTheDocument()

    snapshotLoad.resolve(buildRuntimeSnapshot({
      teamRuntimeState: 'coldResume',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          model: 'opus',
          projectId: '/projects/taurhaus',
          sessionStatus: 'offline',
          paneId: '%1',
        },
        {
          name: 'frontend-dev',
          role: 'member',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          projectId: '/projects/taurhaus',
          sessionStatus: 'offline',
          paneId: '%2',
        },
      ],
    }))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
      expect(screen.queryByTestId('mesh-mode-gate')).not.toBeInTheDocument()
    })

    liveRefresh.resolve(buildLiveTeamStatus({
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          model: 'opus',
          projectId: '/projects/taurhaus',
          sessionStatus: 'offline',
          paneId: '%1',
        },
        {
          name: 'frontend-dev',
          role: 'member',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          projectId: '/projects/taurhaus',
          sessionStatus: 'offline',
          paneId: '%2',
        },
      ],
    }))
  })

  it('dedupes project snapshot IPC while activation visibility churn happens during a pending load', async () => {
    const snapshotLoad = deferred()
    coordinationGetProjectMeshSnapshot.mockReturnValueOnce(snapshotLoad.promise)

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    const meshTab = screen.getByTestId('mesh-tab')
    const host = meshTab.parentElement
    expect(host).toBeTruthy()

    host.classList.add('hidden')
    await Promise.resolve()
    host.classList.remove('hidden')
    await Promise.resolve()
    host.classList.add('hidden')
    await Promise.resolve()
    host.classList.remove('hidden')
    await Promise.resolve()

    expect(coordinationGetProjectMeshSnapshot).toHaveBeenCalledTimes(1)

    snapshotLoad.resolve(buildProjectMeshSnapshot({
      meshAvailable: true,
      tmuxAvailable: true,
      teamName: null,
      teamStatus: null,
      warnings: [],
    }))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })
  })

  it('background live refresh patches member status after cached render', async () => {
    vi.useFakeTimers()
    setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())
    const liveRefresh = deferred()
    coordinationGetLiveTeamStatus.mockReturnValueOnce(liveRefresh.promise)

    try {
      render(MeshTab, {
        props: {
          dark: true,
          modelCatalog: TEST_MODEL_CATALOG,
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

      expect(coordinationGetLiveTeamStatus).not.toHaveBeenCalled()
      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS)
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(1)

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
      await Promise.resolve()

      await waitFor(() => {
        expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Active')
      })
    } finally {
      vi.useRealTimers()
    }
  })

  it('revalidates stale cached runtime snapshots so a corrected active team replaces the old cached team', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-03-11T05:00:00Z'))
    setMeshCache('/projects/taurhaus', buildRuntimeSnapshot({
      teamName: 'towerhouse-product-team',
      leadName: 'old-lead',
    }))
    vi.setSystemTime(new Date('2026-03-11T05:00:06Z'))

    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot({
      teamName: 'taurhaus-team',
      leadName: 'team-lead',
    }))
    coordinationGetLiveTeamStatus.mockResolvedValueOnce(buildLiveTeamStatus({
      teamName: 'taurhaus-team',
    }))

    try {
      render(MeshTab, {
        props: {
          dark: false,
          modelCatalog: TEST_MODEL_CATALOG,
          projectPath: '/projects/taurhaus',
        },
      })

      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('towerhouse-product-team')
      expect(coordinationGetProjectMeshSnapshot).not.toHaveBeenCalled()

      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS - 1)
      expect(coordinationGetProjectMeshSnapshot).not.toHaveBeenCalled()

      // Regression: backend active-team repairs were invisible in the running app because MeshTab
      // trusted stale cached project snapshots indefinitely and never revalidated canonical team
      // discovery after cache hydration.
      await vi.advanceTimersByTimeAsync(1)

      await waitFor(() => {
        expect(coordinationGetProjectMeshSnapshot).toHaveBeenCalledWith('/projects/taurhaus')
        expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('taurhaus-team')
      })
    } finally {
      vi.useRealTimers()
    }
  })

  it('polls runtime status and updates an agent node to offline when the member disconnects', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-03-11T00:00:00Z'))

    try {
      setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())
      coordinationGetLiveTeamStatus
        .mockResolvedValueOnce(buildLiveTeamStatus())
        .mockResolvedValueOnce(buildLiveTeamStatus({
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
              roleId: 'codex-architect',
              roleName: 'Codex Architect',
              focusArea: 'Architecture decisions and structural review',
              contextSummary: 'Carries long-lived context around module boundaries and reviews.',
              behaviorSummary: 'Handles pattern choices and escalates direction changes.',
              projectId: 'proj-web',
              description: 'Implements UI surface details for the mesh canvas.',
              sessionStatus: 'offline',
              paneId: null,
            },
          ],
        }))

      render(MeshTab, {
        props: {
          dark: false,
          modelCatalog: TEST_MODEL_CATALOG,
          projectPath: '/projects/taurhaus',
        },
      })

      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()

      await fireEvent.click(screen.getByTestId('mesh-node-agent'))
      await flushUi()
      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Idle')

      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS)
      await flushUi()
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(1)

      await vi.advanceTimersByTimeAsync(2000)
      await flushUi()
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(2)

      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Offline')
      expect(screen.getByTestId('mesh-node-detail-focus')).toBeDisabled()
    } finally {
      vi.useRealTimers()
    }
  })

  it('treats non-live runtime statuses as stopped when live refresh reconciles the runtime bar', async () => {
    vi.useFakeTimers()

    try {
      setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())
      coordinationGetLiveTeamStatus.mockResolvedValueOnce(buildLiveTeamStatus({
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            projectId: 'proj-core',
            sessionStatus: 'stopped',
            paneId: null,
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
            projectId: 'proj-web',
            sessionStatus: 'terminated',
            paneId: null,
          },
        ],
      }))

      render(MeshTab, {
        props: {
          dark: false,
          modelCatalog: TEST_MODEL_CATALOG,
          projectPath: '/projects/taurhaus',
        },
      })

      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-runtime-primary-action')).toHaveTextContent('Add Agent')

      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS)
      await flushUi()
      await flushUi()

      await waitFor(() => {
        expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(1)
        expect(screen.getByTestId('mesh-runtime-summary-line')).toHaveTextContent('2 members • 0 active • 2 stopped')
        expect(screen.getByTestId('mesh-runtime-state-copy')).toHaveTextContent('All members stopped')
        expect(screen.getByTestId('mesh-runtime-primary-action')).toHaveTextContent('Resume Team')
        expect(screen.queryByTestId('mesh-runtime-add-agent')).not.toBeInTheDocument()
      })
    } finally {
      vi.useRealTimers()
    }
  })

  it('skips overlapping runtime poll ticks while a previous refresh is still pending', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-03-11T00:00:00Z'))

    try {
      setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())
      const slowRefresh = deferred()
      coordinationGetLiveTeamStatus
        .mockImplementationOnce(() => slowRefresh.promise)
        .mockResolvedValueOnce(buildLiveTeamStatus({
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
              roleId: 'codex-architect',
              roleName: 'Codex Architect',
              focusArea: 'Architecture decisions and structural review',
              contextSummary: 'Carries long-lived context around module boundaries and reviews.',
              behaviorSummary: 'Handles pattern choices and escalates direction changes.',
              projectId: 'proj-web',
              description: 'Implements UI surface details for the mesh canvas.',
              sessionStatus: 'active',
              paneId: '%2',
            },
          ],
        }))

      render(MeshTab, {
        props: {
          dark: false,
          modelCatalog: TEST_MODEL_CATALOG,
          projectPath: '/projects/taurhaus',
        },
      })

      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()

      await fireEvent.click(screen.getByTestId('mesh-node-agent'))
      await flushUi()
      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Idle')

      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(0)

      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS)
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(1)

      await vi.advanceTimersByTimeAsync(2000)
      await flushUi()
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(1)

      await vi.advanceTimersByTimeAsync(2000)
      await flushUi()
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(1)

      slowRefresh.resolve(buildLiveTeamStatus({
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
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            projectId: 'proj-web',
            description: 'Implements UI surface details for the mesh canvas.',
            sessionStatus: 'offline',
            paneId: null,
          },
        ],
      }))

      await flushUi()
      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Offline')

      await vi.advanceTimersByTimeAsync(2000)
      await flushUi()
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(2)

      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Active')
    } finally {
      vi.useRealTimers()
    }
  })

  it('clears the runtime poll in-flight guard after an error so later ticks recover', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-03-11T00:00:00Z'))

    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})
    try {
      setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())
      coordinationGetLiveTeamStatus
        .mockResolvedValueOnce(buildLiveTeamStatus())
        .mockRejectedValueOnce(new Error('slow backend failed'))
        .mockResolvedValueOnce(buildLiveTeamStatus({
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
              roleId: 'codex-architect',
              roleName: 'Codex Architect',
              focusArea: 'Architecture decisions and structural review',
              contextSummary: 'Carries long-lived context around module boundaries and reviews.',
              behaviorSummary: 'Handles pattern choices and escalates direction changes.',
              projectId: 'proj-web',
              description: 'Implements UI surface details for the mesh canvas.',
              sessionStatus: 'active',
              paneId: '%2',
            },
          ],
        }))

      render(MeshTab, {
        props: {
          dark: false,
          modelCatalog: TEST_MODEL_CATALOG,
          projectPath: '/projects/taurhaus',
        },
      })

      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()

      await fireEvent.click(screen.getByTestId('mesh-node-agent'))
      await flushUi()
      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Idle')

      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS)
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(1)

      await vi.advanceTimersByTimeAsync(2000)
      await flushUi()
      expect(warnSpy).toHaveBeenCalledWith('[meshTab] runtime status refresh failed:', expect.any(Error))

      await vi.advanceTimersByTimeAsync(2000)
      await flushUi()
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(3)

      expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Active')
    } finally {
      warnSpy.mockRestore()
      vi.useRealTimers()
    }
  })

  it('abandons an in-flight runtime refresh when switching away to another top-level tab', async () => {
    vi.useFakeTimers()
    setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())
    const liveRefresh = deferred()
    coordinationGetLiveTeamStatus.mockReturnValueOnce(liveRefresh.promise)
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildProjectMeshSnapshot({
      meshAvailable: true,
      tmuxAvailable: true,
      teamName: null,
      teamStatus: null,
      warnings: [],
    }))

    try {
      const view = render(MeshTab, {
        props: {
          dark: false,
          modelCatalog: TEST_MODEL_CATALOG,
          projectPath: '/projects/taurhaus',
        },
      })

      await waitFor(() => {
        expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
      })

      await vi.advanceTimersByTimeAsync(300)
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(1)

      const overviewTab = document.createElement('button')
      overviewTab.type = 'button'
      overviewTab.dataset.testid = 'tab-overview'
      document.body.appendChild(overviewTab)

      await fireEvent.pointerDown(overviewTab)
      screen.getByTestId('mesh-tab').parentElement.classList.add('hidden')
      await view.rerender({
        dark: false,
        projectPath: '/projects/other-project',
      })

      expect(coordinationGetProjectMeshSnapshot).not.toHaveBeenCalled()

      screen.getByTestId('mesh-tab').parentElement.classList.remove('hidden')

      await waitFor(() => {
        expect(coordinationGetProjectMeshSnapshot).toHaveBeenCalledWith('/projects/other-project')
        expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
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
            sessionStatus: 'offline',
            paneId: null,
          },
        ],
      }))
      await Promise.resolve()

      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
      expect(coordinationGetProjectMeshSnapshot).toHaveBeenCalledTimes(1)

      overviewTab.remove()
    } finally {
      vi.useRealTimers()
    }
  })

  it('pauses mesh runtime polling as soon as project navigation starts outside the mesh tab', async () => {
    vi.useFakeTimers()
    setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())

    const projectItem = document.createElement('button')
    projectItem.type = 'button'
    projectItem.dataset.testid = 'project-item'
    projectItem.textContent = 'Other project'
    document.body.appendChild(projectItem)

    const unrelatedButton = document.createElement('button')
    unrelatedButton.type = 'button'
    unrelatedButton.textContent = 'Outside'
    document.body.appendChild(unrelatedButton)

    try {
      render(MeshTab, {
        props: {
          dark: false,
          modelCatalog: TEST_MODEL_CATALOG,
          projectPath: '/projects/taurhaus',
        },
      })

      await waitFor(() => {
        expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
      })

      await fireEvent.pointerDown(unrelatedButton)
      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS)
      await vi.advanceTimersByTimeAsync(2000)

      const pollCountAfterUnrelatedPointer = coordinationGetLiveTeamStatus.mock.calls.length
      expect(pollCountAfterUnrelatedPointer).toBeGreaterThan(0)

      coordinationGetLiveTeamStatus.mockClear()

      await fireEvent.pointerDown(projectItem)
      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS)
      await vi.advanceTimersByTimeAsync(4000)

      expect(coordinationGetLiveTeamStatus).not.toHaveBeenCalled()
    } finally {
      projectItem.remove()
      unrelatedButton.remove()
      vi.useRealTimers()
    }
  })

  it('starts a fresh runtime refresh when the mesh view reactivates on another project while an older refresh is still pending', async () => {
    vi.useFakeTimers()
    setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())
    setMeshCache('/projects/other-project', buildRuntimeSnapshot({
      teamName: 'other-team',
      leadName: 'other-lead',
    }))

    const firstRefresh = deferred()
    const secondRefresh = deferred()
    coordinationGetLiveTeamStatus
      .mockImplementationOnce(() => firstRefresh.promise)
      .mockImplementationOnce(() => secondRefresh.promise)

    try {
      const view = render(MeshTab, {
        props: {
          dark: false,
          modelCatalog: TEST_MODEL_CATALOG,
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

      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS)
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(1)

      await view.rerender({
        dark: false,
        projectPath: '/projects/other-project',
      })
      await waitFor(() => {
        expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('other-team')
      })

      await vi.advanceTimersByTimeAsync(INITIAL_RUNTIME_REFRESH_DELAY_MS + 2000)
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(2)

      secondRefresh.resolve(buildLiveTeamStatus({
        teamName: 'other-team',
        leadName: 'other-lead',
        members: [
          {
            name: 'other-lead',
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
      await Promise.resolve()

      firstRefresh.resolve(buildLiveTeamStatus({
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
            sessionStatus: 'offline',
            paneId: null,
          },
        ],
      }))
      await Promise.resolve()

      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('other-team')
      expect(coordinationGetLiveTeamStatus).toHaveBeenCalledTimes(2)
    } finally {
      vi.useRealTimers()
    }
  })

  it('shows the role hover card in the runtime mesh tab after delayed hover', async () => {
    vi.useFakeTimers()
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    const node = screen.getByTestId('mesh-node-agent')
    await fireEvent.mouseEnter(node)
    await vi.advanceTimersByTimeAsync(200)

    expect(screen.getByTestId('mesh-node-role-card')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-role-card-role-name')).toHaveTextContent('Codex Architect')
    expect(screen.getByTestId('mesh-node-role-card-tool-model')).toHaveTextContent('Codex · gpt-5.4 high')
    expect(screen.getByTestId('mesh-node-role-card-status')).toHaveTextContent('Idle')
    expect(screen.getByTestId('mesh-node-role-card-summary')).toHaveTextContent(
      'Architecture decisions and structural review'
    )
    expect(screen.getByTestId('mesh-node-role-card-hint')).toHaveTextContent('Click for details')

    vi.useRealTimers()
  })

  it('shows role summary fields on cold load before live-status refresh resolves', async () => {
    vi.useFakeTimers()
    const liveStatusPending = deferred()
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())
    coordinationGetLiveTeamStatus.mockReturnValueOnce(liveStatusPending.promise)

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    const node = screen.getByTestId('mesh-node-agent')
    await fireEvent.mouseEnter(node)
    await vi.advanceTimersByTimeAsync(200)

    expect(screen.getByTestId('mesh-node-role-card-role-name')).toHaveTextContent('Codex Architect')
    expect(screen.getByTestId('mesh-node-role-card-tool-model')).toHaveTextContent('Codex · gpt-5.4 high')
    expect(screen.getByTestId('mesh-node-role-card-status')).toHaveTextContent('Idle')
    expect(screen.getByTestId('mesh-node-role-card-summary')).toHaveTextContent(
      'Architecture decisions and structural review'
    )
    expect(screen.getByTestId('mesh-node-role-card-hint')).toHaveTextContent('Click for details')

    liveStatusPending.resolve(buildLiveTeamStatus())
    vi.useRealTimers()
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
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-availability-inline')).toHaveTextContent(
        'Mesh CLI is unavailable'
      )
    })

    expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-mode-gate')).not.toBeInTheDocument()
    expect(coordinationPreflightCheck).not.toHaveBeenCalled()
  })

  it('suppresses skipped team folder discovery warnings from the inline availability banner', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildProjectMeshSnapshot({
      meshAvailable: true,
      tmuxAvailable: true,
      warnings: ["skipped team folder 'taureval-1773711699719' because config is missing or invalid"],
    }))

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/espn_fantasy_desktop',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    expect(screen.queryByTestId('mesh-availability-inline')).not.toBeInTheDocument()
  })

  it('empty -> setup transition via inline roster edit', async () => {
    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-team-name-display'))
    await fireEvent.input(screen.getByTestId('mesh-builder-team-name-input'), {
      target: { value: 'taurhaus-team' },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-builder-catalog')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-roster')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-lead-empty')).toBeInTheDocument()
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
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-role-lead-default'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-builder-lead-card')).toBeInTheDocument()
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

  it('re-hydrates from the canonical project snapshot after initialize so runtime shows the active team', async () => {
    coordinationGetProjectMeshSnapshot
      .mockResolvedValueOnce(buildProjectMeshSnapshot({
        teamName: null,
        teamStatus: null,
        warnings: [],
      }))
      .mockResolvedValueOnce(buildRuntimeSnapshot({
        teamName: 'taurhaus-team',
      }))

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-role-lead-default'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-builder-lead-card')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-team-name-display'))
    await fireEvent.input(screen.getByTestId('mesh-builder-team-name-input'), {
      target: { value: 'taurhaus-team' },
    })
    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('taurhaus-team')
    })
    expect(coordinationGetProjectMeshSnapshot).toHaveBeenCalledTimes(2)
  })

  it('opens the template browser from the builder and hands it the selected project', async () => {
    // Regression: the panel that carries "Export as Claude Code agents" was
    // rendered by no production caller, so the action could not be reached.
    exportAgentDefinitions.mockResolvedValue({ written: ['lead-default'], skipped: [] })

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
        projectId: 'project-1',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })
    expect(screen.queryByTestId('template-browser-panel')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-template-open-browser'))

    await waitFor(() => {
      expect(screen.getByTestId('template-browser-panel')).toBeInTheDocument()
    })

    await waitFor(() => {
      expect(screen.getByTestId('role-export-agents-button')).toBeEnabled()
    })
    await fireEvent.click(screen.getByTestId('role-export-agents-button'))

    await waitFor(() => {
      expect(exportAgentDefinitions).toHaveBeenCalledWith('project-1')
    })
  })

  it('keeps setup composition inline while runtime slideovers still open and close', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    const runtimeView = render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-form')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-cancel'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-more-toggle'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-disband')).toBeInTheDocument()
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
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    const catalogSearch = screen.getByTestId('mesh-builder-role-search')
    await fireEvent.click(screen.getByTestId('mesh-template-browse-catalog'))
    expect(catalogSearch).toHaveFocus()
    expect(screen.queryByTestId('template-browser-panel')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-team-name-display'))
    await fireEvent.input(screen.getByTestId('mesh-builder-team-name-input'), {
      target: { value: 'taurhaus-team' },
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-builder-catalog')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-roster')).toBeInTheDocument()
    expect(screen.queryByTestId('team-customizer-panel')).not.toBeInTheDocument()
  })

  it('initializes cleanly after selecting a lead role from the inline catalog', async () => {
    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-builder-role-lead-default')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-role-lead-default'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-builder-lead-card')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-team-name-display'))
    await fireEvent.input(screen.getByTestId('mesh-builder-team-name-input'), {
      target: { value: 'taurhaus-team' },
    })

    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalledWith(
        expect.objectContaining({
          teamName: 'taurhaus-team',
          lead: expect.objectContaining({
            name: 'team-lead',
            cliTool: 'claude',
            model: 'opus',
            roleId: 'lead-default',
          }),
        })
      )
    })
  })

  it('normalizes WSL UNC project paths before generating patterned agent names', async () => {
    listRoleTemplates.mockResolvedValueOnce([
      {
        roleId: 'lead-default',
        name: 'Lead',
        kind: 'lead',
        cliTool: 'claude',
        model: 'opus',
      },
      {
        roleId: 'design-specialist',
        name: 'Design Specialist',
        kind: 'agent',
        cliTool: 'claude',
        model: 'claude-opus-4.5',
        defaultNamePattern: 'design-{project}-{n}',
      },
    ])

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '\\\\wsl.localhost\\Ubuntu\\home\\user\\projects\\2ksim',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-builder-role-lead-default')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-builder-role-design-specialist')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-role-lead-default'))
    await fireEvent.click(screen.getByTestId('mesh-template-browse-catalog'))
    await fireEvent.click(screen.getByTestId('mesh-builder-role-design-specialist'))

    await fireEvent.click(screen.getByTestId('mesh-builder-team-name-display'))
    await fireEvent.input(screen.getByTestId('mesh-builder-team-name-input'), {
      target: { value: '2ksim-team' },
    })
    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalledWith(
        expect.objectContaining({
          teamName: '2ksim-team',
          agents: expect.arrayContaining([
            expect.objectContaining({
              name: 'design-2ksim-1',
            }),
          ]),
        })
      )
    })
  })

  it('reset returns setup state back to empty mode', async () => {
    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
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
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
        availableProjects: [{ id: 'proj-api', name: 'API' }],
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-form')).toBeInTheDocument()
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-role-card-agent-default')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-role-card-agent-default'))

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
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })
    listRoleTemplates.mockReset()
    listRoleTemplates.mockReturnValueOnce(rolesLoad.promise)

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-form')).toBeInTheDocument()
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-role-loading')).toBeInTheDocument()
    })

    rolesLoad.resolve([
      { roleId: 'agent-default', name: 'Agent', kind: 'agent', cliTool: 'codex', model: 'gpt-5.4 high' },
    ])

    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-role-card-agent-default')).toBeInTheDocument()
    })
  })

  // Regression: b345de1 (PR 5c) read the effort from `role.reasoningEffort` while
  // the role response keeps it under `defaults`, so hot-adding a role that
  // declares `high` autofilled and submitted the catalog default instead.
  it('preserves selected role metadata when hot-adding a runtime agent', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    await renderRuntime({
      availableProjects: [{ id: 'proj-core', name: 'Core' }],
    })
    listRoleTemplates.mockReset()
    listRoleTemplates.mockResolvedValueOnce([
      {
        roleId: 'codex-architect',
        name: 'Codex Architect',
        kind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.4',
        defaults: { cliTool: 'codex', model: 'gpt-5.4', reasoningEffort: 'high' },
        focusArea: 'Architecture decisions and structural review',
        contextSummary: 'Carries long-lived context around module boundaries and reviews.',
        behaviorSummary: 'Handles pattern choices and escalates direction changes.',
        instructions: 'Own structural review',
      },
    ])

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-form')).toBeInTheDocument()
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-role-card-codex-architect')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-role-card-codex-architect'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-model-select')).toHaveValue('gpt-5.4')
      expect(screen.getByTestId('mesh-add-agent-model-select-effort')).toHaveValue('high')
    })
    await fireEvent.input(screen.getByTestId('mesh-add-agent-name-input'), {
      target: { value: 'review-architect' },
    })
    await fireEvent.change(screen.getByTestId('mesh-add-agent-project-select'), {
      target: { value: 'proj-core' },
    })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-submit'))

    await waitFor(() => {
      expect(coordinationAddAgent).toHaveBeenCalledWith(
        expect.objectContaining({
          teamName: 'architecture-final',
          agent: expect.objectContaining({
            name: 'review-architect',
            cliTool: 'codex',
            model: 'gpt-5.4',
            reasoningEffort: 'high',
            roleId: 'codex-architect',
            roleName: 'Codex Architect',
            focusArea: 'Architecture decisions and structural review',
            contextSummary: 'Carries long-lived context around module boundaries and reviews.',
            behaviorSummary: 'Handles pattern choices and escalates direction changes.',
            instructions: 'Own structural review',
            description: 'Own structural review',
          }),
        })
      )
    })
  })

  // Regression: b345de1 (PR 5c) offered an empty "default" reasoning-effort option
  // on the runtime add-agent form too. Choosing it sent reasoningEffort: null,
  // which `hydrate_member_model_fields` (member_activation.rs) refills from the
  // selected role template, so the member still started at the role's effort.
  it('hides the inherit-global effort option for a role that declares one', async () => {
    await renderRuntime({
      availableProjects: [{ id: 'proj-core', name: 'Core' }],
    })
    listRoleTemplates.mockReset()
    listRoleTemplates.mockResolvedValueOnce([
      {
        roleId: 'codex-architect',
        name: 'Codex Architect',
        kind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.4',
        defaults: { cliTool: 'codex', model: 'gpt-5.4', reasoningEffort: 'high' },
        behaviorSummary: 'Handles pattern choices and escalates direction changes.',
        instructions: 'Own structural review',
      },
    ])

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-role-card-codex-architect')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-role-card-codex-architect'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-model-select-effort')).toHaveValue('high')
    })
    const effortOptions = Array.from(
      screen.getByTestId('mesh-add-agent-model-select-effort').querySelectorAll('option')
    ).map((option) => option.value)
    expect(effortOptions).not.toContain('')
  })

  it('captures runtime node as role and saves through upsertRoleTemplate', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
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
            model: 'gpt-5.4',
            reasoningEffort: 'high',
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
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/my-app',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-dev-team'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalledWith(
        expect.objectContaining({
          teamName: 'my-app-team',
          presetId: 'dev-team',
          lead: expect.objectContaining({
            cliTool: '',
          }),
          agents: expect.arrayContaining([
            expect.objectContaining({ cliTool: '' }),
          ]),
        })
      )
    })

  })

  it.each([
    {
      presetId: 'custom-codex-team',
      leadRoleId: 'codex-orchestrator',
      leadTool: 'codex',
      leadModel: 'gpt-5.4 high',
    },
    {
      presetId: 'custom-antigravity-team',
      leadRoleId: 'antigravity-orchestrator',
      leadTool: 'agy',
      leadModel: 'gemini-3.7-flash-high',
    },
  ])('composes and initializes backend-loaded non-Claude preset $presetId cleanly', async ({
    presetId,
    leadRoleId,
    leadTool,
    leadModel,
  }) => {
    listTeamPresets.mockResolvedValueOnce([
      {
        presetId,
        name: `Preset ${presetId}`,
        description: 'Lead + agents',
        leadRoleId,
        roleCount: 3,
        agentCount: 2,
        tools: [leadTool, 'codex'],
      },
    ])
    getTeamPreset.mockResolvedValueOnce({
      presetId,
      name: `Preset ${presetId}`,
      leadRoleId,
      agentSlots: [{ roleId: 'agent-default', count: 2 }],
    })
    composeTeam.mockResolvedValueOnce({
      roster: [
        {
          name: 'team-lead',
          roleId: leadRoleId,
          roleKind: 'lead',
          cliTool: leadTool,
          model: leadModel,
          instructions: 'Own orchestration',
          focusArea: 'Lead orchestration',
          contextSummary: 'Keeps the team aligned.',
          behaviorSummary: 'Coordinates specialists and escalates blockers.',
          behavioralContract: { communication: [], execution: [], escalation: [] },
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
      ],
      warnings: [],
      validationErrors: [],
    })

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/my-app',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await waitFor(() => {
      expect(screen.getByTestId(`mesh-template-preset-${presetId}`)).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId(`mesh-template-preset-${presetId}`))

    await waitFor(() => {
      expect(composeTeam).toHaveBeenCalledWith(
        expect.objectContaining({
          leadRoleId,
        })
      )
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalledWith(
        expect.objectContaining({
          teamName: 'my-app-team',
          presetId,
          lead: expect.objectContaining({
            cliTool: '',
            model: '',
            projectId: '/projects/my-app',
          }),
          agents: expect.arrayContaining([
            expect.objectContaining({ cliTool: '', model: '' }),
          ]),
        })
      )
    })
  })

  // Regression: b345de1 (PR 5c) let the advanced preset editor pin the lead's model
  // and effort, but the quick-preset launch only forwarded the role id and slots, so
  // composition rebuilt the lead from the role defaults and the pin never launched.
  it('forwards a preset lead override to composition', async () => {
    listTeamPresets.mockResolvedValueOnce([
      {
        presetId: 'lead-pinned-team',
        name: 'Lead Pinned Team',
        description: 'Lead pinned to a model and effort',
        leadRoleId: 'codex-orchestrator',
        roleCount: 1,
        agentCount: 1,
        tools: ['codex'],
      },
    ])
    getTeamPreset.mockResolvedValueOnce({
      presetId: 'lead-pinned-team',
      name: 'Lead Pinned Team',
      leadRoleId: 'codex-orchestrator',
      leadOverrides: { model: 'gpt-5.6-terra', reasoningEffort: 'xhigh' },
      agentSlots: [{ roleId: 'agent-default', count: 1 }],
    })
    composeTeam.mockResolvedValueOnce({
      roster: [],
      warnings: [],
      validationErrors: [],
    })

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/my-app',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-template-preset-lead-pinned-team')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-template-preset-lead-pinned-team'))

    await waitFor(() => {
      expect(composeTeam).toHaveBeenCalledWith(
        expect.objectContaining({
          leadRoleId: 'codex-orchestrator',
          overrides: expect.objectContaining({
            lead: expect.objectContaining({ model: 'gpt-5.6-terra', reasoningEffort: 'xhigh' }),
          }),
        })
      )
    })
  })

  it('loads quick preset members into the editable roster before initialization', async () => {
    coordinationInitializeTeam.mockClear()
    getTeamPreset.mockResolvedValueOnce({
      presetId: 'full-team',
      name: 'Full Team',
      leadRoleId: 'v3-lead-claude',
      agentSlots: [
        {
          roleId: 'v3-architect-codex',
          count: 1,
          overrides: { namePattern: 'architect' },
        },
        {
          roleId: 'v3-developer-codex',
          count: 2,
        },
      ],
    })
    composeTeam.mockResolvedValueOnce({
      roster: [
        {
          name: 'team-lead',
          roleId: 'v3-lead-claude',
          roleName: 'V3 Team Lead (Claude)',
          roleKind: 'lead',
          cliTool: 'claude',
          model: 'opus',
          instructions: 'Own orchestration',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
        {
          name: 'architect',
          roleId: 'v3-architect-codex',
          roleName: 'V3 Architect (Codex)',
          roleKind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          instructions: 'Own structural review',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
        {
          name: 'dev-1',
          roleId: 'v3-developer-codex',
          roleName: 'V3 Developer (Codex)',
          roleKind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          instructions: 'Own implementation',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
        {
          name: 'dev-2',
          roleId: 'v3-developer-codex',
          roleName: 'V3 Developer (Codex)',
          roleKind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          instructions: 'Own implementation',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
      ],
      warnings: [],
      validationErrors: [],
    })
    listTeamPresets.mockResolvedValueOnce([
      {
        presetId: 'full-team',
        name: 'Full Team',
        description: 'Lead + agents',
        leadRoleId: 'v3-lead-claude',
        roleCount: 4,
        agentCount: 3,
        tools: ['claude', 'codex'],
      },
    ])

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/my-app',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-full-team'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-builder-lead-edit-toggle')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-builder-agent-edit-toggle-architect')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-builder-agent-edit-toggle-dev-1')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-builder-agent-edit-toggle-dev-2')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-lead-edit-toggle'))
    await fireEvent.click(screen.getByTestId('mesh-builder-agent-edit-toggle-architect'))
    await fireEvent.click(screen.getByTestId('mesh-builder-agent-edit-toggle-dev-1'))
    await fireEvent.click(screen.getByTestId('mesh-builder-agent-edit-toggle-dev-2'))

    expect(screen.getByTestId('mesh-builder-lead-name-input')).toHaveValue('team-lead')
    expect(screen.getByTestId('mesh-builder-agent-name-input-architect')).toHaveValue('architect')
    expect(screen.getByTestId('mesh-builder-agent-name-input-dev-1')).toHaveValue('dev-1')
    expect(screen.getByTestId('mesh-builder-agent-name-input-dev-2')).toHaveValue('dev-2')

    expect(coordinationInitializeTeam).not.toHaveBeenCalled()
  })

  it('uses backend-composed preset roster for setup names but sends a minimal preset payload on initialize', async () => {
    coordinationInitializeTeam.mockClear()
    getTeamPreset.mockResolvedValueOnce({
      presetId: 'full-team',
      name: 'Full Team',
      leadRoleId: 'v3-lead-claude',
      agentSlots: [
        {
          roleId: 'v3-architect-codex',
          count: 1,
          overrides: { namePattern: 'architect' },
        },
        {
          roleId: 'v3-developer-codex',
          count: 2,
        },
      ],
    })
    composeTeam.mockResolvedValueOnce({
      roster: [
        {
          name: 'team-lead',
          roleId: 'v3-lead-claude',
          roleKind: 'lead',
          cliTool: 'claude',
          model: 'opus',
          instructions: 'Own orchestration',
          focusArea: 'Team sequencing and escalation',
          contextSummary: 'Keeps the full delivery plan and blocker state in view.',
          behaviorSummary: 'Coordinates specialists and escalates blockers.',
          behavioralContract: { communication: ['sync'], execution: ['plan'], escalation: ['escalate'] },
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
        {
          name: 'architect',
          roleId: 'v3-architect-codex',
          roleKind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          instructions: 'Own structural review',
          focusArea: 'Architecture decisions and structural review',
          contextSummary: 'Carries long-lived context around module boundaries and reviews.',
          behaviorSummary: 'Handles pattern choices and escalates direction changes.',
          behavioralContract: { communication: [], execution: [], escalation: [] },
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
        {
          name: 'dev-1',
          roleId: 'v3-developer-codex',
          roleKind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          instructions: 'Own implementation',
          focusArea: 'Scoped implementation',
          contextSummary: 'Owns code changes, tests, and debugging within assigned scope.',
          behaviorSummary: 'Implements narrowly and escalates blockers.',
          behavioralContract: { communication: [], execution: [], escalation: [] },
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
        {
          name: 'dev-2',
          roleId: 'v3-developer-codex',
          roleKind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          instructions: 'Own implementation',
          focusArea: 'Scoped implementation',
          contextSummary: 'Owns code changes, tests, and debugging within assigned scope.',
          behaviorSummary: 'Implements narrowly and escalates blockers.',
          behavioralContract: { communication: [], execution: [], escalation: [] },
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
      ],
      warnings: [],
      validationErrors: [],
    })

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/my-app',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-full-team'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalled()
    })

    const request = coordinationInitializeTeam.mock.calls.at(-1)?.[0]
    expect(request?.lead?.name).toBe('team-lead')
    expect(request?.presetId).toBe('full-team')
    expect(request?.lead).toEqual(expect.objectContaining({
      cliTool: '',
      model: '',
      roleId: null,
      roleName: null,
      instructions: null,
    }))
    expect(request?.agents?.map((agent) => agent.name)).toEqual([
      'architect',
      'dev-1',
      'dev-2',
    ])
    expect(request?.agents.every((agent) => (
      agent.cliTool === ''
      && agent.model === ''
      && agent.roleId === null
      && agent.roleName === null
      && agent.instructions === null
    ))).toBe(true)
  })

  it('detaches edited preset rosters before initialize so the backend receives a custom payload', async () => {
    coordinationInitializeTeam.mockClear()
    getTeamPreset.mockResolvedValueOnce({
      presetId: 'full-team',
      name: 'Full Team',
      leadRoleId: 'v3-lead-claude',
      agentSlots: [
        {
          roleId: 'v3-architect-codex',
          count: 1,
          overrides: { namePattern: 'architect' },
        },
        {
          roleId: 'v3-developer-codex',
          count: 2,
        },
      ],
    })
    composeTeam.mockResolvedValueOnce({
      roster: [
        {
          name: 'team-lead',
          roleId: 'v3-lead-claude',
          roleKind: 'lead',
          cliTool: 'claude',
          model: 'opus',
          instructions: 'Own orchestration',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
        {
          name: 'architect',
          roleId: 'v3-architect-codex',
          roleKind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4',
          reasoningEffort: 'high',
          instructions: 'Own structural review',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
        {
          name: 'dev-1',
          roleId: 'v3-developer-codex',
          roleKind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4',
          reasoningEffort: 'high',
          instructions: 'Own implementation',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
        {
          name: 'dev-2',
          roleId: 'v3-developer-codex',
          roleKind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4',
          reasoningEffort: 'high',
          instructions: 'Own implementation',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
      ],
      warnings: [],
      validationErrors: [],
    })

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/my-app',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-full-team'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-builder-agent-remove-dev-2')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-agent-remove-dev-2'))
    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalled()
    })

    const request = coordinationInitializeTeam.mock.calls.at(-1)?.[0]
    expect(request?.presetId).toBeUndefined()
    expect(request?.lead).toEqual(expect.objectContaining({
      name: 'team-lead',
      cliTool: 'claude',
      model: 'opus',
      roleId: 'v3-lead-claude',
    }))
    expect(request?.agents).toEqual([
      // Regression: ff40911 stripped the effort suffix and 5d2ce27 aliased the
      // model, so a roster asking for `high` launched at the user's global effort.
      // The initialize payload now carries the split, canonical pair.
      expect.objectContaining({
        name: 'architect',
        cliTool: 'codex',
        model: 'gpt-5.4',
        reasoningEffort: 'high',
        roleId: 'v3-architect-codex',
      }),
      expect.objectContaining({
        name: 'dev-1',
        cliTool: 'codex',
        model: 'gpt-5.4',
        reasoningEffort: 'high',
        roleId: 'v3-developer-codex',
      }),
    ])
  })

  // Regression: b345de1 (PR 5c) let the builder edit the model and effort per
  // member but still saved every slot with `overrides: null`, so reloading the
  // saved preset restored the role defaults and lost the selection.
  it('saves the builder roster model and effort as slot overrides', async () => {
    upsertTeamPreset.mockResolvedValue(undefined)
    getTeamPreset.mockResolvedValueOnce({
      presetId: 'full-team',
      name: 'Full Team',
      leadRoleId: 'v3-lead-claude',
      agentSlots: [{ roleId: 'v3-developer-codex', count: 1 }],
    })
    composeTeam.mockResolvedValueOnce({
      roster: [
        {
          name: 'team-lead',
          roleId: 'v3-lead-claude',
          roleKind: 'lead',
          cliTool: 'claude',
          model: 'opus',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
        {
          name: 'dev-1',
          roleId: 'v3-developer-codex',
          roleKind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4',
          reasoningEffort: 'xhigh',
          capabilities: [],
          projectBinding: 'lead_project',
          projectId: null,
        },
      ],
      warnings: [],
      validationErrors: [],
    })

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/my-app',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-full-team'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-builder-save-preset')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-save-preset'))

    await waitFor(() => {
      expect(upsertTeamPreset).toHaveBeenCalledWith(
        expect.objectContaining({
          leadRoleId: 'v3-lead-claude',
          agentSlots: [
            expect.objectContaining({
              roleId: 'v3-developer-codex',
              overrides: { model: 'gpt-5.4', reasoningEffort: 'xhigh' },
            }),
          ],
        })
      )
    })
  })

  // Regression: b345de1 (PR 5c) normalized the runtime effort but dropped it in
  // the `createAgent` mapping and in `MeshRuntimeView`'s `detailNode`, so the
  // effort the backend reported never reached the node detail for either role.
  it('shows the reported reasoning effort in the runtime node detail', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(
      buildRuntimeSnapshot({
        members: [
          {
            name: 'team-lead',
            role: 'lead',
            cliTool: 'claude',
            model: 'opus',
            reasoningEffort: 'high',
            projectId: '/projects/taurhaus',
            sessionStatus: 'active',
            paneId: '%1',
          },
          {
            name: 'frontend-dev',
            role: 'member',
            cliTool: 'codex',
            model: 'gpt-5.6-terra',
            reasoningEffort: 'xhigh',
            projectId: '/projects/taurhaus',
            sessionStatus: 'idle',
            paneId: '%2',
          },
        ],
      })
    )

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-tool-model')).toHaveTextContent(
        'gpt-5.6-terra · xhigh'
      )
    })

    await fireEvent.click(screen.getByTestId('mesh-node-lead'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-tool-model')).toHaveTextContent('opus · high')
    })
  })

  it('matches teams when lead project path uses windows drive notation', async () => {
    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildRuntimeSnapshot())

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
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
        modelCatalog: TEST_MODEL_CATALOG,
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
        modelCatalog: TEST_MODEL_CATALOG,
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
      expect(screen.getByTestId('mesh-error')).toHaveTextContent('cannot disband')
    })
  })

  it('invalidates cached runtime state after disband so tab re-entry stays empty', async () => {
    setMeshCache('/projects/taurhaus', buildRuntimeSnapshot())

    const runtimeView = render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    expect(coordinationGetProjectMeshSnapshot).not.toHaveBeenCalled()

    await fireEvent.click(screen.getByTestId('mesh-runtime-more-toggle'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-disband')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-runtime-disband'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })

    coordinationGetProjectMeshSnapshot.mockResolvedValueOnce(buildProjectMeshSnapshot({
      meshAvailable: true,
      tmuxAvailable: true,
      teamName: null,
      teamStatus: null,
      warnings: [],
    }))

    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })
    expect(getMeshCache('/projects/taurhaus')).toBeNull()

    runtimeView.unmount()

    render(MeshTab, {
      props: {
        dark: false,
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })
    expect(coordinationGetProjectMeshSnapshot).toHaveBeenCalledTimes(1)
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

  it('opens the runtime detail immediately after clicking a node', async () => {
    await renderRuntime()

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))

    expect(screen.getByTestId('mesh-node-detail')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-name')).toHaveTextContent('Codex Architect')
    expect(screen.getByTestId('mesh-node-detail-subject')).toHaveTextContent('frontend-dev')
  })

  // Regression: e857c8f spent a 32ms budget against the wall clock, so the
  // assertion measured host load rather than the component and failed under a
  // loaded full-suite run ("expected 53.7 to be less than or equal to 32",
  // reproduced in 1 of 3 runs with every core busy). e9b9e08 then froze
  // `performance.now`, which made both budgets unfalsifiable: with the clock
  // held still the rendered stage always reads 0ms and the visible stage always
  // reads exactly the one frame the test advances, so the "under 32ms" and
  // "under 120ms" claims held even with 500ms of synchronous stall injected
  // into the click handler (verified by mutation — the old test passed).
  //
  // What a JSDOM test can hold the component to is scheduling, not wall-clock
  // time, so that is now all it claims: the detail renders inside the click's
  // own turns with no frame spent, and exactly one frame carries it from
  // rendered to fully visible. The millisecond numbers below are the fake
  // clock's own units and exist to pin those frame boundaries; a real latency
  // budget needs a browser benchmark, not this lane.
  it('renders the runtime node detail in the click\'s own turns and shows it one frame later', async () => {
    await renderRuntime()

    const FRAME_MS = 16
    // Non-zero: the component treats a falsy start stamp as "not measuring".
    let fakeNow = 1_000
    let framesRun = 0
    const frameCallbacks = []
    const debugSpy = vi.spyOn(console, 'debug').mockImplementation(() => {})
    const nowSpy = vi.spyOn(performance, 'now').mockImplementation(() => fakeNow)
    const rafSpy = vi
      .spyOn(globalThis, 'requestAnimationFrame')
      .mockImplementation((callback) => frameCallbacks.push(callback))
    globalThis.__TAURHAUS_MESH_DETAIL_PERF__ = true

    const perfLog = (stage) =>
      debugSpy.mock.calls
        .map(([, payload]) => payload)
        .find((payload) => payload?.stage === stage)

    const runPendingFrames = () => {
      fakeNow += FRAME_MS
      framesRun += 1
      const pending = frameCallbacks.splice(0, frameCallbacks.length)
      pending.forEach((callback) => callback(fakeNow))
      return pending.length
    }

    try {
      await fireEvent.click(screen.getByTestId('mesh-node-agent'))

      expect(screen.getByTestId('mesh-node-detail')).toBeInTheDocument()

      // The detail renders within the click's own microtask turns — no frame is
      // spent getting there, and none is needed to log it.
      let renderTurns = 0
      while (!perfLog('rendered') && renderTurns < 4) {
        await tick()
        renderTurns += 1
      }
      const renderedLog = perfLog('rendered')
      expect(renderedLog, 'the detail must render in the click\'s own turns').toBeDefined()
      expect(renderTurns).toBeLessThanOrEqual(1)
      expect(framesRun, 'no frame may be spent before the detail renders').toBe(0)
      // Zero on the fake clock: the stage was logged without crossing a frame.
      expect(renderedLog.elapsedMs).toBe(0)
      expect(perfLog('visible')).toBeUndefined()

      // One frame — and only one — carries the detail from rendered to fully
      // visible. A second chained frame would leave the stage unlogged here.
      expect(runPendingFrames(), 'the detail must request a frame').toBeGreaterThan(0)
      await tick()

      const visibleLog = perfLog('visible')
      expect(visibleLog, 'the detail must be visible on the first frame').toBeDefined()
      // Exactly one frame of the fake clock, measured from the click.
      expect(visibleLog.elapsedMs).toBe(FRAME_MS)
    } finally {
      delete globalThis.__TAURHAUS_MESH_DETAIL_PERF__
      rafSpy.mockRestore()
      nowSpy.mockRestore()
      debugSpy.mockRestore()
    }
  })

  it('keeps runtime actions visible for offline agents and disables focus when pane is missing', async () => {
    const onFocusPane = vi.fn()
    await renderRuntime({
      onFocusPane,
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
        modelCatalog: TEST_MODEL_CATALOG,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-empty')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-builder-role-lead-default'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-setup')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-builder-role-agent-default'))
    await waitFor(() => {
      expect(screen.getAllByTestId('mesh-node-agent')).toHaveLength(1)
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-agent-remove-agent-default-1'))
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

  it('auto-closes the runtime detail after a successful resume action', async () => {
    await renderRuntime()

    await fireEvent.click(screen.getByTestId('mesh-node-agent'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-resume')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-resume'))

    await waitFor(() => {
      expect(coordinationResumeMember).toHaveBeenCalledWith('architecture-final', 'frontend-dev')
      expect(screen.queryByTestId('mesh-node-detail')).not.toBeInTheDocument()
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
    coordinationAddAgent.mockRejectedValueOnce(new Error('add failed'))

    await renderRuntime({
      availableProjects: [{ id: 'proj-core', name: 'Core' }],
    })
    listRoleTemplates.mockReset()
    listRoleTemplates.mockResolvedValueOnce([
      {
        roleId: 'runtime-agent',
        name: 'Runtime Agent',
        kind: 'agent',
        cliTool: 'agy',
        model: '',
        instructions: '',
      },
    ])

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-form')).toBeInTheDocument()
    })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-role-card-runtime-agent')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-role-card-runtime-agent'))
    expect(screen.getByTestId('mesh-add-agent-tool-select')).toHaveValue('agy')
    expect(screen.getByTestId('mesh-add-agent-model-select')).toHaveValue('gemini-3.7-flash-high')

    await fireEvent.click(screen.getByTestId('mesh-add-agent-unlock-toggle'))
    await fireEvent.change(screen.getByTestId('mesh-add-agent-tool-select'), {
      target: { value: 'claude' },
    })
    expect(screen.getByTestId('mesh-add-agent-model-select')).toHaveValue('opus')

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

  it('filters the runtime add-agent role catalog by tool and kind', async () => {
    await renderRuntime({
      availableProjects: [{ id: 'proj-core', name: 'Core' }],
    })
    listRoleTemplates.mockReset()
    listRoleTemplates.mockResolvedValueOnce([
      {
        roleId: 'lead-claude',
        name: 'Lead Claude',
        kind: 'lead',
        cliTool: 'claude',
        model: 'opus',
        instructions: 'Lead coordination',
      },
      {
        roleId: 'agent-codex',
        name: 'Agent Codex',
        kind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.4 high',
        instructions: 'Codex implementation',
      },
      {
        roleId: 'agent-antigravity',
        name: 'Agent Antigravity',
        kind: 'agent',
        cliTool: 'agy',
        model: 'gemini-3.7-flash-high',
        instructions: 'Antigravity analysis',
      },
    ])

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-role-card-agent-codex')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-add-agent-role-card-agent-antigravity')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-filter-tool-codex'))
    expect(screen.getByTestId('mesh-add-agent-role-card-agent-codex')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-add-agent-role-card-agent-antigravity')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-add-agent-filter-tool-codex'))
    await fireEvent.click(screen.getByTestId('mesh-add-agent-filter-kind-lead'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-add-agent-role-card-lead-claude')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-add-agent-role-card-lead-claude')).toBeDisabled()
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
        modelCatalog: TEST_MODEL_CATALOG,
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
