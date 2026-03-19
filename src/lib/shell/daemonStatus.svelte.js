import { describeDaemonSetupError } from '../errorCopy.js'
import {
  applyShellDaemonStatusSnapshot,
  canCheckDaemonUpdate,
  consumeInitialShellDaemonStatus,
  isShellDaemonRecoveryPending,
} from '../daemonStatus.js'

function errorMessage(error) {
  if (error && typeof error === 'object' && typeof error.message === 'string' && error.message.trim()) {
    return error.message
  }
  if (typeof error === 'string' && error.trim()) {
    return error
  }
  return String(error)
}

export function createShellDaemonStatusController({
  getInitialDaemonStatus = () => undefined,
  state,
  ipc,
  onNotice,
  logger = console,
}) {
  let consumedInitialDaemonStatus = false
  let daemonStatusDismissTimer = null
  let daemonStatusRefreshTimer = null
  let daemonRecoveryEscalationTimer = null

  function clearDaemonStatusDismissTimer() {
    if (daemonStatusDismissTimer !== null) {
      clearTimeout(daemonStatusDismissTimer)
      daemonStatusDismissTimer = null
    }
  }

  function clearDaemonStatusRefreshTimer() {
    if (daemonStatusRefreshTimer !== null) {
      clearTimeout(daemonStatusRefreshTimer)
      daemonStatusRefreshTimer = null
    }
  }

  function clearDaemonRecoveryEscalationTimer() {
    if (daemonRecoveryEscalationTimer !== null) {
      clearTimeout(daemonRecoveryEscalationTimer)
      daemonRecoveryEscalationTimer = null
    }
  }

  function cleanup() {
    clearDaemonStatusDismissTimer()
    clearDaemonStatusRefreshTimer()
    clearDaemonRecoveryEscalationTimer()
  }

  function recoveryPending() {
    return isShellDaemonRecoveryPending(state.daemonStatus, {
      initialized: state.daemonStatusInitialized,
    })
  }

  function syncRecoveryEscalation() {
    const recovering = state.daemonStatus === 'busy'
      || state.daemonStatus === 'reconnecting'
      || state.daemonStatus === 'disconnected'

    if (!recovering) {
      state.daemonRecoveryStartedAt = null
      state.daemonRecoveryEscalated = false
      clearDaemonRecoveryEscalationTimer()
      return
    }

    const startedAt = state.daemonRecoveryStartedAt ?? Date.now()
    state.daemonRecoveryStartedAt = startedAt
    const elapsedMs = Date.now() - startedAt
    const shouldEscalate = elapsedMs >= 30_000
    state.daemonRecoveryEscalated = shouldEscalate

    clearDaemonRecoveryEscalationTimer()

    if (!shouldEscalate) {
      daemonRecoveryEscalationTimer = setTimeout(() => {
        state.daemonRecoveryEscalated = true
        daemonRecoveryEscalationTimer = null
      }, 30_000 - elapsedMs)
    }
  }

  function scheduleDaemonStatusRefresh({ delayMs, confirmBusy }) {
    clearDaemonStatusRefreshTimer()
    daemonStatusRefreshTimer = setTimeout(() => {
      daemonStatusRefreshTimer = null
      void loadDaemonStatusWithRefresh({
        allowInitial: false,
        confirmBusy,
        includeUpdateCheck: false,
      })
    }, delayMs)
  }

  async function loadDaemonStatus({ allowInitial = true } = {}) {
    return loadDaemonStatusWithRefresh({
      allowInitial,
      confirmBusy: true,
      includeUpdateCheck: true,
    })
  }

  async function loadDaemonStatusWithRefresh({ allowInitial = true, confirmBusy = true, includeUpdateCheck = true } = {}) {
    const initialDaemonStatus = getInitialDaemonStatus()
    if (allowInitial && !consumedInitialDaemonStatus && initialDaemonStatus !== undefined) {
      consumedInitialDaemonStatus = true
      const initial = consumeInitialShellDaemonStatus(initialDaemonStatus)
      state.daemonStatus = initial.daemonStatus
      state.daemonStatusInitialized = true
      if (initial.needsRefresh) {
        scheduleDaemonStatusRefresh({
          delayMs: initialDaemonStatus === 'busy' ? 450 : 1200,
          confirmBusy: initial.confirmBusyOnRefresh,
        })
      } else {
        clearDaemonStatusRefreshTimer()
      }

      if (includeUpdateCheck) {
        void checkDaemonUpdate()
      }
      return
    }

    try {
      const status = await ipc.getDaemonStatus()
      const next = applyShellDaemonStatusSnapshot(state.daemonStatus, status.status, { confirmBusy })
      state.daemonStatus = next.daemonStatus
      state.daemonStatusInitialized = true
      if (next.needsRefresh) {
        scheduleDaemonStatusRefresh({
          delayMs: next.daemonStatus === 'busy' ? 1500 : 750,
          confirmBusy: next.confirmBusyOnRefresh,
        })
      } else {
        clearDaemonStatusRefreshTimer()
      }
    } catch (error) {
      logger.warn('[daemon] status check failed; preserving current status', {
        error_message: errorMessage(error),
      })
    }

    if (includeUpdateCheck) {
      void checkDaemonUpdate()
    }
  }

  async function checkDaemonUpdate() {
    if (!canCheckDaemonUpdate(state.daemonStatus, { initialized: state.daemonStatusInitialized })) {
      state.daemonUpdateAvailable = null
      return
    }

    try {
      const status = await ipc.checkDaemonInstallStatus()
      const installed = Boolean(status?.installed)
      const needsUpdate = Boolean(status?.needsUpdate ?? status?.needs_update)
      const bundledVersion = String(status?.bundledVersion ?? status?.bundled_version ?? '').trim()
      const installedVersion = String(status?.version ?? '').trim()
      if (installed && needsUpdate && installedVersion && bundledVersion) {
        state.daemonUpdateAvailable = {
          version: installedVersion,
          bundled_version: bundledVersion,
        }
      } else {
        state.daemonUpdateAvailable = null
      }
    } catch (error) {
      logger.warn('[daemon] install-status check failed; skipping update banner', {
        error_message: errorMessage(error),
      })
    }
  }

  async function handleDaemonUpdate() {
    state.daemonUpdating = true
    try {
      await ipc.installDaemon()
      state.daemonUpdateAvailable = null
      state.daemonUpdateDismissed = false
    } catch (error) {
      logger.error('Daemon update failed:', error)
    } finally {
      state.daemonUpdating = false
    }
  }

  async function handleRestartDaemon() {
    if (state.daemonRestarting) return
    state.daemonRestarting = true
    try {
      await ipc.startDaemon()
      await loadDaemonStatusWithRefresh({
        allowInitial: false,
        confirmBusy: false,
        includeUpdateCheck: true,
      })
    } catch (error) {
      logger.error('[daemon] restart failed:', error)
      onNotice?.(describeDaemonSetupError(error, { action: 'restart' }))
    } finally {
      state.daemonRestarting = false
    }
  }

  function dismissDaemonUpdate() {
    state.daemonUpdateDismissed = true
  }

  function handleDaemonStatusEvent(status) {
    state.daemonStatus = status
    state.daemonStatusInitialized = true
    clearDaemonStatusRefreshTimer()
    clearDaemonStatusDismissTimer()

    if (status === 'connected') {
      void checkDaemonUpdate()
      daemonStatusDismissTimer = setTimeout(() => {
        state.daemonStatus = null
        daemonStatusDismissTimer = null
      }, 3000)
    }
  }

  return {
    cleanup,
    recoveryPending,
    syncRecoveryEscalation,
    loadDaemonStatus,
    loadDaemonStatusWithRefresh,
    checkDaemonUpdate,
    handleDaemonUpdate,
    handleRestartDaemon,
    dismissDaemonUpdate,
    handleDaemonStatusEvent,
  }
}
