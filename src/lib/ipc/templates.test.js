import { describe, expect, it } from 'vitest'

import { importRoleFromFile } from './templates.js'
import { MOCK_MODEL_CATALOG } from './mocks/base.js'
import { defaultModelFor, isKnownModel } from '../modelCatalog.js'

describe('templates browser-mode fallbacks', () => {
  // Regression: b345de1 (PR 5c) moved every model list into the settings-backed
  // catalog but left the import fallback naming a model literal, so browser mode
  // could hand back a role pinned to a model the catalog no longer offers.
  it('imports a role with the mock catalog default model', async () => {
    const result = await importRoleFromFile('/tmp/imported-role.md')

    expect(isKnownModel(MOCK_MODEL_CATALOG, 'claude', result.role.defaults.model)).toBe(true)
    expect(result.role.defaults.model).toBe(defaultModelFor(MOCK_MODEL_CATALOG, 'claude'))
  })
})
