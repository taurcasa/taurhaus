import { describe, expect, it } from 'vitest'

import { BEHAVIORAL_CONTRACT_MODES, normalizeBehavioralContract } from './normalize.js'

describe('normalizeBehavioralContract', () => {
  it('OPTIONAL_OBJECT returns null for non-object input', () => {
    expect(normalizeBehavioralContract(undefined, { mode: BEHAVIORAL_CONTRACT_MODES.OPTIONAL_OBJECT })).toBeNull()
    expect(normalizeBehavioralContract([], { mode: BEHAVIORAL_CONTRACT_MODES.OPTIONAL_OBJECT })).toBeNull()
  })

  it('OPTIONAL_OBJECT normalizes missing sections to empty arrays', () => {
    expect(
      normalizeBehavioralContract(
        { communication: ['Acknowledge quickly'] },
        { mode: BEHAVIORAL_CONTRACT_MODES.OPTIONAL_OBJECT }
      )
    ).toEqual({
      communication: ['Acknowledge quickly'],
      execution: [],
      escalation: [],
    })
  })

  it('TEMPLATE_INPUT maps legacy array format to execution rules', () => {
    expect(
      normalizeBehavioralContract(
        [' Report progress ', { rule: 'Escalate blockers' }, { rule: 'Ignore me', enabled: false }],
        { mode: BEHAVIORAL_CONTRACT_MODES.TEMPLATE_INPUT }
      )
    ).toEqual({
      communication: [],
      execution: ['Report progress', 'Escalate blockers'],
      escalation: [],
    })
  })

  it('TEMPLATE_INPUT trims object section lists and drops empty values', () => {
    expect(
      normalizeBehavioralContract(
        {
          communication: [' Ack ', ''],
          execution: [' Ship features '],
          escalation: [' Raise blockers ', null],
        },
        { mode: BEHAVIORAL_CONTRACT_MODES.TEMPLATE_INPUT }
      )
    ).toEqual({
      communication: ['Ack'],
      execution: ['Ship features'],
      escalation: ['Raise blockers'],
    })
  })

  it('TEMPLATE_INPUT falls back to default execution rule when empty', () => {
    expect(normalizeBehavioralContract([], { mode: BEHAVIORAL_CONTRACT_MODES.TEMPLATE_INPUT })).toEqual({
      communication: [],
      execution: ['Execute assigned tasks and report status clearly.'],
      escalation: [],
    })
  })

  it('throws for unknown mode', () => {
    expect(() => normalizeBehavioralContract({}, { mode: 'unknown' })).toThrow(
      'Unsupported behavioral contract normalization mode'
    )
  })
})
