import { liveUsageWindows, windowPressure } from './usageWindows.js'

const LAST_KNOWN_AFTER_MS = 15 * 60 * 1000

/** Match a backend-reported selector value to its detected account directory. */
function accountForSelectorValue(value, accounts = []) {
  const dir = String(value ?? '')
  if (!dir) return null
  if (dir.startsWith('~/')) {
    const tail = dir.slice(1)
    return (
      accounts.find((account) => String(account?.dir ?? account?.config_dir ?? '').endsWith(tail)) ??
      null
    )
  }
  return (
    accounts.find((account) => (account?.dir ?? account?.config_dir) === dir) ?? null
  )
}

/**
 * What a tool's configured launch commands select, and by which spelling.
 *
 * The selector and the alias are independent facts: a base command may spell
 * `CLAUDE_CONFIG_DIR=…` out in Settings with no alias behind it, so an
 * explainer keyed on the expansion misses half the cases. An opaque head
 * outranks both — a command taurhaus cannot see through decides the account
 * itself — and is reported across every base rather than only the matched one,
 * exactly as the Settings authority reads it.
 */
export function baseCommandSelection(bases, accounts = []) {
  const list = Array.isArray(bases) ? bases : []
  const opaqueHead = list.map((base) => base?.opaqueHead ?? base?.opaque_head).find(Boolean) ?? null
  for (const base of list) {
    const account = accountForSelectorValue(base?.selectorValue ?? base?.selector_value, accounts)
    if (!account) continue
    return {
      opaqueHead,
      account,
      alias: base.expansions?.[0] ?? null,
      command: base.command ?? '',
    }
  }
  return { opaqueHead, account: null, alias: null, command: null }
}

function relationshipList(relationships, camel, snake) {
  const value = relationships?.[camel] ?? relationships?.[snake]
  return Array.isArray(value) ? value : []
}

function isDefaultDirectory(account) {
  return Boolean(
    account?.is_default ||
      account?.isDefault ||
      account?.is_process_default ||
      account?.isProcessDefault
  )
}

/**
 * What this tool's launch commands select, as the resolver reads them.
 *
 * `null` when no base carries a selector at all. Otherwise the account the
 * selector names, which is `null` in its own right when the directory it names
 * is one nothing detected — the resolver stops there with no account rather
 * than falling on to the default directory.
 */
function baseCommandSelector(state) {
  const bases = Array.isArray(state?.resolvedBases) ? state.resolvedBases : []
  for (const base of bases) {
    const value = String(base?.selectorValue ?? base?.selector_value ?? '')
    if (!value) continue
    return { account: accountForSelectorValue(value, state?.accounts ?? []) }
  }
  return null
}

/**
 * Whether a usable choice the backend ranks higher owns this tool's launches.
 *
 * The default directory is the last thing the resolver reaches for: a saved
 * global default that is signed in answers every launch before the directory is
 * consulted, and so does a selector the launch command carries. A saved default
 * nothing can use is no supersession at all — the resolver falls past it — and
 * neither is a selector naming an account that cannot run. A selector naming a
 * directory nothing detected does supersede: that launch resolves to no account
 * whatever, so the directory is not what it lands on.
 */
function defaultDirectorySuperseded(account, state) {
  const savedDefaultId = state?.defaultAccountId
  if (
    savedDefaultId &&
    savedDefaultId !== account?.id &&
    (state?.accounts ?? []).some(
      (candidate) => candidate?.id === savedDefaultId && candidate?.logged_in
    )
  ) {
    return true
  }

  const selector = baseCommandSelector(state)
  if (!selector) return false
  if (!selector.account) return true
  return Boolean(selector.account.logged_in) && selector.account.id !== account?.id
}

/**
 * Whether any project pins this account, reading both pin authorities.
 *
 * The relationship index is the stored truth and a pin made during this run is
 * the fresher one: `rememberChoice` writes the choice, not the index. So a
 * project this run has decided about is answered from that decision alone —
 * which is how a cleared pin stops counting before the index is re-read.
 */
function hasPinnedProject(account, state) {
  const choices = state?.projectChoices ?? {}
  if (Object.values(choices).some((accountId) => accountId && accountId === account?.id)) {
    return true
  }
  return relationshipList(
    state?.relationships?.[account?.id],
    'pinnedProjects',
    'pinned_projects'
  ).some((project) => project?.id && !(project.id in choices))
}

function isRelevant(account, state) {
  if (account?.id === state?.defaultAccountId) return true
  if (
    hasPinnedProject(account, state) ||
    relationshipList(state?.relationships?.[account?.id], 'teams', 'teams').length > 0
  ) {
    return true
  }
  return isDefaultDirectory(account) && !defaultDirectorySuperseded(account, state)
}

function liveWindows(usage, now) {
  return liveUsageWindows(usage?.windows, now)
}

function accountDanger(account) {
  return !account?.logged_in || account?.usage?.status === 'unauthorized'
}

function warningWindow(account, now) {
  return liveWindows(account?.usage, now)
    .filter((window) => windowPressure(window) !== 'normal')
    .sort(
      (left, right) =>
        Number(right?.used_percentage ?? right?.usedPercentage) -
        Number(left?.used_percentage ?? left?.usedPercentage)
    )[0] ?? null
}

/**
 * The sidebar's one calm-contract decision.
 *
 * Only accounts something depends on can light the chrome. Sign-in failures
 * outrank quota pressure; otherwise the largest warning window supplies the
 * magnitude, so the button says how close the constraint is rather than merely
 * announcing that one exists.
 */
export function ambientAccountSignal(states, now = Date.now()) {
  const relevant = (states ?? []).flatMap((state) =>
    (state?.accounts ?? [])
      .filter((account) => isRelevant(account, state))
      .map((account) => ({ account, tool: state.tool }))
  )

  const danger = relevant.find(({ account }) => accountDanger(account))
  if (danger) {
    return {
      visible: true,
      tone: 'danger',
      magnitude: 'Sign in',
      account: danger.account,
      tool: danger.tool,
    }
  }

  const warnings = relevant
    .map(({ account, tool }) => ({ account, tool, window: warningWindow(account, now) }))
    .filter(({ window }) => window)
    .sort(
      (left, right) =>
        Number(right.window.used_percentage ?? right.window.usedPercentage) -
        Number(left.window.used_percentage ?? left.window.usedPercentage)
    )
  if (warnings.length > 0) {
    const worst = warnings[0]
    return {
      visible: true,
      tone: 'warning',
      magnitude: `${Math.round(
        Number(worst.window.used_percentage ?? worst.window.usedPercentage)
      )}%`,
      account: worst.account,
      tool: worst.tool,
      window: worst.window,
    }
  }

  return { visible: false, tone: 'calm', magnitude: null, account: null }
}

export function usageIsLastKnown(usage, now = Date.now()) {
  const observed = Date.parse(usage?.observed_at ?? usage?.observedAt ?? '')
  return Number.isFinite(observed) && now - observed > LAST_KNOWN_AFTER_MS
}

const ACCOUNT_ORIGIN_COPY = {
  request: { sentence: 'chosen for this launch', hint: 'this launch' },
  session: { sentence: "resumes this session's account", hint: 'session' },
  project: { sentence: 'pinned to this project', hint: 'pinned' },
  last_used: { sentence: 'last used here', hint: 'last used' },
  global_default: { sentence: 'your global default', hint: 'global default' },
  base_command: { sentence: 'carried by your launch command', hint: 'from launch command' },
  signed_in: { sentence: 'a signed-in account', hint: 'signed in' },
  default_config_dir: { sentence: "the tool's default directory", hint: 'default directory' },
}

// `effectiveAccount` predates the backend wire vocabulary and still supplies
// these local names to AccountChip. Keep that translation explicit instead of
// treating the two provenance contracts as interchangeable.
const EFFECTIVE_ACCOUNT_ORIGIN_HINT = {
  explicit: '',
  session: '',
  pinned: '',
  last_used: 'last used',
  default: 'default',
  base_command: 'from launch command',
  default_config_dir: 'default',
}

/** Product copy for the backend's settled launch-account provenance. */
export function accountOriginSentence(origin) {
  return ACCOUNT_ORIGIN_COPY[origin]?.sentence ?? 'selected for this project'
}

/** Compact copy for AccountChip's frontend-local effective-account origin. */
export function accountOriginHint(origin) {
  if (Object.hasOwn(EFFECTIVE_ACCOUNT_ORIGIN_HINT, origin)) {
    return EFFECTIVE_ACCOUNT_ORIGIN_HINT[origin]
  }
  return ACCOUNT_ORIGIN_COPY[origin]?.hint ?? ''
}
