import { resolve } from 'node:path'

import { applyTmuxIsolation } from './laneTmux.js'
import { E2E_RUN_TOKEN_ENV } from './laneCleanup.js'

export const WORKER_ROOT_ENV_KEYS = [
  'TAURHAUS_DATA_DIR',
  'TAURHAUS_CLAUDE_DIR',
  'CODEX_HOME',
  'GROK_HOME',
  'TAURHAUS_AGY_DIR',
]

const ROOT_SUBDIRS = {
  TAURHAUS_DATA_DIR: 'app-data',
  TAURHAUS_CLAUDE_DIR: 'claude',
  CODEX_HOME: 'codex-home',
  GROK_HOME: 'grok-home',
  TAURHAUS_AGY_DIR: 'agy-home',
}

/** A stable high port derived from the worker's unique session root. */
function workerDaemonPort(sessionTempRoot) {
  let hash = 2166136261
  for (const char of resolve(sessionTempRoot)) {
    hash ^= char.codePointAt(0)
    hash = Math.imul(hash, 16777619) >>> 0
  }
  return 20_000 + (hash % 40_000)
}

/** Build the complete environment for one isolated WDIO worker. */
export function buildWorkerEnv(sessionTempRoot, { baseEnv = {}, runToken = sessionTempRoot } = {}) {
  if (typeof sessionTempRoot !== 'string' || sessionTempRoot.trim() === '') {
    throw new Error('session temp root is required')
  }

  const root = resolve(sessionTempRoot)
  const env = { ...baseEnv }
  for (const key of WORKER_ROOT_ENV_KEYS) {
    env[key] = resolve(root, ROOT_SUBDIRS[key])
  }
  env.TAURHAUS_DAEMON_PORT = String(workerDaemonPort(root))
  env[E2E_RUN_TOKEN_ENV] = String(runToken)
  applyTmuxIsolation(env, root)
  return env
}
