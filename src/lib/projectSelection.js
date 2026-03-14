import { formatUserFacingError } from './format.js'

const PROJECT_SECTION_TIMEOUT_MS = 5000
const PROJECT_SELECTION_DEBOUNCE_MS = 25
let scheduledSelectionBatch = null
const inflightSelectionRequests = new Map()
function resolveScheduledSelectionBatchWith(request) {
  if (!scheduledSelectionBatch) {
    return request
  }

  const batch = scheduledSelectionBatch
  scheduledSelectionBatch = null
  if (batch.timerId) {
    clearTimeout(batch.timerId)
  }

  request
    .then((result) => {
      batch.waiters.forEach((waiter) => waiter.resolve(result))
    })
    .catch((error) => {
      batch.waiters.forEach((waiter) => waiter.reject(error))
    })

  return request
}
const PROJECT_SELECTION_SECTIONS = [
  {
    key: 'detail',
    label: 'Project details',
    fallback: null,
    request: (projectId, ipc) => ipc.getProject(projectId),
    providerRoute: () => 'db',
  },
  {
    key: 'commits',
    label: 'Recent commits',
    fallback: [],
    request: (projectId, ipc) => ipc.getRecentCommits(projectId, 10),
    providerRoute: ({ projectPath, daemonStatus }) => classifyProviderRoute(projectPath, daemonStatus),
  },
  {
    key: 'latest',
    label: 'Latest session',
    fallback: null,
    request: (projectId, ipc) => ipc.getLatestSession(projectId),
    providerRoute: () => 'db',
  },
  {
    key: 'sessionList',
    label: 'Session history',
    fallback: [],
    request: (projectId, ipc) => ipc.listSessions(projectId, 10),
    providerRoute: () => 'db',
  },
  {
    key: 'readme',
    label: 'README',
    fallback: null,
    request: (projectId, ipc) => ipc.getReadme(projectId),
    providerRoute: ({ projectPath, daemonStatus }) => classifyProviderRoute(projectPath, daemonStatus),
  },
  {
    key: 'rels',
    label: 'Relationships',
    fallback: [],
    request: (projectId, ipc) => ipc.getRelationships(projectId),
    providerRoute: () => 'db',
  },
]

function nowMs() {
  if (typeof performance !== 'undefined' && typeof performance.now === 'function') {
    return performance.now()
  }
  return Date.now()
}

function isWslUncPath(path) {
  const normalized = String(path ?? '').trim().replace(/\//g, '\\').toLowerCase()
  return normalized.startsWith('\\\\wsl$\\') || normalized.startsWith('\\\\wsl.localhost\\')
}

function classifyProviderRoute(projectPath, daemonStatus) {
  if (!isWslUncPath(projectPath)) {
    return 'local_provider'
  }
  if (daemonStatus === 'connected' || daemonStatus === 'busy') {
    return 'daemon_provider'
  }
  if (daemonStatus === null || daemonStatus === undefined) {
    return 'provider_route_unknown'
  }
  return 'local_provider_fallback'
}

function batchFlags(batchKind) {
  return {
    batch_kind: batchKind,
    blocking: batchKind === 'blocking',
    deferred: batchKind === 'deferred',
  }
}

function logSelectionEvent(logger, level, message, context) {
  const sink = logger?.[level]
  if (typeof sink !== 'function') return
  sink(message, context)
}

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
  timeoutMs = PROJECT_SECTION_TIMEOUT_MS,
  instrumentation = {}
) {
  const {
    logger = console,
    projectId = null,
    sectionKey = section,
    providerRoute = 'provider_route_unknown',
    batchKind = 'blocking',
  } = instrumentation
  const startedAt = nowMs()

  try {
    const value = await withTimeout(promise, timeoutMs, section)
    const durationMs = Number((nowMs() - startedAt).toFixed(1))
    logSelectionEvent(logger, 'info', '[project-selection] section completed', {
      event: 'project.selection.section.completed',
      project_id: projectId,
      section,
      section_key: sectionKey,
      provider_route: providerRoute,
      duration_ms: durationMs,
      timeout_ms: timeoutMs,
      ok: true,
      retryable_on_daemon_reconnect: false,
      ...batchFlags(batchKind),
    })
    return {
      ok: true,
      section,
      sectionKey,
      value,
      message: null,
      durationMs,
      providerRoute,
      batchKind,
    }
  } catch (err) {
    const durationMs = Number((nowMs() - startedAt).toFixed(1))
    const message = formatUserFacingError(err, `Couldn't load ${section.toLowerCase()}`)
    const retryableOnDaemonReconnect = isTransientDaemonProjectLoadError(message)
    console.warn('[project-load] section fallback applied', {
      section,
      timeout_ms: timeoutMs,
      error_message: message,
      retryable_on_daemon_reconnect: retryableOnDaemonReconnect,
    })
    logSelectionEvent(logger, 'warn', '[project-selection] section completed with fallback', {
      event: 'project.selection.section.completed',
      project_id: projectId,
      section,
      section_key: sectionKey,
      provider_route: providerRoute,
      duration_ms: durationMs,
      timeout_ms: timeoutMs,
      ok: false,
      retryable_on_daemon_reconnect: retryableOnDaemonReconnect,
      error_message: message,
      ...batchFlags(batchKind),
    })

    return {
      ok: false,
      section,
      sectionKey,
      value: fallback,
      message,
      retryableOnDaemonReconnect,
      durationMs,
      providerRoute,
      batchKind,
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
export function createProjectSelectionRequests(projectId, ipc, options = {}) {
  const requests = {}
  for (const section of PROJECT_SELECTION_SECTIONS) {
    requests[section.key] = withFallback(
      section.label,
      section.request(projectId, ipc),
      section.fallback,
      PROJECT_SECTION_TIMEOUT_MS,
      {
        logger: options.logger ?? console,
        projectId,
        sectionKey: section.key,
        providerRoute: section.providerRoute({
          projectPath: options.projectPath,
          daemonStatus: options.daemonStatus,
        }),
        batchKind: options.batchKind ?? 'blocking',
      }
    )
  }
  return requests
}

async function resolveProjectSelectionData(projectId, ipc, options = {}) {
  const logger = options.logger ?? console
  const batchKind = options.batchKind ?? 'blocking'
  const startedAt = nowMs()
  logSelectionEvent(logger, 'info', '[project-selection] batch started', {
    event: 'project.selection.batch.started',
    project_id: projectId,
    section_count: PROJECT_SELECTION_SECTIONS.length,
    project_path: options.projectPath ?? null,
    daemon_status: options.daemonStatus ?? null,
    ...batchFlags(batchKind),
  })

  const requests = createProjectSelectionRequests(projectId, ipc, options)
  const [detail, commits, latest, sessionList, readme, rels] = await Promise.all([
    requests.detail,
    requests.commits,
    requests.latest,
    requests.sessionList,
    requests.readme,
    requests.rels,
  ])

  const results = [detail, commits, latest, sessionList, readme, rels]
  const failedSections = results
    .filter((result) => !result.ok)
    .map((result) => result.sectionKey)
  const retryableSections = results
    .filter((result) => result.retryableOnDaemonReconnect)
    .map((result) => result.sectionKey)
  logSelectionEvent(logger, 'info', '[project-selection] batch completed', {
    event: 'project.selection.batch.completed',
    project_id: projectId,
    duration_ms: Number((nowMs() - startedAt).toFixed(1)),
    section_count: PROJECT_SELECTION_SECTIONS.length,
    failed_section_count: failedSections.length,
    failed_sections: failedSections,
    retryable_section_count: retryableSections.length,
    retryable_sections: retryableSections,
    ...batchFlags(batchKind),
  })

  return { detail, commits, latest, sessionList, readme, rels }
}

function startProjectSelectionDataLoad(projectId, ipc, options = {}) {
  if (inflightSelectionRequests.has(projectId)) {
    return inflightSelectionRequests.get(projectId)
  }

  const request = resolveProjectSelectionData(projectId, ipc, options).finally(() => {
    inflightSelectionRequests.delete(projectId)
  })
  inflightSelectionRequests.set(projectId, request)
  return request
}

/**
 * Load all project sections and return a resolved object.
 * Keep this helper for call sites/tests that still need an all-at-once payload.
 */
export function loadProjectSelectionData(projectId, ipc, options = {}) {
  if (inflightSelectionRequests.has(projectId)) {
    return resolveScheduledSelectionBatchWith(inflightSelectionRequests.get(projectId))
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
      options,
      waiters,
      timerId: setTimeout(async () => {
        const batch = scheduledSelectionBatch
        scheduledSelectionBatch = null
        if (!batch) return

        try {
          const result = await startProjectSelectionDataLoad(batch.projectId, batch.ipc, batch.options)
          batch.waiters.forEach((waiter) => waiter.resolve(result))
        } catch (error) {
          batch.waiters.forEach((waiter) => waiter.reject(error))
        }
      }, PROJECT_SELECTION_DEBOUNCE_MS),
      options,
    }
  })
}

/**
 * Load the non-critical project sections after the shell has already switched
 * to the next project using the lightweight sidebar snapshot.
 */
export function loadDeferredProjectSelectionData(projectId, ipc, options = {}) {
  return loadProjectSelectionData(projectId, ipc, {
    ...options,
    batchKind: options.batchKind ?? 'deferred',
  })
}

/**
 * Speculatively start a full project-selection batch so a subsequent click can
 * reuse the in-flight result without changing visible completeness.
 */
export function prefetchProjectSelectionData(projectId, ipc, options = {}) {
  if (!projectId) return Promise.resolve(null)
  return startProjectSelectionDataLoad(projectId, ipc, {
    ...options,
    batchKind: options.batchKind ?? 'deferred',
  })
}

export function resetProjectSelectionStateForTests() {
  if (scheduledSelectionBatch?.timerId) {
    clearTimeout(scheduledSelectionBatch.timerId)
  }
  scheduledSelectionBatch = null
  inflightSelectionRequests.clear()
}
