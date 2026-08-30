import { resolve } from 'node:path'

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

/** Build the complete environment for one isolated WDIO worker. */
export function buildWorkerEnv(sessionTempRoot, { baseEnv = {} } = {}) {
  if (typeof sessionTempRoot !== 'string' || sessionTempRoot.trim() === '') {
    throw new Error('session temp root is required')
  }

  const root = resolve(sessionTempRoot)
  const env = { ...baseEnv }
  for (const key of WORKER_ROOT_ENV_KEYS) {
    env[key] = resolve(root, ROOT_SUBDIRS[key])
  }
  return env
}
