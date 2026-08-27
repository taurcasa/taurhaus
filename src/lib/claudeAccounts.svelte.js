/**
 * Claude subscriptions (config dirs) and the one decision they need.
 *
 * The backend resolves the account for every launch on its own — request
 * override, then the session being resumed, then the project's stored choice,
 * then the global default. This module exists for the single case the backend
 * cannot decide: a project with no choice of its own, on a host with more than
 * one signed-in subscription and no configured global default. It asks once,
 * remembers the answer for the whole app session, and gets out of the way.
 * With one account it never appears.
 */

import {
  getSettings,
  launchClaudeSession,
  listClaudeAccounts,
  resolveClaudeLaunchAccount,
  setProjectClaudeAccount,
} from './ipc.js'
import { toolDescriptor } from './toolRegistry.js'

export const claudeAccounts = $state({
  /** Every detected account, logged in or not. */
  accounts: [],
  /**
   * Detection could not run — a daemon that dropped out, typically. `accounts`
   * is then the last list we knew, not the current truth.
   */
  degraded: false,
  /** The account configured as the global default, if the user chose one. */
  defaultAccountId: null,
  /**
   * Project choices made in this app session, before the project payloads that
   * carry them are refetched. Keyed by project id.
   */
  projectChoices: {},
  /** Set while a launch waits for the user to pick an account. */
  pending: null,
})

/** Accounts that can actually run a session. */
export function loggedInAccounts() {
  return claudeAccounts.accounts.filter((account) => account.logged_in)
}

/** Accounts to show in a chooser — logged-out ones stay visible but disabled. */
export function resolveChooserAccounts() {
  return claudeAccounts.accounts
}

/**
 * A stored account id, but only while the account it names is detected and
 * signed in. A pinned subscription that logged out has answered nothing: the
 * backend refuses it too, and its launch would land on whatever the fallback
 * finds.
 */
function usableAccountId(accountId) {
  if (!accountId) return null
  return loggedInAccounts().some((account) => account.id === accountId) ? accountId : null
}

/** The configured global default, when it is detected and can run. */
function globalClaudeAccount() {
  const configured = claudeAccounts.defaultAccountId
  if (!configured) return null
  return (
    claudeAccounts.accounts.find((account) => account.id === configured && account.logged_in) ??
    null
  )
}

/** Remember the default the user just chose in Settings. */
export function setGlobalClaudeAccount(accountId) {
  claudeAccounts.defaultAccountId = accountId || null
}

/**
 * The account a project runs on: its own choice, whether made in this session
 * or carried by the project payload. `null` means it inherits the default.
 */
export function effectiveClaudeAccountId(project) {
  const projectId = project?.id
  if (projectId && projectId in claudeAccounts.projectChoices) {
    return claudeAccounts.projectChoices[projectId]
  }
  return project?.claudeAccountId ?? project?.claude_account_id ?? null
}

/**
 * The account a launch from this project would land on today.
 *
 * The same precedence the backend applies, as far as the frontend can see it:
 * the project's pin, then the configured global default, then the account in
 * the default config dir. It is what a menu ticks — "this is the one you get if
 * you just click the row" — and nothing more; the backend still has the last
 * word, and a resume still follows its transcript.
 */
export function activeClaudeAccountId(project) {
  return (
    usableAccountId(effectiveClaudeAccountId(project)) ??
    globalClaudeAccount()?.id ??
    claudeAccounts.accounts.find((account) => account.is_default && account.logged_in)?.id ??
    null
  )
}

/**
 * Pin a project to an account (or `null` to inherit), optimistically: the chip
 * and the next launch see it immediately, and a failed write puts it back.
 */
export function setProjectClaudeAccountChoice(projectId, accountId) {
  if (!projectId) return Promise.resolve()
  const previous = projectId in claudeAccounts.projectChoices
  const previousValue = claudeAccounts.projectChoices[projectId]
  claudeAccounts.projectChoices = {
    ...claudeAccounts.projectChoices,
    [projectId]: accountId ?? null,
  }
  return Promise.resolve(setProjectClaudeAccount(projectId, accountId ?? null)).catch((error) => {
    console.warn('Failed to store the Claude account for this project:', error)
    const restored = { ...claudeAccounts.projectChoices }
    if (previous) restored[projectId] = previousValue
    else delete restored[projectId]
    claudeAccounts.projectChoices = restored
  })
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

/**
 * Re-read what each subscription has left, without re-running detection.
 *
 * Usage moves while the app is open — a status line reports every 30 s of an
 * active session — but the account *list* does not, and it is what the minute
 * of detection caching is for. This asks for the current numbers and copies
 * only those onto the accounts already on screen, so opening the chip never
 * reshuffles it.
 *
 * A refresh that brings no numbers for an account leaves the ones it has. The
 * backend reports nothing while the sink is being rewritten, and a record it
 * has already written does not go away — so blanking the meter would say "this
 * subscription has never reported" about one that reported a minute ago. The
 * numbers carry the moment they were observed and the meter labels its own age,
 * which is why keeping them is the honest answer.
 */
export function refreshClaudeAccountUsage() {
  return Promise.resolve(listClaudeAccounts())
    .then((report) => {
      if (report?.degraded) return
      const usageById = new Map(
        (report?.accounts ?? [])
          .filter((account) => account.usage)
          .map((account) => [account.id, account.usage]),
      )
      claudeAccounts.accounts = claudeAccounts.accounts.map((account) =>
        usageById.has(account.id)
          ? { ...account, usage: usageById.get(account.id) }
          : account,
      )
    })
    .catch((error) => {
      // Usage is a hint, never a gate: the numbers on screen stay as they were.
      console.warn('Failed to refresh Claude subscription usage:', error)
    })
}

/**
 * Detected accounts, keeping the numbers this answer did not bring its own for.
 *
 * The same rule as a usage refresh, for the same reason: usage is unreadable
 * for as long as the sink is being rewritten, and a record already written does
 * not go away. Dropping the numbers on a detection pass would blank the meter
 * for an account that reported a minute ago.
 */
function keepKnownUsage(accounts) {
  const known = new Map(
    claudeAccounts.accounts
      .filter((account) => account.usage)
      .map((account) => [account.id, account.usage]),
  )
  return accounts.map((account) => ({
    ...account,
    usage: account.usage ?? known.get(account.id) ?? null,
  }))
}

function detectClaudeAccounts() {
  const accounts = Promise.resolve(listClaudeAccounts()).then((report) => {
    if (report?.degraded) {
      // A daemon that dropped out has signed nobody out. Its empty list is
      // silence, not an answer: keep the accounts we last knew, say they are
      // stale, and leave the next caller free to ask again.
      console.warn('Claude account detection is unavailable:', report.error)
      claudeAccounts.degraded = true
      detection = null
      return
    }
    claudeAccounts.accounts = keepKnownUsage(report?.accounts ?? [])
    claudeAccounts.degraded = false
  })
  const settings = Promise.resolve(getSettings())
    .then((loaded) => {
      claudeAccounts.defaultAccountId = loaded?.terminal?.claude_default_account_id ?? null
    })
    // The global default is a preference, not a gate: an unreadable settings
    // blob leaves it unset and the chooser asks, which is the safe direction.
    .catch(() => {})

  return Promise.all([accounts, settings])
    .catch((error) => {
      // Detection is a convenience, never a gate: a launch during an outage
      // goes ahead on whatever the backend resolves. The accounts we already
      // know stay on screen — a failed call is not evidence they are gone —
      // and the failure is not cached, so a daemon that connects a moment
      // later restores the real list.
      console.warn('Failed to detect Claude accounts:', error)
      claudeAccounts.degraded = true
      detection = null
    })
}

/** Modes whose account the history decides, not the user. */
const HISTORY_MODES = new Set(['resume', 'continue'])

/**
 * Whether the backend already knows which subscription this launch runs on.
 *
 * `--resume` and `--continue` only see the history of the config dir they run
 * in, and the transcript that owns a project's history names it. That lookup is
 * the backend's; asking the user instead would pin the resume to an answer
 * which outranks the transcript and opens a different history.
 */
async function backendPlacesLaunch(projectId, mode) {
  if (!HISTORY_MODES.has(mode)) return false
  try {
    const placed = await resolveClaudeLaunchAccount(projectId, mode)
    return !(placed?.needsChoice ?? placed?.needs_choice ?? true)
  } catch (error) {
    // Asking is the safe direction: the launch gets an account either way, and
    // this way the user picks it.
    console.warn('Failed to resolve the Claude account for this launch:', error)
    return false
  }
}

/**
 * Launch a session, asking which subscription to use only when the answer is
 * genuinely unknown.
 *
 * `accountId` is the user having already answered — picked from the context
 * menu's account submenu. There is nothing left to ask, so nothing is asked:
 * the launch runs on that account, and a project that had chosen nothing keeps
 * it, which is what the chooser's "Remember for this project" already defaults
 * to. A project with a pin of its own is left alone: one launch elsewhere is
 * not a decision to move.
 *
 * `launch` is injected by tests; production uses the IPC directly.
 */
export async function requestClaudeLaunch({
  project,
  mode,
  tool = 'claude',
  accountId = null,
  remember = true,
  launch = launchClaudeSession,
  onError = null,
}) {
  const projectId = project?.id
  if (!projectId) return

  const run = (chosenAccountId) =>
    Promise.resolve(launch(projectId, mode, tool, chosenAccountId ?? null)).catch((error) => {
      if (onError) onError(error)
      else console.error('[cmd-center] launch FAILED:', error)
    })

  if (!toolDescriptor(tool)?.capabilities.accountSelection) return run(null)

  if (accountId) {
    const stored = remember && !effectiveClaudeAccountId(project)
      ? setProjectClaudeAccountChoice(projectId, accountId)
      : Promise.resolve()
    return stored.then(() => run(accountId))
  }

  // Detection may still be in flight (a launch clicked during startup); asking
  // an empty list would skip the chooser and run on the backend default.
  await refreshClaudeAccounts()
  if (loggedInAccounts().length < 2) return run(null)

  // A choice that can still run is the answer. It is not passed along: the
  // backend applies it, and anything that outranks it, on its own.
  const alreadyChosen = usableAccountId(effectiveClaudeAccountId(project)) || globalClaudeAccount()
  if (alreadyChosen || (await backendPlacesLaunch(projectId, mode))) return run(null)

  // The chooser is where the user compares subscriptions, so it opens on the
  // current numbers rather than on whatever detection last cached.
  await refreshClaudeAccountUsage()

  // The chooser owns the rest of the flow: this call is done once it is open.
  claudeAccounts.pending = {
    projectId,
    projectName: project?.name ?? '',
    mode,
    tool,
    confirm: (accountId, remember) => {
      claudeAccounts.pending = null
      const stored = remember
        ? setProjectClaudeAccountChoice(projectId, accountId)
        : Promise.resolve()
      // The pick is passed explicitly: it is this launch's answer, and the
      // backend must not have to wait for the write to land.
      return stored.then(() => run(accountId))
    },
    cancel: () => {
      claudeAccounts.pending = null
    },
  }
}

/** Test seam: the store is module state shared by the whole app. */
export function resetClaudeAccountsForTest() {
  claudeAccounts.accounts = []
  claudeAccounts.degraded = false
  claudeAccounts.defaultAccountId = null
  claudeAccounts.projectChoices = {}
  claudeAccounts.pending = null
  detection = null
}
