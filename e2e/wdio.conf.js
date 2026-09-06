/**
 * WebdriverIO configuration for Tauri e2e tests.
 *
 * BATCHED PERSISTENT MODE: Specs are split into small groups (3 each).
 * Each group runs in its own worker session = its own app instance.
 * This keeps per-operation latency low (~95ms vs ~165ms with all specs
 * in one session) while avoiding per-spec app startup overhead.
 *
 * Nine serial behavioral sessions: eight use generated onboarding through
 * supported setup commands; one exercises the real wizard on a virgin root.
 * The sealed manifest in specList.js declares paid and docs-capture exclusions.
 * Breadth recipes disable both bail flags; named runs remain fail-fast.
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
 *
 * None of those run a paid lane. `compaction-codex-hooks` and
 * `managed-stage-codex` spend real subscription turns and are only ever started
 * by name:
 *
 *   E2E_INSTALL_DAEMON=0 just test-e2e-spec compaction-codex-hooks
 *   E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-codex
 *
 * `E2E_INSTALL_DAEMON` stays at its safe default of 0: every worker launches
 * the checkout-local daemon on its own private port, so 1 only rebuilds and
 * restarts the operator's installed daemon and changes nothing about the run.
 */

import { spawn, spawnSync } from 'node:child_process'
import { createHash, randomUUID } from 'node:crypto'
import { relative, resolve } from 'node:path'
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { appendDriverStderr, collectFailureArtifacts } from './failure-artifacts.js'
import { createCodexScratchHome } from './helpers/codexScratchHome.js'
import {
  E2E_RUN_TOKEN_ENV,
  cleanupStaleProcessLedgers,
  createOwnedProcessLedger,
  findRunTokenProcessRecords,
} from './helpers/laneCleanup.js'
import {
  WORKER_ROOT_ENV_KEYS,
  buildWorkerEnv,
  findAvailableWorkerDaemonPort,
  prepareWorkerHome,
} from './helpers/workerEnv.js'
import { CODEX_SCRATCH_SPECS, buildSpecList, captureSpecs, listSpecFiles, paidSpecs } from './specList.js'
import { needsWizard, seedOnboarding, invokeApp } from './helpers/onboarding.js'
import { finishRun, selectedSpecFiles, updateRunSummary } from './runSummary.js'

const projectRoot = resolve(import.meta.dirname, '..')
const specsDir = resolve(import.meta.dirname, 'specs')
// Mirrors the Rust rule (commands/mesh.rs WSL_MESH_BINARY_PATH): the
// operator's installed mesh lives at ~/.local/bin/mesh; keep them in step.
const operatorMeshBinaryPath = resolve(homedir(), '.local', 'bin', 'mesh')

function resolveCargoTargetDir() {
  const result = spawnSync(
    'cargo',
    ['metadata', '--format-version', '1', '--no-deps', '--manifest-path', 'src-tauri/Cargo.toml'],
    { cwd: projectRoot, encoding: 'utf8' }
  )
  if (result.error || result.status !== 0) {
    throw new Error(`Failed to resolve Cargo target directory: ${result.error?.message || result.stderr}`)
  }
  const targetDirectory = JSON.parse(result.stdout).target_directory
  if (typeof targetDirectory !== 'string' || targetDirectory.length === 0) {
    throw new Error('Cargo metadata did not return target_directory')
  }
  return targetDirectory
}

const cargoTargetDir = resolveCargoTargetDir()
const binaryPath = resolve(cargoTargetDir, 'debug', 'taurhaus')
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
const specList = buildSpecList(specsDir)
const specGroupIndexByPath = new Map()
for (const [groupIndex, group] of specList.entries()) {
  for (const specPath of group) {
    specGroupIndexByPath.set(resolve(specPath), groupIndex)
  }
}

let tauriDriver
let buildProcess
let sessionTempRoot = null
let tauriDriverStderrBuffer = ''
let sessionAppLogPaths = []
let sessionDaemonLogPaths = []
let sessionRunToken = ''
let processLedger = null
let ownedProcessRefreshTimer = null

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

// The specs that drive a real Codex subscription. They run against a scratch
// CODEX_HOME rather than the operator's own, so the app process — which installs
// the managed hook and renders the member's `CODEX_HOME='…'` launch prefix — has
// to be started with that root already in its environment.
let previousCodexHome = null
let codexHomeOverridden = false

function prepareCodexScratchHome(specs, scratchHome) {
  const wanted = (specs ?? []).some((spec) =>
    CODEX_SCRATCH_SPECS.some((name) => resolve(spec).endsWith(name))
  )
  if (!wanted) return

  const sourceHome = process.env.E2E_CODEX_SOURCE_HOME || resolve(homedir(), '.codex')
  const { copied, generated, missing } = createCodexScratchHome(sourceHome, scratchHome)
  previousCodexHome = process.env.CODEX_HOME ?? null
  codexHomeOverridden = true
  process.env.CODEX_HOME = scratchHome
  console.log(
    `[e2e] Codex scratch home ${scratchHome} from ${sourceHome}` +
      ` (copied: ${copied.join(', ') || 'none'}; generated: ${generated.join(', ')}` +
      `${missing.length ? `; missing: ${missing.join(', ')}` : ''})`
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

// Every worker uses the tmux server named by buildWorkerEnv. The app is what
// creates managed panes, so the override has to be in place before tauri-driver
// starts it, and inherited TMUX must stay absent.
let previousTmuxEnvironment = null
let tmuxSocketDir = ''

function prepareIsolatedTmux(workerEnv) {
  previousTmuxEnvironment = { TMUX_TMPDIR: process.env.TMUX_TMPDIR ?? null, TMUX: process.env.TMUX ?? null }
  tmuxSocketDir = workerEnv.TMUX_TMPDIR
  process.env.TMUX_TMPDIR = tmuxSocketDir
  delete process.env.TMUX
  // tmux creates `$TMUX_TMPDIR/tmux-<uid>` but not its parent, and fails when
  // the parent is missing.
  mkdirSync(tmuxSocketDir, { recursive: true })
  console.log(`[e2e] tmux server for this session: ${tmuxSocketDir} (inherited TMUX cleared)`)
}

/**
 * Take the lane's own tmux server down before its socket directory is deleted.
 *
 * The spec kills it in its own teardown; this is the path a crashed or killed
 * run takes, where removing the temp root would otherwise orphan a server —
 * and the panes, and the CLIs in them — with its socket gone.
 */
function killIsolatedTmuxServer() {
  if (!tmuxSocketDir) return
  const env = { ...process.env, TMUX_TMPDIR: tmuxSocketDir }
  delete env.TMUX
  spawnSync('tmux', ['kill-server'], { env, stdio: 'ignore', timeout: 5_000 })
}

function restoreTmuxIsolation() {
  if (!previousTmuxEnvironment) return
  for (const [key, value] of Object.entries(previousTmuxEnvironment)) {
    if (value === null) delete process.env[key]
    else process.env[key] = value
  }
  previousTmuxEnvironment = null
  tmuxSocketDir = ''
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

function refreshOwnedProcessRecords() {
  if (!processLedger || !sessionRunToken) return
  for (const record of findRunTokenProcessRecords(sessionRunToken)) {
    if (record.pid !== process.pid) processLedger.record(record)
  }
}

function startOwnedProcessRefresh() {
  if (ownedProcessRefreshTimer) clearInterval(ownedProcessRefreshTimer)
  ownedProcessRefreshTimer = setInterval(refreshOwnedProcessRecords, 1_000)
  ownedProcessRefreshTimer.unref()
}

function stopOwnedProcessRefresh() {
  if (!ownedProcessRefreshTimer) return
  clearInterval(ownedProcessRefreshTimer)
  ownedProcessRefreshTimer = null
}

function cleanupTauriDriver() {
  stopOwnedProcessRefresh()
  refreshOwnedProcessRecords()
  processLedger?.cleanup()
  tauriDriver = null
  processLedger?.remove()
  processLedger = null
  sessionRunToken = ''
}

function cleanupSessionTempRoot() {
  if (!sessionTempRoot) return
  rmSync(sessionTempRoot, { recursive: true, force: true })
  sessionTempRoot = null
  sessionAppLogPaths = []
  sessionDaemonLogPaths = []
}

function cleanupAllE2eArtifacts() {
  if (buildProcess && buildProcess.exitCode === null) {
    try { process.kill(-buildProcess.pid, 'SIGTERM') } catch { /* already exited */ }
  }
  cleanupTauriDriver()
  killIsolatedTmuxServer()
  cleanupSessionTempRoot()
  restoreCodexHome()
  restoreTmuxIsolation()
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
    if (process.env.E2E_RUN_SUMMARY) {
      try { updateRunSummary(summary => finishRun(summary, 1)) } catch { /* preparation may not have created it */ }
    }
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
    require: [resolve(import.meta.dirname, 'runSummary.js')],
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
    // The daemon may start after the session-level `before` hook. Refresh on
    // every test boundary as well as on the timer so hard-killed workers leave
    // useful on-disk identities for the next cleanup pass.
    refreshOwnedProcessRecords()

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
  async onPrepare(launchConfig) {
    const startedAt = new Date().toISOString()
    cleanupStaleProcessLedgers(projectRoot)
    mkdirSync(wdioOutputDir, { recursive: true })
    const summaryDir = mkdtempSync(resolve(wdioOutputDir, 'run-'))
    process.env.E2E_RUN_SUMMARY = resolve(summaryDir, 'run-summary.json')
    writeFileSync(process.env.E2E_RUN_SUMMARY, JSON.stringify({
      started_at: startedAt,
      build_ms: 0,
      build_reused: process.env.E2E_SKIP_BUILD === '1',
      wall_ms: null,
      boots: 0,
      wizard_walks: 0,
      revision: spawnSync('git', ['rev-parse', 'HEAD'], { cwd: projectRoot, encoding: 'utf8' }).stdout.trim(),
      dirty: Boolean(spawnSync('git', ['status', '--porcelain'], { cwd: projectRoot, encoding: 'utf8' }).stdout.trim()),
      binary: binaryPath,
      binary_sha256: null,
      complete: false,
      exclusions: [],
      specs: {},
    }))
    console.log(`[e2e] Run summary: ${process.env.E2E_RUN_SUMMARY}`)
    try {
      const selected = selectedSpecFiles(launchConfig)
      updateRunSummary(summary => {
        summary.exclusions = listSpecFiles(specsDir).filter(name => !selected.some(path => path.endsWith(`/${name}`))).map(spec => ({
          spec,
          reason: paidSpecs.includes(spec) ? 'paid; explicitly named only'
            : captureSpecs.includes(spec) ? 'on-demand documentation capture'
              : 'explicit suite selection/exclusion',
        }))
        summary.specs = Object.fromEntries(selected.map(path => [relative(projectRoot, path), null]))
      })

      if (process.env.E2E_SKIP_BUILD === '1') {
        console.log('[e2e] Skipping build (E2E_SKIP_BUILD=1)')
        updateRunSummary(summary => { summary.binary_sha256 = createHash('sha256').update(readFileSync(binaryPath)).digest('hex') })
        return
      }

      console.log('[e2e] Building Tauri debug binary...')
      const buildStarted = Date.now()
      await new Promise((resolve, reject) => {
        const build = spawn('just', ['build-e2e'], {
          cwd: projectRoot,
          stdio: 'inherit',
          detached: true,
        })
        buildProcess = build
        build.on('error', reject)
        build.on('close', (code) => {
          buildProcess = null
          if (code === 0) {
            console.log('[e2e] Build complete')
            resolve()
          } else {
            reject(new Error(`Build failed with exit code ${code}`))
          }
        })
      }).finally(() => updateRunSummary(summary => { summary.build_ms = Date.now() - buildStarted }))
      updateRunSummary(summary => { summary.binary_sha256 = createHash('sha256').update(readFileSync(binaryPath)).digest('hex') })
    } catch (error) {
      updateRunSummary(summary => { summary.preparation_error = String(error) })
      // WDIO swallows ordinary onPrepare errors. Clear the worker schedule and
      // let onComplete fail the recorded incomplete run instead of booting apps.
      launchConfig.specs = []
      throw error
    }
  },

  /**
   * Start tauri-driver before each worker session.
   * With batched groups, this runs once per manifest group.
   */
  async beforeSession(_config, _capabilities, specs) {
    // Guard against stale processes from aborted runs before starting a new worker.
    cleanupAllE2eArtifacts()

    sessionTempRoot = mkdtempSync(`${tmpdir()}/taurhaus-e2e-${process.pid}-`)
    sessionRunToken = randomUUID()
    processLedger = createOwnedProcessLedger({ checkoutRoot: projectRoot, runToken: sessionRunToken })
    const daemonPort = await findAvailableWorkerDaemonPort(sessionTempRoot)
    const paidCodexWorker = (specs ?? []).some((spec) =>
      CODEX_SCRATCH_SPECS.some((name) => resolve(spec).endsWith(name))
    )
    const workerEnv = buildWorkerEnv(sessionTempRoot, {
      baseEnv: process.env,
      runToken: sessionRunToken,
      daemonBinaryPath: resolve(cargoTargetDir, 'debug', 'taurhaus-daemon'),
      daemonPort,
      skipCliVersionProbes: !paidCodexWorker,
    })
    const tauriDataDir = workerEnv.TAURHAUS_DATA_DIR
    const tauriClaudeDir = workerEnv.TAURHAUS_CLAUDE_DIR
    const e2eProjectsDir = `${sessionTempRoot}/projects`
    const taurhausFixtureProject = `${e2eProjectsDir}/taurhaus`
    const ledgerFixtureProject = `${e2eProjectsDir}/ledger`
    for (const key of WORKER_ROOT_ENV_KEYS) {
      mkdirSync(workerEnv[key], { recursive: true })
    }
    prepareWorkerHome(workerEnv.HOME, { meshBinaryPath: operatorMeshBinaryPath, blockRealClis: !paidCodexWorker })
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
    prepareCodexScratchHome(specs, workerEnv.CODEX_HOME)
    for (const key of [
      'HOME',
      'PATH',
      ...WORKER_ROOT_ENV_KEYS,
      'TAURHAUS_DAEMON_PORT',
      'TAURHAUS_DAEMON_BINARY',
      'TAURHAUS_SKIP_CLI_VERSION_PROBES',
      E2E_RUN_TOKEN_ENV,
    ]) {
      if (workerEnv[key] === undefined) delete process.env[key]
      else process.env[key] = workerEnv[key]
    }
    console.log(`[e2e] daemon port for this worker: ${workerEnv.TAURHAUS_DAEMON_PORT}`)
    prepareIsolatedTmux(workerEnv)

    tauriDriver = spawn(
      localTauriDriverPath,
      ['--port', String(wdioPort), '--native-port', String(nativeWebDriverPort), '--native-driver', nativeWebKitDriverPath],
      {
        env: workerEnv,
        stdio: ['ignore', 'pipe', 'pipe'],
        detached: true,
      }
    )
    processLedger.recordPid(tauriDriver.pid, { processGroup: true })
    startOwnedProcessRefresh()
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

  async before(_capabilities, specs) {
    // WebKitWebDriver and the app exist only after WDIO creates the session.
    refreshOwnedProcessRecords()
    updateRunSummary(summary => {
      summary.boots++
      if (needsWizard(specs)) summary.wizard_walks++
    })
    try {
      if (needsWizard(specs)) return
      await browser.waitUntil(
        () => browser.execute(() => Boolean(window.__TAURI_INTERNALS__)),
        { timeout: 30_000, timeoutMsg: 'Tauri setup IPC unavailable' }
      )
      await seedOnboarding(invokeApp, process.env.E2E_PROJECTS_DIR)
      await browser.refresh()
      await $('[data-testid="tab-overview"]').waitForExist({ timeout: 30_000 })
      if (await $('[data-testid="first-run-wizard"]').isExisting()) {
        throw new Error('Seeded root returned to onboarding after reload')
      }
    } catch (error) {
      // WDIO logs and swallows config-hook errors. The Mocha root hook must
      // also fail, or a spec with permissive prerequisites could report green.
      process.env.E2E_SETUP_ERROR = String(error)
      throw error
    }
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
  async onComplete(exitCode) {
    cleanupAllE2eArtifacts()
    cleanupStaleProcessLedgers(projectRoot)
    const summary = updateRunSummary(summary => finishRun(summary, exitCode))
    if (!summary.complete) throw new Error(`Required E2E coverage did not all execute and pass: ${process.env.E2E_RUN_SUMMARY}`)
  },
}
