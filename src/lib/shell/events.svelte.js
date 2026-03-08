export function setupSessionPollingLifecycle({
  isTauri,
  sessionBridgeLive,
  startPolling,
  stopPolling,
  doc = document,
}) {
  if (isTauri && sessionBridgeLive) {
    return () => {}
  }

  startPolling()

  function onVisibilityChange() {
    if (doc.hidden) {
      stopPolling()
    } else {
      startPolling()
    }
  }

  doc.addEventListener('visibilitychange', onVisibilityChange)

  return () => {
    stopPolling()
    doc.removeEventListener('visibilitychange', onVisibilityChange)
  }
}

export function setupShellEventListeners({
  enabled,
  loadEventApi,
  onProjectGitChanged,
  onSessionImported,
  onProjectsReseedComplete,
  onProjectFilesChanged,
  onDaemonStatus,
  onSessionsUpdated,
  onTmuxFocusChanged,
  onHydrateSessions,
  logger = console,
}) {
  if (!enabled) {
    return () => {}
  }

  let destroyed = false
  const cleanups = []

  function runListenerHandler(eventName, handler, event) {
    try {
      const maybePromise = handler(event.payload)
      if (maybePromise && typeof maybePromise.then === 'function') {
        maybePromise.catch((error) => {
          logger.error(`[events] listener '${eventName}' handler failed:`, error)
        })
      }
    } catch (error) {
      logger.error(`[events] listener '${eventName}' handler failed:`, error)
    }
  }

  function registerListener(listen, eventName, handler) {
    if (typeof handler !== 'function') return

    listen(eventName, (event) => runListenerHandler(eventName, handler, event))
      .then((unlisten) => {
        if (destroyed) {
          unlisten()
          return
        }
        cleanups.push(unlisten)
      })
      .catch((error) => {
        logger.error(`[events] failed to register listener '${eventName}':`, error)
      })
  }

  Promise.resolve()
    .then(loadEventApi)
    .then(({ listen }) => {
      if (destroyed) return

      registerListener(listen, 'project-git-changed', onProjectGitChanged)
      registerListener(listen, 'session-imported', onSessionImported)
      registerListener(listen, 'projects-reseed-complete', onProjectsReseedComplete)
      registerListener(listen, 'project-files-changed', onProjectFilesChanged)
      registerListener(listen, 'daemon-status', onDaemonStatus)
      registerListener(listen, 'sessions-updated', onSessionsUpdated)
      registerListener(listen, 'tmux-focus-changed', onTmuxFocusChanged)

      if (!destroyed && typeof onHydrateSessions === 'function') {
        onHydrateSessions()
      }
    })
    .catch((error) => {
      logger.error('[events] failed to initialize Tauri listeners:', error)
    })

  return () => {
    destroyed = true
    while (cleanups.length > 0) {
      const unlisten = cleanups.pop()
      unlisten()
    }
  }
}
