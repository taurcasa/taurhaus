import { describe, expect, it } from 'vitest'

import { settingsUpdatePayload } from './system.js'

describe('settingsUpdatePayload', () => {
  // Regression: 5c6b2681 made normalizeCliVersions rewrite cli_versions to
  // snake_case reads and delete the camelCase spellings, while the backend's
  // CliVersions deserializes rename_all = "camelCase" with no field defaults.
  // Round-tripping the read settings document into update_settings therefore
  // failed every save ("missing field `codexCompactionHooksSupported`").
  // The terminal contract is backend-owned runtime state — attached fresh on
  // every read, replaced wholesale on save — so the update payload must never
  // carry it at all.
  it('never sends the backend-owned terminal contract back', () => {
    const payload = settingsUpdatePayload({
      scan_directories: ['~/projects'],
      terminal_contract: {
        cli_versions: { codex_compaction_hooks_supported: true },
      },
    })
    expect(payload).not.toHaveProperty('terminal_contract')
    expect(payload.scan_directories).toEqual(['~/projects'])
  })

  it('passes a contract-free document through unchanged', () => {
    const settings = { activity_thresholds: { active_days: 7 } }
    expect(settingsUpdatePayload(settings)).toEqual(settings)
    expect(settingsUpdatePayload(null)).toBeNull()
  })
})
