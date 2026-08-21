import { normalizeTool } from './meshDefaults.js'

/**
 * Model catalog helpers.
 *
 * The catalog is owned by the backend (`ModelCatalog` on `TerminalPlatformContract`)
 * and reaches the frontend through `settings.terminal_contract.model_catalog`.
 * Nothing here hardcodes a model id: an empty catalog simply resolves to an empty
 * model, and the backend then applies its own catalog default.
 */

export const EMPTY_MODEL_CATALOG = Object.freeze({ claude: [], codex: [], gemini: [] })

// Same effort words as the Rust `ModelSpec::parse_legacy` (session_scanner/launch.rs).
const LEGACY_EFFORTS = ['low', 'medium', 'high', 'xhigh', 'max', 'ultra']

function trimmed(value) {
  return String(value ?? '').trim()
}

export function catalogFor(catalog, tool) {
  const entries = catalog?.[trimmed(tool).toLowerCase()]
  return Array.isArray(entries) ? entries : []
}

export function entryFor(catalog, tool, modelId) {
  const id = trimmed(modelId)
  if (!id) return null
  return catalogFor(catalog, tool).find((entry) => trimmed(entry?.id) === id) ?? null
}

export function isKnownModel(catalog, tool, modelId) {
  return entryFor(catalog, tool, modelId) !== null
}

export function effortsFor(catalog, tool, modelId) {
  const efforts = entryFor(catalog, tool, modelId)?.efforts
  return Array.isArray(efforts) ? efforts.map((effort) => trimmed(effort)).filter(Boolean) : []
}

export function defaultEffortFor(catalog, tool, modelId) {
  const entry = entryFor(catalog, tool, modelId)
  return trimmed(entry?.defaultEffort ?? entry?.default_effort) || null
}

export function defaultModelFor(catalog, tool) {
  const entries = catalogFor(catalog, tool)
  const preferred = entries.find((entry) => !entry?.deprecated) ?? entries[0]
  return trimmed(preferred?.id)
}

/**
 * Splits values that still arrive as one string ("gpt-5.4 high", "gpt-5.4-high").
 * Mirrors the Rust `ModelSpec::parse_legacy` so both sides agree on what a
 * legacy template means.
 */
export function parseLegacyModel(value) {
  const raw = trimmed(value)
  if (!raw) return { model: '', reasoningEffort: null }

  const whitespaceIndex = raw.search(/\s\S*$/)
  const splitIndex = whitespaceIndex >= 0 ? whitespaceIndex : raw.lastIndexOf('-')
  if (splitIndex > 0) {
    const model = raw.slice(0, splitIndex).trim()
    const effort = raw.slice(splitIndex + 1).trim().toLowerCase()
    if (model && LEGACY_EFFORTS.includes(effort)) {
      return { model, reasoningEffort: effort }
    }
  }

  return { model: raw, reasoningEffort: null }
}

function readTool(source) {
  return trimmed(source?.tool ?? source?.cliTool ?? source?.cli_tool)
}

function readModel(source) {
  return parseLegacyModel(source?.model)
}

function readEffort(source) {
  return trimmed(source?.reasoningEffort ?? source?.reasoning_effort) || null
}

/**
 * The single place the model fallback order lives: member -> role defaults ->
 * catalog default. `source` reports which layer answered; a value the catalog
 * does not know reports `custom` and is never rewritten.
 */
export function resolveMemberModel(member, roleDefaults, catalog) {
  const tool = normalizeTool(readTool(member) || readTool(roleDefaults))
  const memberModel = readModel(member)
  const roleModel = readModel(roleDefaults)

  let model = ''
  let source = 'catalog'
  let layerEffort = null

  if (memberModel.model) {
    model = memberModel.model
    source = 'member'
    layerEffort = memberModel.reasoningEffort
  } else if (roleModel.model) {
    model = roleModel.model
    source = 'role'
    layerEffort = roleModel.reasoningEffort ?? readEffort(roleDefaults)
  } else {
    model = defaultModelFor(catalog, tool)
  }

  const reasoningEffort =
    readEffort(member) ?? layerEffort ?? defaultEffortFor(catalog, tool, model)

  if (model && !isKnownModel(catalog, tool, model)) {
    source = 'custom'
  }

  return { model, reasoningEffort: reasoningEffort ?? null, source }
}
