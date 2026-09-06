import { describe, expect, it } from 'vitest'
import { WAIT_MESH_PROPAGATION } from './timing.js'

// Regression: 430e09ee gave scanner-driven recovery waits 20/25s budgets,
// causing healthy but delayed offline/resume propagation to fail in the suite.
// Virtual time exercises the deadline without sleeping or running a scanner.
function observeAt(readyAt) {
  for (let elapsed = 0; elapsed <= WAIT_MESH_PROPAGATION.timeout; elapsed += WAIT_MESH_PROPAGATION.interval) {
    if (elapsed >= readyAt) return true
  }
  throw new Error('Coordination state did not propagate')
}

describe('Mesh propagation budget', () => {
  it('allows scanner and UI propagation beyond the old 25s cutoff', () => {
    expect(observeAt(28_000)).toBe(true)
  })

  it('still fails a state that never propagates within the bounded budget', () => {
    expect(() => observeAt(31_000)).toThrow('Coordination state did not propagate')
    expect(WAIT_MESH_PROPAGATION.interval).toBeGreaterThanOrEqual(100)
  })
})
