import { getContext, setContext } from 'svelte'

const PROJECT_CONTEXT_KEY = Symbol('ProjectContext')

export function setProjectContext(value) {
  setContext(PROJECT_CONTEXT_KEY, value)
  return value
}

export function getProjectContext() {
  return getContext(PROJECT_CONTEXT_KEY) ?? null
}
