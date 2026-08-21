import { describe, expect, it } from 'vitest'

import {
  normalizeComposeTeamResult,
  normalizeRoleTemplateResponse,
  normalizeTeamPresetResponse,
} from './templateResponses.js'

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

describe('templateResponses legacy combined models', () => {
  // Regression: ff40911 and 5d2ce27 stored the effort inside the model string
  // ("gpt-5.4 high"); stores written before PR 5a still hold that form, and an
  // unsplit value reaches the launcher as a model id nobody knows. The response
  // normalizer splits it the same way `ModelSpec::parse_legacy` does.
  it('splits a legacy combined model in role defaults', () => {
    expect(normalizeRoleTemplateResponse({
      role_id: 'dev',
      name: 'Dev',
      defaults: { cli_tool: 'codex', model: 'gpt-5.4 high' },
    }).defaults).toEqual(expect.objectContaining({ model: 'gpt-5.4', reasoningEffort: 'high' }))
  })

  it('splits a legacy combined model in slot overrides', () => {
    expect(normalizeTeamPresetResponse({
      preset_id: 'duo',
      name: 'Duo',
      lead_role_id: 'lead',
      agent_slots: [{ role_id: 'dev', count: 1, overrides: { model: 'gpt-5.4-high' } }],
    }).agentSlots[0].overrides).toEqual(
      expect.objectContaining({ model: 'gpt-5.4', reasoningEffort: 'high' })
    )
  })

  it('keeps an explicit effort ahead of the one folded into the model string', () => {
    expect(normalizeRoleTemplateResponse({
      role_id: 'dev',
      name: 'Dev',
      defaults: { cli_tool: 'codex', model: 'gpt-5.4 high', reasoning_effort: 'xhigh' },
    }).defaults).toEqual(expect.objectContaining({ model: 'gpt-5.4', reasoningEffort: 'xhigh' }))
  })
})

describe('templateResponses role effort lifting', () => {
  // Regression: b345de1 (PR 5c) lifted `defaults.model` to the top level but left
  // the effort behind, so the runtime hot-add path read `role.reasoningEffort` as
  // undefined and replaced a role's declared `high` with the catalog default.
  it('lifts the role defaults effort next to the lifted model', () => {
    expect(normalizeRoleTemplateResponse({
      role_id: 'dev',
      name: 'Dev',
      defaults: { cli_tool: 'codex', model: 'gpt-5.4', reasoning_effort: 'high' },
    })).toEqual(expect.objectContaining({
      cliTool: 'codex',
      model: 'gpt-5.4',
      reasoningEffort: 'high',
    }))
  })
})

describe('templateResponses composed roster', () => {
  // Regression: b345de1 (PR 5c) copied `model` out of every `ResolvedMember` and
  // dropped `reasoningEffort` (composition.rs), so editing a composed preset
  // detached an effort-less roster and initialize silently lost the per-role
  // effort.
  it('keeps the resolved member effort in both spellings', () => {
    const result = normalizeComposeTeamResult({
      roster: [
        {
          name: 'team-lead',
          roleId: 'lead',
          roleKind: 'lead',
          cliTool: 'claude',
          model: 'opus',
          reasoningEffort: 'high',
        },
        {
          name: 'dev-1',
          role_id: 'dev',
          role_kind: 'agent',
          cli_tool: 'codex',
          model: 'gpt-5.4',
          reasoning_effort: 'xhigh',
        },
      ],
    })

    expect(result.roster.map((member) => [member.model, member.reasoningEffort])).toEqual([
      ['opus', 'high'],
      ['gpt-5.4', 'xhigh'],
    ])
  })

  it('splits a legacy combined roster model', () => {
    const result = normalizeComposeTeamResult({
      roster: [{ name: 'dev-1', cliTool: 'codex', model: 'gpt-5.4 high' }],
    })

    expect(result.roster[0]).toEqual(
      expect.objectContaining({ model: 'gpt-5.4', reasoningEffort: 'high' })
    )
  })
})
