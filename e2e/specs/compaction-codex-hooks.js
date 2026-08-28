/**
 * Live Codex compaction through the hook bridge (Tier 2, Linux, paid).
 *
 * This lane proves on a real host that a managed Codex member running with
 * `terminal.harness.codex_compaction = hooks` gets its restored-context card
 * back through `coordination/compact_hook.rs` — not through the JSONL
 * transcript tailer. Both triggers are covered as separate cases: a manual
 * `/compact` typed into the member's pane, and Codex's own automatic
 * compaction.
 *
 * It costs real Codex (and Claude, for the team lead) subscription turns, so it
 * is excluded from `just test-e2e` and `just test-e2e-full` and runs only as
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
 * set on the shared `taurhaus` tmux session for the length of team
 * initialization and removed again straight after.
 */

import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded } from '../helpers/navigation.js'
import { snapshotTmuxPanes, cleanupNewTmuxPanes } from '../helpers/tmux.js'
import { readLogEventsSince, selectEvents } from '../helpers/compactionLog.js'
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
 * context window. Each filler file is ~40 KB, so a turn that reads one adds
 * roughly 10k tokens and the threshold is crossed within the first turns; the
 * cap stops the case at six either way.
 */
const AUTO_COMPACT_TOKEN_LIMIT = 20_000
const AUTO_COMPACTION_MAX_TURNS = 6
const AUTO_COMPACTION_FILLER_LINES = 320

const HOOK_DELIVERY_TIMEOUT_MS = 150_000
const AUTO_TURN_TIMEOUT_MS = 90_000
const TEAM_READY_TIMEOUT_MS = 180_000
/**
 * Grace for the scanner to bind the member's rollout id. It is short on
 * purpose: the session source reads the tool's default home
 * (`~/.codex/sessions`), not `$CODEX_HOME`, so under this lane's scratch home
 * the id never arrives and the bridge matches on cwd instead.
 */
const SESSION_CAPTURE_GRACE_MS = 20_000

const dataDir = process.env.TAURHAUS_DATA_DIR || ''
const codexHome = process.env.CODEX_HOME || ''
const teamsDir = join(TAURHAUS_CLAUDE_DIR, 'teams')
const appLogPath = join(dataDir, 'taurhaus.log.jsonl')

let mainApp = false
let laneEnabled = false
let laneSkipReason = 'Codex compaction prerequisites unavailable'
let originalSettings = null
let managed = null
let tmuxPaneSnapshot = { available: false, paneIds: [], reason: 'snapshot not captured' }
let restorePaneEnvironmentOnTeardown = null
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

function sendPaneLine(paneId, text) {
  tmux(['send-keys', '-t', paneId, '-l', text])
  tmux(['send-keys', '-t', paneId, 'Enter'])
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
        id: `e2e-compaction-${uniqueSuffix}`,
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
 * so the override lives for exactly as long as the pane-creating call and is
 * restored on every path out — including a failed initialization.
 */
async function withPaneEnvironment(work) {
  const restore = applyPaneEnvironment()
  restorePaneEnvironmentOnTeardown = restore
  try {
    return await work()
  } finally {
    restore()
    restorePaneEnvironmentOnTeardown = null
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
  await withPaneEnvironment(async () => {
    createdTeamNames.add(teamName)
    const report = await invokeTauriOrThrow('coordination_initialize_team', {
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

    if (report?.failedStep) {
      throw new Error(`Team initialization failed at ${report.failedStep}: ${report.message}`)
    }

    await browser.waitUntil(
      async () => {
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
  })

  const paneContents = await capturePane(paneId)
  console.log(`[e2e] ${memberName} pane ${paneId} on launch:\n${paneContents.trimEnd()}`)
  if (blockingPrompt(paneContents)) {
    throw new Error(`Codex is parked on an interactive prompt and will not take a turn:\n${paneContents.trimEnd()}`)
  }

  writeOperationalSnapshot(teamName, memberName, TAURHAUS_PROJECT_PATH)

  return { teamName, memberName, paneId, sessionId }
}

/**
 * Best-effort wait for the member to stop working.
 *
 * `/compact` and the filler prompts are typed into a live TUI: sent mid-turn
 * Codex queues them as input instead of acting on them. Activity comes from the
 * scanner, so this never fails the case — it just stops the lane from typing
 * into a busy pane when the signal is there.
 */
async function waitForMemberIdle(teamName, memberName, timeoutMs = 120_000) {
  try {
    await browser.waitUntil(
      async () => {
        const status = await invokeTauriOrThrow('coordination_get_live_team_status', { teamName })
        const member = (status?.members ?? []).find((entry) => entry?.name === memberName)
        return (member?.sessionStatus ?? member?.session_status) === 'idle'
      },
      { timeout: timeoutMs, interval: 2_000, timeoutMsg: 'not idle' }
    )
    return true
  } catch {
    console.log(`[e2e] ${memberName} never reported idle within ${timeoutMs}ms; sending anyway`)
    return false
  }
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

  return last
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
    tmuxPaneSnapshot = snapshotTmuxPanes()

    await waitForAppReady()
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

    originalSettings = await invokeTauriOrThrow('get_settings')
    await setCodexCompactionMode('hooks')

    managed = await initializeManagedCodexTeam()

    // The managed hook is what makes this the hook path rather than the tailer.
    expect(existsSync(join(codexHome, 'hooks.json'))).toBe(true)
    expect(existsSync(join(codexHome, 'hooks', 'taurhaus-session-start-compact.sh'))).toBe(true)

    laneEnabled = true
  })

  after(async function () {
    this.timeout(120_000)

    // Safety net: an abort mid-initialization must not leave the operator's
    // shared tmux session pointing at this run's temp roots.
    restorePaneEnvironmentOnTeardown?.()
    restorePaneEnvironmentOnTeardown = null

    if (originalSettings) {
      await invokeTauri('update_settings', { settings: originalSettings })
    }

    for (const teamName of createdTeamNames) {
      if (!teamName.startsWith('e2e-')) continue
      await invokeTauri('coordination_disband_team', { teamName })
    }
    createdTeamNames.clear()

    const tmuxCleanup = cleanupNewTmuxPanes(tmuxPaneSnapshot)
    if (!tmuxCleanup.attempted) {
      console.log(`[e2e] codex compaction tmux cleanup skipped: ${tmuxCleanup.skippedReason}`)
    } else if (tmuxCleanup.failed.length > 0) {
      console.warn(`[e2e] codex compaction tmux cleanup failures: ${JSON.stringify(tmuxCleanup.failed)}`)
    }
  })

  it('delivers the restored-context card after a manual /compact', async function () {
    if (!laneEnabled) return this.skip()
    this.timeout(300_000)

    await waitForMemberIdle(managed.teamName, managed.memberName)

    const offset = currentLogOffset()
    sendPaneLine(managed.paneId, '/compact')

    let events
    try {
      ;({ events } = await waitForLogEvents(
        offset,
        (collected) => hookDelivery(collected, managed.memberName),
        {
          timeout: HOOK_DELIVERY_TIMEOUT_MS,
          timeoutMsg: `No compaction.codex_hook.delivered for ${managed.memberName} within ${HOOK_DELIVERY_TIMEOUT_MS}ms of /compact`,
        }
      ))
    } catch (error) {
      dumpCompactionEvents('manual /compact timed out', readLog(offset).events)
      throw error
    }

    reportHookPayload('manual', events, managed.memberName)
    const delivered = assertHookBridgeDelivered(events, managed)
    console.log(`[e2e] manual compaction card: ${delivered.additional_context_bytes} bytes of additionalContext`)
  })

  it('delivers the restored-context card after Codex compacts on its own', async function () {
    if (!laneEnabled) return this.skip()
    this.timeout(600_000)

    // Bound the case: lower Codex's own auto-compaction threshold in the
    // scratch home, then restart the member so it reads the new config.
    setAutoCompactTokenLimit(join(codexHome, 'config.toml'), AUTO_COMPACT_TOKEN_LIMIT)

    const resumed = await withPaneEnvironment(async () => {
      tmuxQuietly(['kill-pane', '-t', managed.paneId])
      return await invokeTauriOrThrow('coordination_resume_member', {
        request: { teamName: managed.teamName, memberName: managed.memberName },
      })
    })
    expect(resumed?.resumed).toBe(true)
    const paneId = resumed?.paneId ?? resumed?.pane_id
    expect(paneId).toBeTruthy()
    managed.paneId = paneId

    writeOperationalSnapshot(managed.teamName, managed.memberName, TAURHAUS_PROJECT_PATH)

    const offset = currentLogOffset()
    let collected = []
    let turns = 0

    while (turns < AUTO_COMPACTION_MAX_TURNS && !hookDelivery(collected, managed.memberName)) {
      turns += 1
      await waitForMemberIdle(managed.teamName, managed.memberName, 60_000)
      const filler = writeFillerFile(turns)
      sendPaneLine(managed.paneId, `Read ${filler} and reply with only the number of list items it contains.`)

      try {
        const seen = await waitForLogEvents(
          offset,
          (events) => hookDelivery(events, managed.memberName),
          { timeout: AUTO_TURN_TIMEOUT_MS, timeoutMsg: 'turn budget elapsed' }
        )
        collected = seen.events
      } catch {
        collected = readLog(offset).events
      }
    }

    if (!hookDelivery(collected, managed.memberName)) {
      dumpCompactionEvents('automatic compaction cap reached', collected)
      throw new Error(
        `Codex did not auto-compact within the ${AUTO_COMPACTION_MAX_TURNS}-turn cap ` +
          `(model_auto_compact_token_limit = ${AUTO_COMPACT_TOKEN_LIMIT}); ` +
          'the manual case still proves the hook bridge.'
      )
    }

    reportHookPayload('automatic', collected, managed.memberName)
    const delivered = assertHookBridgeDelivered(collected, managed, { exactlyOne: false })
    console.log(
      `[e2e] automatic compaction reached after ${turns} turn(s); card was ` +
        `${delivered.additional_context_bytes} bytes of additionalContext`
    )
  })

  it('records why the live Codex compaction lane was unavailable', async function () {
    if (laneEnabled) return this.skip()
    expect(typeof laneSkipReason).toBe('string')
    expect(laneSkipReason.length).toBeGreaterThan(0)
    console.log(`[e2e] codex compaction lane skipped: ${laneSkipReason}`)
  })
})
