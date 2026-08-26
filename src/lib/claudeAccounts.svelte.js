/**
 * Claude subscriptions (config dirs) and the one decision they need.
 *
 * The backend resolves the account for every launch on its own — request
 * override, then the session being resumed, then the project's stored choice,
 * then the global default. This module exists for the single case the backend
 * cannot decide: a project with no stored choice on a host that has more than
 * one signed-in subscription. It asks once, remembers the answer, and gets out
 * of the way. With one account it never appears.
 */

import { launchClaudeSession, listClaudeAccounts, setProjectClaudeAccount } from './ipc.js'

export const claudeAccounts = $state({
  /** Every detected account, logged in or not. */
  accounts: [],
  /** Set while a launch waits for the user to pick an account. */
  pending: null,
  loaded: false,
})

/** Accounts that can actually run a session. */
export function loggedInAccounts() {
  return claudeAccounts.accounts.filter((account) => account.logged_in)
}

/** Accounts to show in a chooser — logged-out ones stay visible but disabled. */
export function resolveChooserAccounts() {
  return claudeAccounts.accounts
}

/** In-flight or recent detection, so three mounts do not mean three IPCs. */
let detection = null
const DETECTION_TTL_MS = 60_000

export function refreshClaudeAccounts({ force = false } = {}) {
  if (!force && detection && Date.now() - detection.startedAt < DETECTION_TTL_MS) {
    return detection.promise
  }
  const promise = detectClaudeAccounts()
  detection = { startedAt: Date.now(), promise }
  return promise
}

function detectClaudeAccounts() {
  return listClaudeAccounts()
    .then((accounts) => {
      claudeAccounts.accounts = Array.isArray(accounts) ? accounts : []
    })
    .catch((error) => {
      // Detection is a convenience, never a gate: an unreachable daemon or an
      // older one leaves the list empty and every launch behaves as before.
      console.warn('Failed to detect Claude accounts:', error)
      claudeAccounts.accounts = []
    })
    .finally(() => {
      claudeAccounts.loaded = true
    })
}

function projectAccountId(project) {
  return project?.claude_account_id ?? project?.claudeAccountId ?? null
}

/**
 * Launch a session, asking which subscription to use only when the answer is
 * genuinely unknown.
 *
 * `launch` is injected by tests; production uses the IPC directly.
 */
export function requestClaudeLaunch({
  project,
  mode,
  tool = 'claude',
  launch = launchClaudeSession,
  onError = null,
}) {
  const projectId = project?.id
  if (!projectId) return Promise.resolve()

  const run = (accountId) =>
    Promise.resolve(launch(projectId, mode, tool, accountId ?? null)).catch((error) => {
      if (onError) onError(error)
      else console.error('[cmd-center] launch FAILED:', error)
    })

  const needsChoice =
    tool === 'claude' && !projectAccountId(project) && loggedInAccounts().length >= 2

  if (!needsChoice) return run(null)

  // The chooser owns the rest of the flow: this call is done once it is open.
  claudeAccounts.pending = {
    projectId,
    projectName: project?.name ?? '',
    mode,
    tool,
    confirm: (accountId, remember) => {
      claudeAccounts.pending = null
      const stored = remember
        ? Promise.resolve(setProjectClaudeAccount(projectId, accountId)).catch((error) => {
            console.warn('Failed to remember the Claude account for this project:', error)
          })
        : Promise.resolve()
      return stored.then(() => run(accountId))
    },
    cancel: () => {
      claudeAccounts.pending = null
    },
  }
  return Promise.resolve()
}

/** Test seam: the store is module state shared by the whole app. */
export function resetClaudeAccountsForTest() {
  claudeAccounts.accounts = []
  claudeAccounts.pending = null
  claudeAccounts.loaded = false
  detection = null
}
