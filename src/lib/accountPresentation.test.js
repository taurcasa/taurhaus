import { describe, expect, it } from 'vitest'

import {
  accountOriginSentence,
  ambientAccountSignal,
  baseCommandSelection,
  usageIsLastKnown,
} from './accountPresentation.js'

const account = (overrides = {}) => ({
  id: 'personal',
  label: 'personal@example.com',
  logged_in: true,
  usage: {
    status: 'ok',
    observed_at: '2026-09-02T10:00:00.000Z',
    windows: [
      {
        key: 'weekly',
        title: 'Week',
        used_percentage: 42,
        severity: 'normal',
        is_active: true,
      },
    ],
  },
  ...overrides,
})

describe('ambientAccountSignal', () => {
  it('stays absent while every relevant account has comfortable headroom', () => {
    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: 'personal',
          accounts: [account()],
          relationships: {},
        },
      ])
    ).toEqual({ visible: false, tone: 'calm', magnitude: null, account: null })
  })

  it('shows the worst relevant window magnitude in amber before exhaustion', () => {
    const warning = account({
      usage: {
        status: 'ok',
        windows: [
          { key: 'session', title: 'Session', used_percentage: 87, severity: 'warning' },
          { key: 'weekly', title: 'Week', used_percentage: 71, severity: 'normal' },
        ],
      },
    })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: null,
          accounts: [warning],
          relationships: { personal: { pinnedProjects: [{ id: 'p1' }] } },
        },
      ])
    ).toMatchObject({ visible: true, tone: 'warning', magnitude: '87', account: warning })
  })

  // Regression: 462c18f treated the provider's `is_active` binding-limit flag
  // as window liveness, hiding ordinary Claude and Codex usage before exhaustion.
  it('uses the worst Claude window even when only another limit is binding', () => {
    const claude = account({
      usage: {
        status: 'ok',
        windows: [
          { key: 'session', title: 'Current session', used_percentage: 3, is_active: false },
          { key: 'weekly_all', title: 'Current week', used_percentage: 92, is_active: false },
          { key: 'weekly_scoped', title: 'Current week (Opus)', used_percentage: 29, is_active: true },
        ],
      },
    })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: 'personal',
          accounts: [claude],
          relationships: {},
        },
      ])
    ).toMatchObject({ visible: true, tone: 'warning', magnitude: '92' })
  })

  it('uses Codex windows whose binding-limit flags are all false', () => {
    const codex = account({
      usage: {
        status: 'ok',
        windows: [
          { key: 'five_hour', title: '5h limit', used_percentage: 96, is_active: false },
          { key: 'weekly', title: 'Weekly limit', used_percentage: 88, is_active: false },
        ],
      },
    })

    expect(
      ambientAccountSignal([
        {
          tool: 'codex',
          defaultAccountId: 'personal',
          accounts: [codex],
          relationships: {},
        },
      ])
    ).toMatchObject({ visible: true, tone: 'warning', magnitude: '96' })
  })

  it('uses red for a relevant signed-out or unauthorized account', () => {
    const signedOut = account({ id: 'work', logged_in: false, usage: null })
    const result = ambientAccountSignal([
      {
        tool: 'claude',
        defaultAccountId: 'work',
        accounts: [signedOut],
        relationships: {},
      },
    ])

    // The badge grammar is number-only: a filled danger pill counts the
    // accounts needing action; the tone carries the severity, never a word.
    expect(result).toMatchObject({ visible: true, tone: 'danger', magnitude: '1' })
  })

  it('counts every relevant account needing sign-in as the danger magnitude', () => {
    const signedOutDefault = account({ id: 'work', logged_in: false, usage: null })
    const signedOutPinned = account({ id: 'legacy', logged_in: false, usage: null })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: 'work',
          accounts: [signedOutDefault, signedOutPinned],
          relationships: { legacy: { pinnedProjects: [{ id: 'p1' }] } },
        },
      ])
    ).toMatchObject({ visible: true, tone: 'danger', magnitude: '2' })
  })

  it('ignores an unhealthy account nothing currently depends on', () => {
    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: 'personal',
          accounts: [account(), account({ id: 'old', logged_in: false, usage: null })],
          relationships: {},
        },
      ])
    ).toMatchObject({ visible: false, tone: 'calm' })
  })

  // Regression: 462c18f ignored the detected default-directory account, so a
  // fresh single-account install never surfaced that account's usage pressure.
  it('treats the tool default-directory account as relevant without a saved default', () => {
    const defaultDirectory = account({
      is_default: true,
      usage: {
        status: 'ok',
        windows: [{ key: 'weekly', title: 'Week', used_percentage: 86 }],
      },
    })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: null,
          accounts: [defaultDirectory],
          relationships: {},
        },
      ])
    ).toMatchObject({ visible: true, tone: 'warning', magnitude: '86' })
  })

  // Regression: 462c18f made every default-directory account relevant on its
  // flag alone, so a signed-out process default that a signed-in saved global
  // default has superseded raised a permanent red the launches never touch.
  it('ignores a default-directory account a signed-in global default supersedes', () => {
    const processDefault = account({
      id: 'legacy',
      logged_in: false,
      usage: null,
      is_process_default: true,
      is_default: true,
    })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: 'personal',
          accounts: [processDefault, account()],
          relationships: {},
        },
      ])
    ).toMatchObject({ visible: false, tone: 'calm' })
  })

  it('keeps a superseded default directory relevant while something is pinned to it', () => {
    const processDefault = account({
      id: 'legacy',
      logged_in: false,
      usage: null,
      is_process_default: true,
    })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: 'personal',
          accounts: [processDefault, account()],
          relationships: { legacy: { pinnedProjects: [{ id: 'p1' }] } },
        },
      ])
    ).toMatchObject({ visible: true, tone: 'danger', magnitude: '1' })
  })

  it('keeps the default directory relevant while the saved default is itself signed out', () => {
    const processDefault = account({ id: 'legacy', is_default: true, is_process_default: true })
    const savedDefault = account({ id: 'personal', logged_in: false, usage: null })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: 'personal',
          accounts: [processDefault, savedDefault],
          relationships: {},
        },
      ])
    ).toMatchObject({ visible: true, tone: 'danger', magnitude: '1' })
  })

  // Regression: 6556676e read supersession off the saved global default alone,
  // while the resolver reaches the default directory only after a usable
  // base-command selector has failed too. A signed-out directory under a
  // `CLAUDE_CONFIG_DIR` alias that launches every session on a signed-in
  // account therefore lit a permanent red the launches never touch.
  it('ignores a default directory the launch command selector supersedes', () => {
    const signedOutDirectory = account({
      id: 'legacy',
      logged_in: false,
      usage: null,
      dir: '/home/user/.claude',
      is_default: true,
      is_process_default: true,
    })
    const selected = account({ id: 'work', dir: '/home/user/.claude-work' })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: null,
          accounts: [signedOutDirectory, selected],
          relationships: {},
          resolvedBases: [
            {
              command: 'CLAUDE_CONFIG_DIR=~/.claude-work claude',
              selectorValue: '~/.claude-work',
              expansions: [{ name: 'claude2', body: 'CLAUDE_CONFIG_DIR=~/.claude-work claude' }],
            },
          ],
        },
      ])
    ).toMatchObject({ visible: false, tone: 'calm' })
  })

  it('keeps the default directory relevant when the selector names a signed-out account', () => {
    const signedOutDirectory = account({
      id: 'legacy',
      logged_in: false,
      usage: null,
      dir: '/home/user/.claude',
      is_default: true,
    })
    const selected = account({
      id: 'work',
      logged_in: false,
      usage: null,
      dir: '/home/user/.claude-work',
    })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: null,
          accounts: [signedOutDirectory, selected],
          relationships: {},
          resolvedBases: [
            { command: 'CLAUDE_CONFIG_DIR=~/.claude-work claude', selectorValue: '~/.claude-work' },
          ],
        },
      ])
    ).toMatchObject({ visible: true, tone: 'danger', magnitude: '1' })
  })

  // Regression: 6556676e derived pin relevance from the relationship index
  // alone, which `rememberChoice` never writes. A pin made from Overview or
  // the sidebar left the footer calm about an account that had just become
  // something a project depends on.
  it('counts a pin made during this run before the relationship index catches up', () => {
    const signedOut = account({ id: 'work', logged_in: false, usage: null })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: 'personal',
          accounts: [account(), signedOut],
          relationships: {},
          projectChoices: { p1: 'work' },
        },
      ])
    ).toMatchObject({ visible: true, tone: 'danger', magnitude: '1' })
  })

  it('drops a pin cleared during this run while the index still carries it', () => {
    const signedOut = account({ id: 'work', logged_in: false, usage: null })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: 'personal',
          accounts: [account(), signedOut],
          relationships: { work: { pinnedProjects: [{ id: 'p1' }] } },
          projectChoices: { p1: null },
        },
      ])
    ).toMatchObject({ visible: false, tone: 'calm' })
  })

  it('ignores a default directory a selector nothing detected keeps launches away from', () => {
    const signedOutDirectory = account({
      id: 'legacy',
      logged_in: false,
      usage: null,
      dir: '/home/user/.claude',
      is_default: true,
    })

    expect(
      ambientAccountSignal([
        {
          tool: 'claude',
          defaultAccountId: null,
          accounts: [signedOutDirectory],
          relationships: {},
          resolvedBases: [
            { command: 'CLAUDE_CONFIG_DIR=~/.claude-gone claude', selectorValue: '~/.claude-gone' },
          ],
        },
      ])
    ).toMatchObject({ visible: false, tone: 'calm' })
  })
})

describe('usageIsLastKnown', () => {
  it('marks an observation older than fifteen minutes as last known', () => {
    expect(
      usageIsLastKnown(
        account().usage,
        Date.parse('2026-09-02T10:16:00.000Z')
      )
    ).toBe(true)
  })

  it('keeps a fresh observation solid', () => {
    expect(
      usageIsLastKnown(
        account().usage,
        Date.parse('2026-09-02T10:14:59.000Z')
      )
    ).toBe(false)
  })
})

describe('accountOriginSentence', () => {
  // Regression: 1043f47 invented frontend-only origin aliases and described
  // default_config_dir as the only signed-in account, which the backend never promises.
  it.each([
    ['request', 'chosen for this launch'],
    ['session', "resumes this session's account"],
    ['project', 'pinned to this project'],
    ['last_used', 'last used here'],
    ['global_default', 'your global default'],
    ['base_command', 'carried by your launch command'],
    ['signed_in', 'a signed-in account'],
    ['default_config_dir', "the tool's default directory"],
  ])('renders %s provenance as settled product copy', (origin, sentence) => {
    expect(accountOriginSentence(origin)).toBe(sentence)
  })
})

describe('baseCommandSelection usability', () => {
  // Regression: round-10 review — a signed-out selector account was presented
  // as effective and offered for pin conversion, though the backend resolver
  // rejects it and falls through to a usable choice.
  it('marks a signed-out selector account unusable', () => {
    const selection = baseCommandSelection(
      [{ command: 'CLAUDE_CONFIG_DIR=/home/u/.claude-old claude', selectorValue: '/home/u/.claude-old', expansions: [] }],
      [{ id: 'old', dir: '/home/u/.claude-old', logged_in: false }]
    )
    expect(selection.account?.id).toBe('old')
    expect(selection.usable).toBe(false)
  })

  it('marks a signed-in selector account usable', () => {
    const selection = baseCommandSelection(
      [{ command: 'CLAUDE_CONFIG_DIR=/home/u/.claude-work claude', selectorValue: '/home/u/.claude-work', expansions: [] }],
      [{ id: 'work', dir: '/home/u/.claude-work', logged_in: true }]
    )
    expect(selection.usable).toBe(true)
  })
})

describe('baseCommandSelection', () => {
  const accounts = [
    { id: 'work', display_name: 'work', dir: '/home/user/.claude-work' },
    { id: 'personal', display_name: 'personal', dir: '/home/user/.claude' },
  ]

  // Regression: 186f19a2 keyed the accounts-home explainer on the alias
  // expansion, so a selector typed straight into Settings selected an account
  // that no surface explained.
  it('matches a selector whether an alias or the typed command carries it', () => {
    const typed = baseCommandSelection(
      [
        {
          command: 'CLAUDE_CONFIG_DIR=/home/user/.claude-work claude',
          selectorValue: '/home/user/.claude-work',
          expansions: [],
        },
      ],
      accounts
    )
    expect(typed.account?.id).toBe('work')
    expect(typed.alias).toBeNull()
    expect(typed.command).toBe('CLAUDE_CONFIG_DIR=/home/user/.claude-work claude')

    const aliased = baseCommandSelection(
      [
        {
          command: 'CLAUDE_CONFIG_DIR=/home/user/.claude-work claude',
          selectorValue: '/home/user/.claude-work',
          expansions: [{ name: 'claude2', body: 'CLAUDE_CONFIG_DIR=…' }],
        },
      ],
      accounts
    )
    expect(aliased.alias?.name).toBe('claude2')
  })

  it('reports an opaque head from any base, matched or not', () => {
    const selection = baseCommandSelection(
      [
        { command: 'claude-wrapper', expansions: [], opaqueHead: 'claude-wrapper' },
        {
          command: 'CLAUDE_CONFIG_DIR=/home/user/.claude-work claude',
          selectorValue: '/home/user/.claude-work',
          expansions: [],
        },
      ],
      accounts
    )
    expect(selection.opaqueHead).toBe('claude-wrapper')
    expect(selection.account?.id).toBe('work')
  })

  it('answers with nothing selected when no base carries a known selector', () => {
    expect(baseCommandSelection(undefined, accounts)).toEqual({
      opaqueHead: null,
      account: null,
      usable: false,
      alias: null,
      command: null,
    })
  })
})
