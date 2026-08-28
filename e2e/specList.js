/**
 * Which spec files the default WDIO run executes, and in which worker sessions.
 *
 * Split out of `wdio.conf.js` so the one rule that costs money — a paid lane is
 * never in the default list — can be asserted without booting WebdriverIO.
 */

import { readdirSync } from 'node:fs'
import { resolve } from 'node:path'

// Spec groups by app layer. Each sub-array = one worker session = one app instance.
// Groups are SEALED: new specs form new groups, never expand existing ones.
// See the `wdio.conf.js` header comment for the layer model.
export const specGroups = [
  // Group 1: Content — individual tab workflows (read-only)
  ['overview-interactions.js', 'git-workflow.js', 'files-workflow.js'],
  // Group 2: Features — cross-cutting features (read-only)
  ['tasks-workflow.js', 'cross-tab-navigation.js', 'search-workflow.js'],
  // Group 3: Shell — app chrome & platform integration
  ['theme-and-shortcuts.js', 'context-menu.js', 'daemon-integration.js'],
  // Group 4: Config — state mutation & validation
  ['settings-persistence.js', 'project-lifecycle.js', 'error-handling.js'],
  // Group 5: Guards — regressions & visual capture
  ['regressions.js', 'screenshots.js', 'readme-screenshots.js'],
]

/**
 * Lanes that spend a real, paid subscription when they run.
 *
 * They are never part of the default list: a suite run — including the bare
 * `bunx wdio run e2e/wdio.conf.js` the config header documents — must not cost
 * money for a lane nobody named. Start one by name instead:
 * `just test-e2e-spec compaction-codex-hooks`. WebdriverIO admits a spec named
 * as a file path on the command line whether or not the config lists it, so
 * leaving them out here does not make them unrunnable — only unbookable.
 */
export const CODEX_SCRATCH_SPEC = 'compaction-codex-hooks.js'
export const paidSpecs = [CODEX_SCRATCH_SPEC]

/** Spec files present in `specsDir`, sorted. */
export function listSpecFiles(specsDir) {
  return readdirSync(specsDir).filter(name => name.endsWith('.js')).sort()
}

// Build the spec list. Each group becomes a sub-array (one worker session each).
// Specs not in any group are collected into an "ungrouped" session at the end.
// Paid lanes are in neither: they are named on the command line or not run.
export function buildSpecList(specsDir, specFiles = listSpecFiles(specsDir)) {
  const paid = new Set(paidSpecs)
  const allFiles = specFiles.filter(name => !paid.has(name))

  // Resolve each group, dropping missing files
  const groups = specGroups
    .map(group => group.filter(name => allFiles.includes(name)).map(name => resolve(specsDir, name)))
    .filter(group => group.length > 0)

  // Any new specs not in a defined group run as a catch-all session
  const knownSpecs = new Set(specGroups.flat())
  const ungrouped = allFiles
    .filter(name => !knownSpecs.has(name))
    .map(name => resolve(specsDir, name))
  if (ungrouped.length > 0) groups.push(ungrouped)

  return groups
}
