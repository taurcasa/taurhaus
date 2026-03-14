import { describe, expect, it, vi } from 'vitest'

import {
  setupSessionPollingLifecycle,
  setupShellEventListeners,
} from './events.svelte.js'

function createDeferred() {
  let resolve
  const promise = new Promise((res) => {
    resolve = res
  })
  return { promise, resolve }
}

function flushPromises() {
  return Promise.resolve().then(() => Promise.resolve())
}

describe('setupSessionPollingLifecycle', () => {
  it('skips polling when Tauri bridge is already live', () => {
    const startPolling = vi.fn()
    const stopPolling = vi.fn()
    const doc = {
      hidden: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }

    const cleanup = setupSessionPollingLifecycle({
      isTauri: true,
      sessionBridgeLive: true,
      startPolling,
      stopPolling,
      doc,
    })

    expect(startPolling).not.toHaveBeenCalled()
    cleanup()
    expect(stopPolling).not.toHaveBeenCalled()
    expect(doc.addEventListener).toHaveBeenCalledWith('visibilitychange', expect.any(Function))
    expect(doc.removeEventListener).toHaveBeenCalledWith('visibilitychange', expect.any(Function))
  })

  it('starts polling and toggles it on document visibility changes', () => {
    const listeners = new Map()
    const doc = {
      hidden: false,
      addEventListener: vi.fn((name, handler) => {
        listeners.set(name, handler)
      }),
      removeEventListener: vi.fn((name, handler) => {
        if (listeners.get(name) === handler) {
          listeners.delete(name)
        }
      }),
    }
    const startPolling = vi.fn()
    const stopPolling = vi.fn()

    const cleanup = setupSessionPollingLifecycle({
      isTauri: false,
      sessionBridgeLive: false,
      startPolling,
      stopPolling,
      doc,
    })

    expect(startPolling).toHaveBeenCalledTimes(1)

    doc.hidden = true
    listeners.get('visibilitychange')()
    expect(stopPolling).toHaveBeenCalledTimes(1)

    doc.hidden = false
    listeners.get('visibilitychange')()
    expect(startPolling).toHaveBeenCalledTimes(2)

    cleanup()
    expect(stopPolling).toHaveBeenCalledTimes(2)
    expect(doc.removeEventListener).toHaveBeenCalled()
  })

  it('logs hidden and visible recovery timing for visibility changes', () => {
    const listeners = new Map()
    const doc = {
      hidden: false,
      addEventListener: vi.fn((name, handler) => {
        listeners.set(name, handler)
      }),
      removeEventListener: vi.fn(),
    }
    const logger = {
      info: vi.fn(),
    }
    let currentTime = 100

    const cleanup = setupSessionPollingLifecycle({
      isTauri: false,
      sessionBridgeLive: false,
      startPolling: vi.fn(),
      stopPolling: vi.fn(),
      doc,
      logger,
      now: () => currentTime,
    })

    doc.hidden = true
    listeners.get('visibilitychange')()
    currentTime = 460
    doc.hidden = false
    listeners.get('visibilitychange')()

    const hiddenEvent = logger.info.mock.calls[0][1]
    const visibleEvent = logger.info.mock.calls[1][1]

    expect(hiddenEvent.event).toBe('shell.visibility.hidden')
    expect(hiddenEvent.recovery_mode).toBe('pause_fallback_polling')
    expect(visibleEvent.event).toBe('shell.visibility.visible')
    expect(visibleEvent.hidden_duration_ms).toBe(360)
    expect(visibleEvent.recovery_mode).toBe('resume_fallback_polling')

    cleanup()
  })
})

describe('setupShellEventListeners', () => {
  it('registers handlers and hydrates sessions after startup', async () => {
    const registeredHandlers = new Map()
    const unlisten = vi.fn()
    const onProjectGitChanged = vi.fn()
    const onHydrateSessions = vi.fn()
    const onTmuxFocusChanged = vi.fn()

    const cleanup = setupShellEventListeners({
      enabled: true,
      loadEventApi: async () => ({
        listen: vi.fn((eventName, handler) => {
          registeredHandlers.set(eventName, handler)
          return Promise.resolve(unlisten)
        }),
      }),
      onProjectGitChanged,
      onSessionImported: vi.fn(),
      onProjectsReseedComplete: vi.fn(),
      onProjectFilesChanged: vi.fn(),
      onDaemonStatus: vi.fn(),
      onSessionsUpdated: vi.fn(),
      onTmuxFocusChanged,
      onHydrateSessions,
      logger: console,
    })

    await flushPromises()

    expect(registeredHandlers.has('project-git-changed')).toBe(true)
    expect(registeredHandlers.has('sessions-updated')).toBe(true)
    expect(registeredHandlers.has('tmux-focus-changed')).toBe(true)
    expect(onHydrateSessions).toHaveBeenCalledTimes(1)

    await registeredHandlers.get('project-git-changed')({
      payload: { project_id: 'p1', branch: 'main' },
    })
    expect(onProjectGitChanged).toHaveBeenCalledWith({
      project_id: 'p1',
      branch: 'main',
    })

    await registeredHandlers.get('tmux-focus-changed')({
      payload: { project_id: 'proj-2' },
    })
    expect(onTmuxFocusChanged).toHaveBeenCalledWith({
      project_id: 'proj-2',
    })

    cleanup()
    expect(unlisten).toHaveBeenCalledTimes(7)
  })

  it('disposes listeners that resolve after cleanup', async () => {
    const deferred = createDeferred()
    const unlisten = vi.fn()

    const cleanup = setupShellEventListeners({
      enabled: true,
      loadEventApi: async () => ({
        listen: vi.fn(() => deferred.promise),
      }),
      onProjectGitChanged: vi.fn(),
      onSessionImported: vi.fn(),
      onProjectsReseedComplete: vi.fn(),
      onProjectFilesChanged: vi.fn(),
      onDaemonStatus: vi.fn(),
      onSessionsUpdated: vi.fn(),
      onTmuxFocusChanged: vi.fn(),
      onHydrateSessions: vi.fn(),
      logger: console,
    })

    await flushPromises()
    cleanup()
    deferred.resolve(unlisten)
    await flushPromises()

    expect(unlisten).toHaveBeenCalledTimes(7)
  })

  it('forwards null tmux focus payloads so Shell can clear foreground state', async () => {
    const registeredHandlers = new Map()
    const onTmuxFocusChanged = vi.fn()

    setupShellEventListeners({
      enabled: true,
      loadEventApi: async () => ({
        listen: vi.fn((eventName, handler) => {
          registeredHandlers.set(eventName, handler)
          return Promise.resolve(() => {})
        }),
      }),
      onProjectGitChanged: vi.fn(),
      onSessionImported: vi.fn(),
      onProjectsReseedComplete: vi.fn(),
      onProjectFilesChanged: vi.fn(),
      onDaemonStatus: vi.fn(),
      onSessionsUpdated: vi.fn(),
      onTmuxFocusChanged,
      onHydrateSessions: vi.fn(),
      logger: console,
    })

    await flushPromises()
    await registeredHandlers.get('tmux-focus-changed')({ payload: null })

    expect(onTmuxFocusChanged).toHaveBeenCalledWith(null)
  })
})
