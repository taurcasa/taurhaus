export function projectBasename(path) {
  return String(path || '')
    .split(/[\\/]+/)
    .filter(Boolean)
    .at(-1) || 'project'
}

/**
 * Normalize mixed project values into `{ id, label }`.
 *
 * Supports:
 * - string project paths/ids
 * - object values with `path`, `id`, and/or `name`
 */
export function normalizeProjectOption(project, options = {}) {
  const {
    stringLabel = 'basename',
    objectFallbackLabel = 'basename',
    unnamedLabel = 'Unnamed project',
  } = options

  if (typeof project === 'string') {
    const id = project
    const label = stringLabel === 'raw' ? project : projectBasename(project)
    return { id, label }
  }

  if (project && typeof project === 'object') {
    const id = project.path || project.id || project.name || ''
    if (!id) {
      return { id: '', label: '' }
    }
    const fallbackLabel =
      objectFallbackLabel === 'raw' ? id : projectBasename(project.path || project.id)
    const label = project.name || fallbackLabel || unnamedLabel
    return { id, label }
  }

  return { id: '', label: '' }
}
