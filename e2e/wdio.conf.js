/**
 * WebdriverIO configuration for Tauri e2e tests.
 *
 * BATCHED PERSISTENT MODE: Specs are split into small groups (3 each).
 * Each group runs in its own worker session = its own app instance.
 * This keeps per-operation latency low (~95ms vs ~165ms with all specs
 * in one session) while avoiding per-spec app startup overhead.
 *
 * Groups are organized by app layer (inside-out):
 *   1. Content  — individual tab workflows (read-only)
 *   2. Features — cross-cutting features (read-only)
 *   3. Shell    — app chrome & platform integration
 *   4. Config   — state mutation & validation
 *   5. Guards   — regressions & visual capture
 *
 * Groups are SEALED — new specs form new groups, never expand existing ones.
 *
 * Uses tauri-driver as the WebDriver bridge, which delegates to the
 * platform's native WebDriver (WebKitWebDriver on Linux, msedgedriver on Windows).
 *
 * Prerequisites:
 *   cargo install tauri-driver --locked
 *   WebKitWebDriver available (Linux) or msedgedriver on PATH (Windows)
 *
 * Usage:
 *   just test-e2e                        (builds + runs full suite)
 *   just test-e2e-spec search-workflow   (single spec in its own session)
 *   E2E_SKIP_BUILD=1 npx wdio run e2e/wdio.conf.js  (skip build)
 */

import { spawn } from 'node:child_process'
import { resolve } from 'node:path'
import { readdirSync } from 'node:fs'

const projectRoot = resolve(import.meta.dirname, '..')
const specsDir = resolve(import.meta.dirname, 'specs')

const binaryPath = resolve(projectRoot, 'src-tauri', 'target', 'debug', 'taurhaus')
const wdioLogLevel = process.env.E2E_WDIO_LOG_LEVEL || 'error'

// Spec groups by app layer. Each sub-array = one worker session = one app instance.
// Groups are SEALED: new specs form new groups, never expand existing ones.
// See header comment for the layer model.
const specGroups = [
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

// Build the spec list. Each group becomes a sub-array (one worker session each).
// Specs not in any group are collected into an "ungrouped" session at the end.
function buildSpecList() {
  const allFiles = readdirSync(specsDir).filter(f => f.endsWith('.js')).sort()

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

let tauriDriver

export const config = {
  // ── Runner ──────────────────────────────────────────────────────────────
  runner: 'local',
  hostname: '127.0.0.1',
  port: 4444,
  maxInstances: 1,
  // WDIO defaults to "info", which logs every COMMAND/DATA/RESULT triplet.
  // In the persistent 14-spec suite that adds thousands of synchronous writes
  // and noticeably inflates per-command latency. Keep verbose logs opt-in.
  logLevel: wdioLogLevel,

  // ── Specs ───────────────────────────────────────────────────────────────
  // Multiple sub-arrays: each group gets its own app instance.
  specs: buildSpecList(),

  // ── Capabilities ────────────────────────────────────────────────────────
  capabilities: [
    {
      'tauri:options': {
        application: binaryPath,
      },
    },
  ],

  // ── Timeouts ───────────────────────────────────────────────────────────
  // Global defaults for all waitFor* commands (waitForExist, waitForDisplayed, etc.)
  // Without these, wdio uses its own defaults (which may poll too slowly).
  waitforTimeout: 5_000,
  waitforInterval: 50,

  // ── Framework ───────────────────────────────────────────────────────────
  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 60_000,
  },

  // ── Reporter ────────────────────────────────────────────────────────────
  reporters: ['spec'],

  // ── Hooks ───────────────────────────────────────────────────────────────

  /**
   * Build the Tauri debug binary before running tests.
   * Skip with E2E_SKIP_BUILD=1 if you already have a fresh build.
   */
  async onPrepare() {
    if (process.env.E2E_SKIP_BUILD === '1') {
      console.log('[e2e] Skipping build (E2E_SKIP_BUILD=1)')
      return
    }

    console.log('[e2e] Building Tauri debug binary...')
    return new Promise((resolve, reject) => {
      const build = spawn('npx', ['tauri', 'build', '--debug', '--no-bundle'], {
        cwd: projectRoot,
        stdio: 'inherit',
      })
      build.on('close', (code) => {
        if (code === 0) {
          console.log('[e2e] Build complete')
          resolve()
        } else {
          reject(new Error(`Build failed with exit code ${code}`))
        }
      })
    })
  },

  /**
   * Start tauri-driver before each worker session.
   * With batched groups, this runs once per group (5 times for the full suite).
   */
  async beforeSession() {
    return new Promise((resolve) => {
      tauriDriver = spawn('tauri-driver', [], {
        stdio: [null, process.stdout, process.stderr],
      })

      // Give tauri-driver time to start its WebDriver server
      setTimeout(resolve, 500)
    })
  },

  /**
   * Kill tauri-driver after each worker session ends.
   */
  async afterSession() {
    if (tauriDriver) {
      tauriDriver.kill()
      tauriDriver = null
    }
  },
}
