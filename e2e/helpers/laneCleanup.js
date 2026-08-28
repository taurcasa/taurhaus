/**
 * Host state a lane changed and has to hand back, on every path out.
 *
 * A long, expensive spec is the one an operator interrupts, and an interrupted
 * WebdriverIO run never reaches Mocha's `after` hook: `wdio.conf.js` turns
 * SIGINT and SIGTERM into "clean up the session, then `process.exit`". Anything
 * a lane changed outside its own temp root — a shared tmux session, a running
 * daemon — is therefore left changed unless the undo sits on the signal path.
 *
 * So a lane takes on each undo as it makes the change (`owe`), drops it once the
 * normal teardown has done it (`settled`), and `install` puts the whole set in
 * front of the handler that exits. Undos run synchronously — an `exit` handler
 * cannot await — and each one runs at most once.
 */
export function createLaneCleanup({ logger = console } = {}) {
  const owed = new Map()

  function run() {
    for (const [name, undo] of [...owed]) {
      owed.delete(name)
      try {
        undo()
      } catch (error) {
        logger.warn(`[e2e] lane cleanup step "${name}" failed: ${error?.message ?? error}`)
      }
    }
  }

  return {
    /** Take on an undo for a change just made. A repeated name replaces the old one. */
    owe(name, undo) {
      owed.set(name, undo)
    },
    /** Drop an undo the normal teardown has already carried out. */
    settled(name) {
      owed.delete(name)
    },
    /** Names still owed — for assertions and for reporting a partial teardown. */
    owed() {
      return [...owed.keys()]
    },
    /** Run every remaining undo now. Safe to call again; nothing runs twice. */
    run,
    /**
     * Put the undos in front of the handler that exits.
     *
     * `prependListener` is the point: `wdio.conf.js` registers its own
     * SIGINT/SIGTERM handler when the config module loads — before any spec —
     * and that handler deletes the session temp root and exits without
     * returning, so a listener added after it never runs.
     */
    install(proc = process) {
      proc.prependListener('SIGINT', run)
      proc.prependListener('SIGTERM', run)
      proc.prependListener('exit', run)
    },
  }
}
