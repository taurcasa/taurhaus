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
  it('derives the hook budget from the waits it must contain', () => {
    expect(parallelSpecSource).toMatch(
      /BEFORE_HOOK_TIMEOUT_MS =\s*2 \* \(TEAM_READY_TIMEOUT_MS \+ SESSION_BIND_TIMEOUT_MS\)/
    )
    expect(parallelSpecSource).toMatch(
      /before\(async function \(\) \{\s*this\.timeout\(BEFORE_HOOK_TIMEOUT_MS\)/
    )
  })

  // Regression: a failed AddAgentReport still carries the degradation notes
  // that explain it; logging them only after the throw discarded them on the
  // one path they matter most.
  it('logs hot-add warnings before rejecting a failed add report', () => {
    const addCallback = parallelSpecSource.slice(
      parallelSpecSource.indexOf('add: async (stage) => {'),
      parallelSpecSource.indexOf('waitForBinding: waitForMemberBinding')
    )
    const failureCheck = addCallback.indexOf('if (report?.failedStep)')
    const warningCheck = addCallback.indexOf('if (report?.warnings?.length)')

    expect(warningCheck).toBeGreaterThanOrEqual(0)
    expect(failureCheck).toBeGreaterThan(warningCheck)
    expect(addCallback.indexOf('hot-add warnings:', warningCheck)).toBeGreaterThan(warningCheck)
  })
})

describe('member bind nudge', () => {
  // Regression: measured attempt 2 found the READY prompt and mesh's own
  // onboarding notice parked unsubmitted in the Codex composer — a one-shot
  // Enter is lost during cold start. The bind wait must keep nudging.
  it('retries the submit inside the bind wait instead of sending once', () => {
    const binding = parallelSpecSource.slice(
      parallelSpecSource.indexOf('async function waitForMemberBinding'),
      parallelSpecSource.indexOf('async function initializeParallelTeam')
    )
    expect(binding).toMatch(/lastNudgeAt/)
    expect(binding).toMatch(/reprompted/)
    expect((binding.match(/send-keys/g) ?? []).length).toBeGreaterThanOrEqual(4)
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
  // Regression: 707dc749 hardcoded state_5.sqlite, so a Codex schema-generation
  // bump made the pre-launch diagnostic silently report no state database.
  it('reports every current state database generation without creating one', () => {
    const home = mkdtempSync(join(tmpdir(), 'taurhaus-codex-state-'))
    try {
      expect(codexStateDatabaseDiagnostic(home)).toEqual({
        directory: home,
        filenames: [],
        exists: false,
      })
      expect(existsSync(join(home, 'state_5.sqlite'))).toBe(false)

      writeFileSync(join(home, 'state_5.sqlite'), '')
      writeFileSync(join(home, 'state_6.sqlite'), '')
      writeFileSync(join(home, 'state_6.sqlite-wal'), '')
      expect(codexStateDatabaseDiagnostic(home)).toEqual({
        directory: home,
        filenames: ['state_5.sqlite', 'state_6.sqlite'],
        exists: true,
      })
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

  // Regression: f6dfee6f let a pane-capture failure replace the original bind
  // timeout, hiding which managed member failed to bind its session.
  it('preserves the bind error when pane capture also fails', async () => {
    const bindError = new Error('codex-beta did not bind its scratch-home session')
    const waiting = waitWithPaneTail({
      memberName: 'codex-beta',
      paneId: '%9',
      wait: async () => {
        throw bindError
      },
      capturePane: async () => {
        throw new Error('Webdriver session is gone')
      },
    })

    await expect(waiting).rejects.toMatchObject({
      message:
        'codex-beta did not bind its scratch-home session\n' +
        'codex-beta pane %9 capture tail:\n(pane capture empty)',
      cause: bindError,
    })
  })
})
