import { describe, expect, it } from 'vitest'

import { launchManagedMembersSerially } from './managedStageParallel.js'

describe('launchManagedMembersSerially', () => {
  // Regression: 94fdab40 initialized both managed Codex members together, so
  // Codex 0.151 raced its fresh-home state migration and one launch died.
  it('binds each cold-started member before launching the next one', async () => {
    const events = []
    const members = [{ owner: 'codex-alpha' }, { owner: 'codex-beta' }]

    await launchManagedMembersSerially({
      members,
      initialize: async (member) => events.push(`initialize:${member.owner}`),
      add: async (member) => events.push(`add:${member.owner}`),
      waitForBinding: async (member) => events.push(`bind:${member.owner}`),
    })

    expect(events).toEqual([
      'initialize:codex-alpha',
      'bind:codex-alpha',
      'add:codex-beta',
      'bind:codex-beta',
    ])
  })
})
