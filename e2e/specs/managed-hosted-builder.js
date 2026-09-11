/**
 * Paid Linux lane 3: real builder -> IPC -> protocol-27 daemon -> hosted Codex.
 * Run once, explicitly: E2E_CODEX_SOURCE_HOME=/home/mstie/.codex-account-b
 * E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-hosted-builder
 * No product changes, mocked capabilities, controller retries or paid retries.
 * The worker calls createCodexScratchHome(sourceHome, targetHome) before boot.
 */
import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { createConnection } from 'node:net'
import { dirname, join, resolve } from 'node:path'
import { waitForProjectsLoaded, clickTestId } from '../helpers/navigation.js'
import { setInlineBuilderTeamName, setReactiveInputValue } from '../helpers/meshBuilder.js'
import { clickUntil } from '../helpers/clickUntil.js'
import { assertTmuxIsolation } from '../helpers/laneTmux.js'
import { createLaneCleanup, findRunTokenProcessRecords, killOwnedProcessRecord } from '../helpers/laneCleanup.js'
import { trustProject } from '../helpers/codexScratchHome.js'
import { rolloutPaths } from '../helpers/codexRollout.js'
import { DEFAULT_CANONICAL_POLICY } from '../../src/lib/components/meshTabUtils.js'

const checkout = resolve(import.meta.dirname, '../..')
const data = process.env.TAURHAUS_DATA_DIR || ''
const root = dirname(data)
const codexHome = process.env.CODEX_HOME || ''
const claudeDir = process.env.TAURHAUS_CLAUDE_DIR || ''
const project = process.env.E2E_TAURHAUS_PROJECT_PATH || ''
const team = `e2e-l3-${process.pid}`
const evidence = join(checkout, 'docs/design/evidence/e2e/l3-native-app')
const cleanup = createLaneCleanup()
const results = Array.from({ length: 6 }, (_, i) => ({ step: i + 1, outcome: 'NOT RUN', classification: 'harness', reason: 'Preceding step has not passed' }))
let started = 0
let currentStep = 1
let failed = false
let watchdog
let baseline
const reservations = []
const rpcRows = []
const activity = []

function save(name, value) {
  mkdirSync(evidence, { recursive: true })
  writeFileSync(join(evidence, name), typeof value === 'string' ? value : `${JSON.stringify(value, null, 2)}\n`)
}
function rows(path) {
  if (!existsSync(path)) return []
  // Only complete JSONL records; never a partial tail.
  return readFileSync(path, 'utf8').split('\n').slice(0, -1).filter(Boolean).map(line => JSON.parse(line))
}
function runtime(member = 'beta') {
  return JSON.parse(readFileSync(join(claudeDir, 'teams', team, 'runtime', `${member}.json`), 'utf8'))
}
function tmux(args) {
  assertTmuxIsolation(process.env, root)
  return execFileSync('tmux', args, { encoding: 'utf8', timeout: 5000 })
}
function meter() {
  const turns = []
  for (const path of rolloutPaths(codexHome)) {
    let turnId = null
    let previous = { input_tokens: 0, cached_input_tokens: 0, output_tokens: 0 }
    for (const row of rows(path)) {
      const p = row.payload
      if (row.type === 'event_msg' && p?.type === 'task_started') {
        turnId = p.turn_id
        turns.push({ path, turnId, usage: null, completed: false })
      }
      const turn = turns.findLast(t => t.path === path && t.turnId === turnId)
      if (p?.type === 'token_count' && p.info?.total_token_usage && turn) {
        const total = p.info.total_token_usage
        const delta = Object.fromEntries(Object.keys(previous).map(k => [k, total[k] - previous[k]]))
        assert(Object.values(delta).every(n => Number.isFinite(n) && n >= 0), 'harness: invalid/reset usage counters')
        turn.usage = delta
      }
      if (p?.type === 'task_complete' && turn) {
        turn.completed = true
        if (turn.usage) previous = Object.fromEntries(Object.keys(previous).map(k => [k, previous[k] + turn.usage[k]]))
      }
    }
  }
  let usd = 0
  for (const turn of turns) {
    const u = turn.usage
    // Inherited L7 packet rates; API-equivalent estimate, not an invoice.
    turn.usd = u ? ((u.input_tokens - u.cached_input_tokens) * 0.2 + u.cached_input_tokens * 0.02 + u.output_tokens * 1.2) / 1e6 : null
    usd += turn.usd ?? 0
  }
  const value = { reservations, turns, usd, meteringComplete: turns.every(t => t.usage !== null), rates: { input: 0.2, cached: 0.02, output: 1.2 }, rateSource: 'L7 run7 inherited API-equivalent estimates', elapsedMs: started ? Date.now() - started : 0 }
  save('cost-ledger.json', value)
  return value
}
function budget(inputs = 0, reason = '') {
  const cost = meter()
  assert(Date.now() - started < 900_000, 'harness: 15-minute runtime cap')
  assert(Math.max(reservations.length, cost.turns.length) + inputs <= 10, 'harness: 10-input cap')
  assert(cost.usd + inputs * 0.03 <= 0.20, 'harness: insufficient dollar headroom')
  if (inputs && cost.turns.some(t => t.completed && !t.usage)) throw new Error('harness: completed turn has unverified cost')
  for (let i = 0; i < inputs; i++) reservations.push({ at: new Date().toISOString(), reason, reservedUsd: 0.03 })
  meter()
}
async function poll(predicate, reason, timeout = 90_000) {
  await browser.waitUntil(async () => {
    budget()
    return await predicate()
  }, { timeout, interval: 500, timeoutMsg: reason })
}
async function ipc(command, args = {}) {
  const result = await browser.executeAsync((command, args, done) => {
    window.__TAURI_INTERNALS__.invoke(command, args).then(result => done({ result }), error => done({ error: String(error) }))
  }, command, args)
  if (result.error) throw new Error(`taurhaus: ${command}: ${result.error}`)
  return result.result
}
async function rpc(method, params = {}) {
  const port = Number(process.env.TAURHAUS_DAEMON_PORT)
  assert(port >= 20000 && port < 32000, 'harness: daemon port must be private')
  const id = `l3-${rpcRows.length}`
  const response = await new Promise((resolveResult, reject) => {
    const socket = createConnection({ host: '127.0.0.1', port })
    let buffer = ''
    socket.setTimeout(10_000, () => socket.destroy(new Error('private daemon RPC timeout')))
    socket.on('error', reject)
    socket.on('connect', () => socket.write(`${JSON.stringify({ id, method, params, auth: readFileSync(join(data, 'daemon.token'), 'utf8').trim() })}\n`))
    socket.on('data', chunk => {
      buffer += chunk
      let end
      while ((end = buffer.indexOf('\n')) >= 0) {
        const row = JSON.parse(buffer.slice(0, end)); buffer = buffer.slice(end + 1)
        if (row.id === id) { socket.end(); socket.destroy(); resolveResult(row); return }
      }
    })
  })
  rpcRows.push({ at: new Date().toISOString(), method, params, response })
  save('daemon-rpc.json', rpcRows)
  if (response.error) throw new Error(`taurhaus: ${method}: ${response.error.message}`)
  return response.result
}
async function observeIpc() {
  await browser.execute(() => {
    const original = window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__)
    window.__l3 = { ipc: [], gates: [] }
    const sample = () => {
      const button = document.querySelector('[data-testid="mesh-action-initialize"]')
      if (button) window.__l3.gates.push({ at: Date.now(), disabled: button.disabled, canonical: document.querySelector('[aria-labelledby="mesh-canonical-label"]')?.checked })
    }
    new MutationObserver(sample).observe(document.body, { subtree: true, attributes: true, childList: true })
    window.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (!['check_mesh_install_status', 'coordination_initialize_team', 'coordination_hosted', 'coordination_get_live_team_status'].includes(command)) return original(command, args)
      const row = { command, args, at: Date.now(), draftBefore: document.querySelector('#hosted-input')?.value }
      window.__l3.ipc.push(row)
      try {
        const result = await original(command, args)
        row.result = result; row.acceptedAt = Date.now(); row.draftAtAcceptance = document.querySelector('#hosted-input')?.value
        return result
      } catch (error) { row.error = String(error); throw error }
    }
  })
}
async function snapshot(step) {
  await browser.saveScreenshot(join(evidence, `step-${step}.png`))
  save(`step-${step}-ipc.json`, await browser.execute(() => window.__l3 ?? null))
  if (existsSync(join(data, 'taurhaus.log.jsonl'))) save('daemon.jsonl', rows(join(data, 'taurhaus.log.jsonl')).map(row => JSON.stringify(row)).join('\n') + '\n')
  if (existsSync(join(claudeDir, 'teams', team, 'config.json'))) {
    save(`step-${step}-config.json`, JSON.parse(readFileSync(join(claudeDir, 'teams', team, 'config.json'), 'utf8')))
    for (const member of ['alpha', 'beta']) {
      try { save(`step-${step}-${member}.json`, runtime(member)) } catch { /* partial initialize */ }
    }
  }
  meter()
}
function report() {
  const tables = results.map(r => `| Step ${r.step} | Outcome | Classification | Evidence / reason |\n| --- | --- | --- | --- |\n| ${r.step} | ${r.outcome} | ${r.classification} | ${r.reason.replaceAll('|', '/').replaceAll('\n', ' ')} |`).join('\n\n')
  writeFileSync(join(evidence, '../l3-native-app.md'), `# Linux native app lane 3\n\n${tables}\n\nRaw sidecars: [cost ledger](l3-native-app/cost-ledger.json), [daemon RPC](l3-native-app/daemon-rpc.json), step screenshots and IPC captures. No retry is authorized by this packet.\n`)
}
async function step(number, action) {
  if (failed) throw new Error('harness: prior step failed; paid continuation prohibited')
  currentStep = number
  try {
    await action()
    results[number - 1] = { step: number, outcome: 'PASS', classification: 'observed', reason: `See step-${number}.png and step-${number}-ipc.json` }
  } catch (error) {
    failed = true
    const reason = String(error.message ?? error)
    results[number - 1] = { step: number, outcome: 'FAIL', classification: reason.startsWith('taurhaus:') ? 'taurhaus' : reason.startsWith('mesh:') ? 'mesh' : 'harness', reason }
    throw error
  } finally {
    await snapshot(number).catch(error => save('capture-error.json', { error: String(error) }))
    report()
  }
}

// These are native acceptance tests. No offline controller suite or mock run.
describe('canonical builder and hosted conversation (paid)', function () {
  this.timeout(900_000)
  before(async function () {
    started = Date.now()
    assertTmuxIsolation(process.env, root)
    assert.equal(process.env.E2E_CODEX_SOURCE_HOME, '/home/mstie/.codex-account-b', 'harness: explicit auth source required')
    for (const path of [process.env.HOME, data, codexHome, claudeDir, project]) assert(path?.startsWith(`${root}/`), 'harness: non-private root')
    assert.equal(statSync(join(codexHome, 'auth.json')).mode & 0o777, 0o600)
    mkdirSync(evidence, { recursive: true })
    cleanup.install()
    cleanup.owe('scratch auth', () => rmSync(join(codexHome, 'auth.json'), { force: true }))
    cleanup.owe('owned children', () => {
      const owned = findRunTokenProcessRecords(process.env.TAURHAUS_E2E_RUN_TOKEN).filter(p => p.pid !== process.pid && /^(?:codex|codex-code-mode|claude|mesh|tmux|taurhaus-daemon)/.test(readFileSync(`/proc/${p.pid}/comm`, 'utf8').trim()))
      save('owned-processes.json', owned)
      for (const record of owned) killOwnedProcessRecord(record, { signal: 'SIGTERM' })
    })
    watchdog = setTimeout(() => { failed = true; cleanup.run() }, 900_000)
    watchdog.unref()
    await waitForProjectsLoaded()
    await observeIpc()
    const roles = await ipc('templates_list_roles_full')
    for (const name of ['lead', 'alpha', 'beta']) {
      const kind = name === 'lead' ? 'lead' : 'agent'
      const tool = name === 'lead' ? 'claude' : 'codex'
      const template = structuredClone(roles.find(role => role.kind === kind && role.defaults.cliTool === tool))
      assert(template, `harness: missing ${tool} ${kind} template`)
      template.roleId = `l3-${name}`
      template.name = `L3 ${name}`
      template.defaults.defaultNamePattern = name
      if (tool === 'codex') {
        template.defaults.model = 'gpt-5.6-luna'
        template.defaults.reasoning_effort = 'low'
      }
      template.instructions = `Disposable UI trial. On startup reply READY ${name} in at most five words. For ordinary markers reply in at most ten words. Only execute tools explicitly requested by the operator or the managed Mesh contract. Every mesh command must use --team ${team} --name ${name} --claude-dir ${claudeDir}. Do not spawn agents or self-assign work.`
      template.behavioralContract = { communication: [], execution: [], escalation: [] }
      template.capabilities = []
      await ipc('templates_upsert_role', { request: { template } })
    }
  })

  after(async function () {
    clearTimeout(watchdog)
    await snapshot(currentStep).catch(() => {})
    try { exportJournal() } catch (error) { save('journal-export-error.json', { error: String(error) }) }
    cleanup.run()
    await new Promise(resolve => setTimeout(resolve, 1000))
    const owned = existsSync(join(evidence, 'owned-processes.json')) ? JSON.parse(readFileSync(join(evidence, 'owned-processes.json'), 'utf8')) : []
    for (const record of owned) killOwnedProcessRecord(record)
    save('cleanup.json', { authRemoved: !existsSync(join(codexHome, 'auth.json')), owned, appAndDriver: 'WDIO afterSession owns final app/driver cleanup' })
    report()
  })

  it('1. boots the private protocol-27 app and resolves the canonical capability gate', () => step(1, async () => {
    const binary = join(checkout, 'src-tauri/target/debug/taurhaus')
    const mesh = join(process.env.HOME, '.local/bin/mesh')
    const lock = JSON.parse(readFileSync(join(checkout, 'src-tauri/resources/mesh.lock.json'), 'utf8'))
    const version = JSON.parse(execFileSync(mesh, ['version', '--json'], { encoding: 'utf8', timeout: 5000 }))
    for (const key of Object.keys(lock)) assert.equal(version[key], lock[key], `mesh: lock mismatch: ${key}`)
    assert.match(execFileSync('codex', ['--version'], { encoding: 'utf8', timeout: 5000 }), /\b0\.153\.4\b/)
    const ping = await rpc('ping')
    assert.equal(ping.protocol_version, 27, 'taurhaus: private daemon protocol')
    save('boot.json', { binary, sha256: createHash('sha256').update(readFileSync(binary)).digest('hex'), ping, mesh: version, lock })
    await clickTestId('tab-mesh')
    await poll(async () => await $('[data-testid="mesh-builder-shell"]').isExisting(), 'harness: builder did not appear')
    const observed = await browser.execute(() => window.__l3)
    const capability = observed.ipc.find(row => row.command === 'check_mesh_install_status' && row.result)
    assert(capability, 'harness: capability resolution was not observed')
    assert(observed.gates.some(g => g.disabled && g.at <= capability.acceptedAt), 'harness: unresolved Initialize disabled state was not observed')
    assert.equal(capability.result.canonical_messaging_supported, true, 'mesh: canonical capability unavailable')
    assert.equal(capability.result.hosted_delivery_supported, true, 'mesh: hosted capability unavailable')
    assert.equal(await $('[aria-labelledby="mesh-canonical-label"]').isSelected(), true, 'taurhaus: canonical is not selected')
  }))

  it('2. initializes once through the builder with alpha tmux and beta app_server', () => step(2, async () => {
    await prepareBuilder()
    budget(2, 'creation-time alpha and beta onboarding')
    await browser.saveScreenshot(join(evidence, 'step-2-before-initialize.png'))
    await $('[data-testid="mesh-action-initialize"]').click()
    await poll(async () => {
      const calls = await browser.execute(() => window.__l3.ipc.filter(r => r.command === 'coordination_initialize_team'))
      assert.equal(calls.length, 1, 'harness: initialize must be issued exactly once')
      if (calls[0].error) throw new Error(`taurhaus: initialize failed: ${calls[0].error}`)
      return Boolean(calls[0].result)
    }, 'taurhaus: initialize did not finish', 240_000)
    const call = await browser.execute(() => window.__l3.ipc.find(r => r.command === 'coordination_initialize_team'))
    assert.deepEqual(call.args.request.messaging, { mode: 'canonical', retentionPolicy: DEFAULT_CANONICAL_POLICY })
    assert.equal(call.args.request.agents.find(a => a.name === 'alpha').delivery, 'tmux')
    assert.equal(call.args.request.agents.find(a => a.name === 'beta').delivery, 'app_server')
    assert.equal(call.result.failed_step, null, 'taurhaus: initialize report refused')
    const config = JSON.parse(readFileSync(join(claudeDir, 'teams', team, 'config.json'), 'utf8'))
    assert.equal(config.messaging_format, 2, 'mesh: team is not format 2')
    assert.equal(config.delivery_owner, 'team', 'mesh: delivery owner is not team')
    assert.equal(runtime('alpha').appServer ?? null, null)
    assert(runtime('beta').appServer, 'taurhaus: beta has no hosted attachment')
    for (const member of ['alpha', 'beta']) assert.equal(config.members.find(m => m.name === member).delivery, member === 'alpha' ? 'tmux' : 'app_server')
    save('initialize-result.json', call)
  }))
  it('3. shows beta startup in its native thread and keeps alpha free of hosted controls', () => step(3, async () => {
    await openMember('beta')
    let transcript
    await poll(async () => {
      transcript = await hostedTranscript()
      return transcript.thread?.status?.type === 'idle' && transcript.thread.turns.some(t => t.items?.some(i => i.type === 'agentMessage'))
    }, 'taurhaus: beta startup transcript did not settle')
    const record = runtime()
    assert.equal(transcript.thread.id, record.session_id, 'taurhaus: wrong hosted thread')
    const text = await $('[aria-label="Hosted transcript"]').getText()
    assert(text.includes('READY beta'), 'taurhaus: rendered startup reply missing')
    const pane = tmux(['capture-pane', '-p', '-J', '-S', '-200', '-t', record.paneId])
    assert(pane.includes('READY beta'), 'taurhaus: attached pane lacks the startup reply')
    save('step-3-thread.json', transcript)
    save('step-3-pane.txt', pane)
    baseline = { threadId: transcript.thread.id, paneId: record.paneId, host: record.appServer, generation: record.attachmentGeneration, contextGeneration: record.contextGeneration }
    await browser.saveScreenshot(join(evidence, 'step-3-beta.png'))
    await openMember('alpha')
    assert.equal(await $('[aria-label="Hosted conversation"]').isExisting(), false, 'taurhaus: alpha has hosted controls')
    await browser.saveScreenshot(join(evidence, 'step-3-alpha.png'))
    await openMember('beta')
  }))

  it('4. sends one marker in the same thread and clears the draft only after acceptance', () => step(4, async () => {
    const marker = `L3-A-${process.pid}`
    await sendPanel(`Reply with exactly ${marker}`, 'first panel marker')
    await requireReply(marker)
    const calls = await browser.execute(() => window.__l3.ipc.filter(r => r.command === 'coordination_hosted' && r.args.operation === 'input'))
    assert.equal(calls.length, 1, 'harness: panel input was not single-shot')
    assert.equal(calls[0].draftBefore, `Reply with exactly ${marker}`)
    assert.equal(calls[0].draftAtAcceptance, calls[0].draftBefore, 'taurhaus: draft cleared before acceptance')
    assert.equal(await $('#hosted-input').getValue(), '', 'taurhaus: accepted draft not cleared')
    assert.equal(runtime().paneId, baseline.paneId, 'taurhaus: panel input replaced the pane')
  }))

  it('5. interrupts one active ordinary turn and accepts a fresh marker', () => step(5, async () => {
    await sendPanel('Write 60 short numbered lines about sorting a desk, at most 600 words. No tools.', 'bounded ordinary turn')
    let activeTurn
    await poll(async () => {
      const transcript = await hostedTranscript()
      activeTurn = transcript.thread?.turns.findLast(t => t.status === 'inProgress')
      return Boolean(activeTurn) && await sampleActivity('active')
    }, 'harness: active turn and host-sourced working window not observed')
    await browser.saveScreenshot(join(evidence, 'step-5-working.png'))
    await $('[aria-label="Hosted conversation"] button=Stop turn').click()
    await poll(async () => {
      const transcript = await hostedTranscript()
      return transcript.thread?.turns.some(t => t.id === activeTurn.id && t.status === 'interrupted')
    }, 'taurhaus: interruption not observed')
    const interrupts = await browser.execute(() => window.__l3.ipc.filter(r => r.command === 'coordination_hosted' && r.args.operation === 'interrupt'))
    assert.equal(interrupts.length, 1, 'harness: Stop turn must be clicked once')
    assert(interrupts[0].acceptedAt && !interrupts[0].error, 'taurhaus: interrupt not accepted')
    await poll(async () => await $('#hosted-input').isEnabled(), 'taurhaus: controls remained unresponsive')
    assert.equal((await hostedTranscript()).requests.length, 0, 'taurhaus: unexpected approval under approval-never')
    const marker = `L3-B-${process.pid}`
    await sendPanel(`Reply with exactly ${marker}`, 'post-interruption marker')
    await requireReply(marker)
    await poll(() => sampleActivity('idle'), 'taurhaus: host-sourced idle not observed')
    assert.equal(await $('button=Allow').isExisting(), false, 'taurhaus: unexpected approval controls')
  }))

  it('6. reopens beta without a new thread, onboarding or host launch, then exports', () => step(6, async () => {
    const before = await hostedTranscript()
    const beforeRecord = runtime()
    const beforeTurns = before.thread.turns.map(t => t.id)
    await clickTestId('mesh-node-detail-close')
    await openMember('beta')
    await poll(async () => (await $('[aria-label="Hosted transcript"]').getText()).includes(`L3-B-${process.pid}`), 'taurhaus: reopened transcript missing')
    const after = await hostedTranscript()
    assert.equal(after.thread.id, baseline.threadId, 'taurhaus: reopen changed thread identity')
    assert.deepEqual(after.thread.turns.map(t => t.id), beforeTurns, 'taurhaus: reopen created a new turn')
    const afterRecord = runtime()
    for (const key of ['appServer', 'attachmentGeneration', 'contextGeneration', 'paneId', 'session_id']) assert.deepEqual(afterRecord[key], beforeRecord[key], `taurhaus: reopen changed ${key}`)
    assert.deepEqual(afterRecord.recovery, beforeRecord.recovery, 'taurhaus: reopen changed onboarding receipt')
    assert((await $('[data-testid="mesh-node-detail-name"]').getText()).includes('beta'))
    assert(activity.some(a => a.session?.source === 'host' && a.session?.state === 'active'))
    assert(activity.some(a => a.session?.source === 'host' && a.session?.state === 'idle'))
    const cost = meter()
    assert(cost.meteringComplete && cost.usd <= 0.20, 'harness: incomplete or over-budget cost ledger')
    exportJournal()
  }))

})

async function prepareBuilder() {
  trustProject(join(codexHome, 'config.toml'), project)
  // CLI policy lives only in the generated scratch config; no operator config.
  const configPath = join(codexHome, 'config.toml')
  writeFileSync(configPath, 'approval_policy = "never"\nsandbox_mode = "workspace-write"\nmodel = "gpt-5.6-luna"\nmodel_reasoning_effort = "low"\n' + readFileSync(configPath, 'utf8'))
  budget(1, 'throwaway first-use TUI start; no submitted prompt')
  tmux(['new-session', '-d', '-s', 'taurhaus', '-c', project])
  cleanup.owe('private tmux server', () => { spawnSync('tmux', ['kill-server'], { env: process.env, timeout: 5000 }) })
  for (const [key, value] of Object.entries({ HOME: process.env.HOME, CODEX_HOME: codexHome, CLAUDE_DIR: claudeDir, CLAUDE_CONFIG_DIR: claudeDir, TAURHAUS_CLAUDE_DIR: claudeDir, TAURHAUS_DATA_DIR: data, GROK_HOME: process.env.GROK_HOME, GEMINI_CLI_HOME: join(root, 'gemini'), TAURHAUS_AGY_DIR: process.env.TAURHAUS_AGY_DIR })) {
    tmux(['set-environment', '-g', key, value])
    tmux(['set-environment', '-t', 'taurhaus', key, value])
  }
  const pane = tmux(['new-window', '-d', '-P', '-F', '#{pane_id}', '-t', 'taurhaus', '-c', project, 'codex -m gpt-5.6-luna -c model_reasoning_effort="low" -a never']).trim()
  await poll(async () => /context left|for shortcuts|gpt-5.6-luna/.test(tmux(['capture-pane', '-p', '-t', pane])), 'harness: scratch Codex composer unavailable')
  save('warmup-pane.txt', tmux(['capture-pane', '-p', '-t', pane]))
  tmux(['send-keys', '-t', pane, 'C-c'])
  await poll(async () => !tmux(['list-panes', '-a', '-F', '#{pane_id}']).split('\n').includes(pane), 'harness: warmup did not quit')
  assert(readdirSync(codexHome).some(name => /^state_.*\.sqlite$/.test(name)), 'harness: warmup SQLite missing')
  await setInlineBuilderTeamName(team)
  await clickTestId('mesh-builder-role-l3-lead')
  for (const name of ['alpha', 'beta']) await clickTestId(`mesh-builder-add-l3-${name}`)
  const cards = await $$('[data-testid^="mesh-builder-agent-card-"]')
  assert.equal(cards.length, 2, 'harness: unexpected initial roster')
  for (const card of cards) {
    const id = (await card.getAttribute('data-testid')).replace('mesh-builder-agent-card-', '')
    await clickTestId(`mesh-builder-agent-edit-toggle-${id}`)
    const name = (await $(`[data-testid="mesh-builder-agent-name-input-${id}"]`).getValue()).trim()
    assert(['alpha', 'beta'].includes(name), 'harness: unexpected member name')
    await setReactiveInputValue(`mesh-builder-agent-name-input-${id}`, name)
    await card.$('select:has(option[value="app_server"])').selectByAttribute('value', name === 'alpha' ? 'tmux' : 'app_server')
    await $(`[data-testid="mesh-builder-agent-model-input-${id}"]`).selectByAttribute('value', 'gpt-5.6-luna')
    await $(`[data-testid="mesh-builder-agent-model-input-${id}-effort"]`).selectByAttribute('value', 'low')
  }
  assert.equal(await $('[aria-labelledby="mesh-canonical-label"]').isSelected(), true)
  await poll(async () => await $('[data-testid="mesh-action-initialize"]').isEnabled(), 'taurhaus: configured builder cannot initialize')
}

async function openMember(name) {
  // Only navigation is repeatable. Never use clickUntil for initialize/send/stop.
  await browser.keys('Escape')
  await clickUntil(async () => {
    const nodes = await $$('button[data-node-id]')
    const node = []
    for (const item of nodes) if ((await item.getText()).split('\n').some(line => line.trim() === name)) node.push(item)
    assert.equal(node.length, 1, `harness: missing unique runtime node ${name}`)
    await node[0].click()
  }, async () => {
    const detail = await $('[data-testid="mesh-node-detail"]')
    return await detail.isExisting() && (await detail.getText()).includes(name)
  }, { timeout: 60_000, interval: 500, timeoutMsg: `harness: ${name} detail unavailable` })
  if (name === 'beta') await poll(async () => await $('#hosted-input').isExisting(), 'taurhaus: beta hosted panel unavailable')
}
async function hostedTranscript() {
  return await rpc('coordination.hosted_transcript', { team_name: team, member_name: 'beta' })
}

async function sendPanel(text, reason) {
  budget(1, reason)
  const field = await $('#hosted-input')
  await field.setValue(text)
  const before = await browser.execute(() => window.__l3.ipc.filter(r => r.command === 'coordination_hosted' && r.args.operation === 'input').length)
  await $('[aria-label="Hosted conversation"] button[type="submit"]').click()
  await poll(async () => {
    const calls = await browser.execute(() => window.__l3.ipc.filter(r => r.command === 'coordination_hosted' && r.args.operation === 'input'))
    assert(calls.length <= before + 1, 'harness: duplicate panel submission')
    const call = calls[before]
    if (call?.error) throw new Error(`taurhaus: panel input not accepted: ${call.error}`)
    return call?.acceptedAt != null
  }, 'taurhaus: panel input acceptance unavailable')
}
function itemText(item) {
  return item.text ?? (item.content ?? []).filter(c => c.type === 'text' || c.type === 'input_text' || c.type === 'output_text').map(c => c.text).join('\n')
}
async function requireReply(marker) {
  let transcript
  await poll(async () => {
    transcript = await hostedTranscript()
    return transcript.thread?.status?.type === 'idle' && transcript.thread.turns.flatMap(t => t.items ?? []).some(i => i.type === 'agentMessage' && itemText(i).includes(marker))
  }, 'taurhaus: native marker reply missing')
  assert.equal(transcript.thread.id, baseline.threadId, 'taurhaus: reply is in another thread')
  const items = transcript.thread.turns.flatMap(t => t.items ?? [])
  for (const type of ['userMessage', 'agentMessage']) assert.equal(items.filter(i => i.type === type && itemText(i).includes(marker)).length, 1, `taurhaus: ${type} marker must occur once`)
  await poll(async () => (await $('[aria-label="Hosted transcript"]').getText()).includes(marker), 'taurhaus: UI reply missing')
  const pane = tmux(['capture-pane', '-p', '-J', '-S', '-200', '-t', baseline.paneId])
  assert(pane.includes(marker), 'taurhaus: attached pane marker missing')
  save(`marker-${marker}-pane.txt`, pane)
  save(`marker-${marker}-thread.json`, transcript)
  const rollouts = rolloutPaths(codexHome).filter(path => rows(path).some(r => r.type === 'session_meta' && r.payload?.id === baseline.threadId))
  assert(rollouts.length > 0, 'harness: no native rollout for hosted thread')
  const native = rollouts.flatMap(path => rows(path)).filter(r => r.type === 'response_item' && r.payload?.type === 'message' && itemText(r.payload).includes(marker))
  for (const role of ['user', 'assistant']) assert.equal(native.filter(r => r.payload.role === role).length, 1, `taurhaus: native ${role} marker must occur once`)
  save(`marker-${marker}-native.json`, native)
}

async function sampleActivity(state) {
  const snapshot = await rpc('get_runtime_session_snapshot')
  const session = snapshot.runtime_sessions?.find(s => s.session_id === baseline.threadId)
  const nodes = await $$('button[data-node-id]')
  let title = null
  for (const node of nodes) if ((await node.getText()).split('\n').some(line => line.trim() === 'beta')) title = await node.getAttribute('title')
  activity.push({ at: Date.now(), session, title })
  save('activity.json', activity)
  return session?.source === 'host' && session.state === state && title === `${state === 'active' ? 'Working' : 'Idle'} via daemon-owned thread`
}

function exportJournal() {
  if (!existsSync(join(claudeDir, 'teams', team, 'config.json'))) return
  const pages = []
  for (const member of ['lead', 'alpha', 'beta']) {
    let cursor
    while (true) {
      assert(Date.now() - started < 900_000, 'harness: journal export reached runtime cap')
      const args = ['journal', 'read', '--team', team, '--name', member, '--claude-dir', claudeDir, '--json']
      if (cursor) args.push('--since', cursor)
      const result = spawnSync(join(process.env.HOME, '.local/bin/mesh'), args, { encoding: 'utf8', timeout: 10_000 })
      assert.equal(result.status, 0, 'mesh: journal export failed')
      const page = JSON.parse(result.stdout)
      pages.push({ member, args, page })
      save('journal-export.json', pages)
      if (page.done) break
      assert(page.cursor && page.cursor !== cursor, 'harness: journal cursor made no progress')
      cursor = page.cursor
    }
  }
  save('receipts.json', { alpha: runtime('alpha').recovery, beta: runtime('beta').recovery })
}
