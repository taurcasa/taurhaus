import { describe, expect, it } from 'vitest'

import { normalizeRoleTemplate } from './templateBrowserUtils.js'

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
})
