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

  it('sets the foreground project straight from the focus event project_id', () => {
    // Regression: commits a53ad31 and f9c1e89. Focus used to arrive as raw tmux
    // coordinates written by a tmux hook, so the shell re-matched them against
    // the session store and fell back to a 75 ms getForegroundProject refresh.
    // The daemon hub now resolves the project, so the event carries project_id.
    const getForegroundProject = vi.fn()
    const state = {
      foregroundProjectId: null,
      sessionBridgeLive: false,
    }

    const controller = createShellSessionLifecycleController({
      state,
      getProjects: () => [{ id: 'proj-2', path: '/projects/mesh' }],
      ipc: {
        getForegroundProject,
        listClaudeSessions: vi.fn(),
        navigateToSession: vi.fn(),
      },
      sessionStore: {
        getSessions: () => new Map(),
        applyDaemonSessionUpdate: vi.fn(),
        markSessionPresenceStale: vi.fn(),
      },
      logger: { warn: vi.fn(), error: vi.fn(), debug: vi.fn() },
    })

    controller.handleTmuxFocusChanged({
      session: 'taurhaus',
      window: '2',
      pane_id: '%9',
      project_id: 'proj-2',
    })

    expect(state.foregroundProjectId).toBe('proj-2')
    expect(getForegroundProject).not.toHaveBeenCalled()

    controller.handleTmuxFocusChanged({
      session: 'taurhaus',
      window: '3',
      pane_id: '%11',
      project_id: null,
    })
    expect(state.foregroundProjectId).toBeNull()

    controller.handleTmuxFocusChanged(null)
    expect(state.foregroundProjectId).toBeNull()
    expect(getForegroundProject).not.toHaveBeenCalled()
  })

  it('does not let a late startup fallback overwrite a newer live focus event', async () => {
    // Regression: commit 07ab6c5 made `tmux-focus-changed` the only live focus
    // transport and left `loadForegroundProject` as the startup read. Both write
    // the same field, but the IPC read applied its awaited answer
    // unconditionally — and that answer is a snapshot of the moment the call was
    // made. A focus event landing while the promise was in flight was overwritten
    // by the older value, parking the foreground marker on the wrong project
    // until the next focus change.
    let resolveForeground = () => {}
    const getForegroundProject = vi.fn(
      () => new Promise((resolve) => {
        resolveForeground = resolve
      })
    )
    const state = {
      foregroundProjectId: null,
      sessionBridgeLive: false,
    }

    const controller = createShellSessionLifecycleController({
      state,
      getProjects: () => [],
      ipc: {
        getForegroundProject,
        listClaudeSessions: vi.fn(),
        navigateToSession: vi.fn(),
      },
      sessionStore: {
        getSessions: () => new Map(),
        applyDaemonSessionUpdate: vi.fn(),
        markSessionPresenceStale: vi.fn(),
      },
      logger: { warn: vi.fn(), error: vi.fn(), debug: vi.fn() },
    })

    const pending = controller.loadForegroundProject()
    controller.handleTmuxFocusChanged({ project_id: 'proj-live' })
    expect(state.foregroundProjectId).toBe('proj-live')

    resolveForeground('proj-stale')
    await pending

    expect(state.foregroundProjectId).toBe('proj-live')
  })

  it('does not let a failed startup fallback clear a live focus event', async () => {
    // Same race, error arm: the catch path clears the marker outright, so a
    // fallback that fails after a focus event landed blanked the indicator.
    let rejectForeground = () => {}
    const getForegroundProject = vi.fn(
      () => new Promise((_resolve, reject) => {
        rejectForeground = reject
      })
    )
    const state = {
      foregroundProjectId: null,
      sessionBridgeLive: false,
    }
    const logger = { warn: vi.fn(), error: vi.fn(), debug: vi.fn() }

    const controller = createShellSessionLifecycleController({
      state,
      getProjects: () => [],
      ipc: {
        getForegroundProject,
        listClaudeSessions: vi.fn(),
        navigateToSession: vi.fn(),
      },
      sessionStore: {
        getSessions: () => new Map(),
        applyDaemonSessionUpdate: vi.fn(),
        markSessionPresenceStale: vi.fn(),
      },
      logger,
    })

    const pending = controller.loadForegroundProject()
    controller.handleTmuxFocusChanged({ project_id: 'proj-live' })

    rejectForeground(new Error('daemon gone'))
    await pending

    expect(state.foregroundProjectId).toBe('proj-live')
  })

  it('applies the startup fallback when no focus event raced it', async () => {
    const getForegroundProject = vi.fn().mockResolvedValue('proj-startup')
    const state = {
      foregroundProjectId: null,
      sessionBridgeLive: false,
    }

    const controller = createShellSessionLifecycleController({
      state,
      getProjects: () => [],
      ipc: {
        getForegroundProject,
        listClaudeSessions: vi.fn(),
        navigateToSession: vi.fn(),
      },
      sessionStore: {
        getSessions: () => new Map(),
        applyDaemonSessionUpdate: vi.fn(),
        markSessionPresenceStale: vi.fn(),
      },
      logger: { warn: vi.fn(), error: vi.fn(), debug: vi.fn() },
    })

    await controller.loadForegroundProject()

    expect(state.foregroundProjectId).toBe('proj-startup')

    // A focus event that has already been applied must not freeze the marker:
    // the next load is newer than that event and wins on its own turn.
    controller.handleTmuxFocusChanged({ project_id: 'proj-live' })
    getForegroundProject.mockResolvedValue('proj-later')
    await controller.loadForegroundProject()

    expect(state.foregroundProjectId).toBe('proj-later')
  })

})
