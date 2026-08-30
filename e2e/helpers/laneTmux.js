/**
 * The tmux server a paid lane is allowed to create panes on.
 *
 * taurhaus puts every managed pane in a tmux session called `taurhaus`
 * (`TAURHAUS_TMUX_SESSION_NAME`, not configurable), and a lane has to push its
 * isolated roots into that session's environment for the panes to inherit them.
 * On the operator's own tmux server that is host pollution: the session is
 * usually attached and in use, so `set-environment` hands `TAURHAUS_DATA_DIR`,
 * `CODEX_HOME` and friends — roots the run deletes on the way out — to whatever
 * pane the operator opens next, and a pane the lane fails to account for
 * outlives the run.
 *
 * So the lane runs against a tmux *server* of its own: `TMUX_TMPDIR` points at a
 * directory inside the wdio session temp root, every process that speaks tmux
 * (this one, the app, the daemons it spawns) inherits it, and teardown kills
 * that server outright. Two things decide whether that actually happened, and
 * both are worth checking before a lane spends a subscription turn:
 *
 *   - `TMUX_TMPDIR` has to name the lane's own socket directory;
 *   - `TMUX` has to be *unset*. A client inside a tmux pane resolves the socket
 *     from `$TMUX` and ignores `TMUX_TMPDIR` entirely, and an e2e run started
 *     from a tmux pane inherits it — which is exactly how a lane ends up on the
 *     operator's server while believing it is isolated.
 */

import { join } from 'node:path'

/** The socket directory for the lane's own tmux server. */
export function isolatedTmuxTmpdir(sessionTempRoot) {
  return join(sessionTempRoot, 'tmux')
}

/**
 * Why `environment` would not reach the lane's own tmux server, or `''`.
 *
 * Takes an environment rather than reading `process.env` so the same check can
 * be run against another process's `/proc/<pid>/environ` — the app under test
 * creates the panes, so its environment is the one that decides.
 */
export function tmuxIsolationProblem(environment, sessionTempRoot) {
  const root = String(sessionTempRoot ?? '').trim()
  if (!root) return 'no session temp root is known, so no isolated tmux server can be named'

  const wanted = isolatedTmuxTmpdir(root)
  const tmpdir = String(environment?.TMUX_TMPDIR ?? '').trim()
  if (!tmpdir) return `TMUX_TMPDIR is unset, so tmux would use the operator's own server instead of ${wanted}`
  if (tmpdir !== wanted) return `TMUX_TMPDIR is ${tmpdir}, not the lane's own socket directory ${wanted}`

  const inherited = String(environment?.TMUX ?? '').trim()
  if (inherited) {
    return `TMUX is set (${inherited}), and a client inside a tmux pane resolves that socket rather than TMUX_TMPDIR`
  }
  return ''
}

/** `/proc/<pid>/environ` — NUL-separated `NAME=value` pairs — as an object. */
export function parseProcEnviron(raw) {
  const environment = {}
  for (const entry of String(raw ?? '').split('\0')) {
    if (!entry) continue
    const split = entry.indexOf('=')
    if (split <= 0) continue
    environment[entry.slice(0, split)] = entry.slice(split + 1)
  }
  return environment
}

/**
 * The specs that must run against a tmux server of their own.
 *
 * Only the managed-stage lane, deliberately. `compaction-codex-hooks.js` has
 * the same shape of problem, but it costs a subscription turn to re-verify and
 * nothing here was run against it; moving it is its own change.
 */
const ISOLATED_TMUX_SPECS = ['managed-stage-codex.js']

/** Whether this WDIO session runs a spec that needs its own tmux server. */
export function wantsIsolatedTmux(specs) {
  return (specs ?? []).some((spec) =>
    ISOLATED_TMUX_SPECS.some((name) => String(spec ?? '').endsWith(name))
  )
}

/**
 * Point `environment` at the lane's own tmux server, in place.
 *
 * Both halves matter and only one of them is obvious. Setting `TMUX_TMPDIR` is
 * what names the socket directory; deleting `TMUX` is what makes it count, because
 * a process started from inside a tmux pane — which is how this suite is run —
 * carries a `$TMUX` that every tmux client prefers over `TMUX_TMPDIR`.
 *
 * Returns the socket directory it named, or `''` when there is no session temp
 * root to put one in. The caller creates the directory: tmux makes
 * `$TMUX_TMPDIR/tmux-<uid>` itself but not the parent, and fails outright if the
 * parent is missing.
 */
export function applyTmuxIsolation(environment, sessionTempRoot) {
  const root = String(sessionTempRoot ?? '').trim()
  if (!root) return ''

  const socketDir = isolatedTmuxTmpdir(root)
  environment.TMUX_TMPDIR = socketDir
  delete environment.TMUX
  return socketDir
}
