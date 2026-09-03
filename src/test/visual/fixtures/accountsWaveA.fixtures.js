const now = Date.now()

function usage(used, minutesAgo = 2, status = 'ok') {
  return {
    status,
    observed_at: new Date(now - minutesAgo * 60_000).toISOString(),
    windows: [
      {
        key: 'session',
        title: 'Current session',
        used_percentage: used,
        resets_at: Math.floor(now / 1000) + 2 * 3600,
        severity: used >= 100 ? 'critical' : used >= 80 ? 'warning' : 'normal',
        is_active: true,
      },
      {
        key: 'weekly',
        title: 'Current week',
        used_percentage: Math.max(8, used - 24),
        resets_at: Math.floor(now / 1000) + 2 * 86_400,
        severity: 'normal',
        is_active: true,
      },
    ],
  }
}

const personal = {
  id: 'personal',
  label: 'personal@example.com',
  display_name: 'Personal',
  dir: '/home/user/.claude',
  logged_in: true,
  usage: usage(38),
}

const work = {
  id: 'work',
  label: 'work@example.com',
  display_name: 'Work',
  dir: '/home/user/.claude-work',
  logged_in: true,
  usage: usage(86),
}

function homeStates({ degraded = false, signedOut = false } = {}) {
  const workAccount = signedOut
    ? { ...work, logged_in: false, usage: null }
    : work
  return {
    claude: {
      accounts: [personal, workAccount],
      defaultAccountId: 'personal',
      degraded,
      relationships: {
        work: {
          pinnedProjects: [{ id: 'p1', name: 'taurhaus', path: '/projects/taurhaus' }],
          lastUsedProjects: [{ id: 'p2', name: 'mir', path: '/projects/mir' }],
          teams: [{ name: 'accounts-wave', projectId: 'p1', projectName: 'taurhaus' }],
        },
      },
      resolvedBases: [
        {
          selectorValue: '/home/user/.claude-work',
          expansions: [{ name: 'claude-work' }],
        },
      ],
    },
    codex: {
      accounts: [{
        id: 'codex',
        label: 'codex@example.com',
        display_name: 'Codex Personal',
        dir: '/home/user/.codex',
        logged_in: true,
        usage: usage(52, degraded ? 48 : 4),
      }],
      defaultAccountId: 'codex',
      degraded,
      relationships: {},
      resolvedBases: [],
    },
    agy: {
      accounts: [{
        id: 'agy',
        label: 'agy@example.com',
        display_name: 'Google',
        dir: '/home/user/.gemini',
        logged_in: true,
        usage: usage(21),
      }],
      defaultAccountId: null,
      degraded: false,
      relationships: {},
      resolvedBases: [],
    },
    grok: {
      accounts: [{
        id: 'grok',
        label: 'grok@example.com',
        display_name: 'Grok Personal',
        dir: '/home/user/.grok',
        logged_in: true,
        usage: null,
      }],
      defaultAccountId: null,
      degraded: false,
      relationships: {},
      resolvedBases: [],
    },
  }
}

/** A reading whose provider severity and its percentage disagree. */
function splitUsage(used, severity) {
  const base = usage(used)
  return {
    ...base,
    windows: [{ ...base.windows[0], used_percentage: used, severity }, base.windows[1]],
  }
}

/** Severity, not percentage, decides both rows: amber at 50%, rose at 95%. */
function severitySplitStates() {
  const states = homeStates()
  states.claude = {
    ...states.claude,
    accounts: [
      { ...personal, usage: splitUsage(50, 'warning') },
      { ...work, usage: splitUsage(95, 'critical') },
    ],
  }
  return states
}

/** A base command that spells the selector out, with no alias behind it. */
function baseCommandStates() {
  const states = homeStates()
  states.claude = {
    ...states.claude,
    defaultAccountId: null,
    resolvedBases: [
      {
        command: 'CLAUDE_CONFIG_DIR=/home/user/.claude-work claude',
        selectorValue: '/home/user/.claude-work',
        expansions: [],
      },
    ],
  }
  return states
}

function paired(surface, name, extra = {}) {
  return ['light', 'dark'].map((theme) => ({
    name: `${name}-${theme}`,
    theme,
    surface,
    ...extra,
  }))
}

const pickerAccounts = [personal, work]

export const accountsWaveAScenarios = [
  ...paired('home', 'home-healthy', {
    states: homeStates(),
    expectedTestId: 'accounts-home',
  }),
  ...paired('home', 'home-degraded', {
    states: homeStates({ degraded: true }),
    expectedTestId: 'accounts-degraded-banner',
  }),
  ...paired('home', 'home-signed-out', {
    states: homeStates({ signedOut: true }),
    expectedTestId: 'account-row-details',
  }),
  ...paired('home', 'home-severity-split', {
    states: severitySplitStates(),
    expectedTestId: 'account-row-details',
  }),
  ...paired('home', 'home-base-command', {
    states: baseCommandStates(),
    expectedTestId: 'account-alias-claude',
  }),
  ...paired('board', 'usage-board', {
    states: homeStates(),
    expectedTestId: 'context-menu',
  }),
  ...['modal', 'popover', 'select'].flatMap((skin) => paired('picker', `picker-${skin}`, {
    skin,
    accounts: pickerAccounts,
    expectedTestId: 'account-picker',
  })),
]
