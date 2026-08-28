import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  accountSubmenuApplies,
  accountUsageMeta,
  buildAccountMenuChildren,
  launchDelegatesToTeam,
  TEAM_ACCOUNT_NOTE,
  toolSelectsAccounts,
} from './accountMenu.js'
import { configureToolRegistry, resetToolRegistry } from './toolRegistry.js'

const NOW = Date.parse('2026-08-27T10:00:00Z')

function usage({ fiveHour = null, sevenDay = null, fiveHourResetsAt, sevenDayResetsAt } = {}) {
  return {
    five_hour:
      fiveHour == null
        ? null
        : {
            used_percentage: fiveHour,
            resets_at: fiveHourResetsAt ?? Math.floor(NOW / 1000) + 3600,
          },
    seven_day:
      sevenDay == null
        ? null
        : {
            used_percentage: sevenDay,
            resets_at: sevenDayResetsAt ?? Math.floor(NOW / 1000) + 90_000,
          },
    observed_at: new Date(NOW - 60_000).toISOString(),
  }
}

const PRIMARY = { id: 'account-1', email: 'a@example.com', display_name: 'Who', logged_in: true }
const SECOND = { id: 'account-2', email: 'b@example.com', display_name: '', logged_in: true }
const LOGGED_OUT = { id: 'account-3', email: 'c@example.com', display_name: 'Work', logged_in: false }

describe('accountMenu', () => {
  afterEach(() => {
    resetToolRegistry()
  })

  it('takes account support from the registry capability, not the tool name', () => {
    expect(toolSelectsAccounts('claude')).toBe(true)
    expect(toolSelectsAccounts('codex')).toBe(true)

    // The next tool to gain accounts needs no change here.
    configureToolRegistry([
      { id: 'codex', label: 'Codex', capabilities: { account_selection: true } },
      { id: 'claude', label: 'Claude', capabilities: { account_selection: false } },
    ])

    expect(toolSelectsAccounts('codex')).toBe(true)
    expect(toolSelectsAccounts('claude')).toBe(false)
  })

  it('offers a submenu only when the host has a real choice', () => {
    expect(accountSubmenuApplies('claude', [PRIMARY])).toBe(false)
    expect(accountSubmenuApplies('claude', [PRIMARY, LOGGED_OUT])).toBe(false)
    expect(accountSubmenuApplies('claude', [PRIMARY, SECOND])).toBe(true)
    expect(accountSubmenuApplies('codex', [PRIMARY, SECOND])).toBe(true)
  })

  it('renders compact usage and drops a window whose reset has passed', () => {
    expect(accountUsageMeta({ usage: usage({ fiveHour: 3.4, sevenDay: 27.2 }) }, NOW)).toBe(
      '5h 3% · 7d 27%'
    )
    expect(
      accountUsageMeta(
        { usage: usage({ fiveHour: 91, sevenDay: 62, fiveHourResetsAt: Math.floor(NOW / 1000) - 60 }) },
        NOW
      )
    ).toBe('7d 62%')
    expect(accountUsageMeta({ usage: null }, NOW)).toBe('')
  })

  it('renders only provider-designated compact windows', () => {
    // Regression: 5680a7a treated every non-session Codex bucket as compact,
    // crowding both 5h and weekly limits into account menu rows.
    const resets_at = Math.floor(NOW / 1000) + 90_000
    const account = {
      usage: {
        windows: [
          { key: 'codex.5h', title: '5h limit', used_percentage: 20, resets_at, compact: false },
          { key: 'codex.weekly', title: 'Weekly limit', used_percentage: 50, resets_at, compact: true },
          {
            key: 'codex_bengalfox.weekly',
            title: 'GPT-5.3-Codex-Spark · Weekly limit',
            used_percentage: 3,
            resets_at,
            compact: true,
          },
        ],
      },
    }

    expect(accountUsageMeta(account, NOW)).toBe(
      'Weekly limit 50% · GPT-5.3-Codex-Spark Weekly limit 3%'
    )
  })

  it('builds one child per account, checked, metered, and disabled where it must be', () => {
    const onSelect = vi.fn()
    const children = buildAccountMenuChildren({
      accounts: [
        { ...PRIMARY, usage: usage({ fiveHour: 3, sevenDay: 27 }) },
        SECOND,
        LOGGED_OUT,
      ],
      activeAccountId: 'account-2',
      onSelect,
    })

    expect(children.map((child) => child.label)).toEqual(['Who', 'b@example.com', 'Work'])
    expect(children[0].check).toBe(false)
    expect(children[1].check).toBe(true)
    expect(children[2].disabled).toBe(true)
    expect(children[2].meta).toBe('not logged in')

    children[1].action()
    expect(onSelect).toHaveBeenCalledWith('account-2')
  })

  // Regression: 74c7761 labelled a row with the display name alone and dropped
  // the account id, so two subscriptions of the same named user produced two
  // identical rows — indistinguishable to the user, and duplicate keys to the
  // menu that renders them.
  it('keeps rows apart when two accounts share a display name', () => {
    const children = buildAccountMenuChildren({
      accounts: [
        { ...PRIMARY, display_name: 'Matthias' },
        { ...SECOND, display_name: 'Matthias' },
      ],
    })

    expect(children.map((child) => child.label)).toEqual([
      'Matthias (a@example.com)',
      'Matthias (b@example.com)',
    ])
    expect(new Set(children.map((child) => child.key)).size).toBe(2)
  })

  it('qualifies repeated provider labels with their config dirs', () => {
    // Regression: 5680a7a labelled every non-ChatGPT Codex home "API key",
    // and the collision qualifier repeated that same text for both choices.
    const children = buildAccountMenuChildren({
      accounts: [
        { id: 'key-1', label: 'API key', dir: '/home/user/.codex', logged_in: true },
        { id: 'key-2', label: 'API key', dir: '/home/user/.codex-work', logged_in: true },
      ],
    })

    expect(children.map((child) => child.label)).toEqual([
      'API key (.codex)',
      'API key (.codex-work)',
    ])
  })

  // Regression: 6ec843e labelled a repeated account by its config dir, which
  // let two rows address the same account id — the only address a launch has.
  // The store now hands over one account per id (`claudeAccounts.test.js`), and
  // a row keyed by its position stays unique whatever a caller passes.
  it('keys rows by position, so a repeated account cannot collide', () => {
    const twice = { ...PRIMARY, display_name: 'Who' }
    const children = buildAccountMenuChildren({ accounts: [twice, { ...twice }] })

    expect(new Set(children.map((child) => child.key)).size).toBe(2)
  })

  // Regression: 74c7761 offered an account on every Claude launch row, but a
  // Continue/Resume for a project that is exactly one team member's is handed
  // to the team runtime, which runs on the team's config dir. The row named an
  // account the launch could not use.
  it('knows a continue or resume the team runtime would take over', () => {
    const member = { group_kind: 'mesh_team', group_id: 'team-a', cli_tool: 'claude' }
    const standalone = { cli_tool: 'claude' }

    expect(launchDelegatesToTeam('continue', 'claude', [member])).toBe(true)
    expect(launchDelegatesToTeam('resume', 'claude', [member])).toBe(true)
    // A fresh session is never delegated: it starts its own history.
    expect(launchDelegatesToTeam('fresh', 'claude', [member])).toBe(false)
    // Another tool's member says nothing about this tool's resume.
    expect(launchDelegatesToTeam('resume', 'codex', [member])).toBe(false)
    expect(launchDelegatesToTeam('resume', 'claude', [standalone])).toBe(false)
    expect(launchDelegatesToTeam('resume', 'claude', [])).toBe(false)
    // Two members of the same tool are ambiguous, and the backend falls back to
    // a raw launch that does honour the account.
    expect(launchDelegatesToTeam('resume', 'claude', [member, { ...member }])).toBe(false)
  })

  it('disables every row with one note when the team decides the account', () => {
    const onSelect = vi.fn()
    const children = buildAccountMenuChildren({
      accounts: [PRIMARY, SECOND, LOGGED_OUT],
      activeAccountId: 'account-1',
      onSelect,
      disabledNote: TEAM_ACCOUNT_NOTE,
    })

    expect(children.every((child) => child.disabled)).toBe(true)
    expect(children.every((child) => child.check === false)).toBe(true)
    expect(children[0].meta).toBe('team runs on default account')
    expect(children[1].meta).toBe('team runs on default account')
    // A logged-out account still says the more specific thing about itself.
    expect(children[2].meta).toBe('not logged in')
  })
})
