import { describe, expect, it } from 'vitest'

import {
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
