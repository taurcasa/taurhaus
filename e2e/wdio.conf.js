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

import { spawn, spawnSync } from 'node:child_process'
import net from 'node:net'
import { resolve } from 'node:path'
import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'

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
let sessionTempRoot = null

function runGitOrThrow(cwd, args, errorMessage) {
  const result = spawnSync('git', args, {
    cwd,
    stdio: 'ignore',
    env: {
      ...process.env,
      GIT_AUTHOR_NAME: 'taurhaus-e2e',
      GIT_AUTHOR_EMAIL: 'e2e@taurhaus.local',
      GIT_COMMITTER_NAME: 'taurhaus-e2e',
      GIT_COMMITTER_EMAIL: 'e2e@taurhaus.local',
    },
  })
  if (result.status !== 0) {
    throw new Error(errorMessage)
  }
}

function createFixtureRepo(repoPath, { title, withHistory = true }) {
  mkdirSync(repoPath, { recursive: true })
  mkdirSync(`${repoPath}/src/utils`, { recursive: true })
  mkdirSync(`${repoPath}/docs`, { recursive: true })
  mkdirSync(`${repoPath}/assets`, { recursive: true })
  mkdirSync(`${repoPath}/node_modules/fake-lib`, { recursive: true })

  writeFileSync(
    `${repoPath}/README.md`,
    `# ${title}

Sample repository used by taurhaus E2E tests.

## Quick Start

- Open Files tab
- Open Git tab
- Open Search and query README
`
  )
  writeFileSync(`${repoPath}/.gitignore`, 'node_modules/\n*.tmp\n')
  writeFileSync(
    `${repoPath}/src/main.js`,
    `import { formatStatus } from './utils/format.js'

export function runApp() {
  return formatStatus('ready')
}
`
  )
  writeFileSync(
    `${repoPath}/src/utils/format.js`,
    `export function formatStatus(value) {
  return \`status:\${value}\`
}
`
  )
  writeFileSync(
    `${repoPath}/docs/guide.md`,
    `# Guide

This guide exists so markdown rendering can be tested.

## Notes

Search should find README and guide references.
`
  )
  writeFileSync(
    `${repoPath}/assets/logo.svg`,
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 20"><text x="2" y="14">taurhaus</text></svg>\n'
  )
  writeFileSync(`${repoPath}/node_modules/fake-lib/index.js`, 'module.exports = "ignored";\n')

  runGitOrThrow(repoPath, ['init', '-q'], 'Failed to initialize e2e fixture git repository')

  // Commit 1: baseline project structure.
  runGitOrThrow(repoPath, ['add', '.'], 'Failed to stage initial e2e fixture files')
  runGitOrThrow(repoPath, ['commit', '-q', '-m', 'chore: initialize e2e fixture project'], 'Failed to create initial e2e fixture commit')

  if (!withHistory) return

  // Commit 2: code + docs update for diff and search coverage.
  writeFileSync(
    `${repoPath}/src/main.js`,
    `import { formatStatus } from './utils/format.js'

export function runApp(mode = 'runtime') {
  return formatStatus(\`ready:\${mode}\`)
}
`
  )
  writeFileSync(
    `${repoPath}/docs/guide.md`,
    `# Guide

Updated guide content for E2E diff coverage.

## Runtime

The runtime mode keeps agents connected.
`
  )
  runGitOrThrow(repoPath, ['add', '.'], 'Failed to stage second e2e fixture commit')
  runGitOrThrow(repoPath, ['commit', '-q', '-m', 'feat: add runtime notes and formatter update'], 'Failed to create second e2e fixture commit')

  // Commit 3: additional file churn.
  writeFileSync(
    `${repoPath}/docs/changelog.md`,
    `# Changelog

## 0.1.0

- Added runtime documentation
- Improved fixture stability
`
  )
  runGitOrThrow(repoPath, ['add', '.'], 'Failed to stage third e2e fixture commit')
  runGitOrThrow(repoPath, ['commit', '-q', '-m', 'docs: add changelog for git history coverage'], 'Failed to create third e2e fixture commit')
}

function isPortOpen(host, port, timeoutMs = 250) {
  return new Promise((resolve) => {
    const socket = new net.Socket()
    let settled = false

    const finish = (result) => {
      if (settled) return
      settled = true
      socket.destroy()
      resolve(result)
    }

    socket.setTimeout(timeoutMs)
    socket.once('connect', () => finish(true))
    socket.once('timeout', () => finish(false))
    socket.once('error', () => finish(false))
    socket.connect(port, host)
  })
}

async function waitForWebDriverReady(host, port, timeoutMs = 5_000, intervalMs = 100) {
  const startedAt = Date.now()
  while (Date.now() - startedAt < timeoutMs) {
    if (await isPortOpen(host, port)) return
    await new Promise((resolve) => setTimeout(resolve, intervalMs))
  }
  throw new Error(`tauri-driver did not open ${host}:${port} within ${timeoutMs}ms`)
}

export const config = {
  // ── Runner ──────────────────────────────────────────────────────────────
  runner: 'local',
  hostname: '127.0.0.1',
  port: 4444,
  connectionRetryTimeout: 10_000,
  connectionRetryCount: 1,
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
    sessionTempRoot = mkdtempSync(`${tmpdir()}/taurhaus-e2e-${process.pid}-`)
    const tauriDataDir = `${sessionTempRoot}/app-data`
    const tauriClaudeDir = `${sessionTempRoot}/claude`
    const e2eProjectsDir = `${sessionTempRoot}/projects`
    const taurhausFixtureProject = `${e2eProjectsDir}/taurhaus`
    const ledgerFixtureProject = `${e2eProjectsDir}/ledger`
    mkdirSync(tauriDataDir, { recursive: true })
    mkdirSync(tauriClaudeDir, { recursive: true })

    createFixtureRepo(taurhausFixtureProject, { title: 'taurhaus fixture', withHistory: true })
    createFixtureRepo(ledgerFixtureProject, { title: 'ledger fixture', withHistory: false })

    process.env.E2E_PROJECTS_DIR = e2eProjectsDir
    process.env.E2E_TAURHAUS_PROJECT_PATH = taurhausFixtureProject

    tauriDriver = spawn('tauri-driver', [], {
      env: {
        ...process.env,
        TAURHAUS_DATA_DIR: tauriDataDir,
        TAURHAUS_CLAUDE_DIR: tauriClaudeDir,
      },
      stdio: [null, process.stdout, process.stderr],
    })

    await waitForWebDriverReady('127.0.0.1', 4444)
  },

  /**
   * Kill tauri-driver after each worker session ends.
   */
  async afterSession() {
    if (tauriDriver) {
      tauriDriver.kill()
      tauriDriver = null
    }
    if (sessionTempRoot) {
      rmSync(sessionTempRoot, { recursive: true, force: true })
      sessionTempRoot = null
    }
  },
}
