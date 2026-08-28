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
import { setAutoCompactTokenLimit } from '../helpers/codexScratchHome.js'
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
 * context window. Each filler file is ~4k tokens, so the threshold is crossed
 * by roughly the third turn and the cap costs at most ~6 turns.
 */
const AUTO_COMPACT_TOKEN_LIMIT = 20_000
const AUTO_COMPACTION_MAX_TURNS = 6
const AUTO_COMPACTION_FILLER_LINES = 320

const HOOK_DELIVERY_TIMEOUT_MS = 150_000
const AUTO_TURN_TIMEOUT_MS = 90_000
const TEAM_READY_TIMEOUT_MS = 180_000

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
    const shown = tmuxQuietly(['show-environment', '-t', TMUX_SESSION, key])
    previous.set(key, shown.ok ? shown.output : null)
    tmuxQuietly(['set-environment', '-t', TMUX_SESSION, key, process.env[key] ?? ''])
  }

  return function restorePaneEnvironment() {
    for (const [key, shown] of previous) {
      // `show-environment` prints `NAME=value`, or `-NAME` when it was unset.
      if (!shown || shown.startsWith('-')) {
        tmuxQuietly(['set-environment', '-t', TMUX_SESSION, '-u', key])
        continue
      }
      const value = shown.slice(shown.indexOf('=') + 1)
      tmuxQuietly(['set-environment', '-t', TMUX_SESSION, key, value])
    }
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

  const restorePaneEnvironment = applyPaneEnvironment()
  let report
  try {
    report = await invokeTauriOrThrow('coordination_initialize_team', {
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
            model: '',
            projectId: TAURHAUS_PROJECT_PATH,
            roleId: agentRoleId,
          },
        ],
      },
    })
  } finally {
    createdTeamNames.add(teamName)
  }

  if (report?.failedStep) {
    restorePaneEnvironment()
    throw new Error(`Team initialization failed at ${report.failedStep}: ${report.message}`)
  }

  let paneId = null
  let sessionId = null
  try {
    await browser.waitUntil(
      async () => {
        const status = await invokeTauriOrThrow('coordination_get_live_team_status', { teamName })
        const member = (status?.members ?? []).find((entry) => entry?.name === memberName)
        paneId = member?.paneId ?? member?.pane_id ?? null
        sessionId = readRuntimeRecord(teamName, memberName)?.session_id ?? null
        return Boolean(paneId) && Boolean(sessionId)
      },
      {
        timeout: TEAM_READY_TIMEOUT_MS,
        interval: 2_000,
        timeoutMsg: `Managed Codex member ${memberName} never reported a pane and a captured session`,
      }
    )
  } finally {
    restorePaneEnvironment()
  }

  writeOperationalSnapshot(teamName, memberName, TAURHAUS_PROJECT_PATH)

  return { teamName, memberName, paneId, sessionId }
}

/** The bridge's own acceptance evidence for one compaction of `memberName`. */
function hookDelivery(events, memberName) {
  const delivered = selectEvents(events, {
    event: 'compaction.codex_hook.delivered',
    match: { member_name: memberName },
  })
  return delivered.length > 0 ? delivered[delivered.length - 1] : null
}

function assertHookBridgeDelivered(events, { teamName, memberName }) {
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
  expect(delivered).toHaveLength(1)
  expect(delivered[0].tool).toBe('codex')
  expect(delivered[0].team_name).toBe(teamName)
  expect(delivered[0].additional_context_bytes).toBeGreaterThan(0)

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

  return delivered[0]
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

    const offset = currentLogOffset()
    sendPaneLine(managed.paneId, '/compact')

    const { events } = await waitForLogEvents(
      offset,
      (collected) => hookDelivery(collected, managed.memberName),
      {
        timeout: HOOK_DELIVERY_TIMEOUT_MS,
        timeoutMsg: `No compaction.codex_hook.delivered for ${managed.memberName} within ${HOOK_DELIVERY_TIMEOUT_MS}ms of /compact`,
      }
    )

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

    const restorePaneEnvironment = applyPaneEnvironment()
    let resumed
    try {
      tmuxQuietly(['kill-pane', '-t', managed.paneId])
      resumed = await invokeTauriOrThrow('coordination_resume_member', {
        request: { teamName: managed.teamName, memberName: managed.memberName },
      })
    } finally {
      restorePaneEnvironment()
    }
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
      throw new Error(
        `Codex did not auto-compact within the ${AUTO_COMPACTION_MAX_TURNS}-turn cap ` +
          `(model_auto_compact_token_limit = ${AUTO_COMPACT_TOKEN_LIMIT}); ` +
          'the manual case still proves the hook bridge.'
      )
    }

    reportHookPayload('automatic', collected, managed.memberName)
    const delivered = assertHookBridgeDelivered(collected, managed)
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
