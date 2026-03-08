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

import { listClaudeSessions, listProjects, recordSessionActivity } from './ipc.js'
import { normalizeProjectPath } from './pathUtils.js'

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
 * @type {Map<number, {firstSeen: number, activeTicks: number, totalTicks: number, lastState: string, lastTransitionTime: number, projectPath: string, projectId: string | null, cliTool: string}>}
 */
let trackers = new Map()
let projectIdByPath = new Map()

function normalizeSessionShape(raw) {
  const session = raw && typeof raw === 'object' ? raw : {}
  const normalized = { ...session }

  if (normalized.project_path === undefined && session.projectPath !== undefined) {
    normalized.project_path = session.projectPath
  }
  if (normalized.project_id === undefined && session.projectId !== undefined) {
    normalized.project_id = session.projectId
  }
  if (normalized.cli_tool === undefined && session.cliTool !== undefined) {
    normalized.cli_tool = session.cliTool
  }
  if (normalized.tmux_session === undefined && session.tmuxSession !== undefined) {
    normalized.tmux_session = session.tmuxSession
  }
  if (normalized.tmux_window === undefined && session.tmuxWindow !== undefined) {
    normalized.tmux_window = session.tmuxWindow
  }
  if (normalized.tmux_pane === undefined && session.tmuxPane !== undefined) {
    normalized.tmux_pane = session.tmuxPane
  }
  if (normalized.tmux_window_name === undefined && session.tmuxWindowName !== undefined) {
    normalized.tmux_window_name = session.tmuxWindowName
  }

  return normalized
}

async function resolveProjectId(projectPath) {
  const key = normalizeProjectPath(projectPath)
  if (projectIdByPath.has(key)) {
    return projectIdByPath.get(key)
  }

  try {
    const projects = await listProjects()
    if (!Array.isArray(projects)) {
      return null
    }
    for (const project of projects) {
      if (!project?.id || !project?.path) continue
      projectIdByPath.set(normalizeProjectPath(project.path), project.id)
    }
  } catch (error) {
    console.warn('[sessionStore] failed to resolve project id for activity persistence:', error)
    return null
  }

  return projectIdByPath.get(key) ?? null
}

async function persistSessionActivity(tracker, startedAt, endedAt, activeDurationMs, totalDurationMs) {
  const projectId = tracker.projectId || await resolveProjectId(tracker.projectPath)
  if (!projectId) {
    console.warn('[sessionStore] skipping session activity persistence: unknown project id', {
      projectPath: tracker.projectPath,
    })
    return
  }

  await recordSessionActivity(
    projectId,
    tracker.cliTool,
    startedAt,
    endedAt,
    activeDurationMs,
    totalDurationMs,
  )
}

function flushTrackedActivity(trackersToFlush) {
  const endedAt = new Date().toISOString()
  const flushes = []

  for (const tracker of trackersToFlush) {
    if (!tracker || tracker.totalTicks <= 0) continue

    const startedAt = new Date(tracker.firstSeen).toISOString()
    const activeDurationMs = tracker.activeTicks * POLL_INTERVAL_MS
    const totalDurationMs = tracker.totalTicks * POLL_INTERVAL_MS

    flushes.push(persistSessionActivity(
      tracker,
      startedAt,
      endedAt,
      activeDurationMs,
      totalDurationMs,
    ).catch((error) => {
      console.warn('[sessionStore] failed to persist session activity stats:', error)
    }))
  }

  return flushes
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
  const list = Array.isArray(result) ? result : []

  // Track which PIDs are still present
  const currentPids = new Set()

  const next = new Map()
  for (const rawSession of list) {
    const session = normalizeSessionShape(rawSession)
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
        projectId: session.project_id || null,
        cliTool: session.cli_tool || 'claude',
      }
      trackers.set(pid, tracker)
    }

    if (!tracker.projectId && session.project_id) {
      tracker.projectId = session.project_id
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

    const key = normalizeProjectPath(session.project_path)
    const list = next.get(key) || []
    list.push(session)
    next.set(key, list)
  }

  // Detect disappeared sessions and persist their stats
  for (const [pid, tracker] of trackers) {
    if (!currentPids.has(pid)) {
      void Promise.allSettled(flushTrackedActivity([tracker]))
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
  const flushes = flushTrackedActivity(trackers.values())
  trackers.clear()
  projectIdByPath.clear()
  return Promise.allSettled(flushes)
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

/**
 * One-shot snapshot hydrate for Tauri runtime startup.
 * Keeps UI event-driven afterwards, but avoids empty indicators while waiting
 * for the first daemon-pushed update.
 */
export async function hydrateFromBackend() {
  try {
    const result = await listClaudeSessions()
    applySessions(Array.isArray(result) ? result : [])
  } catch (err) {
    console.warn('[sessionStore] hydrate failed:', err)
  }
}

/** Get the current sessions map (for testing or direct access). */
/** @public Exposed for tests and direct session diagnostics. */
export function getSessions() {
  return sessions
}

/** Get all sessions for a project. Returns empty array if none. */
export function getSessionsForProject(projectPath) {
  const key = normalizeProjectPath(projectPath)
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
/** @public Exposed for tests and session-tracker diagnostics. */
export function getSessionStats(pid) {
  return trackers.get(pid) ?? null
}
