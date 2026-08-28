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
 *   E2E_SKIP_BUILD=1 bunx wdio run e2e/wdio.conf.js  (skip build)
 */

import { spawn, spawnSync } from 'node:child_process'
import { resolve } from 'node:path'
import { appendFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { appendDriverStderr, collectFailureArtifacts } from './failure-artifacts.js'
import { createCodexScratchHome } from './helpers/codexScratchHome.js'

const projectRoot = resolve(import.meta.dirname, '..')
const specsDir = resolve(import.meta.dirname, 'specs')

const binaryPath = resolve(projectRoot, 'src-tauri', 'target', 'debug', 'taurhaus')
const localTauriDriverPath = resolve(projectRoot, 'node_modules', '.bin', 'tauri-driver')
const nativeWebKitDriverPath = process.env.E2E_NATIVE_DRIVER_PATH || '/usr/bin/WebKitWebDriver'
const wdioLogLevel = process.env.E2E_WDIO_LOG_LEVEL || 'error'
const wdioOutputDir = process.env.E2E_WDIO_OUTPUT_DIR || resolve(tmpdir(), 'taurhaus-e2e-wdio-logs')
const wdioPort = Number(process.env.E2E_WDIO_PORT || (4500 + (process.pid % 300)))
const nativeWebDriverPort = Number(process.env.E2E_NATIVE_WEBDRIVER_PORT || (wdioPort + 1))
const connectionRetryTimeoutMs = Number(process.env.E2E_CONNECTION_RETRY_TIMEOUT_MS || 12_000)
const connectionRetryCount = Number(process.env.E2E_CONNECTION_RETRY_COUNT || 0)
const mochaTimeoutMs = Number(process.env.E2E_MOCHA_TIMEOUT_MS || 25_000)
const mochaBail = process.env.E2E_MOCHA_BAIL !== '0'
const suiteBail = Number(process.env.E2E_BAIL || 1)
const traceTiming = process.env.E2E_TRACE_TIMING === '1'
const traceTimingThresholdMs = Number(process.env.E2E_TRACE_THRESHOLD_MS || 1_500)
const driverPidRegistry = resolve(tmpdir(), `taurhaus-e2e-driver-pids-${wdioPort}.txt`)

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

const specList = buildSpecList()
const specGroupIndexByPath = new Map()
for (const [groupIndex, group] of specList.entries()) {
  for (const specPath of group) {
    specGroupIndexByPath.set(resolve(specPath), groupIndex)
  }
}

let tauriDriver
let sessionTempRoot = null
let tauriDriverStderrBuffer = ''
let sessionAppLogPaths = []
let sessionDaemonLogPaths = []

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

// The one spec that drives a real Codex subscription. It runs against a scratch
// CODEX_HOME rather than the operator's own, so the app process — which installs
// the managed hook and renders the member's `CODEX_HOME='…'` launch prefix — has
// to be started with that root already in its environment.
const CODEX_SCRATCH_SPEC = 'compaction-codex-hooks.js'
let previousCodexHome = null
let codexHomeOverridden = false

function prepareCodexScratchHome(specs, scratchHome) {
  const wanted = (specs ?? []).some((spec) => resolve(spec).endsWith(CODEX_SCRATCH_SPEC))
  if (!wanted) return

  const sourceHome = process.env.E2E_CODEX_SOURCE_HOME || resolve(homedir(), '.codex')
  const { copied, missing } = createCodexScratchHome(sourceHome, scratchHome)
  previousCodexHome = process.env.CODEX_HOME ?? null
  codexHomeOverridden = true
  process.env.CODEX_HOME = scratchHome
  console.log(
    `[e2e] Codex scratch home ${scratchHome} from ${sourceHome}` +
      ` (copied: ${copied.join(', ') || 'none'}${missing.length ? `; missing: ${missing.join(', ')}` : ''})`
  )
}

function restoreCodexHome() {
  if (!codexHomeOverridden) return
  if (previousCodexHome === null) {
    delete process.env.CODEX_HOME
  } else {
    process.env.CODEX_HOME = previousCodexHome
  }
  previousCodexHome = null
  codexHomeOverridden = false
}

async function isWebDriverProtocolReady(host, port, timeoutMs = 500) {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), timeoutMs)
  try {
    const response = await fetch(`http://${host}:${port}/status`, {
      method: 'GET',
      signal: controller.signal,
    })
    return response.status === 200
  } catch {
    return false
  } finally {
    clearTimeout(timeout)
  }
}

async function waitForWebDriverReady(host, port, timeoutMs = 5_000, intervalMs = 100) {
  const startedAt = Date.now()
  while (Date.now() - startedAt < timeoutMs) {
    if (await isWebDriverProtocolReady(host, port)) return
    await new Promise((resolve) => setTimeout(resolve, intervalMs))
  }
  throw new Error(`tauri-driver did not become protocol-ready at ${host}:${port} within ${timeoutMs}ms`)
}

function killByPattern(pattern) {
  if (process.platform === 'win32') return
  spawnSync('pkill', ['-f', pattern], { stdio: 'ignore' })
}

function appendDriverPid(pid) {
  if (!pid) return
  appendFileSync(driverPidRegistry, `${pid}\n`)
}

function readDriverPids() {
  try {
    const lines = readFileSync(driverPidRegistry, 'utf8')
      .split('\n')
      .map(line => Number.parseInt(line.trim(), 10))
      .filter(pid => Number.isInteger(pid) && pid > 0)
    return [...new Set(lines)]
  } catch {
    return []
  }
}

function clearDriverPidRegistry() {
  rmSync(driverPidRegistry, { force: true })
}

function killDriverTree(pid) {
  if (!pid || pid <= 0) return
  try {
    process.kill(-pid, 'SIGKILL')
    return
  } catch {
    // Fall back to direct PID kill.
  }
  try {
    process.kill(pid, 'SIGKILL')
  } catch {
    // no-op
  }
}

function cleanupRegisteredDrivers() {
  for (const pid of readDriverPids()) {
    killDriverTree(pid)
  }
  clearDriverPidRegistry()
}

function cleanupDriverPortFallback() {
  // Last-resort fallback for orphan processes on this worker's ports.
  killByPattern(`tauri-driver --port ${wdioPort} --native-port ${nativeWebDriverPort}`)
  killByPattern(`WebKitWebDriver --port=${nativeWebDriverPort}`)
}

function cleanupStaleDriverProcessesPreRun() {
  // Safe at startup only (before worker sessions begin).
  // Prevents orphan test apps from prior aborted runs.
  killByPattern('tauri-driver --port')
  killByPattern('WebKitWebDriver --port=')
}

function cleanupTauriDriver() {
  if (tauriDriver?.pid) {
    killDriverTree(tauriDriver.pid)
  }
  tauriDriver = null
  cleanupRegisteredDrivers()
  cleanupDriverPortFallback()
}

function cleanupSessionTempRoot() {
  if (!sessionTempRoot) return
  rmSync(sessionTempRoot, { recursive: true, force: true })
  sessionTempRoot = null
  sessionAppLogPaths = []
  sessionDaemonLogPaths = []
}

function cleanupAllE2eArtifacts() {
  cleanupTauriDriver()
  cleanupSessionTempRoot()
  restoreCodexHome()
}

let cleanupHandlersRegistered = false
function registerCleanupHandlers() {
  if (cleanupHandlersRegistered) return
  cleanupHandlersRegistered = true

  process.on('exit', () => {
    cleanupAllE2eArtifacts()
  })

  const handleSignal = () => {
    cleanupAllE2eArtifacts()
    process.exit(1)
  }
  const handleCrash = () => {
    cleanupAllE2eArtifacts()
  }
  process.on('SIGINT', handleSignal)
  process.on('SIGTERM', handleSignal)
  process.on('uncaughtException', handleCrash)
  process.on('unhandledRejection', handleCrash)
}
registerCleanupHandlers()

export const config = {
  // ── Runner ──────────────────────────────────────────────────────────────
  runner: 'local',
  hostname: '127.0.0.1',
  port: wdioPort,
  // Keep transport retries bounded so failed sessions fail fast.
  connectionRetryTimeout: connectionRetryTimeoutMs,
  connectionRetryCount,
  maxInstances: 1,
  // WDIO defaults to "info", which logs every COMMAND/DATA/RESULT triplet.
  // In the persistent 14-spec suite that adds thousands of synchronous writes
  // and noticeably inflates per-command latency. Keep verbose logs opt-in.
  logLevel: wdioLogLevel,
  outputDir: wdioOutputDir,

  // ── Specs ───────────────────────────────────────────────────────────────
  // Stop the overall run after the first failing spec by default.
  // Override with E2E_BAIL=0 when a full matrix is required.
  bail: suiteBail,
  // Multiple sub-arrays: each group gets its own app instance.
  specs: specList,

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
    timeout: mochaTimeoutMs,
    bail: mochaBail,
  },

  // ── Reporter ────────────────────────────────────────────────────────────
  reporters: ['spec'],

  // ── Hooks ───────────────────────────────────────────────────────────────

  beforeHook(test) {
    if (!traceTiming) return
    test.__e2eStart = Date.now()
  },

  afterHook(test, _context, result) {
    if (!traceTiming) return
    const duration = Number(result?.duration || 0)
    if (duration < traceTimingThresholdMs) return
    const title = test?.title ? `${test.title}` : 'hook'
    const passed = result?.error ? 'fail' : 'pass'
    console.log(`[e2e:timing] hook ${passed} ${duration}ms :: ${title}`)
  },

  beforeTest(test) {
    if (!traceTiming) return
    test.__e2eStart = Date.now()
  },

  async afterTest(test, _context, result) {
    if (traceTiming) {
      const duration = Number(result?.duration || 0)
      if (duration >= traceTimingThresholdMs) {
        const parent = test?.parent ? `${test.parent} :: ` : ''
        const title = test?.title ? `${test.title}` : 'unknown test'
        const status = result?.error ? 'fail' : 'pass'
        console.log(`[e2e:timing] test ${status} ${duration}ms :: ${parent}${title}`)
      }
    }

    const failed = Boolean(result?.error) || result?.passed === false
    if (!failed) return

    const rawSpecFile = test?.file || 'unknown-spec.js'
    const resolvedSpecFile = resolve(specsDir, rawSpecFile)
    const groupIndex = specGroupIndexByPath.get(resolvedSpecFile) ?? null
    const parent = test?.parent ? `${test.parent} :: ` : ''
    const testTitle = `${parent}${test?.title || 'unknown test'}`

    try {
      const bundle = await collectFailureArtifacts({
        outputDir: wdioOutputDir,
        specFile: resolvedSpecFile,
        testTitle,
        groupIndex,
        appLogPaths: sessionAppLogPaths,
        daemonLogPaths: sessionDaemonLogPaths,
        driverStderr: tauriDriverStderrBuffer,
        tailLines: 200,
        saveScreenshot: async (screenshotPath) => {
          await browser.saveScreenshot(screenshotPath)
        },
      })
      console.error(`[e2e] failure artifacts collected at ${bundle.artifactDir}`)
    } catch (error) {
      console.warn(`[e2e] failed to collect failure artifacts for "${testTitle}":`, error)
    }
  },

  /**
   * Build the Tauri debug binary before running tests.
   * Skip with E2E_SKIP_BUILD=1 if you already have a fresh build.
   */
  async onPrepare() {
    cleanupStaleDriverProcessesPreRun()

    if (process.env.E2E_SKIP_BUILD === '1') {
      console.log('[e2e] Skipping build (E2E_SKIP_BUILD=1)')
      return
    }

    console.log('[e2e] Building Tauri debug binary...')
    return new Promise((resolve, reject) => {
      const build = spawn('bunx', ['tauri', 'build', '--debug', '--no-bundle'], {
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
  async beforeSession(_config, _capabilities, specs) {
    // Guard against stale processes from aborted runs before starting a new worker.
    cleanupAllE2eArtifacts()

    sessionTempRoot = mkdtempSync(`${tmpdir()}/taurhaus-e2e-${process.pid}-`)
    const tauriDataDir = `${sessionTempRoot}/app-data`
    const tauriClaudeDir = `${sessionTempRoot}/claude`
    const e2eProjectsDir = `${sessionTempRoot}/projects`
    const taurhausFixtureProject = `${e2eProjectsDir}/taurhaus`
    const ledgerFixtureProject = `${e2eProjectsDir}/ledger`
    mkdirSync(tauriDataDir, { recursive: true })
    mkdirSync(tauriClaudeDir, { recursive: true })
    tauriDriverStderrBuffer = ''
    sessionAppLogPaths = [
      `${tauriDataDir}/taurhaus.log.jsonl`,
      `${tauriDataDir}/taurhaus.log`,
    ]
    sessionDaemonLogPaths = [
      process.env.TAURHAUS_DAEMON_LOG_PATH,
      `${tauriDataDir}/taurhaus-daemon.log.jsonl`,
      `${tauriDataDir}/taurhaus-daemon.log`,
      `${tauriDataDir}/daemon.log`,
    ].filter(Boolean)

    createFixtureRepo(taurhausFixtureProject, { title: 'taurhaus fixture', withHistory: true })
    createFixtureRepo(ledgerFixtureProject, { title: 'ledger fixture', withHistory: false })

    process.env.E2E_PROJECTS_DIR = e2eProjectsDir
    process.env.E2E_TAURHAUS_PROJECT_PATH = taurhausFixtureProject
    process.env.TAURHAUS_DATA_DIR = tauriDataDir
    process.env.TAURHAUS_CLAUDE_DIR = tauriClaudeDir
    prepareCodexScratchHome(specs, `${sessionTempRoot}/codex-home`)

    tauriDriver = spawn(
      localTauriDriverPath,
      ['--port', String(wdioPort), '--native-port', String(nativeWebDriverPort), '--native-driver', nativeWebKitDriverPath],
      {
      env: {
        ...process.env,
        TAURHAUS_DATA_DIR: tauriDataDir,
        TAURHAUS_CLAUDE_DIR: tauriClaudeDir,
      },
      stdio: ['ignore', 'pipe', 'pipe'],
      detached: true,
      }
    )
    appendDriverPid(tauriDriver.pid)
    if (tauriDriver?.stdout) {
      tauriDriver.stdout.on('data', (chunk) => {
        process.stdout.write(chunk)
      })
    }
    if (tauriDriver?.stderr) {
      tauriDriver.stderr.on('data', (chunk) => {
        const text = chunk.toString()
        process.stderr.write(text)
        tauriDriverStderrBuffer = appendDriverStderr(tauriDriverStderrBuffer, text)
      })
    }

    await waitForWebDriverReady('127.0.0.1', wdioPort)
  },

  /**
   * Kill tauri-driver after each worker session ends.
   */
  async afterSession() {
    cleanupAllE2eArtifacts()
  },

  /**
   * Final safety net so failed/crashed runs don't leave app instances behind.
   */
  async onComplete() {
    cleanupAllE2eArtifacts()
  },
}
