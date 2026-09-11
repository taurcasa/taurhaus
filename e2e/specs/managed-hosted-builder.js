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
      const owned = findRunTokenProcessRecords(process.env.TAURHAUS_E2E_RUN_TOKEN).filter(p => p.pid !== process.pid)
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
    cleanup.run()
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
    assert.equal(call.result.initialized, true, 'taurhaus: initialize report refused')
    const config = JSON.parse(readFileSync(join(claudeDir, 'teams', team, 'config.json'), 'utf8'))
    assert.equal(config.format_version, 2, 'mesh: team is not format 2')
    assert.equal(config.delivery_owner, 'team', 'mesh: delivery owner is not team')
    assert.equal(runtime('alpha').delivery, 'tmux')
    assert.equal(runtime('beta').delivery, 'app_server')
    save('initialize-result.json', call)
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
