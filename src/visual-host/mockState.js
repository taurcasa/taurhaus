function createDefaults() {
  return {
    ipc: {
      getLatestSession: null,
      getRecentCommits: [],
      getRelationships: [],
      navigateToSession: undefined,
      launchClaudeSession: { ok: true },
      stopClaudeSession: undefined,
      removeProject: undefined,
      openExternalUrl: undefined,
    },
    sessionStore: {
      sessionsByProject: {},
      sessionByProject: {},
    },
  }
}

const state = createDefaults()

function cloneRecord(record = {}) {
  return { ...record }
}

function cloneArrayRecord(record = {}) {
  return Object.fromEntries(
    Object.entries(record).map(([key, value]) => [key, Array.isArray(value) ? [...value] : []])
  )
}

export function resetVisualHostState() {
  const defaults = createDefaults()
  state.ipc = { ...defaults.ipc }
  state.sessionStore = {
    sessionsByProject: cloneArrayRecord(defaults.sessionStore.sessionsByProject),
    sessionByProject: cloneRecord(defaults.sessionStore.sessionByProject),
  }
  return state
}

export function configureVisualHostState(config = {}) {
  resetVisualHostState()
  if (config.ipc && typeof config.ipc === 'object') {
    state.ipc = {
      ...state.ipc,
      ...config.ipc,
    }
  }

  if (config.sessionStore && typeof config.sessionStore === 'object') {
    state.sessionStore = {
      sessionsByProject: cloneArrayRecord(config.sessionStore.sessionsByProject),
      sessionByProject: cloneRecord(config.sessionStore.sessionByProject),
    }
  }

  return state
}

export function resolveVisualHostMock(name, args = []) {
  const value = state.ipc[name]
  if (typeof value === 'function') {
    return value(...args)
  }
  return value
}

export function getVisualHostSession(projectPath) {
  return state.sessionStore.sessionByProject[projectPath] ?? null
}

export function getVisualHostSessions(projectPath) {
  const list = state.sessionStore.sessionsByProject[projectPath]
  return Array.isArray(list) ? list : []
}

resetVisualHostState()
