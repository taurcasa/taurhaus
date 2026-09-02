const LAST_KNOWN_AFTER_MS = 15 * 60 * 1000

/** Match a backend-reported selector value to its detected account directory. */
export function accountForSelectorValue(value, accounts = []) {
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

function relationshipList(relationships, camel, snake) {
  const value = relationships?.[camel] ?? relationships?.[snake]
  return Array.isArray(value) ? value : []
}

function isRelevant(account, state) {
  if (account?.id === state?.defaultAccountId) return true
  if (account?.is_default || account?.isDefault || account?.is_process_default || account?.isProcessDefault) {
    return true
  }
  const relationships = state?.relationships?.[account?.id]
  return (
    relationshipList(relationships, 'pinnedProjects', 'pinned_projects').length > 0 ||
    relationshipList(relationships, 'teams', 'teams').length > 0
  )
}

function liveWindows(usage, now) {
  return (Array.isArray(usage?.windows) ? usage.windows : []).filter((window) => {
    const used = Number(window?.used_percentage ?? window?.usedPercentage)
    if (!Number.isFinite(used)) return false
    const reset = Number(window?.resets_at ?? window?.resetsAt)
    return !Number.isFinite(reset) || reset * 1000 > now
  })
}

function accountDanger(account) {
  return !account?.logged_in || account?.usage?.status === 'unauthorized'
}

function warningWindow(account, now) {
  return liveWindows(account?.usage, now)
    .filter((window) => {
      const used = Number(window?.used_percentage ?? window?.usedPercentage)
      return used >= 80 || window?.severity === 'warning' || window?.severity === 'critical'
    })
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
