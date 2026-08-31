import { describe, expect, it } from 'vitest'
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { codexStateDatabaseDiagnostic, launchManagedMembersSerially } from './managedStageParallel.js'

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

describe('codexStateDatabaseDiagnostic', () => {
  // Regression: 94fdab40 launched the paid members without recording whether
  // Codex's fresh-home state migration had already created its database.
  it('reports state_5.sqlite presence without creating it', () => {
    const home = mkdtempSync(join(tmpdir(), 'taurhaus-codex-state-'))
    try {
      expect(codexStateDatabaseDiagnostic(home)).toEqual({
        path: join(home, 'state_5.sqlite'),
        exists: false,
      })
      expect(existsSync(join(home, 'state_5.sqlite'))).toBe(false)

      writeFileSync(join(home, 'state_5.sqlite'), '')
      expect(codexStateDatabaseDiagnostic(home).exists).toBe(true)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })
})
