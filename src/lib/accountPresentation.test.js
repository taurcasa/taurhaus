import { describe, expect, it } from 'vitest'

import {
  accountOriginSentence,
  ambientAccountSignal,
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
    ).toMatchObject({ visible: true, tone: 'warning', magnitude: '87%', account: warning })
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
    ).toMatchObject({ visible: true, tone: 'warning', magnitude: '92%' })
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
    ).toMatchObject({ visible: true, tone: 'warning', magnitude: '96%' })
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

    expect(result).toMatchObject({ visible: true, tone: 'danger', magnitude: 'Sign in' })
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
    ).toMatchObject({ visible: true, tone: 'warning', magnitude: '86%' })
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
