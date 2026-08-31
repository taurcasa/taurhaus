/**
 * Two managed Codex stages work concurrently in two Git worktrees on one team
 * (Tier 2, Linux, paid). W4 experiment 5.
 *
 * This lane spends real Codex subscription turns and is named-only in
 * `e2e/specList.js`:
 *
 *     E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-parallel
 *
 * The Claude lead is an identity and inbox in the isolated, credential-free
 * Claude root; it never takes a turn. The two Codex members are the cost
 * ceiling. Each starts at the assignment's requested `medium` effort in a
 * different detached worktree of one fixture repo. The two create+assign
 * pipelines run under `Promise.all`, and acceptance comes from durable records:
 * each checked-out HEAD tree contains only its own RESULT commit and files,
 * each member inbox contains only its own assignment notice, and both mesh task
 * records are completed with their own completion timestamp.
 *
 * The W2 portion is explicitly a scanner-contract read-back: after the live
 * stages finish, this lane writes a production-shaped summary containing their
 * real task ids and timestamps under a synthetic session id. The
 * credential-free lead does not emit a Workflow run, so this does not claim
 * that a lead-produced run tree filed the stages.
 *
 * Every writable product root, fixture path and tmux socket lives below the
 * WDIO worker's session temp root. `CODEX_HOME` is the generated scratch home,
 * `CLAUDE_DIR` is explicitly put into the managed panes, and every mesh command
 * also names the scratch Claude root. Teardown kills only the private tmux
 * server this worker started.
 */

import { execFileSync, spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join, resolve } from 'node:path'

import { ensureMainApp, waitForAppReady } from '../helpers.js'
import { trustProject } from '../helpers/codexScratchHome.js'
import { createLaneCleanup } from '../helpers/laneCleanup.js'
import { assertTmuxIsolation, isolatedTmuxTmpdir, parseProcEnviron, tmuxIsolationProblem } from '../helpers/laneTmux.js'
import {
  codexStateDatabaseDiagnostic,
  launchManagedMembersSerially,
  waitWithPaneTail,
} from '../helpers/managedStageParallel.js'
import {
  attentionRecord,
  assignTaskAsync,
  createTaskAsync,
  findBlockedMessage,
  findResultMessage,
  readInbox,
  resultContractViolations,
  taskAssignmentNoticeIds,
  taskRecord,
} from '../helpers/meshTaskContract.js'
import { waitForProjectsLoaded } from '../helpers/navigation.js'
import {
  captureStageDelivery,
  completedParallelRunSummary,
  managedStageVocabulary,
  stageWindowOverlap,
} from '../helpers/parallelStageEvidence.js'
import { TAURHAUS_CLAUDE_DIR } from '../helpers/platform.js'
import {
  addStageFixtureWorktree,
  createStageFixtureProject,
  runFixtureTestsAtCommit,
  worktreeTreeDiff,
} from '../helpers/stageFixtureProject.js'

const MIN_CODEX_VERSION = [0, 147, 0]
const MIN_MESH_VERSION = [0, 2, 23]
const TAURHAUS_MESH_BINARY = join(homedir(), '.local', 'bin', 'mesh')
const APP_BINARY = resolve(import.meta.dirname, '..', '..', 'src-tauri', 'target', 'debug', 'taurhaus')
const FEATURE_PR_WORKFLOW = resolve(import.meta.dirname, '..', '..', '.claude', 'workflows', 'feature-pr.js')
const TMUX_SESSION = 'taurhaus'
const EFFORT = 'medium'
const DEADLINE_MINUTES = 10

const TEAM_READY_TIMEOUT_MS = 240_000
const SESSION_BIND_TIMEOUT_MS = 180_000
const RESULT_TIMEOUT_MS = 1_200_000
// Delivery closes within mesh's effort-wait bound; a lost projection should
// be reported minutes after the stages start, not at the 20-minute RESULT cap.
const DELIVERY_TIMEOUT_MS = 180_000

const dataDir = process.env.TAURHAUS_DATA_DIR || ''
const codexHome = process.env.CODEX_HOME || ''
const claudeDir = TAURHAUS_CLAUDE_DIR
const teamsDir = join(claudeDir, 'teams')
const sessionTempRoot = dataDir ? dirname(dataDir) : ''
const projectsDir = process.env.E2E_PROJECTS_DIR || (sessionTempRoot ? join(sessionTempRoot, 'projects') : '')
const MANAGED_STAGE_VOCABULARY = managedStageVocabulary(readFileSync(FEATURE_PR_WORKFLOW, 'utf8'), 'codex')

const PANE_ENVIRONMENT = new Map(
  [
    ['TAURHAUS_DATA_DIR', dataDir],
    ['TAURHAUS_CLAUDE_DIR', claudeDir],
    ['CODEX_HOME', codexHome],
    ['CLAUDE_DIR', claudeDir],
  ].filter(([, value]) => Boolean(value))
)

const laneCleanup = createLaneCleanup()
laneCleanup.install()
const PANE_ENVIRONMENT_STEP = 'tmux-session-environment'
const LANE_PANES_STEP = 'lane-tmux-server'

const uniqueSuffix = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`
const TEAM_NAME = `e2e-managed-parallel-${uniqueSuffix}`
const LEAD_NAME = 'e2e-lead'
const MEMBER_ALPHA = 'codex-alpha'
const MEMBER_BETA = 'codex-beta'

let laneEnabled = false
let laneSkipReason = 'parallel managed-stage prerequisites unavailable'
let fixtureSource = ''
let fixtureBaseline = ''
let fixtureSetupError = ''
const createdTeamNames = new Set()
const measured = {}

const stages = [
  {
    key: 'alpha',
    owner: MEMBER_ALPHA,
    worktree: projectsDir ? join(projectsDir, 'stage-alpha') : '',
    modulePath: 'src/lib/greet-alpha.js',
    testPath: 'src/lib/greet-alpha.test.js',
    exportName: 'greetAlpha',
  },
  {
    key: 'beta',
    owner: MEMBER_BETA,
    worktree: projectsDir ? join(projectsDir, 'stage-beta') : '',
    modulePath: 'src/lib/greet-beta.js',
    testPath: 'src/lib/greet-beta.test.js',
    exportName: 'greetBeta',
  },
]

// The fixture must predate the first-run project scan in this spec's `before`.
// Source and both worktrees are below the worker temp root and disappear with it.
if (projectsDir) {
  try {
    fixtureSource = join(projectsDir, 'stage-source')
    mkdirSync(fixtureSource, { recursive: true })
    fixtureBaseline = createStageFixtureProject(fixtureSource).headCommit
    for (const stage of stages) addStageFixtureWorktree(fixtureSource, stage.worktree, fixtureBaseline)
  } catch (error) {
    fixtureSetupError = String(error?.message ?? error)
    fixtureSource = ''
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
  const response = await invokeTauri(command, args)
  if (!response.ok) throw new Error(response.error || `Failed to invoke ${command}`)
  return response.result
}

async function invokeTauriWithTimeout(command, args = undefined, timeoutMs = 10_000) {
  return await Promise.race([
    invokeTauri(command, args),
    new Promise((resolvePromise) => {
      setTimeout(() => resolvePromise({ ok: false, error: `Timed out after ${timeoutMs}ms` }), timeoutMs)
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
    if (problem) return `the app process ${pid} is not on the lane's own tmux server: ${problem}`
  }
  return ''
}

function hostSkipReason() {
  if (process.platform !== 'linux') return `The parallel managed-stage lane is Linux-only (got ${process.platform})`
  if (!dataDir) return 'TAURHAUS_DATA_DIR is not set for this session'
  if (!process.env.TAURHAUS_CLAUDE_DIR) return 'TAURHAUS_CLAUDE_DIR is not set for this session'
  if (!existsSync(join(codexHome, 'auth.json'))) return `no Codex credentials were copied into ${codexHome}`
  if (fixtureSetupError) return `the parallel stage fixture could not be created: ${fixtureSetupError}`
  if (!fixtureSource || stages.some((stage) => !stage.worktree)) return 'E2E_PROJECTS_DIR is unavailable'
  if (!commandExists('claude')) return 'claude CLI is not on PATH for the team lead'
  if (!commandExists('bun')) return 'bun is not on PATH for fixture validation'

  const tmuxProblem = tmuxIsolationProblem(process.env, sessionTempRoot)
  if (tmuxProblem) return `the lane needs a tmux server of its own: ${tmuxProblem}`
  const appTmuxProblem = appTmuxIsolationProblem()
  if (appTmuxProblem) return `the lane needs a tmux server of its own: ${appTmuxProblem}`

  const codexVersion = parseVersion('codex', ['--version'])
  if (!codexVersion) return 'codex CLI is not on PATH'
  if (!versionAtLeast(codexVersion, MIN_CODEX_VERSION)) {
    return `codex ${codexVersion.join('.')} predates ${MIN_CODEX_VERSION.join('.')}`
  }
  for (const binary of ['mesh', TAURHAUS_MESH_BINARY]) {
    const version = parseVersion(binary, ['--version'])
    if (!version) return `mesh binary is unavailable at ${binary}`
    if (!versionAtLeast(version, MIN_MESH_VERSION)) {
      return `${binary} is mesh ${version.join('.')}, older than ${MIN_MESH_VERSION.join('.')}`
    }
  }
  return ''
}

function readRuntimeRecord(memberName) {
  try {
    return JSON.parse(readFileSync(join(teamsDir, TEAM_NAME, 'runtime', `${memberName}.json`), 'utf8'))
  } catch {
    return null
  }
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
      if (!shown) tmuxQuietly(['set-environment', '-t', TMUX_SESSION, '-u', key])
      else if (shown.startsWith('-')) tmuxQuietly(['set-environment', '-t', TMUX_SESSION, '-r', key])
      else tmuxQuietly(['set-environment', '-t', TMUX_SESSION, key, shown.slice(shown.indexOf('=') + 1)])
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
    console.log(`[e2e] parallel-stage tmux cleanup skipped: ${problem}`)
    return
  }
  const listed = tmuxQuietly(['list-panes', '-a', '-F', '#{pane_id}\t#{pane_current_path}'])
  if (listed.ok && listed.output) console.log(`[e2e] parallel-stage panes at teardown:\n${listed.output}`)
  const killed = tmuxQuietly(['kill-server'])
  console.log(
    `[e2e] ${killed.ok ? 'killed' : 'did not kill'} the lane tmux server ` +
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

async function pickCodexModel() {
  const settings = await invokeTauriOrThrow('get_settings')
  const contract = settings?.terminalContract ?? settings?.terminal_contract ?? {}
  const catalog = contract?.modelCatalog ?? contract?.model_catalog ?? {}
  const usable = (Array.isArray(catalog?.codex) ? catalog.codex : []).find((entry) => entry?.deprecated !== true)
  if (!usable) throw new Error('The backend model catalog offers no supported Codex model')
  return usable.id
}

async function pickRoleIds() {
  const roles = await invokeTauriOrThrow('templates_list_roles_full')
  const entries = Array.isArray(roles) ? roles : []
  const toolOf = (role) => String(role?.defaults?.cliTool ?? role?.defaults?.cli_tool ?? '').toLowerCase()
  const idOf = (role) => role?.roleId ?? role?.role_id ?? null
  const lead = entries.find((role) => role?.kind === 'lead' && toolOf(role) === 'claude')
  const agent = entries.find((role) => role?.kind === 'agent' && toolOf(role) === 'codex')
  if (!lead || !agent) throw new Error('Claude lead and Codex agent roles are required')
  return { leadRoleId: idOf(lead), agentRoleId: idOf(agent) }
}

function managedAgentRequest(stage, { agentRoleId, model }) {
  return {
    name: stage.owner,
    cliTool: 'codex',
    model,
    reasoningEffort: EFFORT,
    projectId: stage.worktree,
    roleId: agentRoleId,
  }
}

async function waitForMemberBinding(stage) {
  await browser.waitUntil(async () => Boolean(readRuntimeRecord(stage.owner)?.pane_id), {
    timeout: TEAM_READY_TIMEOUT_MS,
    interval: 2_000,
    timeoutMsg: `${stage.owner} did not report a managed pane`,
  })

  const runtime = readRuntimeRecord(stage.owner)
  expect(runtime?.appliedEffort).toBe(EFFORT)
  const pane = await capturePane(runtime.pane_id)
  console.log(`[e2e] ${stage.owner} launched in ${stage.worktree} on ${runtime.pane_id}:\n${pane.trimEnd()}`)
  if (BLOCKING_PANE_PROMPTS.some((pattern) => pattern.test(pane))) {
    throw new Error(`${stage.owner} is parked on an interactive prompt:\n${pane.trimEnd()}`)
  }

  // The onboarding submission can land while Codex is still starting. Add a
  // tiny prompt and submit once; the resulting first turn gives the managed
  // member the durable session id a real stage requires.
  tmuxQuietly(['send-keys', '-t', runtime.pane_id, '-l', 'Reply with only the word READY.'])
  await browser.pause(600)
  tmuxQuietly(['send-keys', '-t', runtime.pane_id, 'Enter'])

  await waitWithPaneTail({
    memberName: stage.owner,
    paneId: runtime.pane_id,
    capturePane,
    wait: async () =>
      await browser.waitUntil(
        async () => {
          await invokeTauri('coordination_get_live_team_status', { teamName: TEAM_NAME })
          return Boolean(readRuntimeRecord(stage.owner)?.session_id)
        },
        {
          timeout: SESSION_BIND_TIMEOUT_MS,
          interval: 3_000,
          timeoutMsg: `${stage.owner} did not bind its scratch-home session`,
        }
      ),
  })
}

async function initializeParallelTeam() {
  const { leadRoleId, agentRoleId } = await pickRoleIds()
  const model = await pickCodexModel()
  const launchInputs = { agentRoleId, model }
  createdTeamNames.add(TEAM_NAME)
  laneCleanup.owe(LANE_PANES_STEP, killLaneTmuxServer)

  // Codex 0.151 must not cold-start two processes against one fresh CODEX_HOME:
  // attempt 1 lost beta to `state_5.sqlite ... migration 22: duplicate column
  // name: agent_path`. Bind alpha first, then launch beta through add-agent.
  await launchManagedMembersSerially({
    members: stages,
    initialize: async (stage) => {
      const report = await withPaneEnvironment(async () =>
        await invokeTauriOrThrow('coordination_initialize_team', {
          request: {
            teamName: TEAM_NAME,
            teamDescription: 'W4 experiment 5: two concurrent managed stages in isolated worktrees',
            leadMode: 'launch_new',
            lead: {
              name: LEAD_NAME,
              cliTool: 'claude',
              model: '',
              projectId: fixtureSource,
              roleId: leadRoleId,
            },
            agents: [managedAgentRequest(stage, launchInputs)],
          },
        })
      )
      if (report?.failedStep) {
        throw new Error(`Team initialization failed at ${report.failedStep}: ${report.message}`)
      }
    },
    add: async (stage) => {
      const report = await withPaneEnvironment(async () =>
        await invokeTauriOrThrow('coordination_add_agent', {
          request: {
            teamName: TEAM_NAME,
            agent: managedAgentRequest(stage, launchInputs),
          },
        })
      )
      if (report?.failedStep) {
        throw new Error(`Adding ${stage.owner} failed at ${report.failedStep}: ${report.message}`)
      }
    },
    waitForBinding: waitForMemberBinding,
  })
  return model
}

function completionSignal(stage, taskId) {
  const result =
    `RESULT #${taskId}\\n` +
    `{"commit":"<full 40-char sha>","files":["${stage.modulePath}","${stage.testPath}"],` +
    `"validation":"bun test passed"}`
  return (
    `On success, send exactly one result with ` +
    `mesh send ${LEAD_NAME} $'${result}' --team ${TEAM_NAME} --name ${stage.owner} ` +
    `--claude-dir ${claudeDir} --summary result, then run mesh task complete ${taskId} ` +
    `--summary '${stage.key} greeting committed and tested' --team ${TEAM_NAME} ` +
    `--name ${stage.owner} --claude-dir ${claudeDir}. On a real blocker send BLOCKED #${taskId} <reason>.`
  )
}

async function createAndAssignStage(stage) {
  const subject = `Add the ${stage.key} greet function`
  const description = `W4 experiment 5 bounded ${stage.key} slice in its own worktree.`
  const firstStep =
    `In ${stage.worktree}, create ${stage.modulePath} exporting function ${stage.exportName}(name) ` +
    `that returns Hello, <name>! using a template literal.`
  const deliverable =
    `One git commit in ${stage.worktree} adding only ${stage.modulePath} and ${stage.testPath}. ` +
    `The Bun test imports ${stage.exportName}, asserts ${stage.exportName}("ada") === "Hello, ada!", ` +
    `and passes under bun test. Install nothing.`
  const why = `experiment 5: isolated ${stage.key} worktree`
  const created = await createTaskAsync({
    claudeDir,
    team: TEAM_NAME,
    actor: LEAD_NAME,
    subject,
    description,
    effort: EFFORT,
    why,
    deadline: DEADLINE_MINUTES,
    firstStep,
    deliverable,
  })
  const taskId = String(created.id)
  await assignTaskAsync({
    claudeDir,
    team: TEAM_NAME,
    actor: LEAD_NAME,
    taskId,
    owner: stage.owner,
    effort: EFFORT,
    why,
    deadline: DEADLINE_MINUTES,
    firstStep,
    deliverable,
    completionSignal: completionSignal(stage, taskId),
  })
  const record = taskRecord({ claudeDir, team: TEAM_NAME, actor: LEAD_NAME, taskId })
  const assignedStage = {
    ...stage,
    taskId,
    assignedAt: record?.metadata?.assigned_at ?? null,
  }
  assignedStage.deliveryPromise = captureStageDelivery({
    taskId,
    owner: stage.owner,
    timeout: DELIVERY_TIMEOUT_MS,
    waitUntil: (predicate, options) => browser.waitUntil(predicate, options),
    refreshTask: () => taskRecord({ claudeDir, team: TEAM_NAME, actor: LEAD_NAME, taskId }),
    readAttention: () => attentionRecord({ claudeDir, team: TEAM_NAME, taskId }),
  }).then((deliveredAt) => {
    assignedStage.deliveredAt = deliveredAt
    return deliveredAt
  })
  return assignedStage
}

async function waitForStageCompletion(stage) {
  let found = null
  let record = null
  await browser.waitUntil(
    async () => {
      const leadInbox = readInbox({ claudeDir, team: TEAM_NAME, member: LEAD_NAME })
      const blocked = findBlockedMessage(leadInbox, stage.taskId)
      if (blocked) throw new Error(`${stage.owner} blocked task #${stage.taskId}: ${blocked.reason}`)
      found = findResultMessage(leadInbox, stage.taskId)
      record = taskRecord({ claudeDir, team: TEAM_NAME, actor: LEAD_NAME, taskId: stage.taskId })
      return Boolean(found && record?.status === 'completed' && record?.completion?.kind === 'result')
    },
    {
      timeout: RESULT_TIMEOUT_MS,
      interval: 3_000,
      timeoutMsg: `${stage.owner} did not return RESULT and complete task #${stage.taskId}`,
    }
  )
  return { stage, result: found, record }
}

function assignmentNoticeTaskIds(memberName) {
  return taskAssignmentNoticeIds(readInbox({ claudeDir, team: TEAM_NAME, member: memberName }))
}

/**
 * Put a production-shaped completed summary under a synthetic scanner session,
 * then read it back through the production W2 IPC scanner.
 *
 * The experiment assigns the two mesh stages directly so its only paid
 * sessions are the two Codex members. The credential-free lead emits no
 * Workflow summary. This fixture records the two real task ids and RESULT
 * timestamps, derives its vocabulary from the production workflow emitter,
 * and proves only the scanner contract over that synthesized input.
 */
async function readScannerContractRunTree(completed, overlap) {
  const sessionId = `e2e-scanner-contract-${uniqueSuffix}`
  const runId = `w4-exp5-${uniqueSuffix}`
  const sessionDir = join(claudeDir, 'projects', `e2e-parallel-scanner-${uniqueSuffix}`, sessionId)
  const runDir = join(sessionDir, 'subagents', 'workflows', runId)
  const workflowDir = join(sessionDir, 'workflows')
  const scriptDir = join(workflowDir, 'scripts')
  mkdirSync(runDir, { recursive: true })
  mkdirSync(scriptDir, { recursive: true })

  const stageRecords = completed.map(({ stage, record }) => ({
    key: stage.key,
    taskId: stage.taskId,
    model: measured.model,
    resultAt: record.completion.at,
  }))
  const startedAt = new Date(Math.min(...completed.map(({ stage }) => Date.parse(stage.assignedAt)))).toISOString()
  const finishedAt = new Date(Math.max(...stageRecords.map((stage) => Date.parse(stage.resultAt)))).toISOString()
  const summary = completedParallelRunSummary({
    runId,
    workflowName: 'feature-pr-parallel-isolation',
    startedAt,
    finishedAt,
    stages: stageRecords,
    vocabulary: MANAGED_STAGE_VOCABULARY,
  })

  const scriptPath = join(scriptDir, `feature-pr-parallel-isolation-${runId}.js`)
  writeFileSync(
    scriptPath,
    `export const meta = { name: 'feature-pr-parallel-isolation', description: 'W4 experiment 5', phases: [{ title: ${JSON.stringify(MANAGED_STAGE_VOCABULARY.phaseTitle)} }] }\n`
  )
  writeFileSync(join(workflowDir, `${runId}.json`), `${JSON.stringify(summary, null, 2)}\n`)

  const listed = await invokeTauriOrThrow('list_workflow_runs', { sessionId })
  const listedRun = (Array.isArray(listed) ? listed : []).find((run) => run?.run_id === runId)
  expect(listedRun?.status).toBe('completed')
  expect(listedRun?.totals?.agents).toBe(2)

  const run = await invokeTauriOrThrow('get_workflow_run', { sessionId, runId })
  expect(run.status).toBe('completed')
  expect(run.phases).toEqual([MANAGED_STAGE_VOCABULARY.phaseTitle])
  expect(run.agents.map((agent) => agent.label).sort()).toEqual(
    stages.map((stage) => `${MANAGED_STAGE_VOCABULARY.labelPrefix}${stage.key}`).sort()
  )
  expect(
    run.agents.every(
      (agent) => agent.phase === MANAGED_STAGE_VOCABULARY.phaseTitle && agent.state === 'done'
    )
  ).toBe(true)
  expect(run.result?.tasks?.map(String).sort()).toEqual(stageRecords.map((stage) => stage.taskId).sort())
  expect(run.result?.evidenceSource).toBe('synthesized-scanner-contract')
  expect(resolve(run.script_path)).toBe(resolve(scriptPath))
  return {
    evidenceSource: run.result.evidenceSource,
    sessionId,
    runId,
    status: run.status,
    phase: run.phases[0],
    labels: run.agents.map((agent) => agent.label).sort(),
    overlap,
  }
}

function assertStageTreeEvidence(completed, sibling) {
  const { stage, result } = completed
  expect(resultContractViolations(result.payload)).toEqual([])
  const reportedCommit = String(result.payload.commit).toLowerCase()
  expect(reportedCommit).toMatch(/^[0-9a-f]{40}$/)
  expect(reportedCommit).not.toBe(fixtureBaseline)
  expect([...result.payload.files].sort()).toEqual([stage.modulePath, stage.testPath].sort())

  const evidence = worktreeTreeDiff(stage.worktree, fixtureBaseline)
  expect(evidence.headCommit).toBe(reportedCommit)
  expect(evidence.workingTreeStatus).toBe('')
  expect(evidence.entries).toEqual([
    { status: 'A', path: stage.modulePath },
    { status: 'A', path: stage.testPath },
  ])
  expect(existsSync(join(stage.worktree, sibling.modulePath))).toBe(false)
  expect(existsSync(join(stage.worktree, sibling.testPath))).toBe(false)

  const validation = runFixtureTestsAtCommit(stage.worktree, reportedCommit, { root: sessionTempRoot })
  if (!validation.passed) {
    throw new Error(`${stage.owner}'s tests fail at ${reportedCommit}:\n${validation.output}`)
  }
  return { reportedCommit, evidence, validation: validation.command }
}

describe('parallel managed Codex stages', function () {
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

    for (const stage of stages) trustProject(join(codexHome, 'config.toml'), stage.worktree)
    const stateDatabase = codexStateDatabaseDiagnostic(codexHome)
    console.log(`[e2e] Codex state DB before managed member launches: ${stateDatabase.path} exists=${stateDatabase.exists}`)
    measured.model = await initializeParallelTeam()
    measured.sessions = Object.fromEntries(
      stages.map((stage) => [stage.owner, readRuntimeRecord(stage.owner)?.session_id ?? null])
    )
    expect(new Set(Object.values(measured.sessions)).size).toBe(2)
    laneEnabled = true
  })

  after(async function () {
    this.timeout(180_000)
    if (Object.keys(measured).length > 0) {
      console.log(`[e2e] parallel managed stages measured: ${JSON.stringify(measured, null, 2)}`)
    }
    for (const teamName of createdTeamNames) {
      if (!teamName.startsWith('e2e-')) continue
      await invokeTauriWithTimeout('coordination_disband_team', { teamName }, 60_000)
    }
    createdTeamNames.clear()
    laneCleanup.run()
  })

  it('completes two assignments in isolated worktrees and inboxes', async function () {
    if (!laneEnabled) return this.skip()
    this.timeout(1_800_000)

    // Both create+assign pipelines are live together; neither waits for the
    // other task to be created, assigned, delivered, or completed.
    const assigned = await Promise.all(stages.map((stage) => createAndAssignStage(stage)))
    expect(new Set(assigned.map((stage) => stage.taskId)).size).toBe(2)
    for (const stage of assigned) {
      expect(Number.isFinite(Date.parse(stage.assignedAt))).toBe(true)
      const record = taskRecord({ claudeDir, team: TEAM_NAME, actor: LEAD_NAME, taskId: stage.taskId })
      expect(record.owner).toBe(stage.owner)
      expect(record.deadlineMinutes).toBe(String(DEADLINE_MINUTES))
      expect(record.metadata?.effort).toBe(EFFORT)
    }

    const [, completed] = await Promise.all([
      Promise.all(assigned.map((stage) => stage.deliveryPromise)),
      Promise.all(assigned.map((stage) => waitForStageCompletion(stage))),
    ])
    const byKey = Object.fromEntries(completed.map((entry) => [entry.stage.key, entry]))

    // Positive inbox evidence: each member retains exactly its own assignment
    // notice. This checks contents, not merely that no cross-delivery error was
    // logged.
    expect(assignmentNoticeTaskIds(MEMBER_ALPHA)).toEqual([byKey.alpha.stage.taskId])
    expect(assignmentNoticeTaskIds(MEMBER_BETA)).toEqual([byKey.beta.stage.taskId])

    const alphaTree = assertStageTreeEvidence(byKey.alpha, byKey.beta.stage)
    const betaTree = assertStageTreeEvidence(byKey.beta, byKey.alpha.stage)
    expect(alphaTree.reportedCommit).not.toBe(betaTree.reportedCommit)

    const completions = completed.map(({ stage, result, record }) => {
      expect(record.status).toBe('completed')
      expect(record.completion.kind).toBe('result')
      expect(record.completion.result).toEqual(result.payload)
      expect(record.completion.at).toBe(result.message.timestamp)
      expect(Number.isFinite(Date.parse(record.completion.at))).toBe(true)
      return { taskId: stage.taskId, at: record.completion.at }
    })
    expect(completions[0].at).not.toBe(completions[1].at)

    const windows = completed.map(({ stage, record }) => ({
      key: stage.key,
      assignedAt: stage.assignedAt,
      resultAt: record.completion.at,
    }))
    const deliveredWindows = completed.map(({ stage, record }) => ({
      key: stage.key,
      deliveredAt: stage.deliveredAt,
      resultAt: record.completion.at,
    }))
    const overlap = stageWindowOverlap(deliveredWindows[0], deliveredWindows[1])
    expect(overlap).not.toBeNull()
    expect(overlap.durationMs).toBeGreaterThan(0)

    // The production-shaped fixture uses the stage vocabulary parsed from the
    // workflow emitter. The scanner read-back below checks that contract; it
    // is not evidence that the credential-free lead emitted a Workflow run.
    const scannerContractRunTree = await readScannerContractRunTree(completed, overlap)

    Object.assign(measured, {
      team: TEAM_NAME,
      assignments: assigned.map(({ key, owner, taskId, assignedAt, worktree }) => ({
        key,
        owner,
        taskId,
        assignedAt,
        worktree,
      })),
      completions,
      windows,
      deliveredWindows,
      overlap,
      scannerContractRunTree,
      worktreeTrees: {
        alpha: alphaTree.evidence,
        beta: betaTree.evidence,
      },
      inboxTaskIds: {
        [MEMBER_ALPHA]: assignmentNoticeTaskIds(MEMBER_ALPHA),
        [MEMBER_BETA]: assignmentNoticeTaskIds(MEMBER_BETA),
      },
    })
  })

  it('records why the paid parallel-stage lane was unavailable', async function () {
    if (laneEnabled) return this.skip()
    expect(typeof laneSkipReason).toBe('string')
    expect(laneSkipReason.length).toBeGreaterThan(0)
    console.log(`[e2e] parallel managed-stage lane skipped: ${laneSkipReason}`)
  })
})
