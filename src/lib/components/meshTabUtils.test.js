import { describe, expect, it } from 'vitest'

import {
  buildInitializationRequest,
  buildTeamConfigFromPreset,
  buildTeamConfigFromRuntimeStatus,
  deriveCrossProjectMeta,
  projectNameFromPath,
} from './meshTabUtils.js'

describe('meshTabUtils cross-project metadata', () => {
  it('extracts a stable project basename from WSL UNC paths', () => {
    expect(projectNameFromPath('\\\\wsl.localhost\\Ubuntu\\home\\mstie\\projects\\2ksim')).toBe(
      '2ksim'
    )
  })

  it('derives cross-project metadata from explicit camelCase fields', () => {
    expect(
      deriveCrossProjectMeta(
        {
          projectId: '/home/user/projects/mesh',
          isCrossProject: true,
          projectLabel: 'mesh',
        },
        '/home/user/projects/taurhaus'
      )
    ).toEqual({
      isCrossProject: true,
      projectLabel: 'mesh',
    })
  })

  it('derives cross-project metadata from snake_case backend fields', () => {
    expect(
      deriveCrossProjectMeta(
        {
          project_id: '/home/user/projects/mesh',
          is_cross_project: true,
          project_label: 'mesh',
        },
        '/home/user/projects/taurhaus'
      )
    ).toEqual({
      isCrossProject: true,
      projectLabel: 'mesh',
    })
  })

  it('falls back to normalized project-path comparison when explicit fields are missing', () => {
    expect(
      deriveCrossProjectMeta(
        {
          projectId: 'C:\\Users\\me\\code\\mesh',
        },
        '/mnt/c/Users/me/code/taurhaus'
      )
    ).toEqual({
      isCrossProject: true,
      projectLabel: 'mesh',
    })
  })

  it('keeps local members local when normalized project paths match', () => {
    expect(
      deriveCrossProjectMeta(
        {
          projectId: '\\\\wsl.localhost\\Ubuntu\\home\\user\\projects\\taurhaus',
        },
        '/home/user/projects/taurhaus'
      )
    ).toEqual({
      isCrossProject: false,
      projectLabel: '',
    })
  })

  it('treats case-variant Windows paths as the same project', () => {
    expect(
      deriveCrossProjectMeta(
        {
          projectId: 'c:\\users\\me\\code\\taurhaus',
        },
        'C:\\Users\\Me\\Code\\Taurhaus'
      )
    ).toEqual({
      isCrossProject: false,
      projectLabel: '',
    })
  })

  it('normalizes runtime status members with cross-project fields for the mesh canvas', () => {
    const config = buildTeamConfigFromRuntimeStatus({
      leadName: 'team-lead',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          model: 'opus',
          projectId: '/home/user/projects/taurhaus',
          sessionStatus: 'active',
          isCrossProject: false,
          projectLabel: '',
        },
        {
          name: 'mesh-expert',
          role: 'member',
          cliTool: 'gemini',
          model: '2.5-pro',
          projectId: '/home/user/projects/mesh',
          sessionStatus: 'active',
          isCrossProject: true,
          projectLabel: 'mesh',
        },
      ],
    })

    expect(config.lead.isCrossProject).toBe(false)
    expect(config.agents).toEqual([
      expect.objectContaining({
        id: 'mesh-expert',
        isCrossProject: true,
        projectLabel: 'mesh',
      }),
    ])
  })

  it('preserves runtime role summary fields from backend member status', () => {
    const config = buildTeamConfigFromRuntimeStatus({
      leadName: 'team-lead',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          model: 'opus',
          sessionStatus: 'active',
          roleName: 'Claude Orchestrator',
          focusArea: 'Team sequencing and escalation',
          contextSummary: 'Keeps the full delivery plan and blocker state in view.',
          behaviorSummary: 'Coordinates specialists and escalates blockers.',
        },
        {
          name: 'frontend-dev',
          role: 'member',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          sessionStatus: 'active',
          role_name: 'Codex Architect',
          focus_area: 'Architecture decisions and structural review',
          context_summary: 'Carries long-lived context around module boundaries and reviews.',
          behavior_summary: 'Handles pattern choices and escalates direction changes.',
        },
      ],
    })

    expect(config.lead).toEqual(expect.objectContaining({
      roleName: 'Claude Orchestrator',
      focusArea: 'Team sequencing and escalation',
      contextSummary: 'Keeps the full delivery plan and blocker state in view.',
      behaviorSummary: 'Coordinates specialists and escalates blockers.',
    }))
    expect(config.agents).toEqual([
      expect.objectContaining({
        id: 'frontend-dev',
        roleName: 'Codex Architect',
        focusArea: 'Architecture decisions and structural review',
        contextSummary: 'Carries long-lived context around module boundaries and reviews.',
        behaviorSummary: 'Handles pattern choices and escalates direction changes.',
      }),
    ])
  })

  it('keeps rich role metadata for custom initialization requests', () => {
    const request = buildInitializationRequest({
      initializationMode: 'custom',
      lead: {
        name: 'team-lead',
        tool: 'claude',
        model: 'opus',
        projectId: '/projects/taurhaus',
        description: 'Owns orchestration and escalation.',
        roleId: 'claude-orchestrator',
        roleName: 'Claude Orchestrator',
        focusArea: 'Team sequencing and escalation',
        contextSummary: 'Keeps the delivery plan and blocker state in view.',
        behaviorSummary: 'Coordinates specialists and escalates blockers.',
        instructions: 'Drive the team plan and keep dependencies aligned.',
      },
      agents: [
        {
          name: 'frontend-dev',
          tool: 'codex',
          model: 'gpt-5.4 high',
          projectId: '/projects/taurhaus',
          description: 'Owns UI implementation.',
          roleId: 'codex-developer',
          roleName: 'Codex Developer',
          focusArea: 'Scoped implementation',
          contextSummary: 'Owns code changes, tests, and debugging within assigned scope.',
          behaviorSummary: 'Implements narrowly and escalates blockers.',
          instructions: 'Implement the assigned UI surface and verify it locally.',
        },
      ],
    }, 'taurhaus-team', '/projects/taurhaus')

    expect(request.lead).toEqual(expect.objectContaining({
      roleId: 'claude-orchestrator',
      roleName: 'Claude Orchestrator',
      focusArea: 'Team sequencing and escalation',
      contextSummary: 'Keeps the delivery plan and blocker state in view.',
      behaviorSummary: 'Coordinates specialists and escalates blockers.',
      instructions: 'Drive the team plan and keep dependencies aligned.',
    }))
    expect(request.agents).toEqual([
      expect.objectContaining({
        roleId: 'codex-developer',
        roleName: 'Codex Developer',
        focusArea: 'Scoped implementation',
        contextSummary: 'Owns code changes, tests, and debugging within assigned scope.',
        behaviorSummary: 'Implements narrowly and escalates blockers.',
        instructions: 'Implement the assigned UI surface and verify it locally.',
      }),
    ])
  })

  it('builds a minimal preset initialization payload and omits role metadata', () => {
    const request = buildInitializationRequest({
      initializationMode: 'preset',
      presetId: 'full-team',
      lead: {
        name: 'team-lead',
        tool: 'claude',
        model: 'opus',
        projectId: '/projects/taurhaus',
        roleId: 'v3-lead-claude',
        roleName: 'V3 Team Lead (Claude)',
        focusArea: 'Team sequencing and escalation',
        contextSummary: 'Keeps the delivery plan and blocker state in view.',
        behaviorSummary: 'Coordinates specialists and escalates blockers.',
        instructions: 'Drive the team plan and keep dependencies aligned.',
      },
      agents: [
        {
          name: 'architect',
          tool: 'codex',
          model: 'gpt-5.4 high',
          projectId: '/projects/taurhaus',
          roleId: 'v3-architect-codex',
          roleName: 'V3 Architect (Codex)',
          focusArea: 'Architecture decisions and structural review',
          contextSummary: 'Carries long-lived context around module boundaries and reviews.',
          behaviorSummary: 'Handles pattern choices and escalates direction changes.',
          instructions: 'Review structure and boundaries.',
        },
      ],
    }, 'taurhaus-team', '/projects/taurhaus')

    expect(request).toEqual({
      teamName: 'taurhaus-team',
      teamDescription: null,
      leadMode: 'launch_new',
      presetId: 'full-team',
      lead: {
        name: 'team-lead',
        cliTool: '',
        model: '',
        projectId: '/projects/taurhaus',
        description: null,
        roleId: null,
        roleName: null,
        focusArea: null,
        contextSummary: null,
        behaviorSummary: null,
        instructions: null,
        behavioralContract: null,
        capabilities: null,
      },
      agents: [
        {
          name: 'architect',
          cliTool: '',
          model: '',
          projectId: '/projects/taurhaus',
          description: null,
          roleId: null,
          roleName: null,
          focusArea: null,
          contextSummary: null,
          behaviorSummary: null,
          instructions: null,
          behavioralContract: null,
          capabilities: null,
        },
      ],
    })
  })

  it('preserves a non-Claude lead from preset composition results', () => {
    const config = buildTeamConfigFromPreset(
      {
        presetId: 'codex-team',
        leadRoleId: 'codex-orchestrator',
        tools: ['codex', 'gemini'],
        agentSlots: [],
      },
      {
        roster: [
          {
            name: 'team-lead',
            roleId: 'codex-orchestrator',
            roleName: 'Codex Orchestrator',
            roleKind: 'lead',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
          },
        ],
      },
      '/projects/taurhaus'
    )

    expect(config.lead).toEqual(expect.objectContaining({
      roleId: 'codex-orchestrator',
      roleName: 'Codex Orchestrator',
      tool: 'codex',
      model: 'gpt-5.4 high',
    }))
  })

  it('does not backfill Claude when preset lead metadata is absent', () => {
    const config = buildTeamConfigFromPreset(
      {
        presetId: 'unknown-lead',
        leadRoleId: 'mystery-lead',
        tools: [],
        agentSlots: [],
      },
      null,
      '/projects/taurhaus'
    )

    expect(config.lead).toEqual(expect.objectContaining({
      roleId: 'mystery-lead',
      tool: '',
      model: '',
    }))
  })
})
