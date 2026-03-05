import { formatUserFacingError } from './format.js'

/**
 * Normalize unknown errors for degraded project-load sections.
 */
export function projectLoadErrorMessage(err) {
  return formatUserFacingError(err, "Couldn't load project data")
}

/**
 * Resolve a project-load section and fall back to a safe value on error.
 */
export async function withFallback(section, promise, fallback) {
  try {
    const value = await promise
    return { ok: true, section, value, message: null }
  } catch (err) {
    return {
      ok: false,
      section,
      value: fallback,
      message: formatUserFacingError(err, `Couldn't load ${section.toLowerCase()}`),
    }
  }
}

/**
 * Load Shell project sections in parallel with per-section fallbacks.
 */
export async function loadProjectSelectionData(projectId, ipc) {
  const [detail, commits, latest, sessionList, readme, rels] = await Promise.all([
    withFallback('Project details', ipc.getProject(projectId), null),
    withFallback('Recent commits', ipc.getRecentCommits(projectId, 10), []),
    withFallback('Latest session', ipc.getLatestSession(projectId), null),
    withFallback('Session history', ipc.listSessions(projectId, 10), []),
    withFallback('README', ipc.getReadme(projectId), null),
    withFallback('Relationships', ipc.getRelationships(projectId), []),
  ])

  return { detail, commits, latest, sessionList, readme, rels }
}
