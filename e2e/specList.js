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
export const specGroups = {
  // Individual tab workflows (read-only).
  content: ['overview-interactions.js', 'git-workflow.js', 'files-workflow.js'],
  // Cross-cutting features (read-only).
  features: ['tasks-workflow.js', 'cross-tab-navigation.js', 'search-workflow.js'],
  // App chrome and platform integration.
  shell: ['theme-and-shortcuts.js', 'context-menu.js', 'daemon-integration.js', 'screenshots.js'],
  // State mutation and validation.
  config: ['settings-persistence.js', 'project-lifecycle.js', 'error-handling.js'],
  // Standalone UI and detail-state capture.
  ui: ['critical-smoke.js', 'role-detail-screenshots.js'],
  wizard: ['first-run-wizard.js'],
  // Template storage, editing, and roster screenshots.
  templates: ['template-crud-ui.js', 'template-screenshots.js', 'templates.js'],
  // Team runtime and recovery workflows.
  mesh: ['mesh-recovery.js', 'mesh-screenshots.js', 'mesh-workflow.js'],
  // Real session actions and runtime session presentation.
  tmux: ['command-center-real-actions.js', 'session-management.js', 'regressions.js'],
}

/**
 * Lanes that spend a real, paid subscription when they run.
 *
 * They are never part of the default list: a suite run — including the bare
 * `bunx wdio run e2e/wdio.conf.js` the config header documents — must not cost
 * money for a lane nobody named. Start one by name instead:
 * `just test-e2e-spec compaction-codex-hooks`. WebdriverIO admits a spec named
 * as a file path on the command line whether or not the config lists it, so
 * leaving them out here does not make them unrunnable — only unbookable.
 *
 * All of them drive a real Codex subscription and must never touch the
 * operator's own `~/.codex`, so naming one on the command line is also what
 * tells `wdio.conf.js` to build the scratch `CODEX_HOME` for the session.
 */
export const CODEX_SCRATCH_SPECS = [
  'compaction-codex-hooks.js',
  'managed-stage-codex.js',
  'managed-stage-deadline.js',
  'managed-stage-parallel.js',
]
export const paidSpecs = [...CODEX_SCRATCH_SPECS]
export const captureSpecs = ['general-screenshots.js', 'readme-screenshots.js']

/** Spec files present in `specsDir`, sorted. */
export function listSpecFiles(specsDir) {
  return readdirSync(specsDir).filter(name => name.endsWith('.js')).sort()
}

// Build the sealed spec list. Each group becomes one worker session. Paid lanes
// are named on the command line; every other file must belong to a named group.
export function buildSpecList(specsDir, specFiles = listSpecFiles(specsDir)) {
  const excluded = new Set([...paidSpecs, ...captureSpecs])
  const allFiles = specFiles.filter(name => !excluded.has(name))
  const groups = Object.values(specGroups)
  const knownSpecs = new Set(groups.flat())
  const ungrouped = allFiles.filter(name => !knownSpecs.has(name))
  if (ungrouped.length > 0) {
    throw new Error(
      `Ungrouped E2E specs: ${ungrouped.join(', ')}. ` +
      'Add each file to a named specGroups group, paidSpecs, or captureSpecs.'
    )
  }

  // A focused fixture list can include only part of the real manifest.
  return groups
    .map(group => group.filter(name => allFiles.includes(name)).map(name => resolve(specsDir, name)))
    .filter(group => group.length > 0)
}
