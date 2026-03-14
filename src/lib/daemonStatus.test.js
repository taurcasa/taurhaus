import { describe, expect, it } from 'vitest'

import {
  applyShellDaemonStatusSnapshot,
  canCheckDaemonUpdate,
  consumeInitialShellDaemonStatus,
  isShellDaemonRecoveryPending,
  normalizeShellDaemonStatus,
} from './daemonStatus.js'

describe('daemonStatus', () => {
  it('normalizes healthy daemon states away from the banner', () => {
    expect(normalizeShellDaemonStatus('connected')).toBeNull()
    expect(normalizeShellDaemonStatus('not_configured')).toBeNull()
    expect(normalizeShellDaemonStatus('disconnected')).toBe('disconnected')
  })

  it('treats initial busy splash state as unconfirmed and rechecks it', () => {
    // Regression: a splash-time busy snapshot used to be shown in Shell without
    // a follow-up probe, leaving a stale busy warning after startup settled.
    const initial = consumeInitialShellDaemonStatus('busy')

    expect(initial).toEqual({
      daemonStatus: null,
      needsRefresh: true,
      confirmBusyOnRefresh: false,
    })

    const refreshed = applyShellDaemonStatusSnapshot(initial.daemonStatus, 'connected', {
      confirmBusy: initial.confirmBusyOnRefresh,
    })

    expect(refreshed.daemonStatus).toBeNull()
    expect(refreshed.needsRefresh).toBe(false)
  })

  it('requires a second busy observation before surfacing the busy banner', () => {
    // Regression: a single lock-contention snapshot from getDaemonStatus()
    // used to stick as a user-facing warning until some unrelated later event.
    const firstBusy = applyShellDaemonStatusSnapshot(null, 'busy', { confirmBusy: true })

    expect(firstBusy).toEqual({
      daemonStatus: null,
      needsRefresh: true,
      confirmBusyOnRefresh: false,
    })

    const confirmedBusy = applyShellDaemonStatusSnapshot(firstBusy.daemonStatus, 'busy', {
      confirmBusy: firstBusy.confirmBusyOnRefresh,
    })

    expect(confirmedBusy.daemonStatus).toBe('busy')
    expect(confirmedBusy.needsRefresh).toBe(true)
  })

  it('clears a confirmed busy banner once a healthy snapshot returns', () => {
    const next = applyShellDaemonStatusSnapshot('busy', 'connected', { confirmBusy: false })

    expect(next).toEqual({
      daemonStatus: null,
      needsRefresh: false,
      confirmBusyOnRefresh: true,
    })
  })

  it('treats startup daemon state as recovering until the first status snapshot arrives', () => {
    // Regression: Shell auto-selected the first project before daemon status
    // hydration finished, so retryable daemon load failures were surfaced as
    // final startup warnings instead of being retried after reconnect.
    expect(isShellDaemonRecoveryPending(null, { initialized: false })).toBe(true)
    expect(canCheckDaemonUpdate(null, { initialized: false })).toBe(false)
  })

  it('allows update checks once daemon status has settled healthy', () => {
    expect(isShellDaemonRecoveryPending(null, { initialized: true })).toBe(false)
    expect(canCheckDaemonUpdate(null, { initialized: true })).toBe(true)
  })
})
