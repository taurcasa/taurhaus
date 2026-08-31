import { describe, expect, it } from 'vitest'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  codexStateDatabaseDiagnostic,
  launchManagedMembersSerially,
  waitWithPaneTail,
} from './managedStageParallel.js'

const parallelSpecSource = readFileSync(
  join(process.cwd(), 'e2e/specs/managed-stage-parallel.js'),
  'utf8'
)

describe('parallel managed-stage spec contract', () => {
  // Regression: 8438876a serialized two 420-second member bring-up budgets
  // without expanding the 900-second before-hook cap that contains them.
  it('leaves enough hook time for serialized cold-start waits and setup overhead', () => {
    const beforeHookTimeout = parallelSpecSource.match(
      /before\(async function \(\) \{\s*this\.timeout\(([\d_]+)\)/
    )

    expect(beforeHookTimeout?.[1]).toBe('1_500_000')
  })
})

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

describe('waitWithPaneTail', () => {
  // Regression: 94fdab40 let a session-bind timeout hide the managed member's
  // pane, so the Codex startup failure that caused the timeout was absent.
  it('adds the failing member pane tail to a bind-wait error', async () => {
    const waiting = waitWithPaneTail({
      memberName: 'codex-beta',
      paneId: '%9',
      tailLines: 2,
      wait: async () => {
        throw new Error('bind deadline expired')
      },
      capturePane: async () => 'old output\nmigration failed\nlocal database appears to be damaged\n',
    })

    await expect(waiting).rejects.toThrow(
      'bind deadline expired\ncodex-beta pane %9 capture tail:\nmigration failed\nlocal database appears to be damaged'
    )
  })
})
