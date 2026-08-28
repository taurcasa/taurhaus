/**
 * Live Codex compaction through the hook bridge (Tier 2, Linux, paid).
 *
 * This lane proves on a real host that a managed Codex member running with
 * `terminal.harness.codex_compaction = hooks` gets its restored-context card
 * back through `coordination/compact_hook.rs` — not through the JSONL
 * transcript tailer.
 *
 * Acceptance is what Codex does with the card, not what taurhaus logged about
 * itself: `compaction.codex_hook.delivered` is emitted before the response is
 * serialized and written to stdout, so the terminal assertion is that the
 * card's unique marker — a token that exists nowhere but this run's operational
 * snapshot — turns up in Codex's own rollout transcript.
 *
 * Two cases, because the two triggers do not behave the same. Measured on this
 * host with a probe that registered all three compaction hooks against a
 * scratch Codex home, on 0.149.0 and again on 0.150.1:
 *
 *   - automatic (`trigger: auto`): `PreCompact` → `PostCompact` →
 *     `SessionStart(source=compact)`. taurhaus registers the last of those, so
 *     the bridge runs, the card comes back on the hook's stdout, and Codex
 *     writes it into the rollout as a `developer` message. This is the case
 *     that matters in real use and the one that proves the path, so it runs
 *     first: Mocha bails on the first failure, and a manual case that fails
 *     must not take the delivery proof down with it.
 *   - manual (`/compact`, `trigger: manual`): `PreCompact` → `PostCompact` and
 *     *no* `SessionStart`. The bridge is never invoked. The second case pins
 *     that measured contract — Codex compacts (its own transcript boundary
 *     proves it) and nothing reaches the bridge — so the runbook's
 *     "operator-triggered `/compact`" stays documented as unusable for the hook
 *     path only while it is still true. If Codex starts sending `SessionStart`
 *     for a manual compaction, that case fails and both are updated.
 *
 * It costs real Codex (and Claude, for the team lead) subscription turns, so
 * `e2e/specList.js` keeps it out of the config's spec list — no suite run picks
 * it up — and it runs only as
 *
 *     E2E_INSTALL_DAEMON=1 just test-e2e-spec compaction-codex-hooks
 *
 * Isolation: `TAURHAUS_DATA_DIR` and `TAURHAUS_CLAUDE_DIR` are the wdio session
 * temp roots, and `CODEX_HOME` is a scratch copy holding only `auth.json` and
 * `config.toml` (see `e2e/helpers/codexScratchHome.js`). The real `~/.codex`
 * and `~/.claude` are read once, at copy time, and never written.
 *
 * The hook runs as its own process spawned by Codex, so it resolves the teams
 * dir and the log sink from *its* environment — which it inherits from the
 * pane. tmux panes inherit the session environment, so the isolated roots are
 * set on the shared `taurhaus` tmux session for the length of the one call that
 * creates panes, and removed again the moment it returns: the session belongs
 * to the operator, and anything they launch in it while the override is up
 * would be pointed at roots this run later deletes.
 *
 * Everything this lane changes outside its own temp root — that tmux session
 * environment, the panes it opens in the operator's session, the compaction
 * mode the settings IPC pushes to the operator's shared daemon — is taken on as
 * an undo with `laneCleanup` the moment the change is made. A run that costs
 * money and takes minutes is the one an operator interrupts, and an interrupt
 * never reaches Mocha's `after`: `wdio.conf.js` deletes the session temp root
 * and exits on the first SIGINT. The undos sit in front of that handler.
 */

import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded } from '../helpers/navigation.js'
import { createLaneCleanup } from '../helpers/laneCleanup.js'
import { DEFAULT_DAEMON_PORT, setDaemonCodexCompactionMode } from '../helpers/daemonCompaction.js'
import { readLogEventsSince, selectEvents } from '../helpers/compactionLog.js'
import { countCompactionBoundaries, pathsContainingMarker, rolloutPaths } from '../helpers/codexRollout.js'
import { setAutoCompactTokenLimit, trustProject } from '../helpers/codexScratchHome.js'
import { TAURHAUS_PROJECT_PATH, TAURHAUS_CLAUDE_DIR } from '../helpers/platform.js'

/** Codex gained the stable `SessionStart(source=compact)` hook in 0.147. */
const MIN_CODEX_VERSION = [0, 147, 0]
/** The taurhaus-owned tmux session every managed pane is created in. */
const TMUX_SESSION = 'taurhaus'
/** Roots the hook process must resolve from the pane it was spawned under. */
const PANE_ENV_KEYS = ['TAURHAUS_DATA_DIR', 'TAURHAUS_CLAUDE_DIR', 'CODEX_HOME']

/**
 * Cost bound for the automatic case. Codex auto-compacts when the thread
 * crosses `model_auto_compact_token_limit`; lowering it to 20k reaches the same
 * code path after a couple of turns instead of paying for a full ~250k-token
 * context window. Each filler file is ~130 KB, which is more than the lowered
 * threshold on its own: a probe on this host crossed it on the first turn. The
 * cap stops the case at six turns either way.
 */
const AUTO_COMPACT_TOKEN_LIMIT = 20_000
const AUTO_COMPACTION_MAX_TURNS = 6
const AUTO_COMPACTION_FILLER_LINES = 900

const HOOK_DELIVERY_TIMEOUT_MS = 150_000
/** Headroom for the hook process to run and flush after a compaction lands. */
const HOOK_SETTLE_MS = 15_000
/**
 * How long Codex gets to write the card into its rollout after the bridge
 * returned it. Measured on this host: the `developer` message carrying the
 * `additionalContext` was in the transcript within a second of the hook
 * running, so this is headroom, not a guess.
 */
const CARD_CONSUMED_TIMEOUT_MS = 60_000
const AUTO_TURN_TIMEOUT_MS = 90_000
const TEAM_READY_TIMEOUT_MS = 180_000
/**
 * Grace for the scanner to bind the member's rollout id. It is short on
 * purpose: the session source reads the tool's default home
 * (`~/.codex/sessions`), not `$CODEX_HOME`, so under this lane's scratch home
 * the id never arrives and the bridge matches on cwd instead.
 */
const SESSION_CAPTURE_GRACE_MS = 20_000

/**
 * The token that proves the card reached Codex.
 *
 * It goes into the operational snapshot's task id, so the bridge renders it into
 * the card (`Current task: #<id> — <subject>`) and nothing else in the run can
 * put it anywhere. Finding it in Codex's own rollout is the difference between
 * "taurhaus says it delivered" and "the member got its context back".
 */
const CONTEXT_MARKER = `taurhaus-e2e-restored-context-${Date.now()}-${Math.floor(Math.random() * 10_000)}`

const dataDir = process.env.TAURHAUS_DATA_DIR || ''
const codexHome = process.env.CODEX_HOME || ''
const teamsDir = join(TAURHAUS_CLAUDE_DIR, 'teams')
const appLogPath = join(dataDir, 'taurhaus.log.jsonl')
const codexNotifyPath = join(dataDir, 'codex-notify.jsonl')
/** The wdio session's temp root — every path this lane creates lives under it. */
const sessionTempRoot = dataDir ? dirname(dataDir) : ''

/**
 * Undos for host state this lane changes, in front of the handler that exits.
 *
 * Installed at module scope so that owing a step is all it takes to be on the
 * signal path — there is no second wiring step to forget.
 */
const laneCleanup = createLaneCleanup()
laneCleanup.install()

const PANE_ENVIRONMENT_STEP = 'tmux-session-environment'
const LANE_PANES_STEP = 'lane-tmux-panes'
const DAEMON_MODE_STEP = 'daemon-compaction-mode'

let mainApp = false
let laneEnabled = false
let laneSkipReason = 'Codex compaction prerequisites unavailable'
let originalSettings = null
let managed = null
const createdTeamNames = new Set()
const uniqueSuffix = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`

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

/** The Codex composer's prompt marker and its empty-state placeholder. */
const COMPOSER_MARKER = '\u203a'
const COMPOSER_PLACEHOLDER = 'Ask Codex to do anything'

function composerHolds(paneContents, text) {
  return paneContents.includes(`${COMPOSER_MARKER} ${text}`) || paneContents.includes(`${COMPOSER_MARKER}${text}`)
}

/**
 * Type one line into a Codex pane and make sure it was actually submitted.
 *
 * A slash command opens the TUI's command popup, and the first Enter only
 * accepts the completion — the text stays in the composer and a second Enter
 * sends it. Typing `/compact` and pressing Enter once therefore does nothing at
 * all, which is how the first live attempt at the manual case timed out. Enter
 * is re-sent until the composer no longer holds the text.
 */
async function sendPaneLine(paneId, text, attempts = 4) {
  tmux(['send-keys', '-t', paneId, '-l', text])
  await browser.pause(600)
  // Two Enters unconditionally: the first accepts the popup completion for a
  // slash command, the second sends it. On an empty composer the extra Enter is
  // a no-op, so a plain prompt is unaffected.
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    tmux(['send-keys', '-t', paneId, 'Enter'])
    await browser.pause(900)
    const pane = await capturePane(paneId)
    // An empty composer draws its placeholder; treat a capture that shows
    // neither the text nor the placeholder as "still redrawing" and try again,
    // because reading a half-drawn screen as success is how `/compact` was
    // left sitting unsent.
    if (composerHolds(pane, text)) continue
    if (pane.includes(COMPOSER_PLACEHOLDER)) return true
  }
  return !composerHolds(await capturePane(paneId), text)
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

function parseCodexVersion() {
  let raw
  try {
    raw = execFileSync('codex', ['--version'], { encoding: 'utf8', timeout: 10_000 })
  } catch {
    return null
  }
  const match = String(raw).match(/(\d+)\.(\d+)\.(\d+)/)
  return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : null
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
  if (process.platform !== 'linux') return `Codex compaction lane is Linux-only (got ${process.platform})`
  if (!dataDir) return 'TAURHAUS_DATA_DIR is not set for this session'
  if (!process.env.TAURHAUS_CLAUDE_DIR) return 'TAURHAUS_CLAUDE_DIR is not set for this session'
  if (!codexHome) return 'CODEX_HOME scratch copy was not prepared for this session'
  if (!existsSync(join(codexHome, 'auth.json'))) {
    return `no Codex credentials were copied into ${codexHome} (is ~/.codex/auth.json present?)`
  }
  if (!commandExists('claude')) return 'claude CLI is not on PATH for the team lead'
  const version = parseCodexVersion()
  if (!version) return 'codex CLI is not on PATH'
  if (!versionAtLeast(version, MIN_CODEX_VERSION)) {
    return `codex ${version.join('.')} predates the ${MIN_CODEX_VERSION.join('.')} hook contract`
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

function readRuntimeRecord(teamName, memberName) {
  try {
    return JSON.parse(readFileSync(join(teamsDir, teamName, 'runtime', `${memberName}.json`), 'utf8'))
  } catch {
    return null
  }
}

/**
 * Give the member a resumable task context.
 *
 * The bridge refuses to reinject without one (`no_resumable_task_context`), and
 * taurhaus derives the snapshot from owned project tasks, which this fixture
 * project has none of. Writing the store record directly is the smallest thing
 * that makes the delivery path reachable.
 */
function writeOperationalSnapshot(teamName, memberName, projectPath) {
  const dir = join(teamsDir, teamName, 'state', 'operational')
  mkdirSync(dir, { recursive: true })
  writeFileSync(
    join(dir, `${memberName}.json`),
    `${JSON.stringify({
      version: 1,
      team_name: teamName,
      member_name: memberName,
      updated_at: new Date().toISOString(),
      task: {
        id: CONTEXT_MARKER,
        subject: 'Verify Codex compaction reinjection through the hook bridge',
        status: 'in_progress',
      },
      assignment_footer: {
        execution_mode: 'implement',
        file_ownership_boundary: ['README.md'],
        adjacent_fix_policy: 'no',
        validation_expectation: 'none',
        response_expectation: 'report-on-completion',
      },
      ownership: { override_allowed: false, active_override_reason: null },
      working_set: { project_path: projectPath, focal_files: ['README.md'] },
    }, null, 2)}\n`
  )
}

/** Set the isolated roots on the shared taurhaus tmux session, returning a restore fn. */
function applyPaneEnvironment() {
  tmuxQuietly(['new-session', '-d', '-s', TMUX_SESSION])

  const previous = new Map()
  for (const key of PANE_ENV_KEYS) {
    const value = process.env[key]
    // An empty value would hand the pane a broken root, which is worse than
    // leaving the operator's own environment in place.
    if (!value) continue
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
      const value = shown.slice(shown.indexOf('=') + 1)
      tmuxQuietly(['set-environment', '-t', TMUX_SESSION, key, value])
    }
  }
}

/**
 * Run `work` with the isolated roots visible to panes created inside it.
 *
 * The `taurhaus` tmux session is shared with whatever the operator is running,
 * so `work` must be the pane-creating call and nothing else: every second the
 * override is up is a second in which a pane the operator opens themselves
 * inherits this run's temp roots, which are deleted when the run ends. Readiness
 * polling, live status and session-id capture all happen after the restore.
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
    console.log(`[e2e] codex compaction tmux cleanup skipped: ${listed.error}`)
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
 * single-spec session, so the query lands exactly as the splash unloads: WebKit
 * answers "no such frame: Callback was not called before the unload event" and
 * the before hook dies before any of the lane's own work starts.
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
      // The lost frame sticks to the session, so re-attach to the window before
      // querying again — otherwise every retry fails the same way.
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

/**
 * The newest Codex model the backend catalog still offers.
 *
 * The bundled `codex-developer` role pins a model Codex has since deprecated,
 * and a member launched on it stops at a migration prompt before its first
 * turn. The catalog is ordered newest first and marks what is retired.
 */
async function pickCodexModel() {
  const settings = await invokeTauriOrThrow('get_settings')
  const contract = settings?.terminalContract ?? settings?.terminal_contract ?? {}
  const catalog = contract?.modelCatalog ?? contract?.model_catalog ?? {}
  const entries = Array.isArray(catalog?.codex) ? catalog.codex : []
  const usable = entries.find((entry) => entry?.deprecated !== true)
  if (!usable) throw new Error('The backend model catalog offers no supported Codex model')
  return {
    model: usable.id,
    reasoningEffort: usable.defaultEffort ?? usable.default_effort ?? null,
  }
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

function codexCompactionMode(settings) {
  const harness = settings?.terminal?.harness ?? {}
  return harness.codexCompaction ?? harness.codex_compaction ?? null
}

/**
 * Hand the daemon back the mode it was running.
 *
 * `update_settings` pushes the mode to the connected daemon, and on this host
 * that is the operator's own daemon on 17233 — the isolated `TAURHAUS_DATA_DIR`
 * does not insulate it. The settings IPC puts it back on a clean teardown; this
 * is the same restoration for the paths where there is no app left to ask.
 */
function restoreDaemonCompactionMode(mode) {
  const port = originalSettings?.daemon?.port ?? DEFAULT_DAEMON_PORT
  const result = setDaemonCodexCompactionMode(mode, { port })
  console.log(
    result.ok
      ? `[e2e] daemon compaction mode put back to ${mode}`
      : `[e2e] daemon compaction mode may still be "hooks" — restoring ${mode} failed: ${result.error}`
  )
}

async function setCodexCompactionMode(mode) {
  const settings = await invokeTauriOrThrow('get_settings')
  const next = JSON.parse(JSON.stringify(settings))
  next.terminal = next.terminal ?? {}
  next.terminal.harness = { ...(next.terminal.harness ?? {}), codexCompaction: mode }
  await invokeTauriOrThrow('update_settings', { settings: next })
}

/** Initialize a Claude-led team with one managed Codex member and wait for its pane. */
async function initializeManagedCodexTeam() {
  const teamName = `e2e-codex-compaction-${uniqueSuffix}`
  const memberName = 'codex-compaction-agent'
  const { leadRoleId, agentRoleId } = await pickRoleIds()
  const codexModel = await pickCodexModel()
  console.log(`[e2e] managed Codex member will run ${codexModel.model} (effort ${codexModel.reasoningEffort ?? 'default'})`)

  let paneId = null
  let sessionId = null
  createdTeamNames.add(teamName)
  // Panes appear inside the call below and outlive a killed run, so the undo is
  // owed before the first one exists rather than after the last one is found.
  laneCleanup.owe(LANE_PANES_STEP, killLanePanes)

  // Only this call creates panes (`pipelines/initialize.rs` launches each member
  // inline and records its pane id before returning), so it is the only thing
  // the shared session's environment is redirected for.
  const report = await withPaneEnvironment(async () =>
    await invokeTauriOrThrow('coordination_initialize_team', {
      request: {
        teamName,
        teamDescription: 'E2E lane for Codex compaction through the hook bridge',
        leadMode: 'launch_new',
        lead: {
          name: 'e2e-lead',
          cliTool: 'claude',
          model: '',
          projectId: TAURHAUS_PROJECT_PATH,
          roleId: leadRoleId,
        },
        agents: [
          {
            name: memberName,
            cliTool: 'codex',
            model: codexModel.model,
            reasoningEffort: codexModel.reasoningEffort,
            projectId: TAURHAUS_PROJECT_PATH,
            roleId: agentRoleId,
          },
        ],
      },
    })
  )

  if (report?.failedStep) {
    throw new Error(`Team initialization failed at ${report.failedStep}: ${report.message}`)
  }

  // The member's runtime record is the pane authority — it is what taurhaus
  // itself sends keys to. The live roster is only a fallback: it reports a
  // pane id from reconciliation, which stayed null for this member for three
  // minutes on the first live run while the pane was up the whole time.
  await browser.waitUntil(
    async () => {
      paneId = readRuntimeRecord(teamName, memberName)?.pane_id ?? null
      if (paneId) return true
      const status = await invokeTauriOrThrow('coordination_get_live_team_status', { teamName })
      const member = (status?.members ?? []).find((entry) => entry?.name === memberName)
      paneId = member?.paneId ?? member?.pane_id ?? null
      return Boolean(paneId)
    },
    {
      timeout: TEAM_READY_TIMEOUT_MS,
      interval: 2_000,
      timeoutMsg: `Managed Codex member ${memberName} never reported a pane`,
    }
  )

  // The rollout id is a nicety, not a requirement: the bridge falls back to
  // matching the payload's cwd against the member's project path.
  await browser.waitUntil(
    async () => {
      sessionId = readRuntimeRecord(teamName, memberName)?.session_id ?? null
      return Boolean(sessionId)
    },
    { timeout: SESSION_CAPTURE_GRACE_MS, interval: 2_000, timeoutMsg: 'no rollout id' }
  ).catch(() => {
    console.log(`[e2e] ${memberName} has no captured rollout id; the bridge will match on cwd`)
  })

  const paneContents = await capturePane(paneId)
  console.log(`[e2e] ${memberName} pane ${paneId} on launch:\n${paneContents.trimEnd()}`)
  if (blockingPrompt(paneContents)) {
    throw new Error(`Codex is parked on an interactive prompt and will not take a turn:\n${paneContents.trimEnd()}`)
  }

  // The onboarding message can be left sitting in the composer when its Enter
  // lands while Codex is still starting. Anything typed next would be appended
  // to it — `/compact` would go out as prose, not as a command — so submit
  // whatever is there first. An empty composer ignores a bare Enter.
  const turnsBeforeOnboarding = completedTurns()
  tmuxQuietly(['send-keys', '-t', paneId, 'Enter'])
  await waitForTurnAfter(turnsBeforeOnboarding)

  writeOperationalSnapshot(teamName, memberName, TAURHAUS_PROJECT_PATH)

  return { teamName, memberName, paneId, sessionId }
}

/**
 * Turns Codex has finished, counted from its own notify sink.
 *
 * Managed launches point Codex's `notify` at the daemon, which appends one
 * `agent-turn-complete` record per turn to `<data dir>/codex-notify.jsonl`.
 * That is the only turn signal this lane can rely on: the session scanner reads
 * the tool's default home, so under the scratch `CODEX_HOME` it never binds the
 * member and the roster's `sessionStatus` never moves.
 */
function completedTurns() {
  try {
    return readFileSync(codexNotifyPath, 'utf8').split('\n').filter((line) => line.trim()).length
  } catch {
    return 0
  }
}

/**
 * Best-effort wait for a turn to finish before typing into the pane again.
 *
 * `/compact` and the filler prompts go into a live TUI: sent mid-turn they are
 * appended to whatever is in the composer instead of acting on their own. This
 * never fails a case — a missing signal only means the lane types anyway.
 */
async function waitForTurnAfter(previousTurns, timeoutMs = 90_000) {
  try {
    await browser.waitUntil(async () => completedTurns() > previousTurns, {
      timeout: timeoutMs,
      interval: 1_000,
      timeoutMsg: 'no turn completed',
    })
    return true
  } catch {
    console.log(`[e2e] no Codex turn completed within ${timeoutMs}ms; continuing anyway`)
    return false
  }
}

/**
 * Compaction boundaries Codex has written to its own transcripts.
 *
 * This is the harness's own record that a compaction happened, independent of
 * whether any hook ran — which is exactly what the manual case needs in order
 * to tell "Codex did not compact" from "Codex compacted without calling us".
 */
function rolloutCompactionCount() {
  return countCompactionBoundaries(rolloutPaths(codexHome))
}

/** Transcripts in which Codex recorded the card the bridge handed back. */
function rolloutsWithCard() {
  return pathsContainingMarker(rolloutPaths(codexHome), CONTEXT_MARKER)
}

/**
 * Print every compaction event seen in a window.
 *
 * A failed run of this lane costs real turns to repeat, so a timeout has to
 * leave enough behind to diagnose without paying for a second one.
 */
function dumpCompactionEvents(label, events) {
  const compaction = selectEvents(events, { eventPrefix: 'compaction.' })
  console.error(`[e2e] ${label}: ${compaction.length} compaction event(s) seen`)
  for (const record of compaction) {
    console.error(
      `[e2e]   ${record.event} ${JSON.stringify({
        tool: record.tool ?? null,
        member_name: record.member_name ?? null,
        session_id: record.session_id ?? null,
        source: record.source ?? null,
        skip_reason: record.skip_reason ?? null,
        failure_stage: record.failure_stage ?? null,
        'error.message': record['error.message'] ?? null,
      })}`
    )
  }
}

/** The bridge's own acceptance evidence for one compaction of `memberName`. */
function hookDelivery(events, memberName) {
  const delivered = selectEvents(events, {
    event: 'compaction.codex_hook.delivered',
    match: { member_name: memberName },
  })
  return delivered.length > 0 ? delivered[delivered.length - 1] : null
}

/**
 * `exactlyOne` holds for a trigger the lane controls: one `/compact` must
 * produce one card. Codex's own auto-compaction can fire more than once inside
 * the window, so that case only requires at least one.
 */
function assertHookBridgeDelivered(events, { teamName, memberName }, { exactlyOne = true } = {}) {
  const received = selectEvents(events, { event: 'compaction.codex_hook.received' })
  const resolved = selectEvents(events, {
    event: 'compaction.codex_hook.resolved',
    match: { member_name: memberName },
  })
  const delivered = selectEvents(events, {
    event: 'compaction.codex_hook.delivered',
    match: { member_name: memberName },
  })

  expect(received.length).toBeGreaterThan(0)
  expect(resolved.length).toBeGreaterThan(0)
  if (exactlyOne) {
    expect(delivered).toHaveLength(1)
  } else {
    expect(delivered.length).toBeGreaterThan(0)
  }
  const last = delivered[delivered.length - 1]
  expect(last.tool).toBe('codex')
  expect(last.team_name).toBe(teamName)
  expect(last.additional_context_bytes).toBeGreaterThan(0)

  // The card reached the harness as a recorded injection, not a skip.
  const injected = selectEvents(events, {
    event: 'compaction.injected',
    match: { member_name: memberName, tool: 'codex' },
  })
  expect(injected.length).toBeGreaterThan(0)

  // …and the transcript tailer stayed out of it: in hooks mode the extractor
  // and its signal log own nothing for this member.
  expect(selectEvents(events, { event: 'compaction.signal_emitted', match: { member_name: memberName } })).toEqual([])
  expect(selectEvents(events, { event: 'compaction.detected', match: { member_name: memberName } })).toEqual([])
  expect(selectEvents(events, { eventPrefix: 'compaction.extractor.' })).toEqual([])

  // Nothing failed on the way out either. `delivered` is emitted before the
  // response is serialized and written, and a failure at that last step is
  // reported separately (`emit_compact_hook_cli_failed`, stage
  // `serialize_response`), so it has to be asserted separately too.
  const failures = [
    ...selectEvents(events, { event: 'compaction.codex_hook.failed' }),
    ...selectEvents(events, { event: 'compaction.compact_hook.failed' }),
  ]
  expect(failures.map((record) => record.failure_stage ?? record['error.message'] ?? 'unknown')).toEqual([])

  return last
}

/**
 * The acceptance signal: Codex took the card into its own conversation.
 *
 * Everything above is taurhaus reporting on taurhaus. The bridge emits
 * `delivered` before `run_compact_hook_cli` serializes the response and writes
 * it to stdout (`coordination/compact_hook.rs`), and nothing in this process
 * can see whether Codex read it — so the card carries a marker that exists
 * nowhere else, and this waits for that marker to appear in Codex's rollout.
 * Measured on this host: it lands as a `developer` message a second after the
 * hook returns.
 */
async function waitForCardInCodexTranscript() {
  try {
    await browser.waitUntil(async () => rolloutsWithCard().length > 0, {
      timeout: CARD_CONSUMED_TIMEOUT_MS,
      interval: 1_000,
      timeoutMsg: 'no rollout carried the card',
    })
  } catch {
    const transcripts = rolloutPaths(codexHome)
    throw new Error(
      `The bridge delivered the card but Codex never recorded it: marker ${CONTEXT_MARKER} ` +
        `is in none of the ${transcripts.length} rollout transcript(s) under ${codexHome} ` +
        `within ${CARD_CONSUMED_TIMEOUT_MS}ms. The member was told nothing, whatever the ` +
        'delivery events say.'
    )
  }

  const carrying = rolloutsWithCard()
  console.log(`[e2e] Codex recorded the restored-context card in ${carrying.join(', ')}`)
  return carrying
}

/** What Codex actually put on the wire, printed so a run can be read back. */
function reportHookPayload(label, events, memberName) {
  const received = selectEvents(events, { event: 'compaction.codex_hook.received' })
  const last = received[received.length - 1] ?? {}
  console.log(
    `[e2e] ${label} compaction hook payload for ${memberName}: ` +
      JSON.stringify({
        hook_event_name: last.hook_event_name ?? null,
        source: last.source ?? null,
        trigger: last.trigger ?? null,
        session_id: last.session_id ?? null,
        transcript_path: last.transcript_path ?? null,
      })
  )
}

function writeFillerFile(index) {
  const path = join(TAURHAUS_PROJECT_PATH, `e2e-compaction-filler-${index}.md`)
  const body = Array.from(
    { length: AUTO_COMPACTION_FILLER_LINES },
    (_, line) =>
      `- filler ${index}.${line}: the managed Codex member reads this line only to grow its thread context toward the lowered auto-compaction threshold.`
  ).join('\n')
  writeFileSync(path, `# Compaction filler ${index}\n\n${body}\n`)
  return `e2e-compaction-filler-${index}.md`
}

describe('Codex compaction via hooks', function () {
  this.timeout(600_000)

  before(async function () {
    this.timeout(600_000)
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
    trustProject(join(codexHome, 'config.toml'), TAURHAUS_PROJECT_PATH)
    // Bound the case before the member ever launches, so it reads the lowered
    // threshold from its first turn and no restart is needed to apply it.
    setAutoCompactTokenLimit(join(codexHome, 'config.toml'), AUTO_COMPACT_TOKEN_LIMIT)

    originalSettings = await invokeTauriOrThrow('get_settings')
    const previousMode = codexCompactionMode(originalSettings)
    // Owed before the flip, not after: a settings update that fails partway
    // through has still told the daemon.
    if (previousMode && previousMode !== 'hooks') {
      laneCleanup.owe(DAEMON_MODE_STEP, () => restoreDaemonCompactionMode(previousMode))
    }
    await setCodexCompactionMode('hooks')

    managed = await initializeManagedCodexTeam()

    // The managed hook is what makes this the hook path rather than the tailer.
    expect(existsSync(join(codexHome, 'hooks.json'))).toBe(true)
    expect(existsSync(join(codexHome, 'hooks', 'taurhaus-session-start-compact.sh'))).toBe(true)

    laneEnabled = true
  })

  after(async function () {
    this.timeout(120_000)

    if (originalSettings) {
      const restored = await invokeTauriWithTimeout('update_settings', { settings: originalSettings })
      // The app tells the daemon on its way through this call. A call that
      // failed or timed out did not, so the direct restoration stays owed.
      if (restored.ok) laneCleanup.settled(DAEMON_MODE_STEP)
      else console.warn(`[e2e] settings restore failed (${restored.error}); restoring the daemon mode directly`)
    }

    for (const teamName of createdTeamNames) {
      if (!teamName.startsWith('e2e-')) continue
      await invokeTauriWithTimeout('coordination_disband_team', { teamName }, 30_000)
    }
    createdTeamNames.clear()

    // Whatever is still owed — the pane environment if the run aborted inside
    // initialization, the panes disband did not take with it — is the same set
    // an interrupt would have run, so run it through the same path.
    laneCleanup.run()
  })

  it('delivers the restored-context card after Codex compacts on its own', async function () {
    if (!laneEnabled) return this.skip()
    this.timeout(600_000)

    const offset = currentLogOffset()
    let collected = []
    let turns = 0

    while (turns < AUTO_COMPACTION_MAX_TURNS && !hookDelivery(collected, managed.memberName)) {
      turns += 1
      const turnsBefore = completedTurns()
      const filler = writeFillerFile(turns)
      await sendPaneLine(managed.paneId, `Read ${filler} and reply with only the number of list items it contains.`)

      try {
        const seen = await waitForLogEvents(
          offset,
          (events) => hookDelivery(events, managed.memberName),
          { timeout: AUTO_TURN_TIMEOUT_MS, timeoutMsg: 'turn budget elapsed' }
        )
        collected = seen.events
      } catch {
        collected = readLog(offset).events
        // Let the turn land before typing the next prompt into the composer.
        await waitForTurnAfter(turnsBefore, 30_000)
      }
    }

    if (!hookDelivery(collected, managed.memberName)) {
      dumpCompactionEvents('automatic compaction cap reached', collected)
      console.error(`[e2e] pane at the cap:\n${(await capturePane(managed.paneId)).trimEnd()}`)
      // Which of the two failures this is decides where to look next: Codex not
      // compacting at all is a driving problem, Codex compacting without the
      // bridge hearing about it is a harness-contract problem.
      throw new Error(
        `Codex did not deliver a compaction card within the ${AUTO_COMPACTION_MAX_TURNS}-turn cap ` +
          `(model_auto_compact_token_limit = ${AUTO_COMPACT_TOKEN_LIMIT}); ` +
          `Codex wrote ${rolloutCompactionCount()} compaction boundary/boundaries in that window.`
      )
    }

    reportHookPayload('automatic', collected, managed.memberName)
    const delivered = assertHookBridgeDelivered(collected, managed, { exactlyOne: false })
    console.log(
      `[e2e] automatic compaction reached after ${turns} turn(s); card was ` +
        `${delivered.additional_context_bytes} bytes of additionalContext`
    )

    // The delivery events are diagnostics. This is the acceptance signal.
    await waitForCardInCodexTranscript()
  })

  it('compacts on a manual /compact without reaching the hook bridge', async function () {
    if (!laneEnabled) return this.skip()
    this.timeout(300_000)

    // A dead pane would spend the whole timeout looking like a slow compaction.
    const alive = tmuxQuietly(['display-message', '-p', '-t', managed.paneId, '#{pane_id}'])
    if (!alive.ok || alive.output !== managed.paneId) {
      throw new Error(`Managed member pane ${managed.paneId} is gone; nothing to compact (${alive.error ?? alive.output})`)
    }

    // Type into a settled composer: sent mid-turn, `/compact` is appended to
    // whatever is already there and goes out as prose. The previous case leaves
    // the turn that compacted still running, so this waits for it to finish and
    // shrugs if there was nothing in flight.
    await waitForTurnAfter(completedTurns(), 60_000)

    const boundariesBefore = rolloutCompactionCount()
    const offset = currentLogOffset()
    const submitted = await sendPaneLine(managed.paneId, '/compact')
    expect(submitted).toBe(true)

    // Codex's own transcript is the proof that a compaction happened at all.
    try {
      await browser.waitUntil(async () => rolloutCompactionCount() > boundariesBefore, {
        timeout: HOOK_DELIVERY_TIMEOUT_MS,
        interval: 2_000,
        timeoutMsg: `Codex wrote no compaction boundary within ${HOOK_DELIVERY_TIMEOUT_MS}ms of /compact`,
      })
    } catch (error) {
      console.error(`[e2e] pane when the manual compaction did not land:\n${(await capturePane(managed.paneId)).trimEnd()}`)
      dumpCompactionEvents('manual /compact produced no boundary', readLog(offset).events)
      throw error
    }

    // Give the bridge every chance to be called before concluding it was not.
    await browser.pause(HOOK_SETTLE_MS)
    const seen = readLog(offset).events
    const hookEvents = selectEvents(seen, { eventPrefix: 'compaction.codex_hook.' })
    console.log(
      `[e2e] manual /compact: Codex wrote ${rolloutCompactionCount() - boundariesBefore} compaction ` +
        `boundary/boundaries and the bridge saw ${hookEvents.length} hook event(s)`
    )
    dumpCompactionEvents('manual /compact', seen)

    // Pinned harness contract, measured on Codex 0.149.0 and 0.150.1: a manual
    // compaction fires PreCompact and PostCompact only. taurhaus registers
    // `SessionStart` with matcher `compact`, so the bridge is never invoked and
    // no card is produced. If this starts failing, Codex has changed and the
    // runbook's manual trigger became usable for the hook path — update both.
    expect(hookEvents).toEqual([])
    expect(selectEvents(seen, { eventPrefix: 'compaction.compact_hook.' })).toEqual([])
  })

  it('records why the live Codex compaction lane was unavailable', async function () {
    if (laneEnabled) return this.skip()
    expect(typeof laneSkipReason).toBe('string')
    expect(laneSkipReason.length).toBeGreaterThan(0)
    console.log(`[e2e] codex compaction lane skipped: ${laneSkipReason}`)
  })
})
