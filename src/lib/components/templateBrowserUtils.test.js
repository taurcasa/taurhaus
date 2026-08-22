import { describe, expect, it } from 'vitest'

import {
  defaultLeadRoleId,
  normalizePresetDraft,
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

    // The legacy combined spelling is split here, the same way the roster splits
    // it downstream, so both directions of the editor read one role default.
    expect(teamConfig.lead).toEqual(expect.objectContaining({
      roleId: 'codex-orchestrator',
      roleName: 'Codex Orchestrator',
      tool: 'codex',
      model: 'gpt-5.4',
      reasoningEffort: 'high',
    }))
  })
})

describe('templateBrowserUtils preset slot overrides', () => {
  const roleTemplates = [
    {
      roleId: 'codex-orchestrator',
      name: 'Codex Orchestrator',
      kind: 'lead',
      cliTool: 'codex',
      model: 'gpt-5.4',
      reasoningEffort: 'medium',
    },
    {
      roleId: 'codex-developer',
      name: 'Codex Developer',
      kind: 'agent',
      cliTool: 'codex',
      model: 'gpt-5.4',
      reasoningEffort: 'medium',
    },
  ]

  // Regression: b345de1 (PR 5c) gave the preset editor a ModelSelect but
  // `normalizePresetDraft` rebuilt every slot as {roleId, count, projectBinding,
  // projectId}, dropping `overrides`. Opening a preset that pinned a model/effort
  // and pressing Save wrote the pins away and reverted the roster to the plain
  // role defaults.
  it('keeps canonical slot overrides on the normalized draft', () => {
    const draft = normalizePresetDraft(
      {
        presetId: 'override-team',
        name: 'Override Team',
        leadRoleId: 'codex-orchestrator',
        agent_slots: [
          {
            role_id: 'codex-developer',
            count: 2,
            overrides: {
              model: 'gpt-5.6-terra',
              reasoning_effort: 'xhigh',
              name_pattern: 'dev-{n}',
            },
          },
        ],
      },
      roleTemplates
    )

    expect(draft.agentSlots[0].overrides).toEqual(expect.objectContaining({
      model: 'gpt-5.6-terra',
      reasoningEffort: 'xhigh',
      name_pattern: 'dev-{n}',
    }))
  })

  it('splits a legacy combined override model into model and effort', () => {
    const draft = normalizePresetDraft(
      {
        leadRoleId: 'codex-orchestrator',
        agentSlots: [{ roleId: 'codex-developer', count: 1, overrides: { model: 'gpt-5.4 high' } }],
      },
      roleTemplates
    )

    expect(draft.agentSlots[0].overrides).toEqual(expect.objectContaining({
      model: 'gpt-5.4',
      reasoningEffort: 'high',
    }))
  })

  it('drops an override object that pins nothing', () => {
    const draft = normalizePresetDraft(
      {
        leadRoleId: 'codex-orchestrator',
        agentSlots: [{ roleId: 'codex-developer', count: 1, overrides: { model: null, namePattern: null } }],
      },
      roleTemplates
    )

    expect(draft.agentSlots[0].overrides).toBeNull()
  })

  // Regression: b345de1 (PR 5c) built the customizer team config from the role
  // defaults only, so a preset slot that overrode the model/effort showed the
  // role's model in the editor instead of the one the preset actually pins.
  it('applies slot overrides over the role defaults in the customizer config', () => {
    const teamConfig = presetDraftToTeamConfig(
      {
        presetId: 'override-team',
        name: 'Override Team',
        leadRoleId: 'codex-orchestrator',
        agentSlots: [
          {
            roleId: 'codex-developer',
            count: 2,
            overrides: { model: 'gpt-5.6-terra', reasoningEffort: 'xhigh' },
          },
        ],
      },
      roleTemplates
    )

    expect(teamConfig.agents).toEqual([
      expect.objectContaining({
        roleId: 'codex-developer',
        model: 'gpt-5.6-terra',
        reasoningEffort: 'xhigh',
        slotIndex: 0,
      }),
      expect.objectContaining({
        roleId: 'codex-developer',
        model: 'gpt-5.6-terra',
        reasoningEffort: 'xhigh',
        slotIndex: 0,
      }),
    ])
  })

  it('keeps the role defaults for a slot without overrides', () => {
    const teamConfig = presetDraftToTeamConfig(
      {
        leadRoleId: 'codex-orchestrator',
        agentSlots: [{ roleId: 'codex-developer', count: 1 }],
      },
      roleTemplates
    )

    expect(teamConfig.agents[0]).toEqual(expect.objectContaining({
      model: 'gpt-5.4',
      reasoningEffort: 'medium',
      slotIndex: 0,
    }))
  })
})
