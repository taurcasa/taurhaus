/**
 * Account rows for a context menu, built from the tool registry.
 *
 * A launch item grows an account submenu because its tool declares
 * `accountSelection`, never because of what the tool is called — the next tool
 * to gain accounts gets the same submenu without touching this file's callers.
 *
 * Usage is rendered as one compact string here and nowhere else. Today it reads
 * the two windows the status-line bridge reports; when the generic usage
 * snapshot lands it changes in this one function.
 */

import { toolDescriptor } from './toolRegistry.js'

/** Menus only ask a question the host can answer more than one way. */
const MIN_ACCOUNTS_FOR_SUBMENU = 2

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
  return String(account?.display_name ?? '').trim() || account?.email || account?.id || ''
}

/**
 * The one place a menu row says how much of a subscription is left.
 *
 * A window whose reset has passed describes a window that no longer exists, so
 * it is dropped — the same rule `ClaudeUsageMeter` applies, for the same
 * reason. An account nothing has reported for gets no meta at all rather than a
 * row of zeroes.
 */
export function accountUsageMeta(account, now = Date.now()) {
  const usage = account?.usage
  if (!usage) return ''
  return [
    { label: '5h', window: usage.five_hour },
    { label: '7d', window: usage.seven_day },
  ]
    .filter(({ window }) => {
      const used = Number(window?.used_percentage)
      if (!Number.isFinite(used)) return false
      if (window.resets_at == null) return true
      const resetsAt = Number(window.resets_at)
      return !(Number.isFinite(resetsAt) && resetsAt * 1000 <= now)
    })
    .map(({ label, window }) => `${label} ${Math.round(Number(window.used_percentage))}%`)
    .join(' · ')
}

/**
 * One child row per detected account.
 *
 * Logged-out accounts stay visible and disabled: a subscription missing from
 * the list is a different fact from one that needs signing in again, and hiding
 * it makes the menu look like it forgot an account.
 */
export function buildAccountMenuChildren({
  accounts = [],
  activeAccountId = null,
  onSelect = () => {},
} = {}) {
  return accounts.map((account) => ({
    label: accountLabel(account),
    meta: account.logged_in ? accountUsageMeta(account) : 'not logged in',
    check: account.id === activeAccountId,
    disabled: !account.logged_in,
    action: () => onSelect(account.id),
  }))
}
