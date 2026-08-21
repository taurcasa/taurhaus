import { normalizeTool, resolveRoleModel, resolveRoleReasoningEffort } from './meshDefaults.js'

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

/**
 * The union of every effort the tool declares anywhere in the catalog. The
 * backend validates an effort against this tool-wide vocabulary for models it
 * does not know (`ModelCatalog::supports_effort`, models/mod.rs), so a custom
 * model id can still be given one.
 */
export function toolEffortsFor(catalog, tool) {
  const seen = []
  for (const entry of catalogFor(catalog, tool)) {
    if (!Array.isArray(entry?.efforts)) continue
    for (const effort of entry.efforts) {
      const normalized = trimmed(effort)
      if (normalized && !seen.includes(normalized)) seen.push(normalized)
    }
  }
  return seen
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

/**
 * The effort a role template declares, including the pre-PR-5a spelling that
 * folded it into the model string. A member bound to such a role never inherits
 * the CLI's global setting: the backend refills an unset effort from the role
 * (`apply_role_template_defaults` in request_normalization.rs,
 * `hydrate_member_model_fields` in member_activation.rs), so the UI must show
 * this value instead of offering an inherit-global choice it cannot deliver.
 */
export function roleDeclaredEffort(roleTemplate) {
  if (!roleTemplate) return null
  return (
    resolveRoleReasoningEffort(roleTemplate) ??
    parseLegacyModel(resolveRoleModel(roleTemplate)).reasoningEffort
  )
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
 *
 * An effort nobody declared stays `null` whenever a real layer supplied the
 * model: the backend does the same (`hydrate_member_model_fields`,
 * member_activation.rs) so the CLI's own global setting keeps applying. The
 * catalog's `defaultEffort` is only a suggestion, and it is used exactly where
 * the catalog also supplied the model.
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
    layerEffort = defaultEffortFor(catalog, tool, model)
  }

  const reasoningEffort = readEffort(member) ?? layerEffort

  if (model && !isKnownModel(catalog, tool, model)) {
    source = 'custom'
  }

  return { model, reasoningEffort: reasoningEffort ?? null, source }
}
