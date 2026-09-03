import { describe, expect, it } from 'vitest'

import { TEST_MODEL_CATALOG as CATALOG } from '../../test/fixtures/modelCatalog.js'
import {
  accountLineLabel,
  buildInitializationRequest,
  buildTeamConfigFromPreset,
  buildTeamConfigFromRuntimeStatus,
  deriveCrossProjectMeta,
  projectNameFromPath,
} from './meshTabUtils.js'

describe('accountLineLabel', () => {
  it('uses one wording rule for account fallback and applied states', () => {
    expect(accountLineLabel({ accountLabel: 'Personal', accountApplied: true })).toBe(
      'Personal · applied'
    )
    expect(accountLineLabel({
      accountLabel: 'Personal',
      accountNote: 'account_fallback',
      accountFallbackFrom: 'Work',
    })).toBe('was Work → now Personal')
    expect(accountLineLabel({})).toBe('')
  })
})

describe('meshTabUtils cross-project metadata', () => {
  it('extracts a stable project basename from WSL UNC paths', () => {
    expect(projectNameFromPath('\\\\wsl.localhost\\Ubuntu\\home\\user\\projects\\2ksim')).toBe(
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
          cliTool: 'agy',
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

  it('preserves runtime snapshot freshness for runtime-mode team config', () => {
    const config = buildTeamConfigFromRuntimeStatus({
      leadName: 'team-lead',
      runtimeSnapshotFreshness: 'attachments_only',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          model: 'opus',
          sessionStatus: 'active',
        },
      ],
    })

    expect(config.runtimeSnapshotFreshness).toBe('attachments_only')
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
        tools: ['codex', 'agy'],
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

  it('continues numbered agent names when preset slot overrides reuse a concrete numbered name', () => {
    const config = buildTeamConfigFromPreset(
      {
        presetId: 'saved-dev-team',
        leadRoleId: 'claude-orchestrator',
        tools: ['claude', 'codex'],
        agentSlots: [
          {
            roleId: 'codex-developer',
            count: 3,
            overrides: { namePattern: 'dev-1' },
          },
        ],
      },
      null,
      '/projects/taurhaus'
    )

    expect(config.agents.map((agent) => agent.name)).toEqual([
      'dev-1',
      'dev-2',
      'dev-3',
    ])
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

describe('meshTabUtils reasoning effort', () => {
  // Regression: b345de1 (PR 5c) normalized the member effort in
  // `buildTeamConfigFromRuntimeStatus` and forwarded it for the lead only, so
  // every non-lead runtime node reached the canvas and the node detail without
  // the effort the backend reported.
  it('forwards the runtime effort for the lead and for every agent', () => {
    const config = buildTeamConfigFromRuntimeStatus({
      leadName: 'team-lead',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          model: 'opus',
          reasoningEffort: 'high',
          sessionStatus: 'active',
        },
        {
          name: 'frontend-dev',
          role: 'member',
          cli_tool: 'codex',
          model: 'gpt-5.6-terra',
          reasoning_effort: 'xhigh',
          sessionStatus: 'active',
        },
      ],
    })

    expect(config.lead.reasoningEffort).toBe('high')
    expect(config.agents).toEqual([
      expect.objectContaining({ id: 'frontend-dev', model: 'gpt-5.6-terra', reasoningEffort: 'xhigh' }),
    ])
  })

  // Regression: b345de1 (PR 5c) resolved a missing effort to the catalog's
  // `defaultEffort` for every member, so initialize pinned an effort the user
  // never chose. The backend leaves it unset so the CLI's global setting applies.
  it('leaves an unset effort unset in the initialize payload', () => {
    const request = buildInitializationRequest(
      {
        initializationMode: 'custom',
        lead: { name: 'team-lead', tool: 'claude', model: 'opus', projectId: '/projects/taurhaus' },
        agents: [
          { name: 'dev-1', tool: 'codex', model: 'gpt-5.4', projectId: '/projects/taurhaus' },
          {
            name: 'dev-2',
            tool: 'codex',
            model: 'gpt-5.6-terra',
            reasoningEffort: 'xhigh',
            projectId: '/projects/taurhaus',
          },
        ],
      },
      'taurhaus-team',
      '/projects/taurhaus',
      CATALOG
    )

    expect(request.lead.reasoningEffort).toBeNull()
    expect(request.agents.map((agent) => [agent.model, agent.reasoningEffort])).toEqual([
      ['gpt-5.4', null],
      ['gpt-5.6-terra', 'xhigh'],
    ])
  })

  it('uses the catalog default model and effort when the member declares no model', () => {
    const request = buildInitializationRequest(
      {
        initializationMode: 'custom',
        lead: { name: 'team-lead', tool: 'claude', projectId: '/projects/taurhaus' },
        agents: [{ name: 'dev-1', tool: 'codex', projectId: '/projects/taurhaus' }],
      },
      'taurhaus-team',
      '/projects/taurhaus',
      CATALOG
    )

    expect(request.agents[0]).toEqual(
      expect.objectContaining({ model: 'gpt-5.6-sol', reasoningEffort: 'low' })
    )
  })

  it('carries selector-capable member account assignments into initialization', () => {
    const request = buildInitializationRequest(
      {
        initializationMode: 'custom',
        lead: {
          name: 'team-lead',
          tool: 'codex',
          accountId: 'codex-work',
          projectId: '/projects/taurhaus',
        },
        agents: [
          {
            name: 'grok-reviewer',
            tool: 'grok',
            accountId: 'grok-personal',
            projectId: '/projects/taurhaus',
          },
        ],
      },
      'taurhaus-team',
      '/projects/taurhaus',
      CATALOG
    )

    expect(request.lead.accountId).toBe('codex-work')
    expect(request.agents[0].accountId).toBe('grok-personal')
  })
})

describe('meshTabUtils task effort', () => {
  it('carries the assignment effort and its reason for the lead and every agent', () => {
    const config = buildTeamConfigFromRuntimeStatus({
      leadName: 'team-lead',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          model: 'opus',
          reasoningEffort: 'high',
          taskEffort: 'medium',
          taskEffortWhy: 'routing only',
          sessionStatus: 'active',
        },
        {
          name: 'frontend-dev',
          role: 'member',
          cli_tool: 'codex',
          model: 'gpt-5.6-terra',
          reasoning_effort: 'low',
          task_effort: 'high',
          task_effort_why: 'the migration is irreversible',
          sessionStatus: 'active',
        },
      ],
    })

    expect(config.lead.taskEffort).toBe('medium')
    expect(config.lead.taskEffortWhy).toBe('routing only')
    expect(config.agents).toEqual([
      expect.objectContaining({
        id: 'frontend-dev',
        reasoningEffort: 'low',
        taskEffort: 'high',
        taskEffortWhy: 'the migration is irreversible',
      }),
    ])
  })

  it('leaves a member with no assignment effort unset', () => {
    const config = buildTeamConfigFromRuntimeStatus({
      leadName: 'team-lead',
      members: [
        { name: 'team-lead', role: 'lead', cliTool: 'claude', model: 'opus' },
        { name: 'frontend-dev', role: 'member', cli_tool: 'codex', model: 'gpt-5.6-terra' },
      ],
    })

    expect(config.lead.taskEffort).toBeNull()
    expect(config.lead.taskEffortWhy).toBeNull()
    expect(config.agents[0].taskEffort).toBeNull()
    expect(config.agents[0].taskEffortWhy).toBeNull()
  })
})

describe('meshTabUtils runtime session identity', () => {
  // Regression: 9e15e4e keyed a node's workflow run tree on the member's Claude
  // session, but createLead/createAgent rebuild a node from a fixed field list
  // and dropped `sessionId`, so no runtime node ever asked for its runs.
  it('carries each member session id onto the lead and agent nodes', () => {
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
          sessionId: 'sess-lead',
        },
        {
          name: 'dev-1',
          role: 'member',
          cliTool: 'codex',
          model: 'gpt-5.6-terra',
          projectId: '/home/user/projects/taurhaus',
          sessionStatus: 'active',
          session_id: 'sess-dev',
        },
      ],
    })

    expect(config.lead.sessionId).toBe('sess-lead')
    expect(config.agents.map((agent) => agent.sessionId)).toEqual(['sess-dev'])
  })

  it('leaves an unattached member without a session', () => {
    const config = buildTeamConfigFromRuntimeStatus({
      leadName: 'team-lead',
      members: [{ name: 'team-lead', role: 'lead', cliTool: 'claude' }],
    })

    expect(config.lead.sessionId).toBeNull()
  })
})

describe('meshTabUtils launch account note', () => {
  it('carries the opaque wrapper note onto the runtime member node', () => {
    const config = buildTeamConfigFromRuntimeStatus({
      leadName: 'team-lead',
      members: [
        { name: 'team-lead', role: 'lead', cliTool: 'claude' },
        {
          name: 'dev-1',
          role: 'member',
          cliTool: 'codex',
          accountApplied: false,
          accountNote: 'opaque_base_command',
          accountNoteDetail: 'team-wrapper',
        },
      ],
    })

    expect(config.agents[0]).toEqual(expect.objectContaining({
      accountApplied: false,
      accountNote: 'opaque_base_command',
      accountNoteDetail: 'team-wrapper',
    }))
  })
})
