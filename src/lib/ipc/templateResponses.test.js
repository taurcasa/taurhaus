import { describe, expect, it } from 'vitest'

import { normalizeRoleTemplateResponse } from './templateResponses.js'

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
