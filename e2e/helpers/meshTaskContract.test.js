import { describe, expect, it } from 'vitest'

import {
  activeDeadlineHeartbeatPlan,
  activeDeadlinePassEvidence,
  effortDeliveryVerdict,
  effortWaitBoundMs,
  expiredEffortWaitProblem,
  extractJsonBlock,
  findBlockedMessage,
  findResultMessage,
  parseResultMessage,
  resultContractViolations,
  stagePollVerdict,
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

describe('effortWaitBoundMs', () => {
  it('is the three minutes mesh defaults to', () => {
    expect(effortWaitBoundMs({})).toBe(180_000)
  })

  it('follows the override mesh reads', () => {
    expect(effortWaitBoundMs({ MESH_EFFORT_WAIT_SECS: '45' })).toBe(45_000)
  })

  it('falls back to the default for a value mesh would not parse', () => {
    expect(effortWaitBoundMs({ MESH_EFFORT_WAIT_SECS: 'nonsense' })).toBe(180_000)
    expect(effortWaitBoundMs({ MESH_EFFORT_WAIT_SECS: '  ' })).toBe(180_000)
  })
})

describe('expiredEffortWaitProblem', () => {
  // Regression: 5e1d0ae accepted any delivery at or after
  // `effort.resume.started`. mesh delivers a held notice for exactly two
  // reasons — the member reached the level, or the wait ran out — and the
  // second is a pure function of how long ago the assignment was made, so the
  // records say which one happened.
  it('names an expiry when delivery came no sooner than the wait bound', () => {
    const problem = expiredEffortWaitProblem({ assignedAtMs: 1_000, deliveredAtMs: 181_000, boundMs: 180_000 })
    expect(problem).toMatch(/expired/)
  })

  it('accepts a delivery inside the bound, which no expiry can produce', () => {
    expect(expiredEffortWaitProblem({ assignedAtMs: 1_000, deliveredAtMs: 2_230, boundMs: 180_000 })).toBe('')
  })

  it('reports unreadable timestamps rather than calling them a pass', () => {
    expect(expiredEffortWaitProblem({ assignedAtMs: NaN, deliveredAtMs: 2_230, boundMs: 180_000 })).toMatch(/timestamp/)
    expect(expiredEffortWaitProblem({ assignedAtMs: 1_000, deliveredAtMs: null, boundMs: 180_000 })).toMatch(/timestamp/)
  })
})

describe('stagePollVerdict', () => {
  it('returns the managed stage timeout shape when the task record becomes stale', () => {
    expect(stagePollVerdict({ id: '42', status: 'stale' })).toEqual({ status: 'timeout' })
  })

  it('keeps polling every non-stale task record', () => {
    expect(stagePollVerdict({ id: '42', status: 'in_progress' })).toBeNull()
    expect(stagePollVerdict(null)).toBeNull()
  })
})

describe('activeDeadlineHeartbeatPlan', () => {
  // Regression: e1c38eef emitted about 10 bytes/s during the long command, so
  // Codex never cleared the production 1 kB/s recent-IO activity threshold.
  it('sustains enough command output for the activity pipeline to observe', () => {
    const plan = activeDeadlineHeartbeatPlan({
      deadlineMinutes: 3,
      passCadenceMs: 30_000,
      intervalMs: 500,
      payloadBytes: 4_096,
    })

    expect(plan.outputBytesPerSecond).toBeGreaterThanOrEqual(1_000)
    expect(plan.command).toContain('"x".repeat(4096)')
    expect(plan.command).toContain('Bun.sleep(500)')
  })

  // Regression: e1c38eef spent 96 seconds of a 120-second deadline in the
  // heartbeat, leaving only 24 seconds for two Codex command turns before the
  // unsuppressed stale action could fire.
  it('covers half-time plus one pass while reserving a full minute to complete', () => {
    const plan = activeDeadlineHeartbeatPlan({
      deadlineMinutes: 3,
      passCadenceMs: 30_000,
      intervalMs: 500,
      payloadBytes: 4_096,
    })

    expect(plan.neededActiveMs).toBe(120_000)
    expect(plan.iterations).toBe(240)
    expect(plan.durationMs).toBe(120_000)
    expect(plan.completionSlackMs).toBe(60_000)
  })
})

describe('activeDeadlinePassEvidence', () => {
  it('joins an eligible self-heal pass to the fresh active record it evaluated', () => {
    expect(
      activeDeadlinePassEvidence({
        assignedAt: '2026-08-31T10:00:00.000Z',
        deadlineMinutes: 2,
        activitySnapshots: [
          { observed_at: '2026-08-31T10:01:05.000Z', activity_confidence: 'likely_working' },
        ],
        passEvents: [{ ts: '2026-08-31T10:01:10.000Z' }],
        deadlineEvents: [],
      })
    ).toEqual({
      halfDueAt: '2026-08-31T10:01:00.000Z',
      passAt: '2026-08-31T10:01:10.000Z',
      activityObservedAt: '2026-08-31T10:01:05.000Z',
      activityConfidence: 'likely_working',
    })
  })

  it('rejects a pre-half pass, idle evidence, or a pass that already acted', () => {
    const base = {
      assignedAt: '2026-08-31T10:00:00.000Z',
      deadlineMinutes: 2,
      activitySnapshots: [{ observed_at: '2026-08-31T10:00:55.000Z', activity_confidence: 'active' }],
      passEvents: [{ ts: '2026-08-31T10:00:59.000Z' }],
      deadlineEvents: [],
    }
    expect(activeDeadlinePassEvidence(base)).toBeNull()
    expect(
      activeDeadlinePassEvidence({
        ...base,
        activitySnapshots: [{ observed_at: '2026-08-31T10:01:05.000Z', activity_confidence: 'idle' }],
        passEvents: [{ ts: '2026-08-31T10:01:10.000Z' }],
      })
    ).toBeNull()
    expect(
      activeDeadlinePassEvidence({
        ...base,
        activitySnapshots: [{ observed_at: '2026-08-31T10:01:05.000Z', activity_confidence: 'active' }],
        passEvents: [{ ts: '2026-08-31T10:01:10.000Z' }],
        deadlineEvents: [{ event: 'deadline.nudge.sent' }],
      })
    ).toBeNull()
  })

  // Regression: e1c38eef filtered out non-active samples before joining the
  // pass, so an older active record could certify a pass that actually read a
  // newer idle record.
  it('rejects an older active sample when a newer idle sample preceded the pass', () => {
    expect(
      activeDeadlinePassEvidence({
        assignedAt: '2026-08-31T10:00:00.000Z',
        deadlineMinutes: 2,
        activitySnapshots: [
          { observed_at: '2026-08-31T10:01:05.000Z', activity_confidence: 'active' },
          { observed_at: '2026-08-31T10:01:08.000Z', activity_confidence: 'idle' },
        ],
        passEvents: [{ ts: '2026-08-31T10:01:10.000Z' }],
        deadlineEvents: [],
      })
    ).toBeNull()
  })
})
