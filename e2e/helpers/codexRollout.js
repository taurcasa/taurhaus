/**
 * Reading Codex's own rollout transcripts under a scratch `CODEX_HOME`.
 *
 * These are the harness's record of what happened, independent of anything
 * taurhaus logged about itself: whether Codex compacted at all, and whether the
 * restored-context card the hook returned actually reached the conversation.
 */

import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

/** Every `rollout-*.jsonl` under `<codexHome>/sessions`, at any depth. */
export function rolloutPaths(codexHome) {
  const found = []
  const walk = (dir) => {
    let entries
    try {
      entries = readdirSync(dir, { withFileTypes: true })
    } catch {
      return
    }
    for (const entry of entries) {
      const path = join(dir, entry.name)
      if (entry.isDirectory()) walk(path)
      else if (entry.name.startsWith('rollout-') && entry.name.endsWith('.jsonl')) found.push(path)
    }
  }
  walk(join(codexHome, 'sessions'))
  return found
}

/**
 * Compaction boundaries Codex wrote across `paths`.
 *
 * Summed over every rollout rather than read from the newest one: a compaction
 * that starts a new session would otherwise read as the count going down. A
 * path that cannot be read contributes nothing — these are polled while Codex
 * is writing them.
 */
export function countCompactionBoundaries(paths) {
  return (paths ?? []).reduce((total, path) => {
    try {
      return total + (readFileSync(path, 'utf8').match(/"type"\s*:\s*"compacted"/g) ?? []).length
    } catch {
      return total
    }
  }, 0)
}

/** The transcripts among `paths` whose text contains `marker`. */
export function pathsContainingMarker(paths, marker) {
  return (paths ?? []).filter((path) => {
    try {
      return readFileSync(path, 'utf8').includes(marker)
    } catch {
      return false
    }
  })
}
