import { getContext, setContext } from 'svelte'

const MODEL_CATALOG_CONTEXT_KEY = Symbol('ModelCatalogContext')

export function setModelCatalogContext(value) {
  setContext(MODEL_CATALOG_CONTEXT_KEY, value)
  return value
}

export function getModelCatalogContext() {
  return getContext(MODEL_CATALOG_CONTEXT_KEY) ?? null
}
