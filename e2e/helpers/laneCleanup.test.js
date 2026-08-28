import { describe, it, expect, vi } from 'vitest'
import { EventEmitter } from 'node:events'

import { createLaneCleanup } from './laneCleanup.js'

function silentLogger() {
  return { log: vi.fn(), warn: vi.fn(), error: vi.fn() }
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
