import { getContext, setContext } from 'svelte'

const SESSION_CONTEXT_KEY = Symbol('SessionContext')

export function setSessionContext(value) {
  setContext(SESSION_CONTEXT_KEY, value)
  return value
}

export function getSessionContext() {
  return getContext(SESSION_CONTEXT_KEY) ?? null
}
