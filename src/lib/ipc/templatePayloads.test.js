import { describe, expect, it } from 'vitest'

import { normalizeRoleTemplateInput, normalizeTeamPresetInput } from './templatePayloads.js'
import { normalizeRoleTemplateResponse } from './templateResponses.js'

describe('templatePayloads normalizeRoleTemplateInput', () => {
  it('preserves explicit non-Claude lead tool and model', () => {
    expect(normalizeRoleTemplateInput({
      roleId: 'codex-orchestrator',
      name: 'Codex Orchestrator',
      kind: 'lead',
      tool: 'codex',
      model: 'gpt-5.4 high',
    })).toEqual(expect.objectContaining({
      roleId: 'codex-orchestrator',
      kind: 'lead',
      defaults: expect.objectContaining({
        cliTool: 'codex',
        model: 'gpt-5.4',
        reasoning_effort: 'high',
      }),
    }))
  })

  it('does not silently backfill Claude defaults for lead roles with missing tool metadata', () => {
    expect(normalizeRoleTemplateInput({
      roleId: 'lead-alpha',
      name: 'Lead Alpha',
      kind: 'lead',
    })).toEqual(expect.objectContaining({
      roleId: 'lead-alpha',
      kind: 'lead',
      defaults: expect.objectContaining({
        cliTool: '',
        model: '',
      }),
    }))
  })

  it('normalizes expanded role schema fields and preserves optional metadata', () => {
    expect(normalizeRoleTemplateInput({
      roleId: 'reviewer',
      name: 'Reviewer',
      communicationStyle: 'Brief and evidence-first.',
      qualityGates: ['Run scoped tests', 'Link proof'],
      definitionOfDone: ['Issue resolved', 'Ready for review'],
      phaseScope: ['review', 'verification'],
      mode: 'review',
      inheritsFrom: 'base-reviewer',
      requiredArtifacts: ['screenshot', 'diff summary'],
      runtimeCompactSummary: {
        rolePurpose: 'Keep reviews compact.',
        keepDoing: ['Find issues'],
        workflowSequence: ['Inspect changes'],
        avoid: ['Speculation'],
        escalateWhen: ['Missing context'],
      },
    })).toEqual(expect.objectContaining({
      communicationStyle: 'Brief and evidence-first.',
      qualityGates: ['Run scoped tests', 'Link proof'],
      definitionOfDone: ['Issue resolved', 'Ready for review'],
      phaseScope: ['review', 'verification'],
      mode: 'review',
      inheritsFrom: 'base-reviewer',
      requiredArtifacts: ['screenshot', 'diff summary'],
      runtimeCompactSummary: expect.objectContaining({
        rolePurpose: 'Keep reviews compact.',
      }),
    }))
  })

  // Regression: a79d392 split reasoning effort into its own backend field, but
  // the role editor mappers erased it and handoff expectations during save.
  it('round-trips reasoning effort and handoff expectations through role editor mappers', () => {
    const response = normalizeRoleTemplateResponse({
      role_id: 'developer-codex',
      name: 'Developer',
      kind: 'agent',
      defaults: {
        cli_tool: 'codex',
        model: 'gpt-5.4',
        reasoning_effort: 'high',
        default_name_pattern: 'dev-{n}',
      },
      handoff_expectations: ['Report tests and changed files'],
    })

    expect(normalizeRoleTemplateInput(response)).toEqual(expect.objectContaining({
      defaults: {
        cliTool: 'codex',
        model: 'gpt-5.4',
        reasoning_effort: 'high',
        defaultNamePattern: 'dev-{n}',
      },
      handoffExpectations: ['Report tests and changed files'],
    }))
  })

  it('round-trips carried capability policy without exposing editor behavior', () => {
    const capabilityPolicy = {
      modelSelection: 'adaptive',
      minimumCapability: 'strong',
      allowedModels: ['gpt-5.6-sol', 'opus'],
      effortBand: ['medium', 'high'],
    }
    const response = normalizeRoleTemplateResponse({
      role_id: 'developer-codex',
      name: 'Developer',
      capability_policy: {
        model_selection: 'adaptive',
        minimum_capability: 'strong',
        allowed_models: ['gpt-5.6-sol', 'opus'],
        effort_band: ['medium', 'high'],
      },
    })

    expect(response.capabilityPolicy).toEqual(capabilityPolicy)
    expect(response).not.toHaveProperty('capability_policy')
    expect(normalizeRoleTemplateInput(response).capabilityPolicy).toEqual(capabilityPolicy)
  })

  // Regression: commits ff40911 and 5d2ce27 kept model and effort in one string,
  // so the launcher stripped the suffix and ran the member at the user's global
  // effort. Saving a role must emit the split, canonical pair.
  it('splits a legacy combined model into model and reasoning_effort on the way out', () => {
    expect(normalizeRoleTemplateInput({
      roleId: 'developer-codex',
      name: 'Developer',
      kind: 'agent',
      tool: 'codex',
      model: 'gpt-5.4 high',
    })).toEqual(expect.objectContaining({
      defaults: expect.objectContaining({
        cliTool: 'codex',
        model: 'gpt-5.4',
        reasoning_effort: 'high',
      }),
    }))
  })

  it('leaves an empty model empty instead of inventing a catalog default', () => {
    expect(normalizeRoleTemplateInput({
      roleId: 'agent-alpha',
      name: 'Agent Alpha',
      kind: 'agent',
      tool: 'codex',
    })).toEqual(expect.objectContaining({
      defaults: expect.objectContaining({
        cliTool: 'codex',
        model: '',
        reasoning_effort: null,
      }),
    }))
  })
})

describe('templatePayloads normalizeTeamPresetInput', () => {
  it('splits legacy slot override models and keeps the explicit effort', () => {
    expect(normalizeTeamPresetInput({
      presetId: 'duo',
      name: 'Duo',
      leadRoleId: 'lead',
      agentSlots: [
        { roleId: 'dev', count: 1, overrides: { model: 'gpt-5.4 high', namePattern: 'dev-{n}' } },
        { roleId: 'qa', count: 1, overrides: { model: 'gpt-5.6-terra', reasoningEffort: 'low' } },
      ],
    }).agentSlots.map((slot) => slot.overrides)).toEqual([
      expect.objectContaining({ model: 'gpt-5.4', reasoning_effort: 'high', namePattern: 'dev-{n}' }),
      expect.objectContaining({ model: 'gpt-5.6-terra', reasoning_effort: 'low' }),
    ])
  })

  // Regression: b345de1 (PR 5c). The advanced preset editor edits the lead's model
  // and effort, so the request has to carry the lead pin in the canonical wire
  // spelling instead of dropping it on the way to the backend.
  it('emits the lead override with the canonical effort spelling', () => {
    expect(normalizeTeamPresetInput({
      presetId: 'duo',
      name: 'Duo',
      leadRoleId: 'lead',
      leadOverrides: { model: 'gpt-5.4 high' },
      agentSlots: [{ roleId: 'dev', count: 1 }],
    }).leadOverrides).toEqual(expect.objectContaining({
      model: 'gpt-5.4',
      reasoning_effort: 'high',
    }))
  })

  it('keeps an absent lead override null', () => {
    expect(normalizeTeamPresetInput({
      presetId: 'solo',
      name: 'Solo',
      leadRoleId: 'lead',
      agentSlots: [{ roleId: 'dev', count: 1 }],
    }).leadOverrides).toBeNull()
  })

  it('keeps null overrides null', () => {
    expect(normalizeTeamPresetInput({
      presetId: 'solo',
      name: 'Solo',
      leadRoleId: 'lead',
      agentSlots: [{ roleId: 'dev', count: 1 }],
    }).agentSlots[0].overrides).toBeNull()
  })
})

// Regression: the lossless normalizers spread their input, so a snake_case
// payload carried `behavioral_contract` / `role_id` / `project_binding` /
// `project_id` through next to the camelCase keys they write — and serde
// rejects a payload naming an aliased field twice ("duplicate field"). Found by
// the Opus review of the lossless-normalizer change (2026-08-30).
describe('templatePayloads keeps one spelling per aliased field', () => {
  it('strips behavioral_contract after reading it', () => {
    const normalized = normalizeRoleTemplateInput({
      id: 'role-1',
      name: 'Role',
      instructions: 'do things',
      behavioral_contract: { mode: 'template', summary: 'from snake' },
    })
    expect(normalized).not.toHaveProperty('behavioral_contract')
    expect(normalized.behavioralContract).toBeTruthy()
  })

  it('strips role_id, project_binding and project_id from every slot', () => {
    const normalized = normalizeTeamPresetInput({
      id: 'preset-1',
      name: 'Preset',
      agent_slots: [{ role_id: 'dev', project_binding: 'lead_project', project_id: 'p-1', count: 2 }],
    })
    const [slot] = normalized.agentSlots
    expect(slot.roleId).toBe('dev')
    expect(slot.projectBinding).toBe('lead_project')
    expect(slot.projectId).toBe('p-1')
    expect(slot).not.toHaveProperty('role_id')
    expect(slot).not.toHaveProperty('project_binding')
    expect(slot).not.toHaveProperty('project_id')
  })
})
