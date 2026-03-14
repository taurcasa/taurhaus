export function normalizeShellDaemonStatus(status) {
  if (status === 'connected' || status === 'not_configured' || status == null) {
    return null
  }

  return status
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
