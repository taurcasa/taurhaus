/**
 * Shared asset cache for project file content.
 *
 * Caches data URIs (images) and file content loaded via IPC so that
 * switching between projects doesn't re-read from disk every time.
 *
 * Invalidation is driven by the file watcher (Phase 5C) — when a file
 * changes on disk, the watcher calls invalidate() to clear the entry.
 * Until then, entries live for the session lifetime.
 */

const cache = new Map()
const MAX_CACHE_ENTRIES = 100

function key(projectId, relativePath) {
  return `${projectId}/${relativePath}`
}

/** Get a cached value, or null if not cached. */
export function get(projectId, relativePath) {
  const cacheKey = key(projectId, relativePath)
  if (!cache.has(cacheKey)) return null
  const value = cache.get(cacheKey)
  // Refresh recency for LRU ordering.
  cache.delete(cacheKey)
  cache.set(cacheKey, value)
  return value ?? null
}

/** Store a value in the cache. */
export function set(projectId, relativePath, data) {
  const cacheKey = key(projectId, relativePath)
  if (cache.has(cacheKey)) {
    cache.delete(cacheKey)
  }
  cache.set(cacheKey, data)

  while (cache.size > MAX_CACHE_ENTRIES) {
    const oldestKey = cache.keys().next().value
    if (!oldestKey) break
    cache.delete(oldestKey)
  }
}

/** Invalidate a single file (called by file watcher on change). */
export function invalidate(projectId, relativePath) {
  cache.delete(key(projectId, relativePath))
}

/** Invalidate all cached files for a project (project removed/re-registered). */
export function invalidateProject(projectId) {
  const prefix = `${projectId}/`
  for (const k of cache.keys()) {
    if (k.startsWith(prefix)) cache.delete(k)
  }
}

/** Clear the entire cache. */
export function clear() {
  cache.clear()
}

/** Number of cached entries (for testing/diagnostics). */
export function size() {
  return cache.size
}
