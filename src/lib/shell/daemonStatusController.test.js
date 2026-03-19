import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createShellDaemonStatusController } from './daemonStatus.svelte.js'

function createState() {
  return {
    daemonStatus: null,
    daemonStatusInitialized: false,
    daemonUpdateAvailable: { version: '0.1.0', bundled_version: '0.2.0' },
    daemonUpdateDismissed: true,
    daemonUpdating: false,
    daemonRestarting: false,
    daemonRecoveryStartedAt: null,
    daemonRecoveryEscalated: false,
  }
}

describe('createShellDaemonStatusController', () => {
  beforeEach(() => {
    vi.useRealTimers()
  })

  it('dismisses a connected daemon banner after the grace period', async () => {
    vi.useFakeTimers()
    const state = createState()

    const controller = createShellDaemonStatusController({
      getInitialDaemonStatus: () => undefined,
      state,
      ipc: {
        getDaemonStatus: vi.fn(),
        checkDaemonInstallStatus: vi.fn().mockResolvedValue({ installed: true, needs_update: false }),
        installDaemon: vi.fn(),
        startDaemon: vi.fn(),
      },
      onNotice: vi.fn(),
      logger: { warn: vi.fn(), error: vi.fn() },
    })

    controller.handleDaemonStatusEvent('connected')
    expect(state.daemonStatus).toBe('connected')

    await vi.advanceTimersByTimeAsync(3000)
    expect(state.daemonStatus).toBeNull()
  })

  it('installs daemon updates and resets banner state', async () => {
    const state = createState()
    const installDaemon = vi.fn().mockResolvedValue(undefined)

    const controller = createShellDaemonStatusController({
      getInitialDaemonStatus: () => undefined,
      state,
      ipc: {
        getDaemonStatus: vi.fn(),
        checkDaemonInstallStatus: vi.fn(),
        installDaemon,
        startDaemon: vi.fn(),
      },
      onNotice: vi.fn(),
      logger: { warn: vi.fn(), error: vi.fn() },
    })

    await controller.handleDaemonUpdate()

    expect(installDaemon).toHaveBeenCalledTimes(1)
    expect(state.daemonUpdating).toBe(false)
    expect(state.daemonUpdateAvailable).toBeNull()
    expect(state.daemonUpdateDismissed).toBe(false)
  })
})
