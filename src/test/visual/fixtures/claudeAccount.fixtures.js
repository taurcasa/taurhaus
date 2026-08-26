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

/**
 * Usage as the status line reports it. Times are relative to render so the
 * fixtures keep meaning "just now" and "hours ago" whenever they are shot.
 */
function usage({ fiveHour, sevenDay, minutesAgo }) {
  const now = Date.now()
  return {
    five_hour: { used_percentage: fiveHour, resets_at: Math.floor(now / 1000) + 2 * 3600 + 600 },
    seven_day: { used_percentage: sevenDay, resets_at: Math.floor(now / 1000) + 41 * 3600 },
    observed_at: new Date(now - minutesAgo * 60_000).toISOString(),
  }
}

export const claudeAccountScenarios = [
  {
    name: 'single-account-light',
    theme: 'light',
    accounts: [PRIMARY],
    projectName: 'taurhaus',
    selectedAccountId: null,
    expected: { chooser: false, chip: false },
  },
  {
    name: 'two-accounts-light',
    theme: 'light',
    accounts: [PRIMARY, SECOND],
    projectName: 'taurhaus',
    selectedAccountId: null,
    expected: { chooser: true, chip: true },
  },
  {
    name: 'two-accounts-dark',
    theme: 'dark',
    accounts: [PRIMARY, SECOND],
    projectName: 'taurhaus',
    selectedAccountId: 'account-2',
    expected: { chooser: true, chip: true },
  },
  {
    name: 'global-default-second-light',
    theme: 'light',
    accounts: [PRIMARY, SECOND],
    projectName: 'taurhaus',
    selectedAccountId: null,
    // Settings named the second subscription the default: the chip and the
    // chooser's Enter answer follow it, not the `~/.claude` dir.
    defaultAccountId: 'account-2',
    expected: { chooser: true, chip: true },
  },
  {
    name: 'logged-out-account-dark',
    theme: 'dark',
    accounts: [PRIMARY, SECOND, LOGGED_OUT],
    projectName: 'mesh',
    selectedAccountId: null,
    expected: { chooser: true, chip: true },
  },
  {
    // The decision this feature exists for: one subscription nearly spent, the
    // other wide open, both reported minutes ago.
    name: 'usage-fresh-light',
    theme: 'light',
    accounts: [
      { ...PRIMARY, usage: usage({ fiveHour: 91, sevenDay: 62, minutesAgo: 2 }) },
      { ...SECOND, usage: usage({ fiveHour: 12, sevenDay: 8, minutesAgo: 4 }) },
    ],
    projectName: 'taurhaus',
    selectedAccountId: null,
    expected: { chooser: true, chip: true },
  },
  {
    name: 'usage-fresh-dark',
    theme: 'dark',
    accounts: [
      { ...PRIMARY, usage: usage({ fiveHour: 26, sevenDay: 17, minutesAgo: 1 }) },
      { ...SECOND, usage: usage({ fiveHour: 78, sevenDay: 44, minutesAgo: 6 }) },
    ],
    projectName: 'taurhaus',
    selectedAccountId: 'account-2',
    expected: { chooser: true, chip: true },
  },
  {
    // Usage only flows while a session of that account runs, so hours-old
    // numbers are the normal case, and they say so instead of their reset.
    name: 'usage-stale-light',
    theme: 'light',
    accounts: [
      { ...PRIMARY, usage: usage({ fiveHour: 54, sevenDay: 33, minutesAgo: 260 }) },
      SECOND,
    ],
    projectName: 'taurhaus',
    selectedAccountId: null,
    expected: { chooser: true, chip: true },
  },
]
