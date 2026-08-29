/**
 * A managed Codex member completes a bounded task through the assignment
 * contract (Tier 2, Linux, paid). W4 experiment 3.
 *
 * The W4 design replaces the `codex exec` babysitter with a *stage*: an
 * assignment to a managed member. `mesh task create/assign` carries the effort,
 * the first step, the deliverable and the completion signal; mesh holds the
 * notice while the member is running at the wrong level; taurhaus resumes a
 * Codex member with `-c model_reasoning_effort="<level>"`; the member reports
 * back with one `RESULT <task-id>` message. This lane runs that whole path on a
 * real host and measures it, because a design that only works on paper is what
 * the experiment exists to rule out.
 *
 * Acceptance is what exists afterwards, not what taurhaus logged about itself:
 * the commit the member reports has to be in the fixture repo, and the test it
 * wrote has to pass when this lane runs it.
 *
 * Two cases, and they are not symmetric:
 *
 *   - the assignment asks for `medium` while the member is running at `low`, so
 *     mesh holds the notice (`pendingEffort: true`), taurhaus stops and resumes
 *     the member at `medium`, the runtime record's `appliedEffort` catches up
 *     and only then does mesh deliver. That ordering is the whole point of the
 *     gate and is asserted from timestamps, not from a delivery that merely
 *     happened.
 *   - a second assignment at the level the member is already running at is
 *     delivered with no hold and no relaunch, which proves the gate acts on a
 *     mismatch rather than on every assignment.
 *
 * "No `effort wait expired`" is asserted through its observable consequence
 * rather than through mesh's own log line: taurhaus spawns the member daemon
 * with `Stdio::null` (`coordination/runtime/process.rs`), so that line reaches
 * nobody. An expired wait delivers the notice *while* `pendingEffort` is still
 * true; a wait that closed properly delivers after `appliedEffort` matched.
 * This lane asserts the second ordering against mesh's own delivery record.
 *
 * It spends real Codex subscription turns, so `e2e/specList.js` keeps it out of
 * the config's spec list — no suite run picks it up — and it runs only as
 *
 *     E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-codex
 *
 * Isolation. `TAURHAUS_DATA_DIR` and `TAURHAUS_CLAUDE_DIR` are the wdio session
 * temp roots and `CODEX_HOME` is a scratch copy holding only `auth.json` plus a
 * generated `config.toml` (`e2e/helpers/codexScratchHome.js`). The real
 * `~/.codex` is read once, at copy time; the real `~/.claude` is never read or
 * written. `CLAUDE_DIR` is set on the panes as well, and that one is not
 * optional: the member runs `mesh` itself here, and taurhaus does not export a
 * Claude root into a member's pane (only the *daemon* it spawns gets
 * `--claude-dir`). Without it the member's own `mesh send` would resolve
 * `~/.claude` and bootstrap this run's team inside the operator's real home.
 * Every mesh command this lane issues also passes `--claude-dir` explicitly,
 * and so does the command the member is told to send, so the isolation does not
 * rest on the pane environment alone.
 *
 * The team lead is a Claude identity and an inbox, not a working agent: it is
 * launched into the isolated `CLAUDE_CONFIG_DIR`, which carries no credentials,
 * so it never takes a turn and this lane spends nothing on Claude. Its inbox is
 * a file mesh writes, which is all the completion signal needs.
 *
 * The tmux helpers below deliberately mirror `compaction-codex-hooks.js` rather
 * than being extracted from it: that lane costs money to re-run, so a shared
 * refactor could not be verified for it in the same change.
 */

import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'

import { ensureMainApp, waitForAppReady } from '../helpers.js'
import { waitForProjectsLoaded } from '../helpers/navigation.js'
import { createLaneCleanup } from '../helpers/laneCleanup.js'
import { readLogEventsSince, selectEvents } from '../helpers/compactionLog.js'
import { rolloutPaths } from '../helpers/codexRollout.js'
import { trustProject } from '../helpers/codexScratchHome.js'
import { TAURHAUS_CLAUDE_DIR } from '../helpers/platform.js'
import { commitExists, createStageFixtureProject, runFixtureTests } from '../helpers/stageFixtureProject.js'
import {
  assignTask,
  attentionRecord,
  createTask,
  findBlockedMessage,
  findResultMessage,
  readInbox,
  taskRecord,
} from '../helpers/meshTaskContract.js'

/** Codex's `-c model_reasoning_effort` resume path is the harness contract here. */
const MIN_CODEX_VERSION = [0, 147, 0]
/** `--effort`/`--why` on `task create/assign` and the pending-effort gate. */
const MIN_MESH_VERSION = [0, 2, 23]
/**
 * The mesh binary taurhaus itself runs.
 *
 * `coordination/mesh_cli.rs` resolves `~/.local/bin/mesh` by absolute path, not
 * through `PATH`, so the member daemon that holds the notice can be a different
 * build from the one this lane calls. Both are checked: on mesh 0.2.22 the
 * gate does not exist and `pendingEffort` is simply absent from `task get`,
 * which would fail this lane on an `undefined` instead of naming the cause.
 */
const TAURHAUS_MESH_BINARY = join(homedir(), '.local', 'bin', 'mesh')
/** The taurhaus-owned tmux session every managed pane is created in. */
const TMUX_SESSION = 'taurhaus'

/** The level the member is launched at, and the level the assignment asks for. */
const LAUNCH_EFFORT = 'low'
const ASSIGNED_EFFORT = 'medium'

const TEAM_READY_TIMEOUT_MS = 240_000
/** The onboarding turn, which is also what opens Codex's thread on disk. */
const ONBOARDING_TURN_TIMEOUT_MS = 120_000
/** How long the scanner gets to bind the member's rollout id on its own. */
const SESSION_BIND_TIMEOUT_MS = 150_000
/** Stop + resume + relaunch, measured end to end. */
const EFFORT_RESUME_TIMEOUT_MS = 300_000
/** From the gate opening to mesh putting the notice in the pane. */
const DELIVERY_TIMEOUT_MS = 180_000
/** The member's own working time on a one-function, one-test slice. */
const RESULT_TIMEOUT_MS = 1_200_000
/** Headroom for the effort pass to run before concluding it started nothing. */
const EFFORT_PASS_SETTLE_MS = 15_000

const dataDir = process.env.TAURHAUS_DATA_DIR || ''
const codexHome = process.env.CODEX_HOME || ''
const claudeDir = TAURHAUS_CLAUDE_DIR
const teamsDir = join(claudeDir, 'teams')
const appLogPath = join(dataDir, 'taurhaus.log.jsonl')
const codexNotifyPath = join(dataDir, 'codex-notify.jsonl')
/** The wdio session's temp root — every path this lane creates lives under it. */
const sessionTempRoot = dataDir ? dirname(dataDir) : ''
const projectsDir = process.env.E2E_PROJECTS_DIR || (sessionTempRoot ? join(sessionTempRoot, 'projects') : '')

/**
 * Roots a pane must carry.
 *
 * `CLAUDE_DIR` is derived, not read: nothing sets it in this process, and it is
 * what routes the member's own mesh calls away from the operator's real home.
 */
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
const LANE_PANES_STEP = 'lane-tmux-panes'

const uniqueSuffix = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`
const TEAM_NAME = `e2e-managed-stage-${uniqueSuffix}`
const LEAD_NAME = 'e2e-lead'
const MEMBER_NAME = 'codex-stage'

let mainApp = false
let laneEnabled = false
let laneSkipReason = 'managed Codex stage prerequisites unavailable'
let managed = null
let fixtureProject = ''
let fixtureSetupError = ''
const createdTeamNames = new Set()
/** Everything the run measured, printed once at teardown for the report. */
const measured = {}

// The fixture project has to exist before the first-run wizard scans
// `E2E_PROJECTS_DIR`, and the wizard runs inside this spec's `before` hook —
// so it is created at module load, which is earlier. It is a throwaway git repo
// under the session temp root and is deleted with it.
if (projectsDir) {
  try {
    fixtureProject = join(projectsDir, 'stage-fixture')
    mkdirSync(fixtureProject, { recursive: true })
    createStageFixtureProject(fixtureProject)
  } catch (error) {
    fixtureSetupError = String(error?.message ?? error)
    fixtureProject = ''
  }
}

function tmux(args) {
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

/** For teardown calls that must not hang the run if the backend is wedged. */
async function invokeTauriWithTimeout(command, args = undefined, timeoutMs = 10_000) {
  return await Promise.race([
    invokeTauri(command, args),
    new Promise((resolve) => {
      setTimeout(() => resolve({ ok: false, error: `Timed out after ${timeoutMs}ms` }), timeoutMs)
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

/** Why this host cannot run the lane, or an empty string when it can. */
function hostSkipReason() {
  if (process.platform !== 'linux') return `The managed-stage lane is Linux-only (got ${process.platform})`
  if (!dataDir) return 'TAURHAUS_DATA_DIR is not set for this session'
  if (!process.env.TAURHAUS_CLAUDE_DIR) return 'TAURHAUS_CLAUDE_DIR is not set for this session'
  if (!codexHome) return 'CODEX_HOME scratch copy was not prepared for this session'
  if (!existsSync(join(codexHome, 'auth.json'))) {
    return `no Codex credentials were copied into ${codexHome} (is ~/.codex/auth.json present?)`
  }
  if (fixtureSetupError) return `the stage fixture project could not be created: ${fixtureSetupError}`
  if (!fixtureProject) return 'E2E_PROJECTS_DIR is not set, so there is nowhere to put the fixture project'
  if (!commandExists('claude')) return 'claude CLI is not on PATH for the team lead'
  if (!commandExists('bun')) return 'bun is not on PATH, so the stage cannot validate its own deliverable'

  const codexVersion = parseVersion('codex', ['--version'])
  if (!codexVersion) return 'codex CLI is not on PATH'
  if (!versionAtLeast(codexVersion, MIN_CODEX_VERSION)) {
    return `codex ${codexVersion.join('.')} predates the ${MIN_CODEX_VERSION.join('.')} resume contract`
  }

  const meshVersion = parseVersion('mesh', ['--version'])
  if (!meshVersion) return 'mesh CLI is not on PATH'
  if (!versionAtLeast(meshVersion, MIN_MESH_VERSION)) {
    return `mesh ${meshVersion.join('.')} predates the ${MIN_MESH_VERSION.join('.')} pending-effort gate`
  }

  const managedMeshVersion = parseVersion(TAURHAUS_MESH_BINARY, ['--version'])
  if (!managedMeshVersion) return `taurhaus's own mesh binary is missing at ${TAURHAUS_MESH_BINARY}`
  if (!versionAtLeast(managedMeshVersion, MIN_MESH_VERSION)) {
    return (
      `${TAURHAUS_MESH_BINARY} is mesh ${managedMeshVersion.join('.')}, which predates the ` +
      `${MIN_MESH_VERSION.join('.')} pending-effort gate; taurhaus runs that binary, not the one on PATH ` +
      '(run `just install-mesh`)'
    )
  }
  return ''
}

function readLog(offset) {
  return readLogEventsSince(appLogPath, offset)
}

function currentLogOffset() {
  return readLog(0).offset
}

/** Poll the app log until `predicate` accepts the events seen since `offset`. */
async function waitForLogEvents(offset, predicate, { timeout, timeoutMsg }) {
  const collected = []
  let cursor = offset
  let matched = null

  await browser.waitUntil(
    async () => {
      const next = readLog(cursor)
      cursor = next.offset
      collected.push(...next.events)
      matched = predicate(collected)
      return Boolean(matched)
    },
    { timeout, interval: 1_000, timeoutMsg }
  )

  return { matched, events: collected }
}

function readRuntimeRecord(memberName = MEMBER_NAME) {
  try {
    return JSON.parse(readFileSync(join(teamsDir, TEAM_NAME, 'runtime', `${memberName}.json`), 'utf8'))
  } catch {
    return null
  }
}

/** Set the isolated roots on the shared taurhaus tmux session, returning a restore fn. */
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
      // `show-environment` prints `NAME=value` when the session sets it, `-NAME`
      // when the session explicitly removes it, and fails when the session says
      // nothing about it (the global environment then decides). `-u` deletes the
      // session entry, which is the restore for that last case; `-r` restores an
      // explicit removal.
      if (!shown) {
        tmuxQuietly(['set-environment', '-t', TMUX_SESSION, '-u', key])
        continue
      }
      if (shown.startsWith('-')) {
        tmuxQuietly(['set-environment', '-t', TMUX_SESSION, '-r', key])
        continue
      }
      tmuxQuietly(['set-environment', '-t', TMUX_SESSION, key, shown.slice(shown.indexOf('=') + 1)])
    }
  }
}

/**
 * Run `work` with the isolated roots visible to panes created inside it.
 *
 * The `taurhaus` tmux session is shared with whatever the operator is running,
 * so `work` is only ever a call that creates panes: team initialization, and
 * the effort resume, which takes the member's pane down and opens a new one.
 * Every second the override is up is a second in which a pane the operator
 * opens themselves inherits roots this run later deletes, so both windows are
 * closed the moment the pane exists.
 */
async function withPaneEnvironment(work) {
  const restore = applyPaneEnvironment()
  // The override outlives the process that set it — tmux keeps it until someone
  // unsets it — so it is owed back from the moment it goes up.
  laneCleanup.owe(PANE_ENVIRONMENT_STEP, restore)
  try {
    return await work()
  } finally {
    restore()
    laneCleanup.settled(PANE_ENVIRONMENT_STEP)
  }
}

/**
 * Kill the panes this lane put in the shared `taurhaus` tmux session.
 *
 * Selection is by working directory, not by "created after we started": the
 * session belongs to whatever the operator is running, and they do open panes
 * while a run is in flight. Every pane this lane creates lives inside the wdio
 * session's temp root, and nothing else does.
 */
function killLanePanes() {
  if (!sessionTempRoot) return
  const listed = tmuxQuietly(['list-panes', '-a', '-F', '#{pane_id}\t#{pane_current_path}'])
  if (!listed.ok) {
    console.log(`[e2e] managed-stage tmux cleanup skipped: ${listed.error}`)
    return
  }

  for (const line of listed.output.split('\n')) {
    const [paneId, path] = line.split('\t')
    if (!paneId || !path?.startsWith(sessionTempRoot)) continue
    const killed = tmuxQuietly(['kill-pane', '-t', paneId])
    console.log(`[e2e] ${killed.ok ? 'killed' : 'failed to kill'} lane pane ${paneId} (${path})`)
  }
}

/**
 * Boot the app, tolerating the splash-to-shell navigation racing the query.
 *
 * `waitForAppReady` opens with a bare element lookup, and this lane is a
 * single-spec session, so the query can land exactly as the splash unloads:
 * WebKit answers "no such frame" and the before hook dies before any of the
 * lane's own work starts.
 */
async function bootApp(attempts = 3) {
  for (let attempt = 1; ; attempt += 1) {
    try {
      await waitForAppReady()
      return
    } catch (error) {
      const message = String(error?.message ?? error)
      if (attempt >= attempts || !/no such frame|unload event|stale element/i.test(message)) throw error
      console.log(`[e2e] app boot query raced the splash transition; retrying (${message.split('\n')[0]})`)
      const handles = await browser.getWindowHandles().catch(() => [])
      if (handles.length > 0) await browser.switchToWindow(handles[0]).catch(() => {})
      await browser.pause(2_000)
    }
  }
}

/**
 * Visible contents of a pane, for logging and for blocking-prompt detection.
 *
 * A TUI mid-redraw captures as an empty screen, which would read as "no prompt
 * up" — the opposite of the truth — so this retries briefly for content.
 */
async function capturePane(paneId, attempts = 5) {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const captured = tmuxQuietly(['capture-pane', '-p', '-J', '-t', paneId])
    if (captured.ok && captured.output.trim()) return captured.output
    await browser.pause(500)
  }
  return ''
}

/**
 * Codex refuses to take a turn while an interactive first-run prompt is up —
 * directory trust, a deprecated-model migration, and whatever it adds next.
 * A member parked on one looks exactly like a member that is merely slow, so
 * the lane names it instead of spending its budget waiting.
 */
const BLOCKING_PANE_PROMPTS = [/Do you trust the contents/i, /will be deprecated/i, /press enter to (continue|confirm)/i]

function blockingPrompt(paneContents) {
  return BLOCKING_PANE_PROMPTS.some((pattern) => pattern.test(paneContents))
}

/** The newest Codex model the backend catalog still offers. */
async function pickCodexModel() {
  const settings = await invokeTauriOrThrow('get_settings')
  const contract = settings?.terminalContract ?? settings?.terminal_contract ?? {}
  const catalog = contract?.modelCatalog ?? contract?.model_catalog ?? {}
  const entries = Array.isArray(catalog?.codex) ? catalog.codex : []
  const usable = entries.find((entry) => entry?.deprecated !== true)
  if (!usable) throw new Error('The backend model catalog offers no supported Codex model')
  return usable.id
}

/** A Claude lead role and a Codex agent role from the template catalog. */
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

/**
 * The taurhaus project id for the fixture path.
 *
 * The task scan runs over registered projects, and the effort pass is driven by
 * that scan — so the fixture has to be a project taurhaus knows, and the lane
 * has to be able to ask for a refresh by id.
 */
async function fixtureProjectId() {
  const projects = await invokeTauriOrThrow('list_projects')
  const match = (Array.isArray(projects) ? projects : []).find((project) => project?.path === fixtureProject)
  if (!match) {
    throw new Error(
      `The fixture project ${fixtureProject} was not registered by the wizard; ` +
        `known paths: ${(projects ?? []).map((project) => project?.path).join(', ') || 'none'}`
    )
  }
  return match.id
}

/**
 * Ask taurhaus to rescan the fixture project's tasks now.
 *
 * The scan is what makes a mesh assignment visible to taurhaus — it rewrites
 * the operational snapshots and then runs the effort pass — and it is otherwise
 * driven by a filesystem watcher. Asking directly removes the watcher from the
 * critical path of a lane that costs money to repeat.
 */
async function refreshFixtureTasks(projectId) {
  await invokeTauriOrThrow('get_project_tasks', { projectId })
}

/**
 * Turns Codex has finished, counted from its own notify sink.
 *
 * Managed launches point Codex's `notify` at the daemon, which appends one
 * `agent-turn-complete` record per turn to `<data dir>/codex-notify.jsonl`.
 * Under a scratch `CODEX_HOME` that is the only turn signal available: the
 * roster's session status is bound to a rollout id the scanner may not have.
 */
function completedTurns() {
  try {
    return readFileSync(codexNotifyPath, 'utf8').split('\n').filter((line) => line.trim()).length
  } catch {
    return 0
  }
}

/** Wait for a Codex turn to finish, reporting rather than throwing on a miss. */
async function waitForTurnAfter(previousTurns, timeoutMs) {
  try {
    await browser.waitUntil(async () => completedTurns() > previousTurns, {
      timeout: timeoutMs,
      interval: 1_000,
      timeoutMsg: 'no turn completed',
    })
    return true
  } catch {
    return false
  }
}

/**
 * Make sure the member has finished at least one turn.
 *
 * Two things depend on it. Codex opens its thread — and writes the rollout the
 * effort switch resumes — on its first turn, so a member that has taken none
 * has no conversation to switch. And a member that cannot take a turn at all
 * (an exhausted subscription, a prompt this lane does not know to answer)
 * cannot do a stage either, and should say so here rather than time out later
 * with the assignment already paid for.
 *
 * The first nudge is a bare Enter, which submits an onboarding message left
 * sitting in the composer when its submit key landed while Codex was still
 * starting, and is a no-op on an empty one. If that produced no turn, the lane
 * asks for the cheapest possible reply instead.
 */
async function ensureMemberHasTakenATurn(paneId) {
  if (completedTurns() > 0) return 'already'

  tmuxQuietly(['send-keys', '-t', paneId, 'Enter'])
  if (await waitForTurnAfter(0, ONBOARDING_TURN_TIMEOUT_MS)) return 'onboarding'

  console.log('[e2e] the onboarding message produced no turn; asking for a one-word reply')
  tmuxQuietly(['send-keys', '-t', paneId, '-l', 'Reply with only the word READY.'])
  await browser.pause(600)
  tmuxQuietly(['send-keys', '-t', paneId, 'Enter'])
  if (await waitForTurnAfter(0, ONBOARDING_TURN_TIMEOUT_MS)) return 'prompted'

  throw new Error(
    `${MEMBER_NAME} finished no Codex turn within ${2 * ONBOARDING_TURN_TIMEOUT_MS}ms, so it has ` +
      `opened no conversation and cannot do a stage. Pane ${paneId}:\n${(await capturePane(paneId)).trimEnd()}`
  )
}

/** Initialize a Claude-led team with one managed Codex member and wait for its pane. */
async function initializeManagedStageTeam() {
  const { leadRoleId, agentRoleId } = await pickRoleIds()
  const model = await pickCodexModel()
  console.log(`[e2e] managed Codex member will run ${model} at effort ${LAUNCH_EFFORT}`)

  createdTeamNames.add(TEAM_NAME)
  // Panes appear inside the call below and outlive a killed run, so the undo is
  // owed before the first one exists rather than after the last one is found.
  laneCleanup.owe(LANE_PANES_STEP, killLanePanes)

  const report = await withPaneEnvironment(async () =>
    await invokeTauriOrThrow('coordination_initialize_team', {
      request: {
        teamName: TEAM_NAME,
        teamDescription: 'E2E lane for a managed Codex stage through the assignment contract',
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

  let paneId = null
  await browser.waitUntil(
    async () => {
      paneId = readRuntimeRecord()?.pane_id ?? null
      if (paneId) return true
      const status = await invokeTauriOrThrow('coordination_get_live_team_status', { teamName: TEAM_NAME })
      const member = (status?.members ?? []).find((entry) => entry?.name === MEMBER_NAME)
      paneId = member?.paneId ?? member?.pane_id ?? null
      return Boolean(paneId)
    },
    {
      timeout: TEAM_READY_TIMEOUT_MS,
      interval: 2_000,
      timeoutMsg: `Managed Codex member ${MEMBER_NAME} never reported a pane`,
    }
  )

  const paneContents = await capturePane(paneId)
  console.log(`[e2e] ${MEMBER_NAME} pane ${paneId} on launch:\n${paneContents.trimEnd()}`)
  if (blockingPrompt(paneContents)) {
    throw new Error(`Codex is parked on an interactive prompt and will not take a turn:\n${paneContents.trimEnd()}`)
  }

  const firstTurn = await ensureMemberHasTakenATurn(paneId)
  console.log(`[e2e] ${MEMBER_NAME} finished its first turn (${firstTurn})`)

  const sessionBinding = await bindMemberSession()
  return { paneId, sessionBinding }
}

/**
 * Make sure the member's runtime record names a conversation to resume.
 *
 * The effort switch resumes the member's own Codex conversation and refuses
 * outright without a session id (`pending_member_effort`, "member has no
 * recorded session to resume"), so this is a precondition of the experiment
 * rather than part of what it measures. taurhaus binds the id itself — the
 * launch pass first, the liveness reconciliation afterwards — and this waits
 * for that. If it does not arrive, the id is read out of the scratch home's own
 * rollout transcripts and written into the record, which is exactly the value
 * the scanner would have written; the run reports which of the two happened so
 * the measurement is never silently propped up.
 */
async function bindMemberSession() {
  try {
    await browser.waitUntil(
      async () => {
        // The liveness pass that reconciles a late session id runs on this call.
        await invokeTauri('coordination_get_live_team_status', { teamName: TEAM_NAME })
        return Boolean(readRuntimeRecord()?.session_id)
      },
      { timeout: SESSION_BIND_TIMEOUT_MS, interval: 3_000, timeoutMsg: 'no rollout id' }
    )
    return 'scanner'
  } catch {
    const rollout = newestScratchRolloutForFixture()
    if (!rollout) {
      throw new Error(
        `${MEMBER_NAME} has no recorded Codex session and no rollout under ${codexHome} names ` +
          `${fixtureProject}; the effort switch would refuse to resume it.`
      )
    }
    const path = join(teamsDir, TEAM_NAME, 'runtime', `${MEMBER_NAME}.json`)
    const record = JSON.parse(readFileSync(path, 'utf8'))
    record.session_id = rollout.sessionId
    record.jsonl_path = rollout.path
    writeFileSync(path, `${JSON.stringify(record, null, 2)}\n`)
    console.log(`[e2e] the scanner did not bind a rollout id; bound ${rollout.sessionId} from ${rollout.path}`)
    return 'lane-bound'
  }
}

/** The newest scratch-home rollout whose `session_meta` names the fixture project. */
function newestScratchRolloutForFixture() {
  let newest = null
  for (const path of rolloutPaths(codexHome)) {
    let head
    try {
      head = readFileSync(path, 'utf8').split('\n', 1)[0]
    } catch {
      continue
    }
    let cwd
    try {
      cwd = JSON.parse(head)?.payload?.cwd
    } catch {
      continue
    }
    if (cwd !== fixtureProject) continue

    // `rollout-<19-char timestamp>-<uuid>.jsonl`; the id is the uuid tail, the
    // same slice `session_scanner/idle/codex.rs` takes.
    const stem = path.split('/').pop().replace(/\.jsonl$/, '')
    if (!stem.startsWith('rollout-') || stem.length <= 28) continue
    const candidate = { sessionId: stem.slice(28), path, name: stem }
    if (!newest || candidate.name > newest.name) newest = candidate
  }
  return newest
}

function meshArgs() {
  return { claudeDir, team: TEAM_NAME, actor: LEAD_NAME }
}

/** The one message the member is told to send, spelled out so it cannot drift. */
function completionSignalFor(taskId, payloadShape) {
  return (
    `send exactly one message and nothing else: ` +
    `mesh send ${LEAD_NAME} 'RESULT #${taskId} ${payloadShape}' ` +
    `--team ${TEAM_NAME} --name ${MEMBER_NAME} --claude-dir ${claudeDir} --summary result`
  )
}

/** Create and assign one bounded stage task; returns its id and the assign time. */
function assignStageTask({ subject, description, firstStep, deliverable, payloadShape }) {
  const created = createTask({
    ...meshArgs(),
    subject,
    description,
    effort: ASSIGNED_EFFORT,
    why: 'experiment 3: bounded slice',
    firstStep,
    deliverable,
  })
  const taskId = String(created.id)
  const assignedAtMs = Date.now()
  assignTask({
    ...meshArgs(),
    taskId,
    owner: MEMBER_NAME,
    effort: ASSIGNED_EFFORT,
    why: 'experiment 3: bounded slice',
    firstStep,
    deliverable,
    completionSignal: completionSignalFor(taskId, payloadShape),
  })
  return { taskId, assignedAtMs }
}

/** mesh's own delivery record for a task, refreshed through the CLI. */
function deliveryRecord(taskId) {
  // Reading `task get` first is what rebuilds the projection this reads.
  taskRecord({ ...meshArgs(), taskId })
  return attentionRecord({ claudeDir, team: TEAM_NAME, taskId })
}

/**
 * Wait until mesh records the notice as delivered, and return its own record.
 *
 * Both timestamps in it are mesh's, so the hold is measured against the clock
 * that decided it rather than against this process's wall clock.
 */
async function waitForDelivery(taskId, timeout) {
  let record = null
  await browser.waitUntil(
    async () => {
      record = deliveryRecord(taskId)
      return Boolean(record?.deliveredAt)
    },
    {
      timeout,
      interval: 2_000,
      timeoutMsg: `mesh never delivered the notice for task #${taskId}`,
    }
  )
  return {
    assignedAtMs: Date.parse(record.assignedAt),
    deliveredAtMs: Date.parse(record.deliveredAt),
    deliveryState: record.deliveryState,
  }
}

/** The member's `RESULT`, failing fast on a `BLOCKED` instead of waiting it out. */
async function waitForResult(taskId, timeout) {
  let found = null
  await browser.waitUntil(
    async () => {
      const messages = readInbox({ claudeDir, team: TEAM_NAME, member: LEAD_NAME })
      const blocked = findBlockedMessage(messages, taskId)
      if (blocked) throw new Error(`${MEMBER_NAME} reported a blocker on #${taskId}: ${blocked.reason}`)
      found = findResultMessage(messages, taskId)
      return Boolean(found)
    },
    {
      timeout,
      interval: 3_000,
      timeoutMsg: `${MEMBER_NAME} never sent RESULT #${taskId} to ${LEAD_NAME}`,
    }
  )
  return found
}

function effortResumeEvents(events, name) {
  return selectEvents(events, { event: name, match: { team_name: TEAM_NAME, member_name: MEMBER_NAME } })
}

describe('managed Codex stage', function () {
  this.timeout(900_000)

  before(async function () {
    this.timeout(900_000)
    await bootApp()
    mainApp = await ensureMainApp()
    if (!mainApp) {
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
    if (report.canInitialize === false || report.meshAvailable === false || report.tmuxAvailable === false || blockingErrors.length > 0) {
      laneSkipReason = blockingErrors[0] || 'Mesh or tmux unavailable'
      return
    }

    // Codex asks about directory trust for an unknown workspace before it will
    // take a turn, and `--yolo` does not answer that. The fixture project is
    // new every run, so the scratch config has to carry it.
    trustProject(join(codexHome, 'config.toml'), fixtureProject)

    const projectId = await fixtureProjectId()
    managed = { projectId, ...(await initializeManagedStageTeam()) }

    const runtime = readRuntimeRecord()
    expect(runtime?.appliedEffort).toBe(LAUNCH_EFFORT)
    measured.sessionBinding = managed.sessionBinding
    laneEnabled = true
  })

  after(async function () {
    this.timeout(180_000)

    if (Object.keys(measured).length > 0) {
      console.log(`[e2e] managed stage measured: ${JSON.stringify(measured, null, 2)}`)
    }

    for (const teamName of createdTeamNames) {
      if (!teamName.startsWith('e2e-')) continue
      await invokeTauriWithTimeout('coordination_disband_team', { teamName }, 60_000)
    }
    createdTeamNames.clear()

    // Whatever is still owed — the pane environment if the run aborted inside a
    // pane-creating window, the panes if disband did not take them with it — is
    // the same set an interrupt would have run, so run it through the same path.
    laneCleanup.run()
  })

  it('holds the assignment until the member is resumed at its effort, then completes it', async function () {
    if (!laneEnabled) return this.skip()
    this.timeout(1_800_000)

    const offset = currentLogOffset()
    let assigned = null
    let resumeEvents = []

    // One window with the isolated roots on the shared tmux session: the effort
    // resume opens the member's replacement pane inside it, and that pane needs
    // the roots as much as the first one did.
    await withPaneEnvironment(async () => {
      assigned = assignStageTask({
        subject: 'Add greet(name) to the stage fixture',
        description:
          'W4 experiment 3 bounded slice: one exported function and one test, committed in the fixture repo.',
        firstStep:
          `In ${fixtureProject}, create src/lib/greet.js whose only export is ` +
          'function greet(name) returning the string Hello, <name>! built with a template literal.',
        deliverable:
          `One git commit in ${fixtureProject} adding src/lib/greet.js and src/lib/greet.test.js. ` +
          "The test imports { greet } from './greet.js' and { expect, test } from 'bun:test' and asserts " +
          'greet("ada") === "Hello, ada!". Run bun test in that directory and make it pass before you commit. ' +
          'Install nothing and change no other file.',
        payloadShape:
          '{"commit":"<full 40-char sha of your commit>","files":["src/lib/greet.js","src/lib/greet.test.js"],"validation":"bun test passed"}',
      })

      // mesh holds the notice: the member is running at `low` and the
      // assignment asks for `medium`.
      const held = taskRecord({ ...meshArgs(), taskId: assigned.taskId })
      expect(held.pendingEffort).toBe(true)
      expect(readRuntimeRecord()?.appliedEffort).toBe(LAUNCH_EFFORT)
      expect(deliveryRecord(assigned.taskId)?.deliveredAt ?? null).toBeNull()

      // The task scan is what makes the assignment visible to taurhaus and runs
      // the effort pass; ask for it rather than waiting on the watcher.
      await refreshFixtureTasks(managed.projectId)

      const started = await waitForLogEvents(
        offset,
        (events) => effortResumeEvents(events, 'effort.resume.started')[0] ?? null,
        { timeout: EFFORT_RESUME_TIMEOUT_MS, timeoutMsg: 'taurhaus never started the Codex effort resume' }
      )
      expect(started.matched.effort).toBe(ASSIGNED_EFFORT)
      expect(started.matched.previous_effort).toBe(LAUNCH_EFFORT)

      const completed = await waitForLogEvents(
        offset,
        (events) => effortResumeEvents(events, 'effort.resume.completed')[0] ?? null,
        { timeout: EFFORT_RESUME_TIMEOUT_MS, timeoutMsg: 'the Codex effort resume never completed' }
      )
      resumeEvents = completed.events
      measured.effortResumeStartedAt = started.matched.ts ?? null
      measured.effortResumeCompletedAt = completed.matched.ts ?? null
    })

    // The relaunch carried the level in Codex's own flag.
    const rendered = selectEvents(resumeEvents, {
      event: 'launch.command.rendered',
      match: { team: TEAM_NAME, member: MEMBER_NAME, tool: 'codex' },
    })
    expect(rendered.length).toBeGreaterThan(0)
    const relaunch = rendered[rendered.length - 1]
    expect(relaunch.reasoning_effort).toBe(ASSIGNED_EFFORT)
    expect(relaunch.command).toContain(`model_reasoning_effort="${ASSIGNED_EFFORT}"`)
    expect(effortResumeEvents(resumeEvents, 'effort.resume.failed')).toEqual([])

    // The record mesh gates on caught up.
    await browser.waitUntil(async () => readRuntimeRecord()?.appliedEffort === ASSIGNED_EFFORT, {
      timeout: 60_000,
      interval: 1_000,
      timeoutMsg: `${MEMBER_NAME} never reported appliedEffort ${ASSIGNED_EFFORT}`,
    })
    const appliedAtMs = Date.now()
    expect(taskRecord({ ...meshArgs(), taskId: assigned.taskId }).pendingEffort).toBe(false)

    // ...and only then did mesh deliver. An expired effort wait would have
    // delivered while `pendingEffort` was still true, which is the failure this
    // ordering rules out; mesh's own "effort wait expired" line goes to a
    // discarded stdout and cannot be read. The boundary is the *start* of the
    // resume, not its completion: the relaunch writes `appliedEffort` inside the
    // pipeline, a moment before `effort.resume.completed` is emitted, so mesh is
    // entitled to deliver in between. What it may never do is deliver during the
    // hold, which is everything before the switch began.
    const delivery = await waitForDelivery(assigned.taskId, DELIVERY_TIMEOUT_MS)
    const resumeStartedAtMs = Date.parse(measured.effortResumeStartedAt)
    expect(delivery.deliveredAtMs).toBeGreaterThanOrEqual(resumeStartedAtMs)
    // `deliveredAt` is the timestamp; `deliveryState` is mesh's own word for
    // the same transition and must have left the held state with it.
    expect(delivery.deliveryState).not.toBe('pending')
    console.log(`[e2e] mesh delivery state for #${assigned.taskId}: ${delivery.deliveryState}`)

    const paneAfterResume = readRuntimeRecord()?.pane_id ?? null
    console.log(`[e2e] ${MEMBER_NAME} resumed into pane ${paneAfterResume}`)

    const result = await waitForResult(assigned.taskId, RESULT_TIMEOUT_MS)
    console.log(`[e2e] RESULT #${assigned.taskId}: ${result.message.text}`)

    // The acceptance signal: the work exists, and it holds up.
    expect(commitExists(fixtureProject, result.payload.commit)).toBe(true)
    const validation = runFixtureTests(fixtureProject)
    if (!validation.passed) {
      throw new Error(`The stage's own test does not pass:\n${validation.output}`)
    }

    const resultAtMs = Date.parse(result.message.timestamp)
    Object.assign(measured, {
      taskId: assigned.taskId,
      // Assignment to delivery: how long mesh held the notice.
      holdMs: delivery.deliveredAtMs - delivery.assignedAtMs,
      // Stop, relaunch, reattach: what the hold was spent on.
      resumeMs: Date.parse(measured.effortResumeCompletedAt) - resumeStartedAtMs,
      // Assignment to the level being in force, as taurhaus recorded it.
      appliedEffortMs: appliedAtMs - assigned.assignedAtMs,
      // Delivery to RESULT: the member's own working time.
      memberMs: resultAtMs - delivery.deliveredAtMs,
      totalMs: resultAtMs - delivery.assignedAtMs,
      commit: result.payload.commit,
      validation: validation.command,
    })
  })

  it('delivers a second assignment at the level the member already runs at, with no hold', async function () {
    if (!laneEnabled) return this.skip()
    this.timeout(600_000)

    // No pane is created here: the member is already at `medium`, so nothing is
    // relaunched and the shared tmux session is left alone.
    const offset = currentLogOffset()
    const assigned = assignStageTask({
      subject: 'Acknowledge the second assignment',
      description: 'W4 experiment 3 negative path: the effort gate must not act when there is no mismatch.',
      firstStep: 'Do not change any file.',
      deliverable: 'No code change and no commit.',
      payloadShape: '{"noop":true}',
    })

    expect(taskRecord({ ...meshArgs(), taskId: assigned.taskId }).pendingEffort).toBe(false)

    // Run the effort pass over the new assignment before reading the log back:
    // it is the task scan, not the delivery, that would start a switch.
    await refreshFixtureTasks(managed.projectId)
    const delivery = await waitForDelivery(assigned.taskId, DELIVERY_TIMEOUT_MS)
    await browser.pause(EFFORT_PASS_SETTLE_MS)

    // A task event runs the effort pass; with the level already in force it must
    // start nothing at all.
    const events = readLog(offset).events
    expect(effortResumeEvents(events, 'effort.resume.started')).toEqual([])
    expect(effortResumeEvents(events, 'effort.resume.failed')).toEqual([])
    expect(readRuntimeRecord()?.appliedEffort).toBe(ASSIGNED_EFFORT)

    measured.secondAssignmentHoldMs = delivery.deliveredAtMs - delivery.assignedAtMs
  })

  it('records why the managed Codex stage lane was unavailable', async function () {
    if (laneEnabled) return this.skip()
    expect(typeof laneSkipReason).toBe('string')
    expect(laneSkipReason.length).toBeGreaterThan(0)
    console.log(`[e2e] managed stage lane skipped: ${laneSkipReason}`)
  })
})
