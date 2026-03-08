import { describe, expect, it } from 'vitest'

import {
  defaultLeadRoleId,
  normalizeRoleTemplate,
  presetDraftToTeamConfig,
} from './templateBrowserUtils.js'

describe('templateBrowserUtils normalizeRoleTemplate', () => {
  it('normalizes context-steering fields from camelCase', () => {
    expect(normalizeRoleTemplate({
      roleId: 'mesh-expert',
      name: 'Mesh Expert',
      focusArea: 'Mesh orchestration',
      contextSummary: 'Owns cross-project runtime awareness.',
      behaviorSummary: 'Advises on boundaries and avoids unrelated code edits.',
    })).toEqual(expect.objectContaining({
      roleId: 'mesh-expert',
      focusArea: 'Mesh orchestration',
      contextSummary: 'Owns cross-project runtime awareness.',
      behaviorSummary: 'Advises on boundaries and avoids unrelated code edits.',
    }))
  })

  it('normalizes context-steering fields from snake_case', () => {
    expect(normalizeRoleTemplate({
      role_id: 'mesh-expert',
      name: 'Mesh Expert',
      focus_area: 'Mesh orchestration',
      context_summary: 'Owns cross-project runtime awareness.',
      behavior_summary: 'Advises on boundaries and avoids unrelated code edits.',
    })).toEqual(expect.objectContaining({
      roleId: 'mesh-expert',
      focusArea: 'Mesh orchestration',
      contextSummary: 'Owns cross-project runtime awareness.',
      behaviorSummary: 'Advises on boundaries and avoids unrelated code edits.',
    }))
  })

  it('flattens backend behavioral contract objects into editor rule entries', () => {
    expect(normalizeRoleTemplate({
      roleId: 'mesh-expert',
      behavioralContract: {
        communication: ['Confirm scope first.'],
        execution: ['Stay within the assigned lane.'],
        escalation: ['Escalate cross-project changes immediately.'],
      },
    })).toEqual(expect.objectContaining({
      behavioralContract: [
        { rule: 'Confirm scope first.', enabled: true },
        { rule: 'Stay within the assigned lane.', enabled: true },
        { rule: 'Escalate cross-project changes immediately.', enabled: true },
      ],
    }))
  })

  it('selects the actual configured lead role id instead of hardcoding Claude', () => {
    expect(defaultLeadRoleId([
      {
        roleId: 'codex-orchestrator',
        kind: 'lead',
        cliTool: 'codex',
      },
      {
        roleId: 'codex-developer',
        kind: 'agent',
        cliTool: 'codex',
      },
    ])).toBe('codex-orchestrator')
  })

  it('builds preset team config from the selected lead role tool and model', () => {
    const teamConfig = presetDraftToTeamConfig(
      {
        presetId: 'codex-team',
        name: 'Codex Team',
        description: 'Codex-led preset',
        leadRoleId: 'codex-orchestrator',
        agentSlots: [],
      },
      [
        {
          roleId: 'codex-orchestrator',
          name: 'Codex Orchestrator',
          kind: 'lead',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
        },
      ]
    )

    expect(teamConfig.lead).toEqual(expect.objectContaining({
      roleId: 'codex-orchestrator',
      roleName: 'Codex Orchestrator',
      tool: 'codex',
      model: 'gpt-5.4 high',
    }))
  })
})
