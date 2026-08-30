import { describe, expect, it } from 'vitest'
import { homedir } from 'node:os'
import { isAbsolute, relative } from 'node:path'

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
    const env = buildWorkerEnv(sessionTempRoot, {
      baseEnv: { HOME: homedir(), PATH: '/usr/bin' },
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
      expect(env[key].startsWith(homedir()), `${key} must not use the operator home`).toBe(false)
    }
    expect(env.PATH).toBe('/usr/bin')
  })
})
