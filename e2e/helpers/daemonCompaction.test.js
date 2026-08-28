import { describe, it, expect, afterEach, vi } from 'vitest'
import { spawn } from 'node:child_process'
import { createServer } from 'node:net'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { daemonTokenPath, handBackCompactionMode, setDaemonCodexCompactionMode } from './daemonCompaction.js'

// The helper blocks the calling thread on purpose — an exit handler cannot
// await — so the daemon it talks to has to be a real process, not a server on
// this event loop, which could never answer while the call is in flight.
const FAKE_DAEMON = `
const net = require('node:net')
const fs = require('node:fs')
const behaviour = process.env.FAKE_BEHAVIOUR || 'ok'
const server = net.createServer((socket) => {
  let buffer = ''
  socket.on('data', (chunk) => {
    buffer += chunk
    const newline = buffer.indexOf('\\n')
    if (newline < 0) return
    const line = buffer.slice(0, newline)
    fs.writeFileSync(process.env.FAKE_REQUEST_LOG, line)
    if (behaviour === 'silent') return
    const request = JSON.parse(line)
    if (behaviour === 'event-first') {
      socket.write(JSON.stringify({ event: 'file_changed', data: { path: '/x' } }) + '\\n')
    }
    socket.write(JSON.stringify(behaviour === 'error'
      ? { id: request.id, error: { code: 'COMPACTION_MODE_APPLY_FAILED', message: 'runtime refused' } }
      : { id: request.id, result: { ok: true } }) + '\\n')
  })
})
server.listen(0, '127.0.0.1', () => process.stdout.write('ready ' + server.address().port + '\\n'))
`

let root
let daemon = null

function startFakeDaemon(behaviour = 'ok') {
  root = mkdtempSync(join(tmpdir(), 'taurhaus-daemon-compaction-'))
  const requestLog = join(root, 'request.jsonl')
  const child = spawn(process.execPath, ['-e', FAKE_DAEMON], {
    env: { ...process.env, FAKE_BEHAVIOUR: behaviour, FAKE_REQUEST_LOG: requestLog },
    stdio: ['ignore', 'pipe', 'inherit'],
  })
  daemon = child

  return new Promise((resolve, reject) => {
    const failed = setTimeout(() => reject(new Error('fake daemon never reported a port')), 10_000)
    child.stdout.on('data', (chunk) => {
      const match = String(chunk).match(/ready (\d+)/)
      if (!match) return
      clearTimeout(failed)
      resolve({ port: Number(match[1]), requestLog })
    })
  })
}

afterEach(() => {
  daemon?.kill('SIGKILL')
  daemon = null
  if (root) rmSync(root, { recursive: true, force: true })
  root = null
})

function tokenFile(token) {
  const path = join(root, 'daemon.token')
  writeFileSync(path, `${token}\n`)
  return path
}

describe('daemon Codex compaction mode', () => {
  // Regression: 3b56a3f ("test(e2e): add the live Codex compaction lane driven
  // through the hook bridge") flipped `terminal.harness.codex_compaction` to
  // `hooks` through the settings IPC, which also tells the connected daemon —
  // the operator's own, shared, on 17233 — to stop its transcript runtime. The
  // original value was put back only by the Mocha `after` hook, and
  // `e2e/wdio.conf.js` exits on the first SIGINT, so interrupting the lane left
  // that daemon in hooks mode until someone restarted it. Restoring it has to
  // work from an exit handler, which cannot await: this is the synchronous,
  // bounded way back.
  it('sends one set_codex_compaction_mode request and reports success', async () => {
    const { port, requestLog } = await startFakeDaemon()
    const token = tokenFile('deadbeef')

    const result = setDaemonCodexCompactionMode('transcript', { port, tokenPath: token })

    expect(result).toEqual({ ok: true })
    const request = JSON.parse(readFileSync(requestLog, 'utf8'))
    expect(request.method).toBe('set_codex_compaction_mode')
    expect(request.params).toEqual({ mode: 'transcript' })
    expect(request.auth).toBe('deadbeef')
    expect(typeof request.id).toBe('string')
  })

  it('omits auth when the daemon token file is not there', async () => {
    const { port, requestLog } = await startFakeDaemon()

    const result = setDaemonCodexCompactionMode('hooks', { port, tokenPath: join(root, 'absent.token') })

    expect(result).toEqual({ ok: true })
    const request = JSON.parse(readFileSync(requestLog, 'utf8'))
    expect(request.params).toEqual({ mode: 'hooks' })
    expect('auth' in request).toBe(false)
  })

  it('reads past a pushed event to find its own response', async () => {
    const { port } = await startFakeDaemon('event-first')

    expect(setDaemonCodexCompactionMode('transcript', { port, tokenPath: join(root, 'absent.token') }))
      .toEqual({ ok: true })
  })

  it('reports a daemon error instead of claiming the mode was restored', async () => {
    const { port } = await startFakeDaemon('error')

    const result = setDaemonCodexCompactionMode('transcript', { port, tokenPath: join(root, 'absent.token') })

    expect(result.ok).toBe(false)
    expect(result.error).toContain('COMPACTION_MODE_APPLY_FAILED')
    expect(result.error).toContain('runtime refused')
  })

  it('gives up on a daemon that never answers instead of hanging the exit', async () => {
    const { port } = await startFakeDaemon('silent')

    const startedAt = Date.now()
    const result = setDaemonCodexCompactionMode('transcript', {
      port,
      tokenPath: join(root, 'absent.token'),
      timeoutMs: 400,
    })

    expect(result.ok).toBe(false)
    expect(Date.now() - startedAt).toBeLessThan(10_000)
  })

  it('reports a refused connection rather than throwing out of a cleanup step', async () => {
    // A port that was listening a moment ago and is not any more: the kernel
    // refuses at once, which is what a stopped daemon looks like.
    const port = await new Promise((resolve) => {
      const probe = createServer()
      probe.listen(0, '127.0.0.1', () => {
        const { port: bound } = probe.address()
        probe.close(() => resolve(bound))
      })
    })

    const result = setDaemonCodexCompactionMode('transcript', { port, tokenPath: '/nonexistent/token' })

    expect(result.ok).toBe(false)
    expect(typeof result.error).toBe('string')
  })

  it('resolves the daemon token the way the app does', () => {
    expect(daemonTokenPath({ XDG_DATA_HOME: '/data/home' }, '/home/op'))
      .toBe('/data/home/taurhaus/daemon.token')
    expect(daemonTokenPath({}, '/home/op'))
      .toBe('/home/op/.local/share/taurhaus/daemon.token')
  })
})

describe('handing the compaction mode back on a normal teardown', () => {
  function silentLogger() {
    return { log: () => {}, warn: () => {} }
  }

  // Regression: 8410b57 ("test(e2e): hand the daemon back its compaction mode on
  // every path out") dropped the direct restoration whenever `update_settings`
  // came back ok. It is not proof: `commands/settings.rs` reconciles the daemon
  // inside that command, and a reconciliation that fails is only logged
  // (`compaction.codex_hook.degraded`) while the command still returns the saved
  // settings. A shared daemon that had disconnected or refused was therefore
  // left in hooks mode with the lane's only fallback already settled.
  it('restores the daemon directly even when the settings IPC reported success', async () => {
    const restoreDaemonMode = vi.fn()

    await handBackCompactionMode({
      updateSettings: async () => ({ ok: true, result: {} }),
      restoreDaemonMode,
      logger: silentLogger(),
    })

    expect(restoreDaemonMode).toHaveBeenCalledTimes(1)
  })

  it('restores the daemon directly when the settings IPC failed, and says so', async () => {
    const restoreDaemonMode = vi.fn()
    const logger = silentLogger()
    logger.warn = vi.fn()

    await handBackCompactionMode({
      updateSettings: async () => ({ ok: false, error: 'Timed out after 10000ms' }),
      restoreDaemonMode,
      logger,
    })

    expect(restoreDaemonMode).toHaveBeenCalledTimes(1)
    expect(logger.warn.mock.calls.flat().join(' ')).toContain('Timed out after 10000ms')
  })

  it('restores the daemon directly when the settings IPC threw', async () => {
    const restoreDaemonMode = vi.fn()

    const outcome = await handBackCompactionMode({
      updateSettings: async () => {
        throw new Error('no such frame')
      },
      restoreDaemonMode,
      logger: silentLogger(),
    })

    expect(restoreDaemonMode).toHaveBeenCalledTimes(1)
    expect(outcome.ok).toBe(false)
    expect(outcome.error).toContain('no such frame')
  })
})
