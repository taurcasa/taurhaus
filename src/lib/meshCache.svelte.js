import { SvelteMap } from 'svelte/reactivity'

export const meshSnapshots = new SvelteMap()

function normalizeProjectPath(projectPath) {
  const value = String(projectPath ?? '').trim()
  return value.length > 0 ? value : ''
}

export function getMeshCache(projectPath) {
  const key = normalizeProjectPath(projectPath)
  if (!key) return null
  return meshSnapshots.get(key) ?? null
}

export function setMeshCache(projectPath, snapshot) {
  const key = normalizeProjectPath(projectPath)
  if (!key) return
  meshSnapshots.set(key, snapshot)
}

export function clearMeshCache(projectPath) {
  const key = normalizeProjectPath(projectPath)
  if (!key) return
  meshSnapshots.delete(key)
}

export function resetMeshCache() {
  meshSnapshots.clear()
}
