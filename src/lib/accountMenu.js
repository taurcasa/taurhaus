/**
 * Account rows for a context menu, built from the tool registry.
 *
 * A launch item grows an account submenu because its tool declares
 * `accountSelection`, never because of what the tool is called — the next tool
 * to gain accounts gets the same submenu without touching this file's callers.
 *
 * Usage is rendered from the provider's ordered windows as one compact string
 * here and nowhere else.
 */

import { toolDescriptor } from './toolRegistry.js'

/** Menus only ask a question the host can answer more than one way. */
const MIN_ACCOUNTS_FOR_SUBMENU = 2

/** The marker the backend puts on a session it runs for a team. */
const TEAM_GROUP_KIND = 'mesh_team'

/** Modes the backend hands to the team runtime when the project is a member's. */
const DELEGATED_MODES = new Set(['continue', 'resume'])

/** Why an account row is offered but cannot be picked. */
export const TEAM_ACCOUNT_NOTE = 'team runs on default account'

/**
 * Whether the team runtime would take this launch over.
 *
 * The backend delegates a generic continue/resume to coordination when the
 * project is exactly one team member's, and a delegated launch runs in the
 * team's own config dir — an account chosen here would go nowhere. The sidebar
 * sees the same fact from the other side: the team marker on the project's live
 * sessions. Two members of one tool are ambiguous to the backend too, and it
 * falls back to a raw launch that does honour the account.
 */
export function launchDelegatesToTeam(mode, tool, sessions = []) {
  if (!DELEGATED_MODES.has(mode)) return false
  const members = (sessions ?? []).filter(
    (session) => session?.group_kind === TEAM_GROUP_KIND && session?.cli_tool === tool
  )
  return members.length === 1
}

/** Whether a tool's launch items can carry an account submenu at all. */
export function toolSelectsAccounts(tool) {
  return Boolean(toolDescriptor(tool)?.capabilities.accountSelection)
}

/** The accounts a session can actually run on. */
function selectableAccounts(accounts) {
  return (accounts ?? []).filter((account) => account?.logged_in)
}

/**
 * Whether a launch item for this tool becomes a submenu parent: only when the
 * host has a real choice to offer.
 */
export function accountSubmenuApplies(tool, accounts) {
  return (
    toolSelectsAccounts(tool) && selectableAccounts(accounts).length >= MIN_ACCOUNTS_FOR_SUBMENU
  )
}

/** Display name, falling back to the email that is always there. */
function accountLabel(account) {
  return (
    String(account?.display_name ?? '').trim() ||
    account?.label ||
    account?.email ||
    account?.id ||
    ''
  )
}

/**
 * Labels that tell the rows apart.
 *
 * Two people's subscriptions can be named the same, and a row the user cannot
 * tell from the one above it is not a choice. The email is what distinguishes
 * them; a name nothing collides with is left as the name.
 */
function labelsFor(accounts) {
  const counts = new Map()
  for (const account of accounts) {
    const label = accountLabel(account)
    counts.set(label, (counts.get(label) ?? 0) + 1)
  }

  return accounts.map((account) => {
    const label = accountLabel(account)
    if ((counts.get(label) ?? 0) < 2) return label
    const identity = account?.label || account?.email
    return identity ? `${label} (${identity})` : label
  })
}

/**
 * The one place a menu row says how much of a subscription is left.
 *
 * A window whose reset has passed describes a window that no longer exists, so
 * it is dropped — the same rule `UsageMeter` applies, for the same
 * reason. An account nothing has reported for gets no meta at all rather than a
 * row of zeroes.
 */
export function accountUsageMeta(account, now = Date.now()) {
  const usage = account?.usage
  if (!usage) return ''
  const windows = Array.isArray(usage.windows)
    ? usage.windows.filter((window) => window.key !== 'session')
    : [
        usage.five_hour ? { title: '5h', ...usage.five_hour } : null,
        usage.seven_day ? { title: '7d', ...usage.seven_day } : null,
      ].filter(Boolean)
  return windows
    .filter((window) => {
      const used = Number(window?.used_percentage)
      if (!Number.isFinite(used)) return false
      if (window.resets_at == null) return true
      const resetsAt = Number(window.resets_at)
      return !(Number.isFinite(resetsAt) && resetsAt * 1000 <= now)
    })
    .map((window) => `${window.title} ${Math.round(Number(window.used_percentage))}%`)
    .join(' · ')
}

/**
 * One child row per detected account.
 *
 * Logged-out accounts stay visible and disabled: a subscription missing from
 * the list is a different fact from one that needs signing in again, and hiding
 * it makes the menu look like it forgot an account.
 *
 * `disabledNote` is the same answer for every row — something other than this
 * menu decides the launch's account. The rows still show, for the same reason:
 * a submenu that empties itself looks broken, one that says why does not. No
 * row is ticked either, because none of them is what the launch would use.
 */
export function buildAccountMenuChildren({
  accounts = [],
  activeAccountId = null,
  onSelect = () => {},
  disabledNote = null,
} = {}) {
  const labels = labelsFor(accounts)
  return accounts.map((account, index) => ({
    // Keyed by position as well as id: a label is not unique enough to key a
    // rendered list by, and the position is unique whatever the caller passes.
    key: `${index}:${account.id ?? ''}`,
    label: labels[index],
    meta: account.logged_in ? disabledNote ?? accountUsageMeta(account) : 'not logged in',
    check: !disabledNote && activeAccountId != null && account.id === activeAccountId,
    disabled: Boolean(disabledNote) || !account.logged_in,
    action: () => onSelect(account.id),
  }))
}
