import { SvelteMap } from 'svelte/reactivity'
import { normalizeProjectPath } from './pathUtils.js'

const meshSnapshots = new SvelteMap()

function nowMs() {
  return Date.now()
}

export function getMeshCacheEntry(projectPath) {
  const key = normalizeProjectPath(projectPath)
  if (!key) return null
  const entry = meshSnapshots.get(key) ?? null
  if (!entry) return null
  if (typeof entry === 'object' && entry !== null && 'snapshot' in entry) {
    return entry
  }
  return {
    snapshot: entry,
    cachedAtMs: 0,
  }
}

export function getMeshCache(projectPath) {
  return getMeshCacheEntry(projectPath)?.snapshot ?? null
}

export function setMeshCache(projectPath, snapshot) {
  const key = normalizeProjectPath(projectPath)
  if (!key) return
  meshSnapshots.set(key, {
    snapshot,
    cachedAtMs: nowMs(),
  })
}

export function clearMeshCache(projectPath) {
  const key = normalizeProjectPath(projectPath)
  if (!key) return
  meshSnapshots.delete(key)
}

export function resetMeshCache() {
  meshSnapshots.clear()
}
