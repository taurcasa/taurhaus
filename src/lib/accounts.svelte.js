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

export function refreshUsage(tool = providerTool()) {
  const id = toolId(tool)
  const state = mutableAccountState(id)
  return Promise.resolve(refreshAccountsUsage(id))
    .then(() => listAccounts(id))
    .then((report) => {
      if (report?.degraded) return
      const usageById = new Map(
        (report?.accounts ?? [])
          .filter((account) => account.usage)
          .map((account) => [account.id, account.usage])
      )
      state.accounts = state.accounts.map((account) =>
        usageById.has(account.id)
          ? { ...account, usage: usageById.get(account.id) }
          : account
      )
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

async function backendPlacesLaunch(projectId, tool, mode) {
  if (!HISTORY_MODES.has(mode)) return false
  try {
    const placed = await resolveLaunchAccount(projectId, tool, mode)
    return !(placed?.needsChoice ?? placed?.needs_choice ?? true)
  } catch (error) {
    console.warn('Failed to resolve the account for this launch:', error)
    return false
  }
}

export async function requestLaunch({
  project,
  mode,
  tool,
  accountId = null,
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

  const effective = effectiveAccount(project, id)
  if (
    (effective.account && effective.origin !== 'default_config_dir') ||
    (await backendPlacesLaunch(projectId, id, mode))
  ) {
    return run(null)
  }

  await refreshUsage(id)
  state.pending = {
    projectId,
    projectName: project?.name ?? '',
    mode,
    tool: id,
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
  for (const state of Object.values(accounts.byTool)) {
    state.accounts = []
    state.degraded = false
    state.defaultAccountId = null
    state.projectChoices = {}
    state.pending = null
  }
  detections.clear()
}
