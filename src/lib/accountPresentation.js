const LAST_KNOWN_AFTER_MS = 15 * 60 * 1000

function relationshipList(relationships, camel, snake) {
  const value = relationships?.[camel] ?? relationships?.[snake]
  return Array.isArray(value) ? value : []
}

function isRelevant(account, state) {
  if (account?.id === state?.defaultAccountId) return true
  const relationships = state?.relationships?.[account?.id]
  return (
    relationshipList(relationships, 'pinnedProjects', 'pinned_projects').length > 0 ||
    relationshipList(relationships, 'teams', 'teams').length > 0
  )
}

function liveWindows(usage, now) {
  return (Array.isArray(usage?.windows) ? usage.windows : []).filter((window) => {
    if (window?.is_active === false || window?.isActive === false) return false
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

/** Product copy for the backend's settled launch-account provenance. */
export function accountOriginSentence(origin) {
  switch (origin) {
    case 'project':
    case 'pinned':
    case 'explicit':
      return 'pinned to this project'
    case 'last_used':
      return 'last used here'
    case 'global_default':
    case 'default':
      return 'your global default'
    case 'base_command':
      return 'carried by your launch command'
    case 'session':
    case 'transcript':
      return "resumes this session's account"
    case 'single_account':
    case 'only_account':
    case 'default_config_dir':
      return 'the only account signed in'
    default:
      return 'selected for this project'
  }
}
