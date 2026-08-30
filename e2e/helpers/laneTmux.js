/**
 * The tmux server an E2E worker is allowed to create panes on.
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
 * So every worker runs against a tmux *server* of its own: `TMUX_TMPDIR` points at a
 * directory inside the wdio session temp root, every process that speaks tmux
 * (this one, the app, the daemons it spawns) inherits it, and teardown kills
 * that server outright. Two things decide whether that actually happened, and
 * both are checked before any tmux-driving spec makes its first call:
 *
 *   - `TMUX_TMPDIR` has to name the lane's own socket directory;
 *   - `TMUX` has to be *unset*. A client inside a tmux pane resolves the socket
 *     from `$TMUX` and ignores `TMUX_TMPDIR` entirely, and an e2e run started
 *     from a tmux pane inherits it — which is exactly how a lane ends up on the
 *     operator's server while believing it is isolated.
 */

import { readdirSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'

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

const TMUX_CALL_PATTERNS = [
  /\bexecFileSync\s*\(\s*['"]tmux['"]/g,
  /\b(?:snapshotTmuxPanes|cleanupNewTmuxPanes)\s*\(/g,
]

function firstTmuxCallOffset(source) {
  let first = -1
  for (const pattern of TMUX_CALL_PATTERNS) {
    pattern.lastIndex = 0
    const match = pattern.exec(source)
    if (match && (first < 0 || match.index < first)) first = match.index
  }
  return first
}

/** Spec filenames whose source invokes tmux directly or through the pane helper. */
export function findTmuxDrivingSpecs(specsDir) {
  return readdirSync(specsDir)
    .filter((name) => name.endsWith('.js'))
    .sort()
    .filter((name) => firstTmuxCallOffset(readFileSync(join(specsDir, name), 'utf8')) >= 0)
}

/** Missing or late isolation assertions in tmux-driving specs. */
export function tmuxIsolationCoverageProblems(specsDir) {
  const problems = []
  for (const name of findTmuxDrivingSpecs(specsDir)) {
    const source = readFileSync(join(specsDir, name), 'utf8')
    const tmuxOffset = firstTmuxCallOffset(source)
    const assertionOffset = source.indexOf('assertTmuxIsolation(')
    if (assertionOffset < 0) {
      problems.push(`${name}: call assertTmuxIsolation before the first tmux call`)
    } else if (assertionOffset > tmuxOffset) {
      problems.push(`${name}: assertTmuxIsolation is after the first tmux call`)
    }
  }
  return problems
}

/** Throw before a spec can address any tmux server it does not own. */
export function assertTmuxIsolation(environment, sessionTempRoot) {
  const root = sessionTempRoot || dirname(String(environment?.TAURHAUS_DATA_DIR ?? ''))
  const problem = tmuxIsolationProblem(environment, root)
  if (problem) throw new Error(`E2E tmux isolation is required: ${problem}`)
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
