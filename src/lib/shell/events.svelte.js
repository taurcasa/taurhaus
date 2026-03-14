function nowMs(now) {
  if (typeof now === 'function') {
    return now()
  }
  if (typeof performance !== 'undefined' && typeof performance.now === 'function') {
    return performance.now()
  }
  return Date.now()
}

export function setupSessionPollingLifecycle({
  isTauri,
  sessionBridgeLive,
  startPolling,
  stopPolling,
  doc = document,
  logger = console,
  now,
}) {
  const bridgeLive = isTauri && sessionBridgeLive
  let hiddenAt = null

  if (!bridgeLive) {
    startPolling()
  }

  function onVisibilityChange() {
    const changedAt = nowMs(now)
    if (doc.hidden) {
      hiddenAt = changedAt
      logger.info('[shell.visibility] document hidden', {
        event: 'shell.visibility.hidden',
        session_bridge_live: bridgeLive,
        recovery_mode: bridgeLive ? 'bridge_live_no_polling_change' : 'pause_fallback_polling',
      })
      if (!bridgeLive) {
        stopPolling()
      }
    } else {
      logger.info('[shell.visibility] document visible', {
        event: 'shell.visibility.visible',
        session_bridge_live: bridgeLive,
        hidden_duration_ms: hiddenAt === null ? null : Number((changedAt - hiddenAt).toFixed(1)),
        recovery_mode: bridgeLive ? 'bridge_live_no_polling_change' : 'resume_fallback_polling',
      })
      hiddenAt = null
      if (!bridgeLive) {
        startPolling()
      }
    }
  }

  doc.addEventListener('visibilitychange', onVisibilityChange)

  return () => {
    if (!bridgeLive) {
      stopPolling()
    }
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
