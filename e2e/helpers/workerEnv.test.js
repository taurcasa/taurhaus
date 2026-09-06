import { mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { isAbsolute, join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'

import {
  WORKER_ROOT_ENV_KEYS,
  assertWorkerMeshAvailable,
  buildWorkerEnv,
  findAvailableWorkerDaemonPort,
  prepareWorkerHome,
} from './workerEnv.js'
import { E2E_RUN_TOKEN_ENV } from './laneCleanup.js'
import { spawn, spawnSync } from 'node:child_process'
import { once } from 'node:events'

function isInside(root, candidate) {
  const pathFromRoot = relative(root, candidate)
  return pathFromRoot !== '' && !pathFromRoot.startsWith('..') && !isAbsolute(pathFromRoot)
}

describe('buildWorkerEnv', () => {
  // Regression: 925c78c3 prepended exit-77 guards ahead of the inherited inert
  // harnesses. Runtime members died immediately, leaving Resume instead of Add Agent.
  it('keeps explicit runtime fixtures alive and shuts down only the generated children', async () => {
    const root = mkdtempSync(join(tmpdir(), 'taurhaus-runtime-stub-'))
    try {
      const env = buildWorkerEnv(root, { baseEnv: { PATH: '/usr/bin:/bin' } })
      prepareWorkerHome(env.HOME, { persistentHarnesses: true })
      for (const tool of ['claude', 'codex', 'agy', 'grok']) {
        const child = spawn(join(env.HOME, '.local', 'bin', tool), [], { env, stdio: 'ignore' })
        const closed = once(child, 'close')
        try {
          await once(child, 'spawn')
          // A bounded liveness check of an inert fixture, not a real CLI or load test.
          await new Promise(resolve => setTimeout(resolve, 100))
          expect(child.exitCode, `${tool} must remain available for runtime Add Agent`).toBeNull()
        } finally {
          if (child.exitCode === null) child.kill('SIGTERM')
          await closed
        }
      }
    } finally { rmSync(root, { recursive: true, force: true }) }
  })
  it('blocks default harness executables in ordinary workers without invoking an installed CLI', () => {
    const root = mkdtempSync(join(tmpdir(), 'taurhaus-cli-guard-'))
    try {
      const env = buildWorkerEnv(root, { baseEnv: { PATH: '/usr/bin:/bin' } })
      prepareWorkerHome(env.HOME)
      expect(env.PATH.split(':')[0]).toBe(join(env.HOME, '.local', 'bin'))
      for (const tool of ['claude', 'codex', 'agy', 'grok']) {
        // Absolute generated path: a missing shim can never fall through to a real CLI.
        const result = spawnSync(join(env.HOME, '.local', 'bin', tool), ['--version'], { env, encoding: 'utf8' })
        expect(result.status).toBe(77)
        expect(result.stderr).toContain('E2E requires an explicit test stub')
      }
    } finally { rmSync(root, { recursive: true, force: true }) }
  })
  // Regression: commit f9c1e893 isolated only the app-data and Claude roots;
  // later Codex, Grok and Antigravity integrations inherited the operator's
  // real homes in ordinary E2E workers.
  it('puts every writable product root under the worker temp root', () => {
    const sessionTempRoot = '/tmp/taurhaus-e2e-1234-worker'
    const operatorHome = homedir()
    const env = buildWorkerEnv(sessionTempRoot, {
      baseEnv: {
        HOME: operatorHome,
        PATH: '/usr/bin',
        TAURHAUS_DATA_DIR: join(operatorHome, '.local/share/com.taurhaus.dev'),
        TAURHAUS_CLAUDE_DIR: join(operatorHome, '.claude'),
        CODEX_HOME: join(operatorHome, '.codex'),
        GROK_HOME: join(operatorHome, '.grok'),
        TAURHAUS_AGY_DIR: join(operatorHome, '.gemini'),
      },
    })

    expect(WORKER_ROOT_ENV_KEYS).toEqual([
      'TAURHAUS_DATA_DIR',
      'TAURHAUS_CLAUDE_DIR',
      'CODEX_HOME',
      'GROK_HOME',
      'TAURHAUS_AGY_DIR',
    ])
    for (const key of WORKER_ROOT_ENV_KEYS) {
      expect(isInside(sessionTempRoot, env[key]), `${key} must be inside the worker root`).toBe(true)
      expect(env[key].startsWith(operatorHome), `${key} must not use the operator home`).toBe(false)
    }

    // Regression: commit fc896344 overrode named product roots but preserved
    // HOME, so account providers still discovered ~/.codex*, ~/.grok*, and
    // ~/.claude* siblings and polled the operator's subscriptions.
    expect(isInside(sessionTempRoot, env.HOME), 'HOME must be inside the worker root').toBe(true)
    expect(env.HOME.startsWith(operatorHome), 'HOME must not use the operator home').toBe(false)
    expect(env.PATH).toBe(`${env.HOME}/.local/bin:/usr/bin`)
  })

  // Regression: commit 272eed7d isolated HOME without the mesh binary that
  // taurhaus resolves exclusively through ~/.local/bin/mesh. Tier-2 workers
  // then reported mesh unavailable and skipped their runtime coverage.
  it('seeds the isolated home with shell startup and the resolved mesh binary', () => {
    const workerRoot = mkdtempSync(join(tmpdir(), 'taurhaus-worker-home-'))
    const workerHome = join(workerRoot, 'home')
    const meshBinary = join(workerRoot, 'operator-mesh')

    try {
      writeFileSync(meshBinary, '#!/bin/sh\n')
      prepareWorkerHome(workerHome, { meshBinaryPath: meshBinary })
      expect(readFileSync(join(workerHome, '.zshrc'), 'utf8')).toContain('taurhaus E2E')
      expect(realpathSync(join(workerHome, '.local', 'bin', 'mesh'))).toBe(realpathSync(meshBinary))
    } finally {
      rmSync(workerRoot, { recursive: true, force: true })
    }
  })

  // Regression: commit 272eed7d made mesh unavailable under the isolated HOME,
  // while the mesh specs converted that broken worker into silent Tier-2 skips.
  it('fails loudly when a worker cannot see the mesh binary', () => {
    expect(() => assertWorkerMeshAvailable({
      meshAvailable: false,
      blockingErrors: ['MESH_MISSING'],
    })).toThrow(/MESH_MISSING/)

    expect(() => assertWorkerMeshAvailable({ meshAvailable: true })).not.toThrow()
  })

  // Regression: commit fc896344 isolated worker roots but left every ordinary
  // E2E app on the operator's tmux server and daemon port 17233.
  it('gives every worker a private tmux server and daemon port', () => {
    const sessionTempRoot = '/tmp/taurhaus-e2e-1234-worker'
    const daemonBinaryPath = '/checkout/src-tauri/target/debug/taurhaus-daemon'
    const env = buildWorkerEnv(sessionTempRoot, {
      baseEnv: {
        TMUX: '/tmp/tmux-1000/default,407334,0',
        TMUX_TMPDIR: '/tmp/operator-tmux',
        TAURHAUS_DAEMON_PORT: '17233',
      },
      daemonBinaryPath,
    })

    expect(env.TMUX_TMPDIR).toBe(join(sessionTempRoot, 'tmux'))
    expect('TMUX' in env).toBe(false)
    expect(Number(env.TAURHAUS_DAEMON_PORT)).toBeGreaterThan(0)
    expect(env.TAURHAUS_DAEMON_PORT).not.toBe('17233')

    // Regression: commit 7908cbf4 gave the worker a private port but still
    // launched whichever daemon happened to be installed for the operator.
    // With E2E_INSTALL_DAEMON=0, that binary can predate isolated auth roots.
    expect(env.TAURHAUS_DAEMON_BINARY).toBe(daemonBinaryPath)
  })

  it('derives a stable private daemon port that differs between worker roots', () => {
    const first = buildWorkerEnv('/tmp/taurhaus-e2e-worker-a').TAURHAUS_DAEMON_PORT
    const repeated = buildWorkerEnv('/tmp/taurhaus-e2e-worker-a').TAURHAUS_DAEMON_PORT
    const second = buildWorkerEnv('/tmp/taurhaus-e2e-worker-b').TAURHAUS_DAEMON_PORT

    expect(repeated).toBe(first)
    expect(second).not.toBe(first)
    expect(Number(first)).toBeGreaterThanOrEqual(20_000)
    expect(Number(first)).toBeLessThan(32_000)
  })

  // Regression: commit 7908cbf4 selected a worker port by hash without
  // checking whether another process already owned it.
  it('walks to another worker port when the derived candidate is occupied', async () => {
    const root = '/tmp/taurhaus-e2e-worker-collision'
    const first = Number(buildWorkerEnv(root).TAURHAUS_DAEMON_PORT)
    const checked = []

    const available = await findAvailableWorkerDaemonPort(root, {
      isPortAvailable: async (candidate) => {
        checked.push(candidate)
        return candidate !== first
      },
    })

    expect(checked).toEqual([first, first + 1])
    expect(available).toBe(first + 1)
  })

  // Regression: commit fc896344 isolated writable roots but left ordinary
  // startup free to probe the operator's real CLI executables.
  it('disables real CLI version probes in ordinary workers', () => {
    const env = buildWorkerEnv('/tmp/taurhaus-e2e-1234-worker', {
      baseEnv: { TAURHAUS_SKIP_CLI_VERSION_PROBES: '' },
    })

    expect(env.TAURHAUS_SKIP_CLI_VERSION_PROBES).toBe('1')
  })

  // Regression: commit 7908cbf4 disabled version probes even in paid Codex
  // hook lanes, leaving hook support unknown and preventing installation.
  it('allows a paid worker to probe its explicitly invoked CLI version', () => {
    const env = buildWorkerEnv('/tmp/taurhaus-e2e-paid-worker', {
      baseEnv: { TAURHAUS_SKIP_CLI_VERSION_PROBES: '1' },
      skipCliVersionProbes: false,
    })

    expect(env.TAURHAUS_SKIP_CLI_VERSION_PROBES).toBeUndefined()
  })

  // Regression: commit 69bb4e1a added a UUID run token to the ownership
  // ledger, but rebuilding the driver environment replaced it with the temp
  // root. Token scans then found none of the driver's descendants.
  it('preserves an inherited run token when deriving a child environment', () => {
    const sessionTempRoot = '/tmp/taurhaus-e2e-1234-worker'
    const first = buildWorkerEnv(sessionTempRoot, { runToken: 'run-uuid-1234' })
    const child = buildWorkerEnv(sessionTempRoot, { baseEnv: first })

    expect(first[E2E_RUN_TOKEN_ENV]).toBe('run-uuid-1234')
    expect(child[E2E_RUN_TOKEN_ENV]).toBe(first[E2E_RUN_TOKEN_ENV])
  })
})
