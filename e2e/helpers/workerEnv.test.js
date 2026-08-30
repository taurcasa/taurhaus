import { describe, expect, it } from 'vitest'
import { homedir } from 'node:os'
import { isAbsolute, join, relative } from 'node:path'

import { WORKER_ROOT_ENV_KEYS, buildWorkerEnv } from './workerEnv.js'

function isInside(root, candidate) {
  const pathFromRoot = relative(root, candidate)
  return pathFromRoot !== '' && !pathFromRoot.startsWith('..') && !isAbsolute(pathFromRoot)
}

describe('buildWorkerEnv', () => {
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
    expect(env.PATH).toBe('/usr/bin')
  })

  // Regression: commit fc896344 isolated worker roots but left every ordinary
  // E2E app on the operator's tmux server and daemon port 17233.
  it('gives every worker a private tmux server and daemon port', () => {
    const sessionTempRoot = '/tmp/taurhaus-e2e-1234-worker'
    const env = buildWorkerEnv(sessionTempRoot, {
      baseEnv: {
        TMUX: '/tmp/tmux-1000/default,407334,0',
        TMUX_TMPDIR: '/tmp/operator-tmux',
        TAURHAUS_DAEMON_PORT: '17233',
      },
    })

    expect(env.TMUX_TMPDIR).toBe(join(sessionTempRoot, 'tmux'))
    expect('TMUX' in env).toBe(false)
    expect(Number(env.TAURHAUS_DAEMON_PORT)).toBeGreaterThan(0)
    expect(env.TAURHAUS_DAEMON_PORT).not.toBe('17233')
  })
})
