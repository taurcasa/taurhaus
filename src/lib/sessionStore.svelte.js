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

import { listClaudeSessions } from './ipc.js'

const POLL_INTERVAL_MS = 500

/** @type {Map<string, object>} Reactive map of project_path → ClaudeSession */
let sessions = $state(new Map())

/** @type {number | null} */
let intervalId = null

/** Generation counter for stale response detection. */
let generation = 0

/** Normalize path: strip trailing slash for consistent matching. */
function normalizePath(path) {
  return path.endsWith('/') ? path.slice(0, -1) : path
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
export function getSessionForProject(projectPath) {
  const key = normalizePath(projectPath)
  return sessions.get(key) ?? null
}
