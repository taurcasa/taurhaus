export const SIDEBAR_GROUPS = [
  { key: 'active', label: 'ACTIVE' },
  { key: 'recent', label: 'RECENT' },
  { key: 'stale', label: 'STALE' },
  { key: 'dormant', label: 'DORMANT' },
]

const projectionCache = new Map()

function normalizeQuery(filterQuery) {
  return String(filterQuery || '').trim().toLowerCase()
}

function computeFilteredProjects(projects, query) {
  if (!query) return projects
  return projects.filter((project) => String(project?.name || '').toLowerCase().includes(query))
}

/**
 * Build memoized sidebar projection data from projects and filter query.
 *
 * Returns a stable object reference for the same projects-array identity and
 * normalized query, avoiding repeated per-render grouping/filtering work.
 */
export function buildSidebarProjection(projects, filterQuery = '') {
  const projectList = Array.isArray(projects) ? projects : []
  const query = normalizeQuery(filterQuery)

  let byQuery = projectionCache.get(projectList)
  if (!byQuery) {
    byQuery = new Map()
    projectionCache.set(projectList, byQuery)
  }

  const cached = byQuery.get(query)
  if (cached) return cached

  const filtered = computeFilteredProjects(projectList, query)
  const grouped = SIDEBAR_GROUPS.map((group) => ({
    ...group,
    items: filtered.filter((project) => project.activity_state === group.key),
  }))

  const projection = {
    filtered,
    grouped,
  }

  byQuery.set(query, projection)
  return projection
}

export function __resetSidebarProjectionCacheForTests() {
  projectionCache.clear()
}
