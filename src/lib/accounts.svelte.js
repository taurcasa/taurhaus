/**
 * Tool-keyed account detection, usage and launch choice state.
 */

import {
  getSettings,
  launchCliSession,
  listAccounts,
  refreshAccountsUsage,
  resolveLaunchAccount,
  setProjectAccount,
} from './ipc.js'
import { toolDescriptor, tools } from './toolRegistry.js'
import { exhaustedUsage } from './usageWindows.js'

const accounts = $state({ byTool: {} })
const EMPTY_STATE = Object.freeze({
  accounts: Object.freeze([]),
  degraded: false,
  defaultAccountId: null,
  projectChoices: Object.freeze({}),
  pending: null,
})

function providerTool() {
  return tools().find((tool) => tool.capabilities.accountSelection)?.id ?? tools()[0]?.id ?? ''
}

function toolId(tool) {
  return String(tool || providerTool())
}

export function accountState(tool = providerTool()) {
  const id = toolId(tool)
  return accounts.byTool[id] ?? EMPTY_STATE
}

function mutableAccountState(tool = providerTool()) {
  const id = toolId(tool)
  if (!accounts.byTool[id]) {
    accounts.byTool[id] = {
      accounts: [],
      degraded: false,
      defaultAccountId: null,
      projectChoices: {},
      pending: null,
    }
  }
  return accounts.byTool[id]
}

for (const tool of tools()) mutableAccountState(tool.id)

export function loggedInAccounts(tool = providerTool()) {
  return accountState(tool).accounts.filter((account) => account.logged_in)
}

export function resolveChooserAccounts(tool = providerTool()) {
  return accountState(tool).accounts
}

function usableAccount(tool, accountId) {
  if (!accountId) return null
  return (
    accountState(tool).accounts.find(
      (account) => account.id === accountId && account.logged_in
    ) ?? null
  )
}

export function setDefaultAccount(tool, accountId) {
  mutableAccountState(tool).defaultAccountId = accountId || null
}

function memoryFor(project, tool) {
  return project?.accountMemory?.[tool] ?? project?.account_memory?.[tool] ?? null
}

function idFromMap(project, camel, snake, tool) {
  return project?.[camel]?.[tool] ?? project?.[snake]?.[tool] ?? null
}

/**
 * The frontend-visible account resolution, including its provenance.
 *
 * The backend remains authoritative for process inspection and transcript
 * ownership; these optional maps are supplied by views that already know
 * those facts.
 */
export function effectiveAccount(project, tool = providerTool()) {
  const state = accountState(tool)
  const candidates = [
    ['explicit', idFromMap(project, 'explicitAccountIds', 'explicit_account_ids', tool)],
    ['session', idFromMap(project, 'sessionAccountIds', 'session_account_ids', tool)],
  ]
  if (project?.id && project.id in state.projectChoices) {
    candidates.push(['pinned', state.projectChoices[project.id]])
  }
  const memory = memoryFor(project, tool)
  if (memory?.origin === 'pinned') {
    candidates.push(['pinned', memory.accountId ?? memory.account_id])
  }
  if (memory?.origin === 'last_used') {
    candidates.push(['last_used', memory.accountId ?? memory.account_id])
  }
  candidates.push(['default', state.defaultAccountId])
  candidates.push([
    'base_command',
    idFromMap(project, 'baseCommandAccountIds', 'base_command_account_ids', tool),
  ])

  for (const [origin, accountId] of candidates) {
    const account = usableAccount(tool, accountId)
    if (account) return { account, origin }
  }
  const fallback =
    state.accounts.find((account) => account.is_process_default && account.logged_in) ??
    state.accounts.find((account) => account.is_default && account.logged_in) ??
    state.accounts.find((account) => account.logged_in) ??
    null
  return { account: fallback, origin: 'default_config_dir' }
}

export function activeAccountId(project, tool = providerTool()) {
  return effectiveAccount(project, tool).account?.id ?? null
}

/**
 * Pin a project to an account, or clear its pin, with optimistic state.
 */
export function rememberChoice(projectOrId, tool, accountId) {
  const projectId = typeof projectOrId === 'object' ? projectOrId?.id : projectOrId
  if (!projectId) return Promise.resolve()
  const state = mutableAccountState(tool)
  const previous = projectId in state.projectChoices
  const previousValue = state.projectChoices[projectId]
  state.projectChoices = { ...state.projectChoices, [projectId]: accountId ?? null }
  return Promise.resolve(setProjectAccount(projectId, tool, accountId ?? null)).catch((error) => {
    console.warn('Failed to store the account for this project:', error)
    const restored = { ...state.projectChoices }
    if (previous) restored[projectId] = previousValue
    else delete restored[projectId]
    state.projectChoices = restored
  })
}

const detections = new Map()
const DETECTION_TTL_MS = 60_000
const USAGE_SYNC_INITIAL_RETRY_MS = 250
const USAGE_SYNC_MAX_RETRY_MS = 16_000
const USAGE_SYNC_DEADLINE_MS = 30_000
const usageSyncTimers = new Map()

export function refreshAccounts(tool = providerTool(), { force = false } = {}) {
  const id = toolId(tool)
  const current = detections.get(id)
  if (!force && current && Date.now() - current.startedAt < DETECTION_TTL_MS) {
    return current.promise
  }
  const promise = detectAccounts(id)
  detections.set(id, { startedAt: Date.now(), promise })
  return promise
}

function usageObservation(account) {
  return account?.usage?.observed_at ?? null
}

function mergeUsageReport(state, report, pending) {
  if (report?.degraded) return pending
  const usageById = new Map(
    (report?.accounts ?? [])
      .filter((account) => account.usage)
      .map((account) => [account.id, account.usage])
  )
  state.accounts = state.accounts.map((account) =>
    usageById.has(account.id) ? { ...account, usage: usageById.get(account.id) } : account
  )
  return new Map(
    [...pending].filter(([accountId, observedAt]) => {
      const next = usageById.get(accountId)?.observed_at ?? null
      return observedAt == null ? next == null : next === observedAt
    })
  )
}

/**
 * One caller waiting for a named account's reading to be superseded.
 *
 * It answers exactly once, whatever ends the chain that carries it: the newer
 * reading, the deadline, a failed read, or another refresh taking the timer
 * over. A launch that waits must never be left waiting.
 */
function usageWatch(accountId, resolve) {
  let answered = false
  return {
    accountId,
    settle(current) {
      if (answered) return
      answered = true
      resolve({ ok: true, current })
    },
  }
}

function stopUsageSync(tool) {
  const previous = usageSyncTimers.get(tool)
  if (!previous) return
  clearTimeout(previous.timer)
  usageSyncTimers.delete(tool)
  previous.watch?.settle(false)
}

function scheduleUsageSync(
  tool,
  pending,
  deadline,
  retryMs = USAGE_SYNC_INITIAL_RETRY_MS,
  watch = null
) {
  let waiting = watch
  if (waiting && !pending.has(waiting.accountId)) {
    waiting.settle(true)
    waiting = null
  }
  if (pending.size === 0 || Date.now() >= deadline) {
    waiting?.settle(false)
    return
  }
  stopUsageSync(tool)
  const timer = setTimeout(async () => {
    if (usageSyncTimers.get(tool)?.timer === timer) usageSyncTimers.delete(tool)
    if (Date.now() >= deadline) {
      waiting?.settle(false)
      return
    }
    try {
      const report = await listAccounts(tool)
      const remaining = mergeUsageReport(mutableAccountState(tool), report, pending)
      scheduleUsageSync(
        tool,
        remaining,
        deadline,
        Math.min(retryMs * 2, USAGE_SYNC_MAX_RETRY_MS),
        waiting
      )
    } catch (error) {
      console.warn('Failed to read refreshed account usage:', error)
      waiting?.settle(false)
    }
  }, Math.min(retryMs, deadline - Date.now()))
  timer?.unref?.()
  usageSyncTimers.set(tool, { timer, watch: waiting })
}

/**
 * Ask for fresh usage, and say what came of it.
 *
 * `refresh_accounts_usage` only *schedules* the fetch on the backend's own
 * poller thread, so the read that follows it still carries the numbers it was
 * asked to replace. `settleFor` is the launch path's answer to that: naming an
 * account makes the promise wait for that account's own reading to be
 * superseded, bounded by the same deadline the background sync uses.
 *
 * An account nothing has ever reported on is not waited for — there is no
 * reading to supersede, and a launch is never held up over one that does not
 * exist.
 *
 * Resolves `{ ok, current }`: `ok` is false when the round trip itself failed,
 * `current` false when the named account's reading is still the older one when
 * the wait runs out. A caller deciding something on those numbers must treat
 * either as "nothing new was learned".
 */
export function refreshUsage(tool = providerTool(), { settleFor = null } = {}) {
  const id = toolId(tool)
  const state = mutableAccountState(id)
  const pending = new Map(
    state.accounts
      .filter((account) => account.logged_in && account.usage_capable !== false)
      .map((account) => [account.id, usageObservation(account)])
  )
  const awaited = settleFor && pending.get(settleFor) != null ? settleFor : null
  return Promise.resolve(refreshAccountsUsage(id))
    .then((scheduled) => listAccounts(id).then((report) => ({ report, scheduled })))
    .then(({ report, scheduled }) => {
      const remaining = mergeUsageReport(state, report, pending)
      if (!scheduled) return { ok: true, current: true }
      const deadline = Date.now() + USAGE_SYNC_DEADLINE_MS
      if (!awaited || !remaining.has(awaited)) {
        scheduleUsageSync(id, remaining, deadline)
        return { ok: true, current: true }
      }
      return new Promise((resolve) => {
        scheduleUsageSync(
          id,
          remaining,
          deadline,
          USAGE_SYNC_INITIAL_RETRY_MS,
          usageWatch(awaited, resolve)
        )
      })
    })
    .catch((error) => {
      console.warn('Failed to refresh account usage:', error)
      return { ok: false, current: false }
    })
}

function keepKnownUsage(state, detected) {
  const known = new Map(
    state.accounts.filter((account) => account.usage).map((account) => [account.id, account.usage])
  )
  return detected.map((account) => ({
    ...account,
    usage: account.usage ?? known.get(account.id) ?? null,
  }))
}

function addressableAccounts(detected) {
  const byId = new Map()
  for (const account of detected) {
    const kept = byId.get(account.id)
    if (!kept || (!kept.logged_in && account.logged_in)) byId.set(account.id, account)
  }
  return [...byId.values()]
}

function detectAccounts(tool) {
  const state = mutableAccountState(tool)
  const detected = Promise.resolve(listAccounts(tool)).then((report) => {
    if (report?.degraded) {
      if (!state.degraded) console.warn('Account detection is unavailable:', report.error)
      state.degraded = true
      detections.delete(tool)
      return
    }
    state.accounts = keepKnownUsage(state, addressableAccounts(report?.accounts ?? []))
    state.degraded = false
  })
  const settings = Promise.resolve(getSettings())
    .then((loaded) => {
      state.defaultAccountId =
        loaded?.terminal?.default_account_ids?.[tool] ??
        loaded?.terminal?.defaultAccountIds?.[tool] ??
        null
    })
    .catch(() => {})

  return Promise.all([detected, settings]).catch((error) => {
    if (!state.degraded) console.warn('Failed to detect accounts:', error)
    state.degraded = true
    detections.delete(tool)
  })
}

const HISTORY_MODES = new Set(['resume', 'continue'])

export function launchFollowsHistory(mode) {
  return HISTORY_MODES.has(mode)
}

/**
 * The account the backend would place this launch on, or `null` when it would
 * place none.
 *
 * The transcript that decides a resume is the backend's to read, and its answer
 * carries the account itself — not merely that something decided. Anything
 * judged about that launch has to be judged about *that* subscription, so the
 * id travels with the answer even when nothing here can name it.
 */
async function backendPlacedAccount(projectId, tool, mode) {
  if (!HISTORY_MODES.has(mode)) return null
  try {
    const placed = await resolveLaunchAccount(projectId, tool, mode)
    if (placed?.needsChoice ?? placed?.needs_choice ?? true) return null
    return { accountId: placed?.accountId ?? placed?.account_id ?? null }
  } catch (error) {
    console.warn('Failed to resolve the account for this launch:', error)
    return null
  }
}

/** The email is what tells two subscriptions of the same person apart. */
function launchAccountLabel(account) {
  return (
    account?.label ||
    account?.email ||
    String(account?.display_name ?? '').trim() ||
    account?.id ||
    ''
  )
}

/**
 * Why the chooser is opening, when something other than the user opened it.
 *
 * `null` is the answer for an account with headroom *and* for one nothing has
 * ever reported on — a launch is never held up over a reading that does not
 * exist.
 */
function exhaustionReason(account) {
  const spent = exhaustedUsage(account?.usage)
  if (!spent) return null
  return {
    kind: spent.kind,
    accountLabel: launchAccountLabel(account),
    windowTitle: spent.window?.title ?? null,
    resetsAt: spent.window?.resets_at ?? null,
  }
}

/**
 * Launch, or ask which subscription to launch on.
 *
 * `choose` picks the trigger. `'auto'` keeps every decision that already stood
 * — an account the caller named, a project's memory, a resume the backend
 * places from its transcript — and only interrupts when the account that
 * decision lands on has nothing left to spend: the one moment the answer
 * changes. `'always'` is the user asking, and skips straight to the dialog with
 * the account they would otherwise have got pre-selected.
 */
export async function requestLaunch({
  project,
  mode,
  tool,
  accountId = null,
  choose = 'auto',
  launch = launchCliSession,
  onError = null,
}) {
  const projectId = project?.id
  if (!projectId) return
  const id = toolId(tool)
  const state = mutableAccountState(id)
  const run = (chosen) =>
    Promise.resolve(launch(projectId, mode, id, chosen ?? null)).catch((error) => {
      if (onError) onError(error)
      else console.error('[cmd-center] launch FAILED:', error)
    })

  if (!toolDescriptor(id)?.capabilities.accountSelection) return run(null)
  if (accountId) return run(accountId)

  await refreshAccounts(id)
  if (loggedInAccounts(id).length < 2) return run(null)

  let reason = null
  let preselectedAccountId = null

  if (choose === 'always') {
    preselectedAccountId = effectiveAccount(project, id).account?.id ?? null
    await refreshUsage(id)
  } else {
    // For a resume the backend outranks anything remembered here: it reads the
    // transcript, which decides the launch whatever this side would have
    // picked.
    const placed = await backendPlacedAccount(projectId, id, mode)
    const memory = effectiveAccount(project, id)
    const remembered = memory.origin === 'default_config_dir' ? null : memory.account?.id ?? null
    const settledAccountId = placed ? placed.accountId : remembered

    if (placed || remembered) {
      // The reading has to be current before it can veto a launch, and the
      // account has to be re-read after it: `refreshUsage` replaces the account
      // records rather than mutating them. A refresh that fails or never lands
      // leaves the launch exactly as it was before any of this existed.
      const refreshed = await refreshUsage(id, { settleFor: settledAccountId })
      if (!refreshed.ok || !refreshed.current) return run(null)
      reason = exhaustionReason(usableAccount(id, settledAccountId))
      if (!reason) return run(null)
    } else {
      await refreshUsage(id)
    }
  }

  state.pending = {
    projectId,
    projectName: project?.name ?? '',
    mode,
    tool: id,
    reason,
    preselectedAccountId,
    confirm: (chosen, remember) => {
      state.pending = null
      const stored = remember ? rememberChoice(projectId, id, chosen) : Promise.resolve()
      return stored.then(() => run(chosen))
    },
    cancel: () => {
      state.pending = null
    },
  }
}

export function pendingAccountChoice() {
  return Object.values(accounts.byTool).find((state) => state.pending)?.pending ?? null
}

export function resetAccountsForTest() {
  for (const tool of [...usageSyncTimers.keys()]) stopUsageSync(tool)
  usageSyncTimers.clear()
  for (const state of Object.values(accounts.byTool)) {
    state.accounts = []
    state.degraded = false
    state.defaultAccountId = null
    state.projectChoices = {}
    state.pending = null
  }
  detections.clear()
}
