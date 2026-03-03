/**
 * Session store — maintains running CLI tool sessions and exposes reactive
 * state keyed by project path.
 *
 * A project can have multiple concurrent sessions (e.g. Claude + Codex).
 * The store groups sessions by normalized project path.
 *
 * Also tracks per-session activity ticks in memory, enriching session
 * objects with _duration, _activeMs, _activePercent, _lastTransition.
 * On session disappearance, persists stats via recordSessionActivity IPC.
 *
 * Update sources:
 * - `startPolling()` / `stopPolling()` for frontend-only mock mode.
 * - `applyDaemonSessionUpdate()` for event-driven daemon updates.
 *
 * Usage:
 *   import { startPolling, stopPolling, getSessionsForProject } from './sessionStore.svelte.js'
 *   startPolling()   // on mount
 *   stopPolling()    // on unmount
 *   getSessionsForProject('/home/user/proj')  // → ClaudeSession[]
 */

import { listClaudeSessions, recordSessionActivity } from './ipc.js'

const POLL_INTERVAL_MS = 500

/** @type {Map<string, object[]>} Reactive map of project_path → ClaudeSession[] */
let sessions = $state(new Map())

/** Whether the poll loop is running. */
let running = false

/** Timer handle for the scheduled next poll (for cleanup on stop). */
let timerId = null

/**
 * In-memory activity trackers keyed by PID.
 * Not reactive — only used to compute enrichment fields on each poll.
 * @type {Map<number, {firstSeen: number, activeTicks: number, totalTicks: number, lastState: string, lastTransitionTime: number, projectPath: string, cliTool: string}>}
 */
let trackers = new Map()

/**
 * Normalize path for consistent matching.
 * - Strips trailing slashes and backslashes
 * - Normalizes WSL UNC prefixes: \\wsl.localhost\ and \\wsl$\ → \\wsl$\
 *   (projects may be registered with either form)
 * - Normalizes Windows drive letters to uppercase (D:\foo, not d:\foo)
 */
function normalizePath(path) {
  let p = path
  // Strip trailing separators
  while (p.length > 1 && (p.endsWith('/') || p.endsWith('\\'))) {
    p = p.slice(0, -1)
  }
  // Normalize \\wsl.localhost\ → \\wsl$\ for consistent matching
  if (p.toLowerCase().startsWith('\\\\wsl.localhost\\')) {
    p = '\\\\wsl$\\' + p.slice('\\\\wsl.localhost\\'.length)
  }
  // Normalize Windows drive letter to uppercase (d:\foo → D:\foo)
  if (/^[a-z]:[/\\]/.test(p)) {
    p = p[0].toUpperCase() + p.slice(1)
  }
  return p
}

/** Perform a single poll and update the sessions map. */
async function poll() {
  try {
    const result = await listClaudeSessions()
    applySessions(result)
  } catch (err) {
    // On error, keep previous state (graceful degradation)
    console.warn('[sessionStore] poll failed:', err)
  }
}

/**
 * Apply a full session snapshot to the store and trackers.
 * Used by both polling and daemon event-driven updates.
 */
function applySessions(result) {
  const now = Date.now()

  // Track which PIDs are still present
  const currentPids = new Set()

  const next = new Map()
  for (const session of result) {
    const pid = session.pid
    currentPids.add(pid)

    // Create or update tracker
    let tracker = trackers.get(pid)
    if (!tracker) {
      tracker = {
        firstSeen: now,
        activeTicks: 0,
        totalTicks: 0,
        lastState: session.state,
        lastTransitionTime: now,
        projectPath: session.project_path,
        cliTool: session.cli_tool || 'claude',
      }
      trackers.set(pid, tracker)
    }

    tracker.totalTicks++
    if (session.state === 'active') {
      tracker.activeTicks++
    }

    // Detect state transition
    if (session.state !== tracker.lastState) {
      tracker.lastState = session.state
      tracker.lastTransitionTime = now
    }

    // Enrich session object with computed fields
    session._duration = now - tracker.firstSeen
    session._activeMs = tracker.activeTicks * POLL_INTERVAL_MS
    session._activePercent = tracker.totalTicks > 0
      ? Math.round((tracker.activeTicks / tracker.totalTicks) * 100)
      : 0
    session._lastTransition = tracker.lastTransitionTime

    const key = normalizePath(session.project_path)
    const list = next.get(key) || []
    list.push(session)
    next.set(key, list)
  }

  // Detect disappeared sessions and persist their stats
  for (const [pid, tracker] of trackers) {
    if (!currentPids.has(pid)) {
      const endedAt = new Date().toISOString()
      const startedAt = new Date(tracker.firstSeen).toISOString()
      const activeDurationMs = tracker.activeTicks * POLL_INTERVAL_MS
      const totalDurationMs = tracker.totalTicks * POLL_INTERVAL_MS

      // Fire-and-forget — don't block updates
      recordSessionActivity(
        tracker.projectPath,
        tracker.cliTool,
        startedAt,
        endedAt,
        activeDurationMs,
        totalDurationMs,
      ).catch(() => {})

      trackers.delete(pid)
    }
  }

  sessions = next
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

/** Start polling for CLI tool sessions. Idempotent — calling twice is safe. */
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
  trackers.clear()
}

/**
 * Apply sessions received from daemon `sessions-updated` events.
 * Payload shape: `{ version: number, sessions: ClaudeSession[] }`.
 */
export function applyDaemonSessionUpdate(payload) {
  const list = Array.isArray(payload) ? payload : payload?.sessions
  if (!Array.isArray(list)) return
  applySessions(list)
}

/** Get the current sessions map (for testing or direct access). */
export function getSessions() {
  return sessions
}

/** Get all sessions for a project. Returns empty array if none. */
export function getSessionsForProject(projectPath) {
  const key = normalizePath(projectPath)
  return sessions.get(key) ?? []
}

/**
 * Get a single session for a project by its path.
 * Returns the first session found (backward compatible).
 * Prefer getSessionsForProject() for multi-tool support.
 */
export function getSessionForProject(projectPath) {
  const list = getSessionsForProject(projectPath)
  return list[0] ?? null
}

/** Get tracker stats for a PID (for testing). Returns null if not tracked. */
export function getSessionStats(pid) {
  return trackers.get(pid) ?? null
}
