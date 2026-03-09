import { formatUserFacingError } from './format.js'

const PROJECT_SECTION_TIMEOUT_MS = 5000
const PROJECT_SELECTION_DEBOUNCE_MS = 25
let scheduledSelectionBatch = null
let scheduledCriticalSelectionBatch = null

function withTimeout(promise, timeoutMs, section) {
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    return promise
  }

  let timerId
  const timeoutPromise = new Promise((_, reject) => {
    timerId = setTimeout(() => {
      reject(new Error(`${section} request timed out after ${timeoutMs}ms`))
    }, timeoutMs)
  })

  return Promise.race([promise, timeoutPromise]).finally(() => {
    if (timerId) clearTimeout(timerId)
  })
}

/**
 * Normalize unknown errors for degraded project-load sections.
 */
function projectLoadErrorMessage(err) {
  return formatUserFacingError(err, "Couldn't load project data")
}

/**
 * Resolve a project-load section and fall back to a safe value on error.
 */
export async function withFallback(
  section,
  promise,
  fallback,
  timeoutMs = PROJECT_SECTION_TIMEOUT_MS
) {
  try {
    const value = await withTimeout(promise, timeoutMs, section)
    return { ok: true, section, value, message: null }
  } catch (err) {
    const message = formatUserFacingError(err, `Couldn't load ${section.toLowerCase()}`)
    console.warn('[project-load] section fallback applied', {
      section,
      timeout_ms: timeoutMs,
      error_message: message,
    })

    return {
      ok: false,
      section,
      value: fallback,
      message,
    }
  }
}

/**
 * Load Shell project sections in parallel with per-section fallbacks.
 */
function createCriticalProjectSelectionRequests(projectId, ipc) {
  return {
    detail: withFallback('Project details', ipc.getProject(projectId), null),
    latest: withFallback('Latest session', ipc.getLatestSession(projectId), null),
    sessionList: withFallback('Session history', ipc.listSessions(projectId, 10), []),
  }
}

function createDeferredProjectSelectionRequests(projectId, ipc) {
  return {
    commits: withFallback('Recent commits', ipc.getRecentCommits(projectId, 10), []),
    readme: withFallback('README', ipc.getReadme(projectId), null),
    rels: withFallback('Relationships', ipc.getRelationships(projectId), []),
  }
}

export function createProjectSelectionRequests(projectId, ipc) {
  return {
    ...createCriticalProjectSelectionRequests(projectId, ipc),
    ...createDeferredProjectSelectionRequests(projectId, ipc),
  }
}

async function resolveCriticalProjectSelectionData(projectId, ipc) {
  const requests = createCriticalProjectSelectionRequests(projectId, ipc)
  const [detail, latest, sessionList] = await Promise.all([
    requests.detail,
    requests.latest,
    requests.sessionList,
  ])

  return { detail, latest, sessionList }
}

export async function loadDeferredProjectSelectionData(projectId, ipc) {
  const requests = createDeferredProjectSelectionRequests(projectId, ipc)
  const [commits, readme, rels] = await Promise.all([
    requests.commits,
    requests.readme,
    requests.rels,
  ])

  return { commits, readme, rels }
}

async function resolveProjectSelectionData(projectId, ipc) {
  const [critical, deferred] = await Promise.all([
    resolveCriticalProjectSelectionData(projectId, ipc),
    loadDeferredProjectSelectionData(projectId, ipc),
  ])
  return { ...critical, ...deferred }
}

/**
 * Load all project sections and return a resolved object.
 * Keep this helper for call sites/tests that still need an all-at-once payload.
 */
export function loadProjectSelectionData(projectId, ipc, options = {}) {
  const debounceMs = Number.isFinite(options.debounceMs)
    ? Math.max(0, options.debounceMs)
    : PROJECT_SELECTION_DEBOUNCE_MS

  if (debounceMs === 0) {
    return resolveProjectSelectionData(projectId, ipc)
  }

  if (scheduledSelectionBatch?.timerId) {
    clearTimeout(scheduledSelectionBatch.timerId)
  }

  return new Promise((resolve, reject) => {
    const waiters = scheduledSelectionBatch?.waiters ?? []
    waiters.push({ resolve, reject })

    scheduledSelectionBatch = {
      projectId,
      ipc,
      waiters,
      timerId: setTimeout(async () => {
        const batch = scheduledSelectionBatch
        scheduledSelectionBatch = null
        if (!batch) return

        try {
          const result = await resolveProjectSelectionData(batch.projectId, batch.ipc)
          batch.waiters.forEach((waiter) => waiter.resolve(result))
        } catch (error) {
          batch.waiters.forEach((waiter) => waiter.reject(error))
        }
      }, debounceMs),
    }
  })
}

export function loadCriticalProjectSelectionData(projectId, ipc, options = {}) {
  const debounceMs = Number.isFinite(options.debounceMs)
    ? Math.max(0, options.debounceMs)
    : PROJECT_SELECTION_DEBOUNCE_MS

  if (debounceMs === 0) {
    return resolveCriticalProjectSelectionData(projectId, ipc)
  }

  if (scheduledCriticalSelectionBatch?.timerId) {
    clearTimeout(scheduledCriticalSelectionBatch.timerId)
  }

  return new Promise((resolve, reject) => {
    const waiters = scheduledCriticalSelectionBatch?.waiters ?? []
    waiters.push({ resolve, reject })

    scheduledCriticalSelectionBatch = {
      projectId,
      ipc,
      waiters,
      timerId: setTimeout(async () => {
        const batch = scheduledCriticalSelectionBatch
        scheduledCriticalSelectionBatch = null
        if (!batch) return

        try {
          const result = await resolveCriticalProjectSelectionData(batch.projectId, batch.ipc)
          batch.waiters.forEach((waiter) => waiter.resolve(result))
        } catch (error) {
          batch.waiters.forEach((waiter) => waiter.reject(error))
        }
      }, debounceMs),
    }
  })
}
