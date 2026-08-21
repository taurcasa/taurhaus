import { describe, expect, it } from 'vitest'

import { normalizeRoleTemplateInput } from './templatePayloads.js'
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
        model: 'gpt-5.4 high',
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
        reasoningEffort: 'high',
        defaultNamePattern: 'dev-{n}',
      },
      handoffExpectations: ['Report tests and changed files'],
    }))
  })
})
