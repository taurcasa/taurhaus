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
      if (!['check_mesh_install_status', 'coordination_initialize_team', 'coordination_hosted', 'coordination_get_live_status'].includes(command)) return original(command, args)
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
})
