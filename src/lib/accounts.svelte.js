/**
 * Tool-keyed account detection, usage and launch choice state.
 */

import {
  getSettings,
  launchCliSession,
  listAccounts,
  refreshAccountsUsage,
  resolveLaunchAccount,
  resolveLaunchBases,
  setProjectAccount,
} from './ipc.js'
import { toolDescriptor, tools } from './toolRegistry.js'
import { exhaustedUsage } from './usageWindows.js'

const accounts = $state({ byTool: {} })
const EMPTY_STATE = Object.freeze({
  accounts: Object.freeze([]),
  resolvedBases: Object.freeze([]),
  resolvingBases: false,
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
      resolvedBases: [],
      resolvingBases: false,
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

/**
 * The sentence for a launch command whose head is not the tool's own CLI.
 *
 * taurhaus renders the account selector in front of it and stops there: it
 * will not run a wrapper to find out what the wrapper does with it.
 */
export function opaqueBaseNotice(head, tool = providerTool()) {
  const label = toolDescriptor(tool)?.label ?? 'CLI'
  return `taurhaus could not select an account: your launch command runs "${head}", which is not the ${label} CLI`
}

/**
 * Why a launch did not run on the account it was given, in one sentence.
 * `null` when the launch applied the account it was asked for.
 */
export function launchAccountNotice(result, { project, tool } = {}) {
  if (result?.account_applied !== false) return null
  if (result.account_note === 'opaque_base_command') {
    return opaqueBaseNotice(result.account_note_detail ?? 'that command', tool)
  }
  return `${project?.name ?? 'This project'} continued on the team's default account`
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
const launchBaseResolutions = new Map()
const DETECTION_TTL_MS = 60_000
const USAGE_SYNC_INITIAL_RETRY_MS = 250
const USAGE_SYNC_MAX_RETRY_MS = 16_000
const USAGE_SYNC_DEADLINE_MS = 30_000
const usageSyncTimers = new Map()

/**
 * Forget what the backend said this tool's launch commands mean.
 *
 * A command the operator has just edited was never resolved, and the answer to
 * the one it replaced describes a command no launch will run. Until a refresh
 * lands, the literal commands in settings are the honest answer.
 */
export function forgetResolvedBases(tool = providerTool()) {
  const id = toolId(tool)
  mutableAccountState(id).resolvedBases = []
  launchBaseResolutions.delete(id)
}

/** Ask what this tool's configured launch commands mean, independently of detection. */
export function refreshResolvedBases(tool = providerTool(), { force = false } = {}) {
  const id = toolId(tool)
  const current = launchBaseResolutions.get(id)
  if (!force && current && Date.now() - current.startedAt < DETECTION_TTL_MS) {
    return current.promise
  }

  const state = mutableAccountState(id)
  state.resolvingBases = true
  let failed = false
  const promise = Promise.resolve()
    .then(() => resolveLaunchBases(id))
    .then((bases) => {
      if (launchBaseResolutions.get(id)?.promise === promise) state.resolvedBases = bases ?? []
    })
    .catch((error) => {
      failed = true
      console.warn('Failed to resolve launch commands:', error)
    })
    .finally(() => {
      if (launchBaseResolutions.get(id)?.promise === promise) {
        state.resolvingBases = false
        if (failed) launchBaseResolutions.delete(id)
      }
    })
  launchBaseResolutions.set(id, { startedAt: Date.now(), promise })
  return promise
}

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

function scheduleUsageSync(tool, pending, deadline, retryMs = USAGE_SYNC_INITIAL_RETRY_MS) {
  if (pending.size === 0 || Date.now() >= deadline) return
  const previous = usageSyncTimers.get(tool)
  if (previous) clearTimeout(previous)
  const timer = setTimeout(async () => {
    if (usageSyncTimers.get(tool) === timer) usageSyncTimers.delete(tool)
    if (Date.now() >= deadline) return
    try {
      const report = await listAccounts(tool)
      const remaining = mergeUsageReport(mutableAccountState(tool), report, pending)
      scheduleUsageSync(
        tool,
        remaining,
        deadline,
        Math.min(retryMs * 2, USAGE_SYNC_MAX_RETRY_MS)
      )
    } catch (error) {
      console.warn('Failed to read refreshed account usage:', error)
    }
  }, Math.min(retryMs, deadline - Date.now()))
  timer?.unref?.()
  usageSyncTimers.set(tool, timer)
}

export function refreshUsage(tool = providerTool()) {
  const id = toolId(tool)
  const state = mutableAccountState(id)
  const pending = new Map(
    state.accounts
      .filter((account) => account.logged_in && account.usage_capable !== false)
      .map((account) => [account.id, usageObservation(account)])
  )
  return Promise.resolve(refreshAccountsUsage(id))
    .then((scheduled) => listAccounts(id).then((report) => ({ report, scheduled })))
    .then(({ report, scheduled }) => {
      const remaining = mergeUsageReport(state, report, pending)
      if (scheduled) {
        scheduleUsageSync(id, remaining, Date.now() + USAGE_SYNC_DEADLINE_MS)
      }
    })
    .catch((error) => {
      console.warn('Failed to refresh account usage:', error)
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
 * changes. That judgement is made on the last usage reading taurhaus holds, the
 * one every other surface is already showing; a fresher one is asked for in the
 * background, never waited on. `'always'` is the user asking, and skips
 * straight to the dialog with the account they would otherwise have got
 * pre-selected.
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
      // Judged on the reading the detection above just returned — the same one
      // the chip and the account menus are showing. A weekly or five-hour
      // limit is slow-moving state and the chooser stays reachable on demand,
      // so nothing is learned by holding a launch open for a fresher number:
      // the refresh asked for here is for the next launch, and for the meters
      // in the dialog if one opens. A degraded detection confirmed nothing
      // about any account, so it vetoes nothing either.
      reason = accountState(id).degraded
        ? null
        : exhaustionReason(usableAccount(id, settledAccountId))
      void refreshUsage(id)
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
  for (const timer of usageSyncTimers.values()) clearTimeout(timer)
  usageSyncTimers.clear()
  for (const state of Object.values(accounts.byTool)) {
    state.accounts = []
    state.resolvedBases = []
    state.resolvingBases = false
    state.degraded = false
    state.defaultAccountId = null
    state.projectChoices = {}
    state.pending = null
  }
  detections.clear()
  launchBaseResolutions.clear()
}
