import { describe, expect, it } from 'vitest'

import {
  catalogFor,
  defaultModelFor,
  effortsFor,
  entryFor,
  isKnownModel,
  parseLegacyModel,
  resolveMemberModel,
  toolEffortsFor,
} from './modelCatalog.js'

const CATALOG = {
  claude: [
    {
      id: 'opus',
      label: 'Opus 5',
      efforts: ['low', 'medium', 'high'],
      defaultEffort: null,
      deprecated: false,
      replacement: null,
    },
  ],
  codex: [
    {
      id: 'gpt-5.4',
      label: 'GPT-5.4',
      efforts: ['low', 'medium', 'high', 'xhigh'],
      defaultEffort: 'medium',
      deprecated: true,
      replacement: 'gpt-5.6-terra',
    },
    {
      id: 'gpt-5.6-terra',
      label: 'GPT-5.6-Terra',
      efforts: ['low', 'medium', 'high', 'xhigh', 'max'],
      defaultEffort: 'high',
      deprecated: false,
      replacement: null,
    },
  ],
  gemini: [
    {
      id: 'gemini-3.1-pro',
      label: 'Gemini 3.1 Pro',
      efforts: [],
      defaultEffort: null,
      deprecated: false,
      replacement: null,
    },
  ],
}

describe('modelCatalog lookups', () => {
  it('returns the entries for a tool and an empty list for unknown tools', () => {
    expect(catalogFor(CATALOG, 'codex').map((entry) => entry.id)).toEqual([
      'gpt-5.4',
      'gpt-5.6-terra',
    ])
    expect(catalogFor(CATALOG, 'grok')).toEqual([])
    expect(catalogFor(null, 'codex')).toEqual([])
  })

  it('defaultModelFor skips deprecated entries', () => {
    expect(defaultModelFor(CATALOG, 'codex')).toBe('gpt-5.6-terra')
    expect(defaultModelFor(CATALOG, 'claude')).toBe('opus')
  })

  it('defaultModelFor falls back to the first entry when every entry is deprecated', () => {
    const allDeprecated = { codex: [{ id: 'gpt-5.4', deprecated: true }] }
    expect(defaultModelFor(allDeprecated, 'codex')).toBe('gpt-5.4')
  })

  it('defaultModelFor returns an empty string without a catalog', () => {
    expect(defaultModelFor({}, 'codex')).toBe('')
  })

  it('effortsFor and isKnownModel answer per model', () => {
    expect(effortsFor(CATALOG, 'codex', 'gpt-5.4')).toEqual(['low', 'medium', 'high', 'xhigh'])
    expect(effortsFor(CATALOG, 'gemini', 'gemini-3.1-pro')).toEqual([])
    expect(effortsFor(CATALOG, 'codex', 'mystery-model')).toEqual([])
    expect(isKnownModel(CATALOG, 'codex', 'gpt-5.4')).toBe(true)
    expect(isKnownModel(CATALOG, 'codex', 'mystery-model')).toBe(false)
    expect(entryFor(CATALOG, 'codex', 'gpt-5.6-terra')?.label).toBe('GPT-5.6-Terra')
  })

  // The backend accepts the tool-wide effort vocabulary for models it does not
  // know (`ModelCatalog::supports_effort`, models/mod.rs), so a custom model id
  // must still be assignable an effort in the UI.
  it('toolEffortsFor unions the efforts the tool declares anywhere in the catalog', () => {
    expect(toolEffortsFor(CATALOG, 'codex')).toEqual(['low', 'medium', 'high', 'xhigh', 'max'])
    expect(toolEffortsFor(CATALOG, 'claude')).toEqual(['low', 'medium', 'high'])
    expect(toolEffortsFor(CATALOG, 'gemini')).toEqual([])
    expect(toolEffortsFor(null, 'codex')).toEqual([])
  })
})

describe('parseLegacyModel', () => {
  // Regression: commits ff40911 and 5d2ce27 shipped combined "model effort" strings
  // that the launcher silently stripped, so a role asking for `high` ran at the
  // user's global `xhigh`. The frontend must split the same three spellings the
  // Rust `ModelSpec::parse_legacy` splits.
  it('splits the space, dash, and padded spellings the Rust parser splits', () => {
    for (const raw of ['gpt-5.4 high', 'gpt-5.4-high', '  gpt-5.4 high  ']) {
      expect(parseLegacyModel(raw)).toEqual({ model: 'gpt-5.4', reasoningEffort: 'high' })
    }
  })

  it('keeps model-only values intact', () => {
    expect(parseLegacyModel('gpt-5.4-mini')).toEqual({
      model: 'gpt-5.4-mini',
      reasoningEffort: null,
    })
    expect(parseLegacyModel('claude-opus-4-6')).toEqual({
      model: 'claude-opus-4-6',
      reasoningEffort: null,
    })
  })

  it('returns empty values for blank input', () => {
    expect(parseLegacyModel('   ')).toEqual({ model: '', reasoningEffort: null })
    expect(parseLegacyModel(null)).toEqual({ model: '', reasoningEffort: null })
  })
})

describe('resolveMemberModel', () => {
  it('prefers the member value', () => {
    expect(
      resolveMemberModel(
        { cliTool: 'codex', model: 'gpt-5.4', reasoningEffort: 'xhigh' },
        { cliTool: 'codex', model: 'gpt-5.6-terra', reasoningEffort: 'low' },
        CATALOG
      )
    ).toEqual({ model: 'gpt-5.4', reasoningEffort: 'xhigh', source: 'member' })
  })

  it('falls back to the role defaults when the member has no model', () => {
    expect(
      resolveMemberModel(
        { cliTool: 'codex' },
        { cliTool: 'codex', model: 'gpt-5.6-terra', reasoningEffort: 'low' },
        CATALOG
      )
    ).toEqual({ model: 'gpt-5.6-terra', reasoningEffort: 'low', source: 'role' })
  })

  it('falls back to the catalog default when neither member nor role has a model', () => {
    expect(resolveMemberModel({ cliTool: 'codex' }, null, CATALOG)).toEqual({
      model: 'gpt-5.6-terra',
      reasoningEffort: 'high',
      source: 'catalog',
    })
  })

  it('reports values outside the catalog as custom without replacing them', () => {
    expect(
      resolveMemberModel({ cliTool: 'codex', model: 'gpt-6-preview' }, null, CATALOG)
    ).toEqual({ model: 'gpt-6-preview', reasoningEffort: null, source: 'custom' })
  })

  it('splits a legacy combined member model', () => {
    expect(resolveMemberModel({ cliTool: 'codex', model: 'gpt-5.4 high' }, null, CATALOG)).toEqual({
      model: 'gpt-5.4',
      reasoningEffort: 'high',
      source: 'member',
    })
  })

  // Regression: b345de1 (PR 5c) let every layer fall through to the catalog's
  // `defaultEffort`, so a member or role that named a model but deliberately left
  // the effort unset was pinned to the catalog default and the initialize payload
  // shipped it. The backend keeps such an effort unset
  // (`hydrate_member_model_fields`, member_activation.rs) so the user's global CLI
  // setting still applies.
  it('keeps an explicitly declared model without an effort unset', () => {
    expect(resolveMemberModel({ cliTool: 'codex', model: 'gpt-5.4' }, null, CATALOG)).toEqual({
      model: 'gpt-5.4',
      reasoningEffort: null,
      source: 'member',
    })

    expect(
      resolveMemberModel({ cliTool: 'codex' }, { cliTool: 'codex', model: 'gpt-5.4' }, CATALOG)
    ).toEqual({
      model: 'gpt-5.4',
      reasoningEffort: null,
      source: 'role',
    })
  })

  it('reads snake_case member fields and infers the tool from the role defaults', () => {
    expect(
      resolveMemberModel(
        { cli_tool: 'claude', reasoning_effort: 'high' },
        { cli_tool: 'claude', model: 'opus' },
        CATALOG
      )
    ).toEqual({ model: 'opus', reasoningEffort: 'high', source: 'role' })
  })

  it('returns an empty model when the catalog has no entry for the tool', () => {
    expect(resolveMemberModel({ cliTool: 'codex' }, null, {})).toEqual({
      model: '',
      reasoningEffort: null,
      source: 'catalog',
    })
  })
})
