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
    name: 'logged-out-account-dark',
    theme: 'dark',
    accounts: [PRIMARY, SECOND, LOGGED_OUT],
    projectName: 'mesh',
    selectedAccountId: null,
    expected: { chooser: true, chip: true },
  },
]
