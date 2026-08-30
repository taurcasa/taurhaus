/**
 * The two account popups, mounted the way the app mounts them.
 *
 * `account.fixtures.js` renders the chooser and the chip on a bare page,
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
 * The provider-window shape a launch actually judges on, unlike the two-field
 * legacy snapshot above: the chooser's automatic trigger reads `windows`.
 */
function providerUsage({ session, week, minutesAgo }) {
  const now = Date.now()
  return {
    observed_at: new Date(now - minutesAgo * 60_000).toISOString(),
    status: 'ok',
    windows: [
      {
        key: 'session',
        title: 'Current session',
        used_percentage: session,
        resets_at: Math.floor(now / 1000) + 2 * 3600 + 600,
        severity: 'normal',
        is_active: true,
      },
      {
        key: 'week',
        title: 'Current week (all models)',
        used_percentage: week,
        resets_at: WEEK_RESETS_AT,
        severity: week >= 100 ? 'critical' : 'normal',
        is_active: true,
      },
    ],
    note: null,
  }
}

const WEEK_RESETS_AT = Math.floor(Date.now() / 1000) + 41 * 3600

/**
 * The launch the chooser interrupted: the remembered subscription has spent
 * its week, so the dialog opens by itself and has to say why before it asks.
 */
const SPENT = [
  { ...PRIMARY, usage: providerUsage({ session: 8, week: 100, minutesAgo: 2 }) },
  { ...SECOND, usage: providerUsage({ session: 12, week: 31, minutesAgo: 3 }) },
  LOGGED_OUT,
]

const EXHAUSTED_REASON = {
  kind: 'exhausted',
  accountLabel: 'stierms@gmail.com',
  windowTitle: 'Current week (all models)',
  resetsAt: WEEK_RESETS_AT,
}

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
function scenario(name, theme, surface, accounts = ACCOUNTS, extra = {}) {
  return {
    name,
    theme,
    surface,
    accounts,
    projectName: 'taurhaus',
    selectedAccountId: null,
    defaultAccountId: null,
    reason: null,
    preselectedAccountId: null,
    ...extra,
  }
}

export const shellPopupsScenarios = [
  scenario('chooser-light', 'light', 'chooser'),
  scenario('chooser-dark', 'dark', 'chooser'),
  scenario('chooser-exhausted-light', 'light', 'chooser', SPENT, {
    reason: EXHAUSTED_REASON,
    preselectedAccountId: 'account-2',
  }),
  scenario('chooser-exhausted-dark', 'dark', 'chooser', SPENT, {
    reason: EXHAUSTED_REASON,
    preselectedAccountId: 'account-2',
  }),
  scenario('chip-menu-light', 'light', 'chip'),
  scenario('chip-menu-dark', 'dark', 'chip'),
  scenario('sidebar-account-submenu-light', 'light', 'sidebar'),
  scenario('sidebar-account-submenu-dark', 'dark', 'sidebar'),
  scenario('sidebar-same-display-name-dark', 'dark', 'sidebar', SAME_NAME),
]
