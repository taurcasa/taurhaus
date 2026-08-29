/**
 * One line the backend attaches to a `daemon-status` event when it is doing
 * something the status word alone cannot explain — today, replacing a daemon
 * whose protocol does not pair with this app. Returns null when the event
 * carries nothing worth putting on screen.
 */
export function daemonStatusNotice(payload) {
  const notice = payload?.notice
  if (typeof notice !== 'string') {
    return null
  }

  const trimmed = notice.trim()
  return trimmed.length > 0 ? trimmed : null
}

export function normalizeShellDaemonStatus(status) {
  if (status === 'connected' || status === 'not_configured' || status == null) {
    return null
  }

  return status
}

export function isShellDaemonRecoveryPending(status, { initialized = true } = {}) {
  if (!initialized) {
    return true
  }

  return status === 'busy' || status === 'disconnected' || status === 'reconnecting'
}

export function canCheckDaemonUpdate(status, { initialized = true } = {}) {
  return !isShellDaemonRecoveryPending(status, { initialized })
}

export function consumeInitialShellDaemonStatus(initialStatus) {
  if (initialStatus === 'busy') {
    return {
      daemonStatus: null,
      needsRefresh: true,
      confirmBusyOnRefresh: false,
    }
  }

  const daemonStatus = normalizeShellDaemonStatus(initialStatus)
  return {
    daemonStatus,
    needsRefresh: daemonStatus !== null,
    confirmBusyOnRefresh: true,
  }
}

export function applyShellDaemonStatusSnapshot(currentStatus, nextStatus, { confirmBusy = true } = {}) {
  if (nextStatus === 'busy') {
    if (confirmBusy && currentStatus !== 'busy') {
      return {
        daemonStatus: currentStatus ?? null,
        needsRefresh: true,
        confirmBusyOnRefresh: false,
      }
    }

    return {
      daemonStatus: 'busy',
      needsRefresh: true,
      confirmBusyOnRefresh: false,
    }
  }

  return {
    daemonStatus: normalizeShellDaemonStatus(nextStatus),
    needsRefresh: false,
    confirmBusyOnRefresh: true,
  }
}
