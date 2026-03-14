import { formatUserFacingError } from './format.js'

const PROJECT_SECTION_TIMEOUT_MS = 5000
const PROJECT_SELECTION_DEBOUNCE_MS = 25
let scheduledSelectionBatch = null
const inflightSelectionRequests = new Map()

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

function isTransientDaemonProjectLoadError(message) {
  const normalized = String(message ?? '').toLowerCase()
  return (
    normalized.includes('without a connected daemon') ||
    normalized.includes('daemon transport error') ||
    normalized.includes('daemon protocol error: daemon error [auth_failed]')
  )
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
      retryable_on_daemon_reconnect: isTransientDaemonProjectLoadError(message),
    })

    return {
      ok: false,
      section,
      value: fallback,
      message,
      retryableOnDaemonReconnect: isTransientDaemonProjectLoadError(message),
    }
  }
}

export function classifyProjectLoadResults(results, { deferRetryableIssues = false } = {}) {
  const issues = results
    .filter((result) => !result.ok)
    .map((result) => ({
      section: result.section,
      message: result.message,
      retryableOnDaemonReconnect: Boolean(result.retryableOnDaemonReconnect),
    }))

  return {
    issues,
    pendingRetry:
      deferRetryableIssues && issues.some((issue) => issue.retryableOnDaemonReconnect),
    visibleIssues: deferRetryableIssues
      ? issues.filter((issue) => !issue.retryableOnDaemonReconnect)
      : issues,
  }
}

/**
 * Load Shell project sections in parallel with per-section fallbacks.
 */
export function createProjectSelectionRequests(projectId, ipc) {
  return {
    detail: withFallback('Project details', ipc.getProject(projectId), null),
    commits: withFallback('Recent commits', ipc.getRecentCommits(projectId, 10), []),
    latest: withFallback('Latest session', ipc.getLatestSession(projectId), null),
    sessionList: withFallback('Session history', ipc.listSessions(projectId, 10), []),
    readme: withFallback('README', ipc.getReadme(projectId), null),
    rels: withFallback('Relationships', ipc.getRelationships(projectId), []),
  }
}

async function resolveProjectSelectionData(projectId, ipc) {
  const requests = createProjectSelectionRequests(projectId, ipc)
  const [detail, commits, latest, sessionList, readme, rels] = await Promise.all([
    requests.detail,
    requests.commits,
    requests.latest,
    requests.sessionList,
    requests.readme,
    requests.rels,
  ])

  return { detail, commits, latest, sessionList, readme, rels }
}

function startProjectSelectionDataLoad(projectId, ipc) {
  if (inflightSelectionRequests.has(projectId)) {
    return inflightSelectionRequests.get(projectId)
  }

  const request = resolveProjectSelectionData(projectId, ipc).finally(() => {
    inflightSelectionRequests.delete(projectId)
  })
  inflightSelectionRequests.set(projectId, request)
  return request
}

/**
 * Load all project sections and return a resolved object.
 * Keep this helper for call sites/tests that still need an all-at-once payload.
 */
export function loadProjectSelectionData(projectId, ipc) {
  if (inflightSelectionRequests.has(projectId)) {
    return inflightSelectionRequests.get(projectId)
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
          const result = await startProjectSelectionDataLoad(batch.projectId, batch.ipc)
          batch.waiters.forEach((waiter) => waiter.resolve(result))
        } catch (error) {
          batch.waiters.forEach((waiter) => waiter.reject(error))
        }
      }, PROJECT_SELECTION_DEBOUNCE_MS),
    }
  })
}

/**
 * Speculatively start a full project-selection batch so a subsequent click can
 * reuse the in-flight result without changing visible completeness.
 */
export function prefetchProjectSelectionData(projectId, ipc) {
  if (!projectId) return Promise.resolve(null)
  return startProjectSelectionDataLoad(projectId, ipc)
}

export function resetProjectSelectionStateForTests() {
  if (scheduledSelectionBatch?.timerId) {
    clearTimeout(scheduledSelectionBatch.timerId)
  }
  scheduledSelectionBatch = null
  inflightSelectionRequests.clear()
}
