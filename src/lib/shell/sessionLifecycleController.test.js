import { describe, expect, it, vi } from 'vitest'

import { createShellSessionLifecycleController } from './sessionLifecycle.svelte.js'

describe('createShellSessionLifecycleController', () => {
  it('marks the bridge live without applying a daemon payload', () => {
    const applyDaemonSessionUpdate = vi.fn()
    const state = {
      foregroundProjectId: null,
      sessionBridgeLive: false,
    }

    const controller = createShellSessionLifecycleController({
      state,
      getProjects: () => [],
      ipc: {
        getForegroundProject: vi.fn(),
        listClaudeSessions: vi.fn(),
        navigateToSession: vi.fn(),
      },
      sessionStore: {
        getSessions: () => new Map(),
        applyDaemonSessionUpdate,
        markSessionPresenceStale: vi.fn(),
      },
      logger: console,
    })

    controller.markSessionBridgeLive()

    expect(state.sessionBridgeLive).toBe(true)
    expect(applyDaemonSessionUpdate).not.toHaveBeenCalled()
  })

  it('focuses a mesh pane by resolving tmux coordinates and project ownership', async () => {
    const navigateToSession = vi.fn().mockResolvedValue(undefined)
    const state = {
      foregroundProjectId: null,
      sessionBridgeLive: false,
    }

    const controller = createShellSessionLifecycleController({
      state,
      getProjects: () => [{ id: 'proj-2', path: '/projects/mesh' }],
      ipc: {
        getForegroundProject: vi.fn(),
        listClaudeSessions: vi.fn().mockResolvedValue([
          {
            tmuxSession: 'taurhaus',
            tmuxWindow: '2',
            tmuxPane: '%2',
            projectPath: '/projects/mesh',
          },
        ]),
        navigateToSession,
      },
      sessionStore: {
        getSessions: () => new Map(),
        applyDaemonSessionUpdate: vi.fn(),
        markSessionPresenceStale: vi.fn(),
      },
      logger: { warn: vi.fn(), error: vi.fn(), debug: vi.fn() },
    })

    await controller.handleMeshFocusPane('%2')

    expect(state.foregroundProjectId).toBe('proj-2')
    expect(navigateToSession).toHaveBeenCalledWith('taurhaus', '2', '%2', true)
  })
})
