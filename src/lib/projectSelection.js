import { formatUserFacingError } from './format.js'

const PROJECT_SECTION_TIMEOUT_MS = 5000

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
export function projectLoadErrorMessage(err) {
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

/**
 * Load all project sections and return a resolved object.
 * Keep this helper for call sites/tests that still need an all-at-once payload.
 */
export async function loadProjectSelectionData(projectId, ipc) {
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
