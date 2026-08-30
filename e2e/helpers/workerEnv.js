import { createServer } from 'node:net'
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

const WORKER_DAEMON_PORT_START = 20_000
const WORKER_DAEMON_PORT_COUNT = 12_000

/** A stable non-ephemeral port derived from the worker's unique session root. */
function workerDaemonPort(sessionTempRoot) {
  let hash = 2166136261
  for (const char of resolve(sessionTempRoot)) {
    hash ^= char.codePointAt(0)
    hash = Math.imul(hash, 16777619) >>> 0
  }
  return WORKER_DAEMON_PORT_START + (hash % WORKER_DAEMON_PORT_COUNT)
}

function bindProbePort(port) {
  return new Promise((resolveAvailable) => {
    const server = createServer()
    server.unref()
    server.once('error', () => resolveAvailable(false))
    server.listen({ host: '127.0.0.1', port, exclusive: true }, () => {
      server.close(() => resolveAvailable(true))
    })
  })
}

/** Find a free worker port without addressing or terminating its current owner. */
export async function findAvailableWorkerDaemonPort(
  sessionTempRoot,
  { isPortAvailable = bindProbePort } = {},
) {
  const first = workerDaemonPort(sessionTempRoot)
  for (let offset = 0; offset < WORKER_DAEMON_PORT_COUNT; offset += 1) {
    const candidate = WORKER_DAEMON_PORT_START +
      ((first - WORKER_DAEMON_PORT_START + offset) % WORKER_DAEMON_PORT_COUNT)
    if (await isPortAvailable(candidate)) return candidate
  }
  throw new Error('no private E2E daemon port is available in 20000-31999')
}

/** Build the complete environment for one isolated WDIO worker. */
export function buildWorkerEnv(
  sessionTempRoot,
  {
    baseEnv = {},
    runToken,
    daemonBinaryPath,
    daemonPort,
    skipCliVersionProbes = true,
  } = {},
) {
  if (typeof sessionTempRoot !== 'string' || sessionTempRoot.trim() === '') {
    throw new Error('session temp root is required')
  }

  const root = resolve(sessionTempRoot)
  const env = { ...baseEnv }
  for (const key of WORKER_ROOT_ENV_KEYS) {
    env[key] = resolve(root, ROOT_SUBDIRS[key])
  }
  env.TAURHAUS_DAEMON_PORT = String(daemonPort ?? workerDaemonPort(root))
  if (daemonBinaryPath) env.TAURHAUS_DAEMON_BINARY = resolve(daemonBinaryPath)
  if (skipCliVersionProbes) env.TAURHAUS_SKIP_CLI_VERSION_PROBES = '1'
  else delete env.TAURHAUS_SKIP_CLI_VERSION_PROBES
  env[E2E_RUN_TOKEN_ENV] = String(runToken ?? baseEnv[E2E_RUN_TOKEN_ENV] ?? sessionTempRoot)
  applyTmuxIsolation(env, root)
  return env
}
