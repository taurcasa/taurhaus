import { EventEmitter } from 'node:events'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { describe, it, expect, vi } from 'vitest'

import {
  E2E_RUN_TOKEN_ENV,
  cleanupStaleProcessLedgers,
  createLaneCleanup,
  createOwnedProcessLedger,
  findRunTokenProcessRecords,
  killOwnedProcessRecord,
} from './laneCleanup.js'

function silentLogger() {
  return { log: vi.fn(), warn: vi.fn(), error: vi.fn() }
}

function writeProcFixture(procRoot, pid, { startTime, runToken }) {
  const processRoot = join(procRoot, String(pid))
  mkdirSync(processRoot, { recursive: true })
  const fieldsAfterCommand = ['S', ...Array(18).fill('0'), String(startTime), '0']
  writeFileSync(join(processRoot, 'stat'), `${pid} (fixture process) ${fieldsAfterCommand.join(' ')}\n`)
  writeFileSync(join(processRoot, 'environ'), `${E2E_RUN_TOKEN_ENV}=${runToken}\0PATH=/usr/bin\0`)
}

describe('lane cleanup', () => {
  it('runs every owed undo once, in the order it was taken on', () => {
    const order = []
    const cleanup = createLaneCleanup({ logger: silentLogger() })
    cleanup.owe('first', () => order.push('first'))
    cleanup.owe('second', () => order.push('second'))

    cleanup.run()
    cleanup.run()

    expect(order).toEqual(['first', 'second'])
    expect(cleanup.owed()).toEqual([])
  })

  it('keeps going when one undo throws, and says which failed', () => {
    const logger = silentLogger()
    const after = vi.fn()
    const cleanup = createLaneCleanup({ logger })
    cleanup.owe('explodes', () => {
      throw new Error('tmux is gone')
    })
    cleanup.owe('after', after)

    cleanup.run()

    expect(after).toHaveBeenCalledTimes(1)
    expect(logger.warn.mock.calls.flat().join(' ')).toContain('explodes')
    expect(logger.warn.mock.calls.flat().join(' ')).toContain('tmux is gone')
  })

  it('drops an undo the normal teardown already settled', () => {
    const undo = vi.fn()
    const cleanup = createLaneCleanup({ logger: silentLogger() })
    cleanup.owe('settled-by-mocha', undo)
    cleanup.settled('settled-by-mocha')

    cleanup.run()

    expect(undo).not.toHaveBeenCalled()
  })

  it('replaces an undo taken on twice under the same name', () => {
    const stale = vi.fn()
    const fresh = vi.fn()
    const cleanup = createLaneCleanup({ logger: silentLogger() })
    cleanup.owe('pane-environment', stale)
    cleanup.owe('pane-environment', fresh)

    cleanup.run()

    expect(stale).not.toHaveBeenCalled()
    expect(fresh).toHaveBeenCalledTimes(1)
  })

  // Regression: 3b56a3f ("test(e2e): add the live Codex compaction lane driven
  // through the hook bridge") left the lane's tmux panes and the daemon's
  // compaction mode to the Mocha `after` hook alone. `e2e/wdio.conf.js` turns
  // SIGINT into "delete the session temp root, then exit", so cancelling the
  // lane — which runs for minutes and costs money, exactly the run an operator
  // interrupts — left managed panes alive over deleted roots. Cleanup has to be
  // on the signal path, and ahead of the handler that exits.
  it('runs on SIGINT before a handler that was registered first', () => {
    const proc = new EventEmitter()
    const order = []
    proc.on('SIGINT', () => order.push('wdio-exits-here'))

    const cleanup = createLaneCleanup({ logger: silentLogger() })
    cleanup.install(proc)
    cleanup.owe('panes', () => order.push('panes'))

    proc.emit('SIGINT')

    expect(order).toEqual(['panes', 'wdio-exits-here'])
  })

  it('runs on SIGTERM and on a plain exit, and only once across both', () => {
    const proc = new EventEmitter()
    const undo = vi.fn()
    const cleanup = createLaneCleanup({ logger: silentLogger() })
    cleanup.install(proc)
    cleanup.owe('panes', undo)

    proc.emit('SIGTERM')
    proc.emit('exit')

    expect(undo).toHaveBeenCalledTimes(1)
  })

  it('owes nothing until a step is taken on, so a bare install is inert', () => {
    const proc = new EventEmitter()
    const cleanup = createLaneCleanup({ logger: silentLogger() })
    cleanup.install(proc)

    expect(() => proc.emit('exit')).not.toThrow()
    expect(cleanup.owed()).toEqual([])
  })

  // Regression: 2daa0b8 ("test(e2e): put the lane's tmux cleanup on the signal
  // path") put the undos in front of SIGINT, SIGTERM and exit only.
  // `e2e/wdio.conf.js` handles `uncaughtException` and `unhandledRejection`
  // separately — it deletes the session temp root and *returns* — and
  // registering those handlers is exactly what stops Node from terminating on a
  // crash. So a crash inside the lane ran no undo at all: the managed panes
  // stayed alive over a deleted root and the operator's daemon stayed in the
  // mode the lane put it in.
  it('runs on a crash a handler already suppresses, ahead of that handler', () => {
    const proc = new EventEmitter()
    const order = []
    proc.on('uncaughtException', () => order.push('wdio-deletes-the-session-root'))

    const cleanup = createLaneCleanup({ logger: silentLogger() })
    cleanup.install(proc)
    cleanup.owe('panes', () => order.push('panes'))

    proc.emit('uncaughtException', new Error('boom'))

    expect(order).toEqual(['panes', 'wdio-deletes-the-session-root'])
  })

  it('runs on a suppressed unhandled rejection too', () => {
    const proc = new EventEmitter()
    const undo = vi.fn()
    proc.on('unhandledRejection', () => {})

    const cleanup = createLaneCleanup({ logger: silentLogger() })
    cleanup.install(proc)
    cleanup.owe('daemon-mode', undo)

    proc.emit('unhandledRejection', new Error('boom'))

    expect(undo).toHaveBeenCalledTimes(1)
  })

  // A crash nobody handles still terminates the process, and Node emits `exit`
  // on its way out, so the undos already run. Listening ourselves would suppress
  // that termination and turn a crash into a hang, so `install` stays out of a
  // crash path no one else is on.
  it('does not take a crash off the default termination path', () => {
    const proc = new EventEmitter()
    const cleanup = createLaneCleanup({ logger: silentLogger() })

    cleanup.install(proc)

    expect(proc.listenerCount('uncaughtException')).toBe(0)
    expect(proc.listenerCount('unhandledRejection')).toBe(0)
  })
})

describe('owned process cleanup', () => {
  // Regression: commit 707ce88a recorded a PID without its Linux start time,
  // so a reused PID could make cleanup kill a process this run never started.
  it('does not kill a recorded process whose PID has been reused', () => {
    const kill = vi.fn()

    const killed = killOwnedProcessRecord(
      { pid: 4242, startTime: '100' },
      { readStartTime: () => '200', kill }
    )

    expect(killed).toBe(false)
    expect(kill).not.toHaveBeenCalled()
  })

  it('kills a recorded process when both PID and start time still match', () => {
    const kill = vi.fn()

    const killed = killOwnedProcessRecord(
      { pid: 4242, startTime: '100' },
      { readStartTime: () => '100', kill }
    )

    expect(killed).toBe(true)
    expect(kill).toHaveBeenCalledWith(4242, 'SIGKILL')
  })

  // Regression: commit 69bb4e1a created an ownership ledger but never proved
  // that token-carrying descendants were discovered and persisted beside the
  // explicitly recorded tauri-driver group leader.
  it('records token-carrying descendants in the persistent ledger', () => {
    const fixtureRoot = mkdtempSync(join(tmpdir(), 'taurhaus-ledger-test-'))
    const procRoot = join(fixtureRoot, 'proc')
    const registryRoot = join(fixtureRoot, 'registry')
    const runToken = 'run-uuid-1234'
    try {
      writeProcFixture(procRoot, 4101, { startTime: '101', runToken })
      writeProcFixture(procRoot, 4102, { startTime: '102', runToken })
      writeProcFixture(procRoot, 4199, { startTime: '199', runToken: 'another-run' })

      const records = findRunTokenProcessRecords(runToken, { procRoot })
      expect(records.map((record) => record.pid)).toEqual([4101, 4102])

      const startTimes = new Map([[9999, 'owner'], [4101, '101'], [4102, '102']])
      const ledger = createOwnedProcessLedger({
        checkoutRoot: '/checkout/a',
        runToken,
        ownerPid: 9999,
        registryRoot,
        readStartTime: (pid) => startTimes.get(pid) ?? null,
      })
      ledger.recordPid(4101, { processGroup: true })
      for (const record of records) ledger.record(record)

      const persisted = JSON.parse(readFileSync(ledger.path, 'utf8'))
      expect(persisted.processes).toHaveLength(2)
      expect(persisted.processes).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ pid: 4101, startTime: '101', processGroup: true }),
          expect.objectContaining({ pid: 4102, startTime: '102' }),
        ])
      )
    } finally {
      rmSync(fixtureRoot, { recursive: true, force: true })
    }
  })

  it('leaves a ledger alone while its owner identity is still alive', () => {
    const registryRoot = mkdtempSync(join(tmpdir(), 'taurhaus-ledger-live-'))
    const kill = vi.fn()
    try {
      const ledger = createOwnedProcessLedger({
        checkoutRoot: '/checkout/a',
        runToken: 'live-run',
        ownerPid: 9001,
        registryRoot,
        readStartTime: () => 'owner-start',
      })
      ledger.recordPid(4101)

      cleanupStaleProcessLedgers('/checkout/a', {
        registryRoot,
        readStartTime: (pid) => pid === 9001 ? 'owner-start' : 'process-start',
        kill,
      })

      expect(kill).not.toHaveBeenCalled()
      expect(existsSync(ledger.path)).toBe(true)
    } finally {
      rmSync(registryRoot, { recursive: true, force: true })
    }
  })

  it('kills matching records and removes a ledger whose owner is gone', () => {
    const registryRoot = mkdtempSync(join(tmpdir(), 'taurhaus-ledger-stale-'))
    const kill = vi.fn()
    try {
      const ledger = createOwnedProcessLedger({
        checkoutRoot: '/checkout/a',
        runToken: 'stale-run',
        ownerPid: 9001,
        registryRoot,
        readStartTime: (pid) => pid === 9001 ? 'owner-start' : 'process-start',
      })
      ledger.recordPid(4101)

      cleanupStaleProcessLedgers('/checkout/a', {
        registryRoot,
        readStartTime: (pid) => pid === 4101 ? 'process-start' : null,
        kill,
      })

      expect(kill).toHaveBeenCalledWith(4101, 'SIGKILL')
      expect(existsSync(ledger.path)).toBe(false)
    } finally {
      rmSync(registryRoot, { recursive: true, force: true })
    }
  })

  it('ignores a ledger that does not claim this checkout root', () => {
    const registryRoot = mkdtempSync(join(tmpdir(), 'taurhaus-ledger-foreign-'))
    const kill = vi.fn()
    try {
      const ledger = createOwnedProcessLedger({
        checkoutRoot: '/checkout/a',
        runToken: 'foreign-run',
        ownerPid: 9001,
        registryRoot,
        readStartTime: () => 'start',
      })
      ledger.recordPid(4101)
      const persisted = JSON.parse(readFileSync(ledger.path, 'utf8'))
      persisted.checkoutRoot = '/checkout/b'
      writeFileSync(ledger.path, `${JSON.stringify(persisted)}\n`)

      cleanupStaleProcessLedgers('/checkout/a', {
        registryRoot,
        readStartTime: () => null,
        kill,
      })

      expect(kill).not.toHaveBeenCalled()
      expect(existsSync(ledger.path)).toBe(true)
    } finally {
      rmSync(registryRoot, { recursive: true, force: true })
    }
  })
})
