/**
 * Session store — polls for running Claude Code sessions and exposes
 * reactive state keyed by project path.
 *
 * Usage:
 *   import { startPolling, stopPolling, getSessionForProject } from './sessionStore.svelte.js'
 *   startPolling()   // on mount
 *   stopPolling()    // on unmount
 *   getSessionForProject('/home/user/proj')  // → ClaudeSession | null
 */

import { listClaudeSessions, isTauri } from './ipc.js'

/** Send a log line to the backend log file (fire-and-forget). */
async function log(msg) {
  console.log('[sessionStore]', msg)
  if (isTauri()) {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      invoke('frontend_log', { level: 'info', message: `[sessionStore] ${msg}` })
    } catch (_) { /* best-effort */ }
  }
}

const POLL_INTERVAL_MS = 500

/** @type {Map<string, object>} Reactive map of project_path → ClaudeSession */
let sessions = $state(new Map())

/** @type {number | null} */
let intervalId = null

/** Generation counter for stale response detection. */
let generation = 0

/**
 * Normalize path for consistent matching.
 * - Strips trailing slashes
 * - Normalizes WSL UNC prefixes: \\wsl.localhost\ and \\wsl$\ → \\wsl$\
 *   (projects may be registered with either form)
 */
function normalizePath(path) {
  let p = path.endsWith('/') ? path.slice(0, -1) : path
  // Normalize \\wsl.localhost\ → \\wsl$\ for consistent matching
  if (p.toLowerCase().startsWith('\\\\wsl.localhost\\')) {
    p = '\\\\wsl$\\' + p.slice('\\\\wsl.localhost\\'.length)
  }
  return p
}

/** Perform a single poll and update the sessions map. */
async function poll() {
  const thisGen = ++generation

  try {
    const result = await listClaudeSessions()

    // Discard stale response (a newer poll has already started)
    if (thisGen !== generation) return

    const next = new Map()
    for (const session of result) {
      const key = normalizePath(session.project_path)
      next.set(key, session)
    }
    // Diagnostic: log first successful poll and any changes
    if (next.size > 0 && (sessions.size !== next.size || thisGen <= 3)) {
      log(`poll: ${next.size} sessions, keys=[${[...next.keys()].join(', ')}], states=[${[...next.values()].map(s => s.state).join(', ')}]`)
    }
    sessions = next
  } catch (err) {
    // On error, keep previous state (graceful degradation)
    if (thisGen === generation) {
      // Only log if this is still the current generation
      console.warn('[sessionStore] poll failed:', err)
    }
  }
}

/** Start polling for Claude Code sessions. Idempotent — calling twice is safe. */
export function startPolling() {
  if (intervalId !== null) return

  // Immediate first poll
  poll()

  intervalId = setInterval(poll, POLL_INTERVAL_MS)
}

/** Stop polling and clean up the interval. */
export function stopPolling() {
  if (intervalId !== null) {
    clearInterval(intervalId)
    intervalId = null
  }
}

/** Get the current sessions map (for testing or direct access). */
export function getSessions() {
  return sessions
}

/** Look up the session for a project by its path. Returns null if none. */
let _lookupLogCount = 0
export function getSessionForProject(projectPath) {
  const key = normalizePath(projectPath)
  const result = sessions.get(key) ?? null
  // Log first few lookups to diagnose path matching
  if (_lookupLogCount < 5) {
    _lookupLogCount++
    log(`lookup: path="${projectPath}" → key="${key}" → ${result ? `MATCH(state=${result.state})` : `MISS(map size=${sessions.size}, keys=[${[...sessions.keys()].join(',')}])`}`)
  }
  return result
}
