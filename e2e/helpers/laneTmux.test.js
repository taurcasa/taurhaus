import { describe, expect, it } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import {
  applyTmuxIsolation,
  callableExportNames,
  findTmuxDrivingSpecs,
  isolatedTmuxTmpdir,
  parseProcEnviron,
  tmuxIsolationCoverageProblems,
  tmuxIsolationProblem,
} from './laneTmux.js'

const root = '/tmp/taurhaus-e2e-1234-abcd'

describe('isolatedTmuxTmpdir', () => {
  it('is a directory inside the session temp root', () => {
    expect(isolatedTmuxTmpdir(root)).toBe(join(root, 'tmux'))
  })
})

describe('tmuxIsolationProblem', () => {
  // Regression: 5e1d0ae ran the paid lane against the operator's own tmux
  // server, where `set-environment` on the shared `taurhaus` session hands
  // temporary roots to any pane the operator opens next. The lane has to know
  // it is on a server of its own before it creates anything there.
  it('accepts an environment pointed at the socket directory the lane owns', () => {
    expect(tmuxIsolationProblem({ TMUX_TMPDIR: isolatedTmuxTmpdir(root) }, root)).toBe('')
  })

  it('rejects an unset TMUX_TMPDIR', () => {
    expect(tmuxIsolationProblem({}, root)).toMatch(/TMUX_TMPDIR/)
  })

  it('rejects a socket directory outside the session temp root', () => {
    expect(tmuxIsolationProblem({ TMUX_TMPDIR: '/tmp/tmux-1000' }, root)).toMatch(/TMUX_TMPDIR/)
  })

  it('rejects an inherited TMUX, which outranks TMUX_TMPDIR for every client', () => {
    const problem = tmuxIsolationProblem(
      { TMUX_TMPDIR: isolatedTmuxTmpdir(root), TMUX: '/tmp/tmux-1000/default,407334,0' },
      root
    )
    expect(problem).toMatch(/TMUX/)
  })

  it('rejects an empty session temp root rather than calling it isolated', () => {
    expect(tmuxIsolationProblem({ TMUX_TMPDIR: '/tmp/whatever/tmux' }, '')).toMatch(/temp root/)
  })
})

describe('parseProcEnviron', () => {
  it('reads the NUL-separated pairs Linux writes', () => {
    const parsed = parseProcEnviron('TMUX_TMPDIR=/tmp/a\0PATH=/usr/bin\0')
    expect(parsed.TMUX_TMPDIR).toBe('/tmp/a')
    expect(parsed.PATH).toBe('/usr/bin')
  })

  it('keeps a value that contains an equals sign', () => {
    expect(parseProcEnviron('TMUX=/tmp/tmux-1000/default,407334,0\0A=b=c\0').A).toBe('b=c')
  })

  it('is empty for empty input', () => {
    expect(parseProcEnviron('')).toEqual({})
  })
})

describe('tmux-driving spec coverage', () => {
  // Regression: commit e654ef8a derived only `export function` declarations,
  // so async functions and callable const exports could bypass the guard.
  it('derives helper names from every callable runtime export', () => {
    expect(callableExportNames({
      syncHelper() {},
      asyncHelper: async () => {},
      constHelper: () => {},
      description: 'not callable',
    })).toEqual(['asyncHelper', 'constHelper', 'syncHelper'])
  })

  // Regression: commit 3c781765 isolated one paid spec through a module-local
  // allowlist while every other tmux-driving spec stayed on the operator server.
  it('derives every tmux-driving spec from its source calls', () => {
    const specsDir = resolve(import.meta.dirname, '..', 'specs')

    expect(findTmuxDrivingSpecs(specsDir)).toEqual([
      'command-center-real-actions.js',
      'compaction-codex-hooks.js',
      'managed-stage-codex.js',
      'managed-stage-deadline.js',
      'managed-stage-parallel.js',
      'mesh-recovery.js',
      'mesh-screenshots.js',
      'mesh-workflow.js',
      'regressions.js',
      'session-management.js',
      'template-crud-ui.js',
      'template-screenshots.js',
    ])
  })

  it('requires every tmux-driving spec to assert isolation before its first tmux call', () => {
    const specsDir = resolve(import.meta.dirname, '..', 'specs')
    expect(tmuxIsolationCoverageProblems(specsDir)).toEqual([])
  })

  // Regression: commit 7908cbf4 compared only raw source offsets, so an
  // assertion inside a tmux wrapper incorrectly covered an earlier runtime
  // snapshot made directly from a before hook.
  it('requires snapshot calls in a before hook to be guarded in that hook', () => {
    const specsDir = mkdtempSync(join(tmpdir(), 'taurhaus-tmux-coverage-'))
    try {
      writeFileSync(
        join(specsDir, 'mesh-recovery.js'),
        `function tmux(args) {
  assertTmuxIsolation(process.env)
  return execFileSync('tmux', args)
}

describe('fixture', () => {
  before(() => {
    snapshotTmuxPanes()
  })
})
`
      )

      expect(tmuxIsolationCoverageProblems(specsDir)).toEqual([
        'mesh-recovery.js: call assertTmuxIsolation in the before hook before snapshotTmuxPanes',
      ])
    } finally {
      rmSync(specsDir, { recursive: true, force: true })
    }
  })

  it('flags a snapshot call that sits outside any before hook', () => {
    const specsDir = mkdtempSync(join(tmpdir(), 'taurhaus-tmux-nohook-'))
    try {
      writeFileSync(
        join(specsDir, 'stray-snapshot.js'),
        'function guard() {\n  assertTmuxIsolation(process.env)\n}\n\nconst panes = snapshotTmuxPanes()\n'
      )

      expect(tmuxIsolationCoverageProblems(specsDir)).toEqual([
        'stray-snapshot.js: call snapshotTmuxPanes from a before hook that asserts isolation first',
      ])
    } finally {
      rmSync(specsDir, { recursive: true, force: true })
    }
  })
})

describe('applyTmuxIsolation', () => {
  // Regression: 5623e78 added the checks that refuse a lane not on its own tmux
  // server, but nothing ever put it there — `wdio.conf.js` started the driver
  // and the app with the operator's `TMUX_TMPDIR` and an inherited `TMUX`, so
  // the lane's own gate would have skipped it on every host that runs it.
  it('leaves the environment satisfying the check the lane gates on', () => {
    const environment = { TMUX: '/tmp/tmux-1000/default,407334,0', PATH: '/usr/bin' }
    const socketDir = applyTmuxIsolation(environment, root)

    expect(socketDir).toBe(isolatedTmuxTmpdir(root))
    expect(tmuxIsolationProblem(environment, root)).toBe('')
    expect('TMUX' in environment).toBe(false)
    expect(environment.PATH).toBe('/usr/bin')
  })

  it('refuses to name a socket directory without a session temp root', () => {
    const environment = {}
    expect(applyTmuxIsolation(environment, '')).toBe('')
    expect(environment.TMUX_TMPDIR).toBeUndefined()
  })
})
