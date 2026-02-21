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

/** Whether the poll loop is running. */
let running = false

/** Timer handle for the scheduled next poll (for cleanup on stop). */
let timerId = null

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
  try {
    const result = await listClaudeSessions()

    const next = new Map()
    for (const session of result) {
      const key = normalizePath(session.project_path)
      next.set(key, session)
    }
    sessions = next
  } catch (err) {
    // On error, keep previous state (graceful degradation)
    console.warn('[sessionStore] poll failed:', err)
  }
}

/**
 * Self-scheduling poll loop. Waits for the current poll to complete
 * before scheduling the next one, guaranteeing at most one in-flight
 * request at a time.
 */
async function pollLoop() {
  if (!running) return
  await poll()
  if (running) {
    timerId = setTimeout(pollLoop, POLL_INTERVAL_MS)
  }
}

/** Start polling for Claude Code sessions. Idempotent — calling twice is safe. */
export function startPolling() {
  if (running) return
  running = true
  pollLoop()
}

/** Stop polling and clean up. */
export function stopPolling() {
  running = false
  if (timerId !== null) {
    clearTimeout(timerId)
    timerId = null
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
