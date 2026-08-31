/**
 * A managed Codex stage reaches its one-shot deadline actions end to end
 * (Tier 2, Linux, paid). W4 experiment 4.
 *
 * This is a measured lane, not a fast simulation. A real managed member runs
 * `mesh task start` and then ends its turn without completing the task. The
 * production 30-second self-heal pass must nudge it once, mark both task stores
 * stale once, and leave the member session alive for a later resumed stage.
 *
 * Deadline arithmetic is intentionally small but exact. `--deadline 1` means
 * 1 minute * 60 seconds/minute = 60 seconds. Half of 60 seconds is 30 seconds,
 * so the first pass at or after assigned_at + 30 s may nudge, and the first
 * pass at or after assigned_at + 60 s must stale the task. Because the pass
 * cadence is itself 30 s, either action may land up to one cadence after its
 * threshold. The lane waits for three recorded `startup.self_heal.completed`
 * events after `assigned_at`; elapsed sleeps are never accepted as proof that
 * those passes ran.
 *
 * Before that stall, a negative path gives the same member a three-minute task.
 * The active command must cover `needed_active = D/2 + 30 s`: 90 + 30 = 120
 * seconds, including a whole pass cadence after half time. That leaves
 * `slack = D/2 - 30 s`: 90 - 30 = 60 seconds for the Codex turn that launches
 * the command and the turn that completes the task. MarkStale is not suppressed
 * by activity, so that completion allowance is part of the lane contract. The
 * heartbeat emits 4096 bytes every 500 ms so Codex's measured read rate clears
 * the production 1 kB/s gate. It keeps one real member command running; it is
 * not a test-process sleep and proves nothing without joined activity/pass
 * records.
 *
 * Every acceptance assertion reads a durable record:
 *
 *   - mesh task records provide pending/in-progress/stale status;
 *   - taurhaus's operational snapshot provides imported `deadline_minutes`
 *     and the persisted `assigned_at`, `nudged_at`, and `stale_at` markers;
 *   - mesh's attention projection provides assignment/delivery timestamps;
 *   - the member inbox proves the nudge landed exactly once;
 *   - JSONL events prove each committed deadline action happened once;
 *   - the runtime record plus the lane's own tmux server prove the member pane,
 *     process, daemon, and resumable session survived.
 *
 * The `stage()` verdict is deliberately shaped like the production courier:
 * it polls `mesh task get --json` and returns `{status: "timeout"}` only when
 * that canonical task record reads `stale`.
 *
 * It spends real Codex subscription turns, so `e2e/specList.js` excludes it
 * from every suite. The orchestrator runs it once, by name, after merge:
 *
 *     E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-deadline
 *
 * Isolation matches `managed-stage-codex.js`. All product roots are under the
 * WDIO worker root, `CODEX_HOME` is the scratch auth copy, every mesh command
 * names the isolated Claude root, and `CLAUDE_DIR` is also placed in the pane
 * because the member itself runs mesh. The app, test, managed pane, and daemons
 * all use the worker's private tmux server and daemon port. Teardown disbands
 * only the scratch team and then kills only that verified-private tmux server.
 */

import { execFileSync, spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join, resolve } from 'node:path'

import { ensureMainApp, waitForAppReady } from '../helpers.js'
import { readLogEventsSince, selectEvents } from '../helpers/compactionLog.js'
import { trustProject } from '../helpers/codexScratchHome.js'
import { createLaneCleanup } from '../helpers/laneCleanup.js'
import {
  activeDeadlineHeartbeatPlan,
  activeDeadlinePassEvidence,
  assignTask,
  attentionRecord,
  createTask,
  readInbox,
  stagePollVerdict,
  taskRecord,
} from '../helpers/meshTaskContract.js'
import { waitForProjectsLoaded } from '../helpers/navigation.js'
import { TAURHAUS_CLAUDE_DIR } from '../helpers/platform.js'
import { createStageFixtureProject } from '../helpers/stageFixtureProject.js'
import {
  assertTmuxIsolation,
  isolatedTmuxTmpdir,
  parseProcEnviron,
  tmuxIsolationProblem,
} from '../helpers/laneTmux.js'

/** Managed Codex launches use the native-hook-capable floor from experiment 3. */
const MIN_CODEX_VERSION = [0, 147, 0]
/** mesh 0.2.24 introduced task deadlines and canonical stage completions. */
const MIN_MESH_VERSION = [0, 2, 24]
const TAURHAUS_MESH_BINARY = join(homedir(), '.local', 'bin', 'mesh')
const TMUX_SESSION = 'taurhaus'
const LAUNCH_EFFORT = 'low'
const DEADLINE_MINUTES = 1
const PASS_CADENCE_MS = 30_000
const ACTIVE_DEADLINE_MINUTES = 3
const ACTIVE_HEARTBEAT = activeDeadlineHeartbeatPlan({
  deadlineMinutes: ACTIVE_DEADLINE_MINUTES,
  passCadenceMs: PASS_CADENCE_MS,
  intervalMs: 500,
  payloadBytes: 4_096,
})
const REQUIRED_PASS_COUNT = 3

const APP_BINARY = resolve(import.meta.dirname, '..', '..', 'src-tauri', 'target', 'debug', 'taurhaus')
const TEAM_READY_TIMEOUT_MS = 240_000
const ONBOARDING_TURN_TIMEOUT_MS = 120_000
const ASSIGNMENT_START_TIMEOUT_MS = 240_000
const DEADLINE_ACTION_TIMEOUT_MS = 180_000
const PASS_EVIDENCE_TIMEOUT_MS = 210_000

const dataDir = process.env.TAURHAUS_DATA_DIR || ''
const codexHome = process.env.CODEX_HOME || ''
const claudeDir = TAURHAUS_CLAUDE_DIR
const teamsDir = join(claudeDir, 'teams')
const appLogPath = join(dataDir, 'taurhaus.log.jsonl')
const codexNotifyPath = join(dataDir, 'codex-notify.jsonl')
const sessionTempRoot = dataDir ? dirname(dataDir) : ''
const projectsDir = process.env.E2E_PROJECTS_DIR || (sessionTempRoot ? join(sessionTempRoot, 'projects') : '')

const PANE_ENVIRONMENT = new Map(
  [
    ['TAURHAUS_DATA_DIR', dataDir],
    ['TAURHAUS_CLAUDE_DIR', claudeDir],
    ['CODEX_HOME', codexHome],
    ['CLAUDE_DIR', claudeDir],
  ].filter(([, value]) => Boolean(value))
)

const PANE_ENVIRONMENT_STEP = 'tmux-session-environment'
const LANE_PANES_STEP = 'deadline-lane-tmux-server'
const laneCleanup = createLaneCleanup()
laneCleanup.install()

const uniqueSuffix = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`
const TEAM_NAME = `e2e-managed-deadline-${uniqueSuffix}`
const LEAD_NAME = 'e2e-lead'
const MEMBER_NAME = 'codex-deadline'

let laneEnabled = false
let laneSkipReason = 'managed Codex deadline prerequisites unavailable'
let fixtureProject = ''
let fixtureSetupError = ''
let fixtureProjectKey = null
const createdTeamNames = new Set()
const measured = {}

// The first-run wizard scans E2E_PROJECTS_DIR inside the before hook, so this
// throwaway project must exist at module load. The worker root removes it.
if (projectsDir) {
  try {
    fixtureProject = join(projectsDir, 'deadline-stage-fixture')
    mkdirSync(fixtureProject, { recursive: true })
    createStageFixtureProject(fixtureProject)
  } catch (error) {
    fixtureSetupError = String(error?.message ?? error)
    fixtureProject = ''
  }
}

function tmux(args) {
  assertTmuxIsolation(process.env)
  return execFileSync('tmux', args, {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    timeout: 5_000,
  }).trim()
}

function tmuxQuietly(args) {
  try {
    return { ok: true, output: tmux(args) }
  } catch (error) {
    return { ok: false, output: '', error: String(error?.message ?? error) }
  }
}

async function invokeTauri(command, args = undefined) {
  return await browser.executeAsync((payload, done) => {
    const tauri = window.__TAURI_INTERNALS__
    if (!tauri || typeof tauri.invoke !== 'function') {
      done({ ok: false, error: 'Tauri internals unavailable' })
      return
    }
    tauri
      .invoke(payload.command, payload.args)
      .then((result) => done({ ok: true, result }))
      .catch((error) => done({ ok: false, error: error?.message ?? String(error) }))
  }, { command, args })
}

async function invokeTauriOrThrow(command, args = undefined) {
  const result = await invokeTauri(command, args)
  if (!result.ok) throw new Error(result.error || `Failed to invoke ${command}`)
  return result.result
}

async function invokeTauriWithTimeout(command, args = undefined, timeoutMs = 10_000) {
  return await Promise.race([
    invokeTauri(command, args),
    new Promise((resolvePromise) => {
      setTimeout(
        () => resolvePromise({ ok: false, error: `Timed out after ${timeoutMs}ms` }),
        timeoutMs
      )
    }),
  ])
}

function parseVersion(program, args) {
  try {
    const raw = execFileSync(program, args, { encoding: 'utf8', timeout: 10_000 })
    const match = String(raw).match(/(\d+)\.(\d+)\.(\d+)/)
    return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : null
  } catch {
    return null
  }
}

function versionAtLeast(version, minimum) {
  for (let index = 0; index < minimum.length; index += 1) {
    const left = version[index] ?? 0
    const right = minimum[index] ?? 0
    if (left !== right) return left > right
  }
  return true
}

function commandExists(program) {
  try {
    execFileSync('which', [program], { stdio: 'ignore', timeout: 5_000 })
    return true
  } catch {
    return false
  }
}

function appProcessIds() {
  const found = spawnSync('pgrep', ['-f', APP_BINARY], { encoding: 'utf8', timeout: 5_000 })
  return String(found.stdout ?? '')
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => /^\d+$/.test(line))
}

function appTmuxIsolationProblem() {
  const pids = appProcessIds()
  if (pids.length === 0) return `no running app process matches ${APP_BINARY}`

  for (const pid of pids) {
    let environment
    try {
      environment = parseProcEnviron(readFileSync(`/proc/${pid}/environ`, 'utf8'))
    } catch (error) {
      return `the app process ${pid} did not expose its tmux environment: ${String(error?.message ?? error)}`
    }
    const problem = tmuxIsolationProblem(environment, sessionTempRoot)
    if (problem) return `the app process ${pid} is not on the lane's private tmux server: ${problem}`
  }
  return ''
}

function hostSkipReason() {
  if (process.platform !== 'linux') return `The managed deadline lane is Linux-only (got ${process.platform})`
  if (!dataDir) return 'TAURHAUS_DATA_DIR is not set for this session'
  if (!process.env.TAURHAUS_CLAUDE_DIR) return 'TAURHAUS_CLAUDE_DIR is not set for this session'
  if (!existsSync(join(codexHome, 'auth.json'))) {
    return `no Codex credentials were copied into ${codexHome} (is ~/.codex/auth.json present?)`
  }
  if (fixtureSetupError) return `the deadline fixture project could not be created: ${fixtureSetupError}`
  if (!fixtureProject) return 'E2E_PROJECTS_DIR is not set, so the fixture project has no isolated root'
  if (!commandExists('claude')) return 'claude CLI is not on PATH for the credential-free lead pane'
  if (!commandExists('bun')) return 'bun is not on PATH for the active negative path'

  const tmuxProblem = tmuxIsolationProblem(process.env, sessionTempRoot)
  if (tmuxProblem) return `the lane needs a tmux server of its own: ${tmuxProblem}`
  const appTmuxProblem = appTmuxIsolationProblem()
  if (appTmuxProblem) return `the app must use the lane's private tmux server: ${appTmuxProblem}`

  const codexVersion = parseVersion('codex', ['--version'])
  if (!codexVersion) return 'codex CLI is not on PATH'
  if (!versionAtLeast(codexVersion, MIN_CODEX_VERSION)) {
    return `codex ${codexVersion.join('.')} predates the ${MIN_CODEX_VERSION.join('.')} managed-session floor`
  }

  for (const [program, label] of [
    ['mesh', 'mesh on PATH'],
    [TAURHAUS_MESH_BINARY, `taurhaus mesh at ${TAURHAUS_MESH_BINARY}`],
  ]) {
    const version = parseVersion(program, ['--version'])
    if (!version) return `${label} is unavailable`
    if (!versionAtLeast(version, MIN_MESH_VERSION)) {
      return `${label} is ${version.join('.')}; deadline records require ${MIN_MESH_VERSION.join('.')}`
    }
  }
  return ''
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch {
    return null
  }
}

function readRuntimeRecord() {
  return readJson(join(teamsDir, TEAM_NAME, 'runtime', `${MEMBER_NAME}.json`))
}

function readOperationalSnapshot() {
  return readJson(join(teamsDir, TEAM_NAME, 'state', 'operational', `${MEMBER_NAME}.json`))
}

function readActivitySnapshot() {
  return readJson(join(teamsDir, TEAM_NAME, 'state', 'activity', `${MEMBER_NAME}.json`))
}

function currentLogOffset() {
  return readLogEventsSince(appLogPath, 0).offset
}

function eventsSince(offset) {
  return readLogEventsSince(appLogPath, offset).events
}

function deadlineEvents(events, event, taskId, deadlineMinutes = DEADLINE_MINUTES) {
  return selectEvents(events, {
    event,
    match: {
      team: TEAM_NAME,
      member: MEMBER_NAME,
      task_id: String(taskId),
      deadline_minutes: deadlineMinutes,
    },
  })
}

function selfHealPassesAfter(events, assignedAt) {
  const assignedAtMs = Date.parse(assignedAt)
  return selectEvents(events, { event: 'startup.self_heal.completed' }).filter(
    (event) => Number.isFinite(assignedAtMs) && Date.parse(event.ts) >= assignedAtMs
  )
}

async function waitForLogEvidence(offset, predicate, { timeout, timeoutMsg }) {
  let evidence = null
  await browser.waitUntil(
    async () => {
      evidence = predicate(eventsSince(offset))
      return Boolean(evidence)
    },
    { timeout, interval: 1_000, timeoutMsg }
  )
  return evidence
}

function applyPaneEnvironment() {
  tmuxQuietly(['new-session', '-d', '-s', TMUX_SESSION])

  const previous = new Map()
  for (const [key, value] of PANE_ENVIRONMENT) {
    const shown = tmuxQuietly(['show-environment', '-t', TMUX_SESSION, key])
    previous.set(key, shown.ok ? shown.output : null)
    tmuxQuietly(['set-environment', '-t', TMUX_SESSION, key, value])
  }

  return function restorePaneEnvironment() {
    for (const [key, shown] of previous) {
      if (!shown) {
        tmuxQuietly(['set-environment', '-t', TMUX_SESSION, '-u', key])
      } else if (shown.startsWith('-')) {
        tmuxQuietly(['set-environment', '-t', TMUX_SESSION, '-r', key])
      } else {
        tmuxQuietly(['set-environment', '-t', TMUX_SESSION, key, shown.slice(shown.indexOf('=') + 1)])
      }
    }
  }
}

async function withPaneEnvironment(work) {
  const restore = applyPaneEnvironment()
  laneCleanup.owe(PANE_ENVIRONMENT_STEP, restore)
  try {
    return await work()
  } finally {
    restore()
    laneCleanup.settled(PANE_ENVIRONMENT_STEP)
  }
}

function killLaneTmuxServer() {
  const problem = tmuxIsolationProblem(process.env, sessionTempRoot)
  if (problem) {
    console.log(`[e2e] deadline tmux cleanup skipped; this is not the lane server: ${problem}`)
    return
  }

  const listed = tmuxQuietly(['list-panes', '-a', '-F', '#{pane_id}\t#{pane_current_path}'])
  if (listed.ok && listed.output) console.log(`[e2e] deadline lane panes at teardown:\n${listed.output}`)
  const killed = tmuxQuietly(['kill-server'])
  console.log(
    `[e2e] ${killed.ok ? 'killed' : 'did not kill'} the deadline lane tmux server ` +
      `(${isolatedTmuxTmpdir(sessionTempRoot)})${killed.ok ? '' : `: ${killed.error}`}`
  )
}

async function bootApp(attempts = 3) {
  for (let attempt = 1; ; attempt += 1) {
    try {
      await waitForAppReady()
      return
    } catch (error) {
      const message = String(error?.message ?? error)
      if (attempt >= attempts || !/no such frame|unload event|stale element/i.test(message)) throw error
      const handles = await browser.getWindowHandles().catch(() => [])
      if (handles.length > 0) await browser.switchToWindow(handles[0]).catch(() => {})
      await browser.pause(2_000)
    }
  }
}

async function capturePane(paneId, attempts = 5) {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const captured = tmuxQuietly(['capture-pane', '-p', '-J', '-t', paneId])
    if (captured.ok && captured.output.trim()) return captured.output
    await browser.pause(500)
  }
  return ''
}

const BLOCKING_PANE_PROMPTS = [/Do you trust the contents/i, /will be deprecated/i, /press enter to (continue|confirm)/i]

function blockingPrompt(paneContents) {
  return BLOCKING_PANE_PROMPTS.some((pattern) => pattern.test(paneContents))
}

async function pickCodexModel() {
  const settings = await invokeTauriOrThrow('get_settings')
  const contract = settings?.terminalContract ?? settings?.terminal_contract ?? {}
  const catalog = contract?.modelCatalog ?? contract?.model_catalog ?? {}
  const entries = Array.isArray(catalog?.codex) ? catalog.codex : []
  const usable = entries.find((entry) => entry?.deprecated !== true)
  if (!usable) throw new Error('The backend model catalog offers no supported Codex model')
  return usable.id
}

async function pickRoleIds() {
  const roles = await invokeTauriOrThrow('templates_list_roles_full')
  const entries = Array.isArray(roles) ? roles : []
  const idOf = (role) => role?.roleId ?? role?.role_id ?? null
  const toolOf = (role) => String(role?.defaults?.cliTool ?? role?.defaults?.cli_tool ?? '').toLowerCase()
  const lead = entries.find((role) => role?.kind === 'lead' && toolOf(role) === 'claude')
  const agent = entries.find((role) => role?.kind === 'agent' && toolOf(role) === 'codex')
  if (!lead) throw new Error('No Claude lead role template is available')
  if (!agent) throw new Error('No Codex agent role template is available')
  return { leadRoleId: idOf(lead), agentRoleId: idOf(agent) }
}

async function registeredFixtureProjectId() {
  const projects = await invokeTauriOrThrow('list_projects')
  const match = (Array.isArray(projects) ? projects : []).find((project) => project?.path === fixtureProject)
  if (!match) throw new Error(`The fixture project ${fixtureProject} was not registered by the wizard`)
  return match.id
}

async function refreshFixtureTasks() {
  await invokeTauriOrThrow('get_project_tasks', { projectId: fixtureProjectKey })
}

function completedTurns() {
  try {
    return readFileSync(codexNotifyPath, 'utf8').split('\n').filter((line) => line.trim()).length
  } catch {
    return 0
  }
}

async function waitForTurnAfter(previousTurns, timeoutMs) {
  try {
    await browser.waitUntil(async () => completedTurns() > previousTurns, {
      timeout: timeoutMs,
      interval: 1_000,
      timeoutMsg: 'no Codex turn completed',
    })
    return true
  } catch {
    return false
  }
}

async function ensureMemberHasTakenATurn(paneId) {
  if (completedTurns() > 0) return 'already'
  tmuxQuietly(['send-keys', '-t', paneId, 'Enter'])
  if (await waitForTurnAfter(0, ONBOARDING_TURN_TIMEOUT_MS)) return 'onboarding'

  tmuxQuietly(['send-keys', '-t', paneId, '-l', 'Reply with only the word READY.'])
  await browser.pause(600)
  tmuxQuietly(['send-keys', '-t', paneId, 'Enter'])
  if (await waitForTurnAfter(0, ONBOARDING_TURN_TIMEOUT_MS)) return 'prompted'

  throw new Error(
    `${MEMBER_NAME} completed no onboarding turn; pane contents:\n${(await capturePane(paneId)).trimEnd()}`
  )
}

async function initializeManagedDeadlineTeam() {
  const { leadRoleId, agentRoleId } = await pickRoleIds()
  const model = await pickCodexModel()

  createdTeamNames.add(TEAM_NAME)
  laneCleanup.owe(LANE_PANES_STEP, killLaneTmuxServer)
  const report = await withPaneEnvironment(async () =>
    await invokeTauriOrThrow('coordination_initialize_team', {
      request: {
        teamName: TEAM_NAME,
        teamDescription: 'Paid E2E measurement for managed stage deadline semantics',
        leadMode: 'launch_new',
        lead: {
          name: LEAD_NAME,
          cliTool: 'claude',
          model: '',
          projectId: fixtureProject,
          roleId: leadRoleId,
        },
        agents: [
          {
            name: MEMBER_NAME,
            cliTool: 'codex',
            model,
            reasoningEffort: LAUNCH_EFFORT,
            projectId: fixtureProject,
            roleId: agentRoleId,
          },
        ],
      },
    })
  )
  if (report?.failedStep) {
    throw new Error(`Team initialization failed at ${report.failedStep}: ${report.message}`)
  }

  await browser.waitUntil(async () => Boolean(readRuntimeRecord()?.pane_id), {
    timeout: TEAM_READY_TIMEOUT_MS,
    interval: 2_000,
    timeoutMsg: `Managed Codex member ${MEMBER_NAME} never reported a pane`,
  })
  const paneId = readRuntimeRecord().pane_id
  const paneContents = await capturePane(paneId)
  console.log(`[e2e] ${MEMBER_NAME} pane ${paneId} on launch:\n${paneContents.trimEnd()}`)
  if (blockingPrompt(paneContents)) {
    throw new Error(`Codex is parked on an interactive prompt:\n${paneContents.trimEnd()}`)
  }
  const firstTurn = await ensureMemberHasTakenATurn(paneId)
  console.log(`[e2e] ${MEMBER_NAME} completed its first turn (${firstTurn})`)
}

function meshArgs() {
  return { claudeDir, team: TEAM_NAME, actor: LEAD_NAME }
}

function memberMeshCommand(operation, taskId, extra = '') {
  return (
    `mesh task ${operation} ${taskId}${extra} --team ${TEAM_NAME} --name ${MEMBER_NAME} ` +
    `--claude-dir ${claudeDir}`
  )
}

function assignDeadlineTask({ subject, description, deadlineMinutes, firstStepFor, deliverable, completionSignalFor }) {
  const created = createTask({
    ...meshArgs(),
    subject,
    description,
    effort: LAUNCH_EFFORT,
    why: 'W4 experiment 4 deadline measurement',
    deadline: deadlineMinutes,
    firstStep: 'Run the exact mesh task start command in the assignment notice.',
    deliverable,
  })
  const taskId = String(created.id)
  const firstStep = firstStepFor(taskId)
  const completionSignal = completionSignalFor(taskId)
  assignTask({
    ...meshArgs(),
    taskId,
    owner: MEMBER_NAME,
    status: 'pending',
    effort: LAUNCH_EFFORT,
    why: 'W4 experiment 4 deadline measurement',
    deadline: deadlineMinutes,
    firstStep,
    deliverable,
    completionSignal,
  })
  return { taskId, created }
}

function refreshAttentionRecord(taskId) {
  taskRecord({ ...meshArgs(), taskId })
  return attentionRecord({ claudeDir, team: TEAM_NAME, taskId })
}

async function waitForAttentionDelivery(taskId) {
  let record = null
  await browser.waitUntil(
    async () => {
      record = refreshAttentionRecord(taskId)
      return Boolean(record?.deliveredAt)
    },
    {
      timeout: ASSIGNMENT_START_TIMEOUT_MS,
      interval: 2_000,
      timeoutMsg: `mesh never recorded delivery for deadline task #${taskId}`,
    }
  )
  return record
}

async function waitForTaskStatus(taskId, status, timeout = ASSIGNMENT_START_TIMEOUT_MS) {
  let record = null
  await browser.waitUntil(
    async () => {
      try {
        record = taskRecord({ ...meshArgs(), taskId })
        return record?.status === status
      } catch {
        return false
      }
    },
    { timeout, interval: 2_000, timeoutMsg: `task #${taskId} never reached ${status}` }
  )
  return record
}

async function waitForOperationalTask(taskId, status) {
  let snapshot = null
  await browser.waitUntil(
    async () => {
      await refreshFixtureTasks()
      snapshot = readOperationalSnapshot()
      return snapshot?.task?.id === String(taskId) && snapshot?.task?.status === status
    },
    {
      timeout: ASSIGNMENT_START_TIMEOUT_MS,
      interval: 2_000,
      timeoutMsg: `operational snapshot never imported #${taskId} as ${status}`,
    }
  )
  return snapshot
}

function nudgeMessages(taskId) {
  return readInbox({ claudeDir, team: TEAM_NAME, member: MEMBER_NAME }).filter((message) => {
    const text = String(message?.text ?? '')
    return text.includes(`Task #${taskId}`) && text.includes('half the deadline is gone')
  })
}

function memberAliveEvidence() {
  const runtime = readRuntimeRecord()
  expect(runtime?.pane_id).toMatch(/^%/)
  expect(runtime?.pane_pid).toBeGreaterThan(0)
  expect(runtime?.daemon_pid).toBeGreaterThan(0)
  expect(typeof runtime?.session_id).toBe('string')
  expect(runtime.session_id.length).toBeGreaterThan(0)

  const pane = tmuxQuietly([
    'display-message',
    '-p',
    '-t',
    runtime.pane_id,
    '#{pane_id}\t#{pane_pid}\t#{pane_dead}\t#{pane_current_command}',
  ])
  expect(pane.ok).toBe(true)
  const [paneId, panePid, paneDead, currentCommand] = pane.output.split('\t')
  expect(paneId).toBe(runtime.pane_id)
  expect(Number(panePid)).toBe(runtime.pane_pid)
  expect(paneDead).toBe('0')
  expect(existsSync(`/proc/${runtime.pane_pid}`)).toBe(true)
  expect(existsSync(`/proc/${runtime.daemon_pid}`)).toBe(true)

  return {
    runtimeLastSeenAt: runtime.last_seen_at ?? runtime.lastSeenAt ?? null,
    paneId,
    panePid: Number(panePid),
    paneDead: Number(paneDead),
    paneCurrentCommand: currentCommand,
    daemonPid: runtime.daemon_pid,
    sessionId: runtime.session_id,
  }
}

function taskRecordTimestamps(record) {
  const metadata = record?.metadata ?? {}
  const candidates = {
    assignedAt: metadata.assigned_at ?? metadata.assignedAt,
    startedAt: metadata.started_at ?? metadata.startedAt,
    stateChangedAt: record?.state_changed_at ?? record?.stateChangedAt,
    updatedAt: record?.updated_at ?? record?.updatedAt,
    completedAt: metadata.completed_at ?? metadata.completedAt,
  }
  return Object.fromEntries(Object.entries(candidates).filter(([, value]) => value != null))
}

describe('managed stage deadline semantics', function () {
  this.timeout(1_200_000)

  before(async function () {
    this.timeout(900_000)
    await bootApp()
    if (!(await ensureMainApp())) {
      laneSkipReason = 'Main app unavailable'
      return
    }
    await waitForProjectsLoaded()

    const hostReason = hostSkipReason()
    if (hostReason) {
      laneSkipReason = hostReason
      return
    }

    const availability = await invokeTauri('coordination_get_feature_availability')
    if (!availability.ok) {
      laneSkipReason = `Feature availability check failed: ${availability.error}`
      return
    }
    const report = availability.result || {}
    const blockingErrors = Array.isArray(report.blockingErrors) ? report.blockingErrors : []
    if (
      report.canInitialize === false ||
      report.meshAvailable === false ||
      report.tmuxAvailable === false ||
      blockingErrors.length > 0
    ) {
      laneSkipReason = blockingErrors[0] || 'Mesh or tmux unavailable'
      return
    }

    trustProject(join(codexHome, 'config.toml'), fixtureProject)
    fixtureProjectKey = await registeredFixtureProjectId()
    await initializeManagedDeadlineTeam()
    expect(readRuntimeRecord()?.appliedEffort).toBe(LAUNCH_EFFORT)
    laneEnabled = true
  })

  after(async function () {
    this.timeout(180_000)
    if (Object.keys(measured).length > 0) {
      console.log(`[e2e] managed deadline measured: ${JSON.stringify(measured, null, 2)}`)
    }

    for (const teamName of createdTeamNames) {
      if (!teamName.startsWith('e2e-')) continue
      await invokeTauriWithTimeout('coordination_disband_team', { teamName }, 60_000)
    }
    createdTeamNames.clear()
    laneCleanup.run()
  })

  it('suppresses the half-time nudge while the member is actively working, then completes normally', async function () {
    if (!laneEnabled) return this.skip()
    this.timeout(480_000)

    const logOffset = currentLogOffset()
    const turnCount = completedTurns()
    const assigned = assignDeadlineTask({
      subject: 'Exercise active deadline suppression',
      description:
        'W4 experiment 4 negative path: keep one honest command active through an eligible half-time pass.',
      deadlineMinutes: ACTIVE_DEADLINE_MINUTES,
      firstStepFor: (taskId) => {
        const start = memberMeshCommand('start', taskId, " --active-form 'Running deadline heartbeat'")
        const complete = memberMeshCommand(
          'complete',
          taskId,
          " --summary 'active deadline suppression completed'"
        )
        return `${start}. Then run exactly: ${ACTIVE_HEARTBEAT.command}. After it exits, run exactly: ${complete}.`
      },
      deliverable: 'Complete the task after the heartbeat. Change no file and send no separate message.',
      completionSignalFor: (taskId) =>
        memberMeshCommand('complete', taskId, " --summary 'active deadline suppression completed'"),
    })

    const attention = await waitForAttentionDelivery(assigned.taskId)
    const inProgressRecord = await waitForTaskStatus(assigned.taskId, 'in_progress')
    const imported = await waitForOperationalTask(assigned.taskId, 'in_progress')
    expect(imported.task.deadline_minutes).toBe(ACTIVE_DEADLINE_MINUTES)
    expect(Number.isFinite(Date.parse(imported.task.assigned_at))).toBe(true)

    const activityByObservedAt = new Map()
    let suppressionEvidence = null
    await browser.waitUntil(
      async () => {
        const activity = readActivitySnapshot()
        if (
          activity?.observed_at &&
          Date.parse(activity.observed_at) >= Date.parse(imported.task.assigned_at)
        ) {
          activityByObservedAt.set(activity.observed_at, activity)
        }

        const events = eventsSince(logOffset)
        const actions = [
          ...deadlineEvents(events, 'deadline.nudge.sent', assigned.taskId, ACTIVE_DEADLINE_MINUTES),
          ...deadlineEvents(events, 'deadline.task.staled', assigned.taskId, ACTIVE_DEADLINE_MINUTES),
        ]
        suppressionEvidence = activeDeadlinePassEvidence({
          assignedAt: imported.task.assigned_at,
          deadlineMinutes: ACTIVE_DEADLINE_MINUTES,
          activitySnapshots: [...activityByObservedAt.values()],
          passEvents: selfHealPassesAfter(events, imported.task.assigned_at),
          deadlineEvents: actions,
        })
        return Boolean(suppressionEvidence)
      },
      {
        timeout: 150_000,
        interval: 1_000,
        timeoutMsg: `no eligible self-heal pass observed active work on task #${assigned.taskId}`,
      }
    )

    expect(suppressionEvidence.activityConfidence).toMatch(/^(active|likely_working)$/)
    expect(nudgeMessages(assigned.taskId)).toEqual([])
    expect(readOperationalSnapshot()?.task?.nudged_at ?? null).toBeNull()
    expect(readOperationalSnapshot()?.task?.stale_at ?? null).toBeNull()

    const completedRecord = await waitForTaskStatus(assigned.taskId, 'completed', 180_000)
    expect(await waitForTurnAfter(turnCount, ONBOARDING_TURN_TIMEOUT_MS)).toBe(true)
    expect(completedRecord.status).toBe('completed')

    const finalEvents = eventsSince(logOffset)
    expect(
      deadlineEvents(finalEvents, 'deadline.nudge.sent', assigned.taskId, ACTIVE_DEADLINE_MINUTES)
    ).toEqual([])
    expect(
      deadlineEvents(finalEvents, 'deadline.task.staled', assigned.taskId, ACTIVE_DEADLINE_MINUTES)
    ).toEqual([])
    expect(nudgeMessages(assigned.taskId)).toEqual([])
    const sessionEvidence = memberAliveEvidence()
    const finalAttention = refreshAttentionRecord(assigned.taskId)

    measured.activitySuppression = {
      taskId: assigned.taskId,
      deadlineMinutes: ACTIVE_DEADLINE_MINUTES,
      halfDueMs: ACTIVE_DEADLINE_MINUTES * 60_000 / 2,
      fullDueMs: ACTIVE_DEADLINE_MINUTES * 60_000,
      heartbeatDurationMs: ACTIVE_HEARTBEAT.durationMs,
      completionSlackMs: ACTIVE_HEARTBEAT.completionSlackMs,
      heartbeatOutputBytesPerSecond: ACTIVE_HEARTBEAT.outputBytesPerSecond,
      taskTransitions: [
        { status: assigned.created.status, recordTimestamps: taskRecordTimestamps(assigned.created) },
        {
          status: inProgressRecord.status,
          at: imported.task.assigned_at,
          recordTimestamps: taskRecordTimestamps(inProgressRecord),
        },
        { status: completedRecord.status, recordTimestamps: taskRecordTimestamps(completedRecord) },
      ],
      operationalImport: {
        updatedAt: imported.updated_at,
        assignedAt: imported.task.assigned_at,
        deadlineMinutes: imported.task.deadline_minutes,
      },
      attention: {
        assignedAt: finalAttention?.assignedAt ?? attention?.assignedAt ?? null,
        deliveredAt: finalAttention?.deliveredAt ?? attention?.deliveredAt ?? null,
        deliveryState: finalAttention?.deliveryState ?? attention?.deliveryState ?? null,
      },
      suppressionEvidence,
      deadlineActionCount: 0,
      sessionEvidence,
    }
  })

  it('nudges once, stales once, returns timeout, and preserves the managed session', async function () {
    if (!laneEnabled) return this.skip()
    this.timeout(600_000)

    const logOffset = currentLogOffset()
    const turnCount = completedTurns()
    const assigned = assignDeadlineTask({
      subject: 'Wait for the managed stage deadline',
      description: 'W4 experiment 4: start honestly, then remain idle so the production deadline pass acts.',
      deadlineMinutes: DEADLINE_MINUTES,
      firstStepFor: (taskId) =>
        `${memberMeshCommand('start', taskId, " --active-form 'Waiting for further instructions'")}. ` +
        'After it succeeds, wait silently for further instructions. Do not complete or block the task, send a ' +
        'message, edit a file, or run another command.',
      deliverable: 'No repository change. Leave the started task open for the deadline pass.',
      completionSignalFor: () => 'Do not report completion until the lead sends further instructions.',
    })

    const attention = await waitForAttentionDelivery(assigned.taskId)
    const inProgressRecord = await waitForTaskStatus(assigned.taskId, 'in_progress')
    const imported = await waitForOperationalTask(assigned.taskId, 'in_progress')

    // This is the honest-stall proof: assignment defaulted to pending, only the
    // member was instructed to run `mesh task start`, and mesh now records the
    // resulting in-progress transition. The completed Codex turn proves it then
    // returned to silence rather than the test holding a fake status open.
    expect(await waitForTurnAfter(turnCount, ONBOARDING_TURN_TIMEOUT_MS)).toBe(true)
    expect(imported.task.deadline_minutes).toBe(DEADLINE_MINUTES)
    expect(Number.isFinite(Date.parse(imported.task.assigned_at))).toBe(true)
    expect(imported.task.nudged_at ?? null).toBeNull()
    expect(imported.task.stale_at ?? null).toBeNull()
    const aliveAfterStart = memberAliveEvidence()

    const nudgeEvent = await waitForLogEvidence(
      logOffset,
      (events) => {
        const stale = deadlineEvents(events, 'deadline.task.staled', assigned.taskId)
        if (stale.length > 0) {
          throw new Error(`task #${assigned.taskId} became stale before its half-time nudge`)
        }
        return deadlineEvents(events, 'deadline.nudge.sent', assigned.taskId)[0] ?? null
      },
      { timeout: DEADLINE_ACTION_TIMEOUT_MS, timeoutMsg: `task #${assigned.taskId} was never nudged` }
    )
    expect(nudgeEvent.deadline_minutes).toBe(DEADLINE_MINUTES)

    const nudgedSnapshot = readOperationalSnapshot()
    expect(nudgedSnapshot?.task?.id).toBe(assigned.taskId)
    expect(nudgedSnapshot?.task?.status).toBe('in_progress')
    expect(Number.isFinite(Date.parse(nudgedSnapshot?.task?.nudged_at))).toBe(true)
    expect(nudgedSnapshot?.task?.stale_at ?? null).toBeNull()
    expect(nudgeMessages(assigned.taskId)).toHaveLength(1)
    const aliveAfterNudge = memberAliveEvidence()

    // This is stage()-shaped rather than a timer verdict: only the canonical
    // `mesh task get --json` record becoming stale returns timeout.
    let staleRecord = null
    let stageVerdict = null
    await browser.waitUntil(
      async () => {
        staleRecord = taskRecord({ ...meshArgs(), taskId: assigned.taskId })
        stageVerdict = stagePollVerdict(staleRecord)
        return Boolean(stageVerdict)
      },
      {
        timeout: DEADLINE_ACTION_TIMEOUT_MS,
        interval: 2_000,
        timeoutMsg: `stage poll never observed task #${assigned.taskId} become stale`,
      }
    )
    expect(stageVerdict).toEqual({ status: 'timeout' })

    const staleEvent = await waitForLogEvidence(
      logOffset,
      (events) => deadlineEvents(events, 'deadline.task.staled', assigned.taskId)[0] ?? null,
      { timeout: 30_000, timeoutMsg: `task #${assigned.taskId} staled without its structured event` }
    )
    const staledSnapshot = readOperationalSnapshot()
    expect(staleRecord.status).toBe('stale')
    expect(staledSnapshot?.task?.id).toBe(assigned.taskId)
    expect(staledSnapshot?.task?.status).toBe('stale')
    expect(Number.isFinite(Date.parse(staledSnapshot?.task?.stale_at))).toBe(true)

    // Wait for records from at least three real passes after assigned_at. This
    // includes a pass after one of the actions when cadence alignment makes the
    // nudge/stale pair happen in the first two eligible passes, and proves the
    // persisted one-shot markers suppress every later attempt.
    const passEvents = await waitForLogEvidence(
      logOffset,
      (events) => {
        const passes = selfHealPassesAfter(events, imported.task.assigned_at)
        return passes.length >= REQUIRED_PASS_COUNT ? passes : null
      },
      {
        timeout: PASS_EVIDENCE_TIMEOUT_MS,
        timeoutMsg: `fewer than ${REQUIRED_PASS_COUNT} self-heal passes were recorded after assigned_at`,
      }
    )

    const finalEvents = eventsSince(logOffset)
    const nudgeEvents = deadlineEvents(finalEvents, 'deadline.nudge.sent', assigned.taskId)
    const staleEvents = deadlineEvents(finalEvents, 'deadline.task.staled', assigned.taskId)
    expect(nudgeEvents).toHaveLength(1)
    expect(staleEvents).toHaveLength(1)
    expect(nudgeMessages(assigned.taskId)).toHaveLength(1)
    expect(readOperationalSnapshot()?.task?.nudged_at).toBe(nudgedSnapshot.task.nudged_at)
    expect(readOperationalSnapshot()?.task?.stale_at).toBe(staledSnapshot.task.stale_at)
    const aliveAfterStale = memberAliveEvidence()

    const finalAttention = refreshAttentionRecord(assigned.taskId)
    Object.assign(measured, {
      deadline: {
        taskId: assigned.taskId,
        deadlineMinutes: DEADLINE_MINUTES,
        passCadenceMs: 30_000,
        halfDueMs: DEADLINE_MINUTES * 60_000 / 2,
        fullDueMs: DEADLINE_MINUTES * 60_000,
        taskTransitions: [
          { status: assigned.created.status, recordTimestamps: taskRecordTimestamps(assigned.created) },
          { status: inProgressRecord.status, at: imported.task.assigned_at, recordTimestamps: taskRecordTimestamps(inProgressRecord) },
          { status: staleRecord.status, at: staledSnapshot.task.stale_at, recordTimestamps: taskRecordTimestamps(staleRecord) },
        ],
        operationalImport: {
          updatedAt: imported.updated_at,
          assignedAt: imported.task.assigned_at,
          deadlineMinutes: imported.task.deadline_minutes,
        },
        attention: {
          assignedAt: finalAttention?.assignedAt ?? attention?.assignedAt ?? null,
          deliveredAt: finalAttention?.deliveredAt ?? attention?.deliveredAt ?? null,
          deliveryState: finalAttention?.deliveryState ?? attention?.deliveryState ?? null,
        },
        nudge: {
          eventAt: nudgeEvent.ts,
          snapshotAt: nudgedSnapshot.task.nudged_at,
          inboxAt: nudgeMessages(assigned.taskId)[0]?.timestamp ?? null,
          eventCount: nudgeEvents.length,
          inboxCount: nudgeMessages(assigned.taskId).length,
        },
        stale: {
          eventAt: staleEvent.ts,
          snapshotAt: staledSnapshot.task.stale_at,
          eventCount: staleEvents.length,
        },
        selfHealPasses: passEvents.slice(0, REQUIRED_PASS_COUNT).map((event) => ({
          at: event.ts,
          durationMs: event.duration_ms,
          teamsScanned: event.teams_scanned,
        })),
        stageVerdict,
        sessionEvidence: {
          afterStart: aliveAfterStart,
          afterNudge: aliveAfterNudge,
          afterStale: aliveAfterStale,
        },
      },
    })
  })

  it('records why the managed deadline lane was unavailable', async function () {
    if (laneEnabled) return this.skip()
    expect(typeof laneSkipReason).toBe('string')
    expect(laneSkipReason.length).toBeGreaterThan(0)
    console.log(`[e2e] managed deadline lane skipped: ${laneSkipReason}`)
  })
})
