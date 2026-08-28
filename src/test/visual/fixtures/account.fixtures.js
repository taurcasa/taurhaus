const PRIMARY = {
  id: 'account-1',
  dir: '/home/user/.claude',
  label: 'stierms@gmail.com',
  display_name: 'Who',
  organization: "stierms@gmail.com's Organization",
  plan: 'claude_max',
  logged_in: true,
  is_default: true,
}

const SECOND = {
  id: 'account-2',
  dir: '/home/user/.claude-account2',
  label: 'm.stier@giesi.com',
  display_name: 'Matthias',
  organization: "m.stier@giesi.com's Organization",
  plan: 'claude_max',
  logged_in: true,
  is_default: false,
}

const LOGGED_OUT = {
  id: 'account-3',
  dir: '/home/user/.claude-work',
  label: 'work@example.com',
  display_name: 'Work',
  organization: 'Acme Inc',
  plan: 'team',
  logged_in: false,
  is_default: false,
}

/**
 * Usage as the provider reports it. Times are relative to render so the
 * fixtures keep meaning "just now" and "hours ago" whenever they are shot.
 */
function usage({ fiveHour, sevenDay, minutesAgo, status = 'ok' }) {
  const now = Date.now()
  return {
    status,
    windows: [
      { key: 'session', title: 'Current session', used_percentage: fiveHour, resets_at: Math.floor(now / 1000) + 2 * 3600 + 600, severity: fiveHour > 89 ? 'critical' : 'normal', is_active: true },
      { key: 'weekly_all', title: 'Current week (all models)', used_percentage: sevenDay, resets_at: Math.floor(now / 1000) + 41 * 3600, severity: 'normal', is_active: true },
      { key: 'weekly_scoped', title: 'Current week (Fable)', used_percentage: Math.min(100, sevenDay + 9), resets_at: Math.floor(now / 1000) + 41 * 3600, severity: sevenDay > 69 ? 'warning' : 'normal', is_active: true },
    ],
    observed_at: new Date(now - minutesAgo * 60_000).toISOString(),
  }
}

/** The same usage, with its five-hour window already past its reset. */
function resetFiveHour(reported) {
  return {
    ...reported,
    windows: reported.windows.map((window) =>
      window.key === 'session'
        ? { ...window, resets_at: Math.floor(Date.now() / 1000) - 600 }
        : window
    ),
  }
}

/**
 * Grok selects a whole account through `GROK_HOME` and publishes no quota
 * endpoint, so its rows carry an identity and never a usage meter — the
 * registry's `usage_note` stands where a meter would be.
 */
const GROK_PRIMARY = {
  id: 'grok-user-1',
  dir: '/home/user/.grok',
  label: 'm.stier@giesi.com',
  display_name: 'Matthias Stier',
  plan: 'supergrok',
  logged_in: true,
  is_default: true,
}

const GROK_SECOND = {
  id: 'grok-user-2',
  dir: '/home/user/.grok-work',
  label: 'work@example.com',
  display_name: 'Work',
  plan: 'supergrok',
  logged_in: true,
  is_default: false,
}

export const accountScenarios = [
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
      { ...PRIMARY, usage: usage({ fiveHour: 54, sevenDay: 33, minutesAgo: 260, status: 'stale' }) },
      SECOND,
    ],
    projectName: 'taurhaus',
    selectedAccountId: null,
    expected: { chooser: true, chip: true },
  },
  {
    // A five-hour window that reset while the app stayed open. The percentage
    // beside it describes a window that no longer exists, and this account is
    // the one that just got its headroom back — so the row goes, and only the
    // seven-day number is still spoken for.
    name: 'usage-window-reset-light',
    theme: 'light',
    accounts: [
      { ...PRIMARY, usage: resetFiveHour(usage({ fiveHour: 91, sevenDay: 62, minutesAgo: 40 })) },
      { ...SECOND, usage: usage({ fiveHour: 12, sevenDay: 8, minutesAgo: 4 }) },
    ],
    projectName: 'taurhaus',
    selectedAccountId: null,
    expected: { chooser: true, chip: true },
  },
  {
    // Grok on two GROK_HOMEs: the chooser and chip work exactly as they do for
    // Claude, but no account carries a usage window.
    name: 'grok-two-accounts-light',
    theme: 'light',
    tool: 'grok',
    accounts: [GROK_PRIMARY, GROK_SECOND],
    projectName: 'taurhaus',
    selectedAccountId: null,
    expected: { chooser: true, chip: true },
  },
  {
    name: 'grok-two-accounts-dark',
    theme: 'dark',
    tool: 'grok',
    accounts: [GROK_PRIMARY, GROK_SECOND],
    projectName: 'taurhaus',
    selectedAccountId: 'grok-user-2',
    expected: { chooser: true, chip: true },
  },
]
