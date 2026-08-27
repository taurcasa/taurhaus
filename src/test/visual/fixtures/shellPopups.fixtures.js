/**
 * The two account popups, mounted the way the app mounts them.
 *
 * `claudeAccount.fixtures.js` renders the chooser and the chip on a bare page,
 * which is why a placement bug could ship unseen: both popups position
 * themselves against ancestors the bare page does not have. These scenarios
 * carry the surrounding markup — the shell frame for the chooser, the Overview
 * header and its scrolling body for the chip — so a screenshot of one is a
 * screenshot of what the user sees.
 */

const PRIMARY = {
  id: 'account-1',
  config_dir: '/home/user/.claude',
  email: 'stierms@gmail.com',
  display_name: 'Who',
  organization: "stierms@gmail.com's Organization",
  seat_tier: 'claude_max',
  logged_in: true,
  is_default: true,
}

const SECOND = {
  id: 'account-2',
  config_dir: '/home/user/.claude-account2',
  email: 'm.stier@giesi.com',
  display_name: 'Matthias',
  organization: "m.stier@giesi.com's Organization",
  seat_tier: 'claude_max',
  logged_in: true,
  is_default: false,
}

const LOGGED_OUT = {
  id: 'account-3',
  config_dir: '/home/user/.claude-work',
  email: 'work@example.com',
  display_name: 'Work',
  organization: 'Acme Inc',
  seat_tier: 'team',
  logged_in: false,
  is_default: false,
}

function usage({ fiveHour, sevenDay, minutesAgo }) {
  const now = Date.now()
  return {
    five_hour: { used_percentage: fiveHour, resets_at: Math.floor(now / 1000) + 2 * 3600 + 600 },
    seven_day: { used_percentage: sevenDay, resets_at: Math.floor(now / 1000) + 41 * 3600 },
    observed_at: new Date(now - minutesAgo * 60_000).toISOString(),
  }
}

const ACCOUNTS = [
  { ...PRIMARY, usage: usage({ fiveHour: 91, sevenDay: 62, minutesAgo: 2 }) },
  { ...SECOND, usage: usage({ fiveHour: 12, sevenDay: 8, minutesAgo: 4 }) },
  LOGGED_OUT,
]

/**
 * One person, one display name, two subscriptions — the case that crashed the
 * submenu when a row's identity was its label.
 */
const SAME_NAME = [
  ACCOUNTS[0],
  { ...ACCOUNTS[1], display_name: 'Who' },
  LOGGED_OUT,
]

/** `surface` picks which of the app's two mount points the host reproduces. */
function scenario(name, theme, surface, accounts = ACCOUNTS) {
  return {
    name,
    theme,
    surface,
    accounts,
    projectName: 'taurhaus',
    selectedAccountId: null,
    defaultAccountId: null,
  }
}

export const shellPopupsScenarios = [
  scenario('chooser-light', 'light', 'chooser'),
  scenario('chooser-dark', 'dark', 'chooser'),
  scenario('chip-menu-light', 'light', 'chip'),
  scenario('chip-menu-dark', 'dark', 'chip'),
  scenario('sidebar-account-submenu-light', 'light', 'sidebar'),
  scenario('sidebar-account-submenu-dark', 'dark', 'sidebar'),
  scenario('sidebar-same-display-name-dark', 'dark', 'sidebar', SAME_NAME),
]
