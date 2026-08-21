import { describe, expect, it } from 'vitest'

import { normalizeRoleTemplateResponse, normalizeTeamPresetResponse } from './templateResponses.js'

describe('templateResponses normalizeRoleTemplateResponse', () => {
  it('normalizes expanded role schema fields from snake_case responses', () => {
    expect(normalizeRoleTemplateResponse({
      role_id: 'quick-dev',
      name: 'Quick Dev',
      kind: 'agent',
      communication_style: 'Minimal and concrete.',
      quality_gates: ['just check-quick passes'],
      definition_of_done: ['Ready for review'],
      phase_scope: ['implementation', 'verification'],
      mode: 'implementation',
      inherits_from: 'base-dev',
      required_artifacts: ['screenshot'],
      runtime_compact_summary: {
        role_purpose: 'Stay compact.',
      },
    })).toEqual(expect.objectContaining({
      roleId: 'quick-dev',
      communicationStyle: 'Minimal and concrete.',
      qualityGates: ['just check-quick passes'],
      definitionOfDone: ['Ready for review'],
      phaseScope: ['implementation', 'verification'],
      mode: 'implementation',
      inheritsFrom: 'base-dev',
      requiredArtifacts: ['screenshot'],
      runtimeCompactSummary: expect.objectContaining({
        role_purpose: 'Stay compact.',
      }),
    }))
  })
})

describe('templateResponses reasoning effort', () => {
  // Regression: commits ff40911 and 5d2ce27 folded the effort into the model
  // string and the launcher stripped it, so a role asking for `high` ran at the
  // user's global `xhigh`. Both spellings of the split field must survive.
  it('reads role defaults effort in both spellings', () => {
    expect(normalizeRoleTemplateResponse({
      role_id: 'dev',
      name: 'Dev',
      defaults: { cli_tool: 'codex', model: 'gpt-5.4', reasoning_effort: 'high' },
    }).defaults).toEqual(expect.objectContaining({ model: 'gpt-5.4', reasoningEffort: 'high' }))

    expect(normalizeRoleTemplateResponse({
      roleId: 'dev',
      name: 'Dev',
      defaults: { cliTool: 'codex', model: 'gpt-5.4', reasoningEffort: 'high' },
    }).defaults).toEqual(expect.objectContaining({ model: 'gpt-5.4', reasoningEffort: 'high' }))
  })

  it('reads slot override effort in both spellings', () => {
    expect(normalizeTeamPresetResponse({
      preset_id: 'duo',
      name: 'Duo',
      lead_role_id: 'lead',
      agent_slots: [
        { role_id: 'dev', count: 1, overrides: { model: 'gpt-5.4', reasoning_effort: 'high' } },
        { role_id: 'qa', count: 1, overrides: { model: 'gpt-5.6-terra', reasoningEffort: 'low' } },
      ],
    }).agentSlots.map((slot) => slot.overrides.reasoningEffort)).toEqual(['high', 'low'])
  })
})
