/**
 * Put the daemon's Codex compaction mode back from a cleanup step.
 *
 * Updating `terminal.harness.codex_compaction` through the settings IPC also
 * tells the connected daemon which mode to run, and that daemon is the
 * operator's own — shared, on 17233, not something a lane's isolated
 * `TAURHAUS_DATA_DIR` insulates. A lane that flips the mode therefore owes the
 * host the original value on every path out, including the interrupt that never
 * reaches Mocha's `after`.
 *
 * An exit handler cannot await, so the request goes out synchronously through a
 * child process (`daemonCompactionRequest.mjs`) and is bounded by a timeout: a
 * daemon that is gone, wedged or unreachable costs the run a few hundred
 * milliseconds and a printed reason, never a hang.
 */

import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

/** The port `models::DaemonSettings` defaults to. */
export const DEFAULT_DAEMON_PORT = 17233

const REQUEST_SCRIPT = join(import.meta.dirname, 'daemonCompactionRequest.mjs')

/**
 * Where the daemon writes its auth token — `dirs::data_dir()/taurhaus`, the
 * same resolution `daemon::auth::token_path()` performs, and outside any
 * `TAURHAUS_DATA_DIR` override.
 */
export function daemonTokenPath(env = process.env, home = homedir()) {
  const dataHome = env.XDG_DATA_HOME || join(home, '.local', 'share')
  return join(dataHome, 'taurhaus', 'daemon.token')
}

function readTokenQuietly(path) {
  try {
    const token = readFileSync(path, 'utf8').trim()
    return token || null
  } catch {
    return null
  }
}

/**
 * Ask the daemon to run `mode` ('hooks' | 'transcript'). Never throws.
 *
 * @returns {{ok: true} | {ok: false, error: string}}
 */
export function setDaemonCodexCompactionMode(mode, options = {}) {
  const {
    port = DEFAULT_DAEMON_PORT,
    tokenPath = daemonTokenPath(),
    timeoutMs = 4_000,
  } = options

  const request = {
    id: `e2e-compaction-mode-${Date.now()}`,
    method: 'set_codex_compaction_mode',
    params: { mode },
  }
  const token = readTokenQuietly(tokenPath)
  if (token) request.auth = token

  let stdout
  try {
    stdout = execFileSync(process.execPath, [REQUEST_SCRIPT], {
      encoding: 'utf8',
      // The child enforces the real bound; this only stops a child that never
      // exits from outliving it.
      timeout: timeoutMs + 2_000,
      stdio: ['ignore', 'pipe', 'pipe'],
      env: {
        ...process.env,
        TAURHAUS_DAEMON_PORT: String(port),
        TAURHAUS_DAEMON_REQUEST: JSON.stringify(request),
        TAURHAUS_DAEMON_TIMEOUT_MS: String(timeoutMs),
      },
    })
  } catch (error) {
    const reason = String(error?.stderr || error?.message || error).trim()
    return { ok: false, error: reason || 'daemon request failed' }
  }

  let response
  try {
    response = JSON.parse(stdout)
  } catch {
    return { ok: false, error: `unreadable daemon response: ${stdout.slice(0, 200)}` }
  }
  if (response?.error) {
    return { ok: false, error: `${response.error.code}: ${response.error.message}` }
  }
  return { ok: true }
}

/**
 * Hand the mode back on a normal teardown: the app first, then the daemon.
 *
 * `update_settings` returning ok is not evidence the daemon took the mode.
 * `commands/settings.rs::reconcile_codex_compaction_setting` pushes it to the
 * connected daemon inside that command and only *logs* a push that failed
 * (`compaction.codex_hook.degraded`); the command still returns the saved
 * settings. A daemon that had disconnected or refused would therefore be left
 * running the lane's mode while the lane counted the restoration done.
 *
 * So both run, always. `restoreDaemonMode` is the synchronous, bounded request
 * an exit handler would have made, and asking a daemon for the mode it is
 * already in costs one no-op round trip — much less than leaving the operator's
 * daemon in hooks mode.
 *
 * @returns the settings-IPC outcome, for the caller to report.
 */
export async function handBackCompactionMode({ updateSettings, restoreDaemonMode, logger = console }) {
  let restored
  try {
    restored = await updateSettings()
  } catch (error) {
    restored = { ok: false, error: String(error?.message ?? error) }
  }
  if (!restored?.ok) {
    logger.warn(`[e2e] settings restore failed (${restored?.error ?? 'no result'}); restoring the daemon mode directly`)
  }
  restoreDaemonMode()
  return restored
}
