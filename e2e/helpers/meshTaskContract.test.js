import { describe, expect, it } from 'vitest'

import {
  effortDeliveryVerdict,
  extractJsonBlock,
  findBlockedMessage,
  findResultMessage,
  parseResultMessage,
  resultContractViolations,
} from './meshTaskContract.js'

describe('extractJsonBlock', () => {
  it('reads a fenced json block', () => {
    const parsed = extractJsonBlock('prose\n```json\n{"commit": "abc"}\n```\nmore')
    expect(parsed).toEqual({ commit: 'abc' })
  })

  it('reads a fenced block with no language tag', () => {
    expect(extractJsonBlock('```\n{"files": ["a"]}\n```')).toEqual({ files: ['a'] })
  })

  it('reads a bare object that runs past other braces', () => {
    const parsed = extractJsonBlock('RESULT #4\n{"commit":"abc","validation":{"command":"bun test","passed":true}}\ntrailing')
    expect(parsed).toEqual({ commit: 'abc', validation: { command: 'bun test', passed: true } })
  })

  it('ignores a brace inside a string', () => {
    expect(extractJsonBlock('{"note":"a } brace","ok":true}')).toEqual({ note: 'a } brace', ok: true })
  })

  it('returns null when there is no object at all', () => {
    expect(extractJsonBlock('RESULT #4 done')).toBeNull()
  })

  it('returns null for an unterminated object', () => {
    expect(extractJsonBlock('{"commit": "abc"')).toBeNull()
  })
})

describe('parseResultMessage', () => {
  const payload = '{"commit":"deadbeef","files":["src/lib/greet.js"],"validation":"bun test passed"}'

  it('accepts the hash-prefixed task id the mesh notice uses', () => {
    const parsed = parseResultMessage(`RESULT #7\n${payload}`, '7')
    expect(parsed.ok).toBe(true)
    expect(parsed.payload.commit).toBe('deadbeef')
  })

  it('accepts a bare task id', () => {
    expect(parseResultMessage(`RESULT 7 ${payload}`, '7').ok).toBe(true)
  })

  it('rejects a result for another task', () => {
    const parsed = parseResultMessage(`RESULT #8\n${payload}`, '7')
    expect(parsed.ok).toBe(false)
    expect(parsed.reason).toMatch(/task/)
  })

  it('rejects a message that does not open with RESULT', () => {
    const parsed = parseResultMessage(`Working on it. RESULT #7 ${payload}`, '7')
    expect(parsed.ok).toBe(false)
    expect(parsed.reason).toMatch(/RESULT/)
  })

  it('rejects a RESULT with no JSON block', () => {
    const parsed = parseResultMessage('RESULT #7 all done', '7')
    expect(parsed.ok).toBe(false)
    expect(parsed.reason).toMatch(/JSON/)
  })
})

describe('findResultMessage', () => {
  const messages = [
    { from: 'codex-stage', text: 'starting now' },
    { from: 'codex-stage', text: 'RESULT #7\n{"commit":"abc"}', timestamp: '2026-08-29T10:00:00.000Z' },
  ]

  it('returns the message and its parsed payload', () => {
    const found = findResultMessage(messages, '7')
    expect(found.message.timestamp).toBe('2026-08-29T10:00:00.000Z')
    expect(found.payload).toEqual({ commit: 'abc' })
  })

  it('returns null while no message qualifies', () => {
    expect(findResultMessage(messages, '8')).toBeNull()
    expect(findResultMessage([], '7')).toBeNull()
  })
})

describe('findBlockedMessage', () => {
  it('reports a blocker so a lane fails fast instead of waiting out its budget', () => {
    const blocked = findBlockedMessage(
      [{ from: 'codex-stage', text: 'BLOCKED #7 bun is not installed' }],
      '7'
    )
    expect(blocked.reason).toBe('bun is not installed')
  })

  it('ignores a blocker for another task', () => {
    expect(findBlockedMessage([{ text: 'BLOCKED #9 nope' }], '7')).toBeNull()
  })
})

describe('resultContractViolations', () => {
  it('accepts the payload the completion signal asks for', () => {
    expect(
      resultContractViolations({
        commit: 'f9370e722d8991eb9e7690c35daaba46e376e637',
        files: ['src/lib/greet.js', 'src/lib/greet.test.js'],
        validation: 'bun test passed',
      })
    ).toEqual([])
  })

  // Regression: 249227f took any JSON object as a result, so a member could
  // answer `{"noop":true}` for a task whose completion signal names commit,
  // files and validation and the lane would still call the stage delivered.
  it('names every field the completion signal asked for and did not get', () => {
    const violations = resultContractViolations({ noop: true })
    expect(violations).toHaveLength(3)
    expect(violations.join(' ')).toMatch(/commit/)
    expect(violations.join(' ')).toMatch(/files/)
    expect(violations.join(' ')).toMatch(/validation/)
  })

  it('rejects a commit that is a symbolic name rather than a sha', () => {
    const violations = resultContractViolations({ commit: 'HEAD', files: ['a.js'], validation: 'ok' })
    expect(violations).toHaveLength(1)
    expect(violations[0]).toMatch(/commit/)
  })

  it('rejects an empty file list and a blank validation', () => {
    const violations = resultContractViolations({ commit: 'deadbeef', files: [], validation: '  ' })
    expect(violations).toHaveLength(2)
  })

  it('rejects anything that is not an object', () => {
    expect(resultContractViolations(null)).toHaveLength(1)
  })
})

describe('effortDeliveryVerdict', () => {
  // Regression: 5e1d0ae asserted only that delivery landed at or after
  // `effort.resume.started`, which an expired effort wait satisfies too: mesh
  // gives up mid-relaunch, delivers to a member still running at `low`, and the
  // resume that lands afterwards makes every later reading look correct.
  it('calls a delivery that arrived before the level was in force what it is', () => {
    expect(
      effortDeliveryVerdict({ appliedEffort: 'low', requiredEffort: 'medium', deliveredAt: '2026-08-29T15:44:57Z' })
    ).toBe('delivered-early')
  })

  it('is holding while the level is not in force and nothing was delivered', () => {
    expect(effortDeliveryVerdict({ appliedEffort: 'low', requiredEffort: 'medium', deliveredAt: null })).toBe('holding')
    expect(effortDeliveryVerdict({ appliedEffort: null, requiredEffort: 'medium' })).toBe('holding')
  })

  it('is in force once the runtime record reports the level, delivered or not', () => {
    expect(effortDeliveryVerdict({ appliedEffort: 'medium', requiredEffort: 'medium', deliveredAt: null })).toBe('in-force')
    expect(
      effortDeliveryVerdict({ appliedEffort: ' Medium ', requiredEffort: 'medium', deliveredAt: '2026-08-29T15:44:57Z' })
    ).toBe('in-force')
  })
})
