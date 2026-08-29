/**
 * Workflow runs for the sessions currently on screen.
 *
 * W2a's scanner returns a snapshot, not a stream, so the UI has to ask again to
 * see a live run move. This store is the one place that asks, on one shared
 * timer for the whole app: a node does not get a timer of its own, and nothing
 * polls while nothing live is expanded.
 *
 * The cadence is deliberately narrow, and split in two so the expensive call
 * stays rare while a run that starts later is still found:
 *
 * - watching a session lists its runs once, immediately;
 * - the 2 s loop carries only the sessions that have an *expanded* live run,
 *   and stops by itself when the last one finishes or is collapsed;
 * - `get_workflow_run` — the expensive call, it reads every agent transcript —
 *   is made only for an expanded live run;
 * - every other watched session is re-listed every 10 s. `list_workflow_runs`
 *   reads a directory and, for a live run, one journal; a run that starts after
 *   a node is already on screen is otherwise invisible for as long as it is
 *   watched, because the snapshot never announces itself.
 *
 * A failed poll keeps the last good runs on screen and records why, because a
 * run that is still on disk has not stopped existing just because one call to
 * the daemon failed.
 */

import { getWorkflowRun, listWorkflowRuns } from './ipc.js'

/** How often an expanded live run is re-read in full. */
const DETAIL_INTERVAL_MS = 2000
/** How often every other watched session is re-listed, looking for a new run. */
const DISCOVERY_INTERVAL_MS = 10_000

const EMPTY_SESSION = Object.freeze({ runs: Object.freeze([]), loaded: false, error: null })

/** sessionId → { runs, loaded, error, collapsed } */
const sessions = $state({ byId: {} })

/** sessionId → number of mounted watchers. Not reactive: nothing renders it. */
const watchers = new Map()
/** sessionId of every refresh currently in flight, so ticks never overlap. */
const inFlight = new Set()
let detailTimer = null
let discoveryTimer = null

function sessionState(sessionId) {
  if (!sessions.byId[sessionId]) {
    sessions.byId[sessionId] = { runs: [], loaded: false, error: null, collapsed: {} }
  }
  return sessions.byId[sessionId]
}

function isLive(run) {
  return String(run?.status ?? '').toLowerCase() === 'live'
}

function runId(run) {
  return String(run?.run_id ?? run?.runId ?? '')
}

function startedAt(run) {
  const value = Number(run?.started_at ?? run?.startedAt)
  return Number.isFinite(value) ? value : 0
}

/** Runs for a session, newest first. Reactive — read it inside `$derived`. */
export function workflowSessionRuns(sessionId) {
  const id = String(sessionId || '')
  const state = id ? sessions.byId[id] : null
  if (!state) return EMPTY_SESSION
  return { runs: state.runs, loaded: state.loaded, error: state.error }
}

/** Whether the viewer has collapsed this run's tree. Live runs start expanded. */
export function isWorkflowRunCollapsed(sessionId, id) {
  const state = sessions.byId[String(sessionId || '')]
  return Boolean(state?.collapsed[String(id)])
}

function hasExpandedLiveRun(sessionId) {
  const state = sessions.byId[sessionId]
  if (!state) return false
  return state.runs.some((run) => isLive(run) && !state.collapsed[runId(run)])
}

/** Watched sessions on one side of the expanded-live-run line. */
function watchedSessions(expanded) {
  return [...watchers.keys()].filter((sessionId) => hasExpandedLiveRun(sessionId) === expanded)
}

function stopTimers() {
  if (detailTimer !== null) {
    clearTimeout(detailTimer)
    detailTimer = null
  }
  if (discoveryTimer !== null) {
    clearTimeout(discoveryTimer)
    discoveryTimer = null
  }
}

function tick(sessionIds) {
  return Promise.allSettled(sessionIds.map((sessionId) => refresh(sessionId))).then(scheduleTicks)
}

function scheduleTicks() {
  if (detailTimer === null && watchedSessions(true).length > 0) {
    detailTimer = setTimeout(() => {
      detailTimer = null
      void tick(watchedSessions(true))
    }, DETAIL_INTERVAL_MS)
  }

  if (discoveryTimer === null && watchers.size > 0) {
    discoveryTimer = setTimeout(() => {
      discoveryTimer = null
      void tick(watchedSessions(false))
    }, DISCOVERY_INTERVAL_MS)
  }
}

async function refresh(sessionId) {
  if (!sessionId || inFlight.has(sessionId)) return
  inFlight.add(sessionId)
  const state = sessionState(sessionId)

  try {
    const listed = await listWorkflowRuns(sessionId)
    const next = (Array.isArray(listed) ? [...listed] : [])
      .sort((left, right) => startedAt(right) - startedAt(left))

    for (const [index, summary] of next.entries()) {
      if (!isLive(summary) || state.collapsed[runId(summary)]) continue
      try {
        next[index] = await getWorkflowRun(sessionId, runId(summary))
      } catch {
        // One unreadable run — a directory removed mid-scan, say — leaves that
        // run as its summary. It has not stopped existing, and the other runs
        // of the session are still true.
      }
    }

    state.runs = next
    state.loaded = true
    state.error = null
  } catch (error) {
    state.loaded = true
    state.error = error?.message ? String(error.message) : String(error)
  } finally {
    inFlight.delete(sessionId)
  }
}

/**
 * Follow one session's runs while a surface is showing them.
 *
 * Returns the unwatch function; the last unwatch stops the shared timer.
 */
export function watchWorkflowSession(sessionId) {
  const id = String(sessionId || '')
  if (!id) return () => {}

  watchers.set(id, (watchers.get(id) ?? 0) + 1)
  if (watchers.get(id) === 1) {
    sessionState(id)
    void refresh(id).then(scheduleTicks)
  }

  return () => {
    const remaining = (watchers.get(id) ?? 0) - 1
    if (remaining > 0) {
      watchers.set(id, remaining)
      return
    }
    watchers.delete(id)
    if (watchers.size === 0) stopTimers()
  }
}

/** Collapse or expand one run's tree. Expanding a live run refreshes it now. */
export function toggleWorkflowRun(sessionId, id) {
  const state = sessionState(String(sessionId || ''))
  const key = String(id)
  if (state.collapsed[key]) {
    delete state.collapsed[key]
    void refresh(String(sessionId)).then(scheduleTicks)
    return
  }
  state.collapsed[key] = true
}

/** @public Test seam: drop every watcher, timer and cached run. */
export function resetWorkflowRunsForTest() {
  stopTimers()
  watchers.clear()
  inFlight.clear()
  sessions.byId = {}
}
