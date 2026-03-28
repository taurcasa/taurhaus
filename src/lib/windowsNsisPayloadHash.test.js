import { describe, expect, it } from 'vitest'

import {
  hashNsisPayloadBytes,
  materializeNsisPayloadBytes,
  TAURI_NSIS_BUNDLE_MARKER,
  TAURI_UNKNOWN_BUNDLE_MARKER,
} from '../../scripts/windows-nsis-payload-hash.mjs'

describe('windows NSIS payload hash helper', () => {
  it('rewrites the Tauri bundle type marker from unknown to NSIS', () => {
    const raw = Buffer.from(`prefix:${TAURI_UNKNOWN_BUNDLE_MARKER}:suffix`, 'ascii')

    const patched = materializeNsisPayloadBytes(raw)

    expect(patched.equals(raw)).toBe(false)
    expect(patched.toString('ascii')).toContain(TAURI_NSIS_BUNDLE_MARKER)
    expect(patched.toString('ascii')).not.toContain(TAURI_UNKNOWN_BUNDLE_MARKER)
    expect(patched).toHaveLength(raw.length)
  })

  it('returns an already-patched NSIS payload unchanged', () => {
    const bundled = Buffer.from(`prefix:${TAURI_NSIS_BUNDLE_MARKER}:suffix`, 'ascii')

    const patched = materializeNsisPayloadBytes(bundled)

    expect(patched.equals(bundled)).toBe(true)
    expect(hashNsisPayloadBytes(bundled)).toHaveLength(64)
  })

  it('rewrites only unknown markers when the payload already contains NSIS markers', () => {
    const mixed = Buffer.from(
      `prefix:${TAURI_NSIS_BUNDLE_MARKER}:middle:${TAURI_UNKNOWN_BUNDLE_MARKER}:suffix`,
      'ascii',
    )

    const patched = materializeNsisPayloadBytes(mixed)

    expect(patched.toString('ascii')).toContain(`${TAURI_NSIS_BUNDLE_MARKER}:middle:${TAURI_NSIS_BUNDLE_MARKER}`)
    expect(patched.toString('ascii')).not.toContain(TAURI_UNKNOWN_BUNDLE_MARKER)
    expect(patched).toHaveLength(mixed.length)
  })

  it('throws when the Tauri bundle marker is missing', () => {
    expect(() => materializeNsisPayloadBytes(Buffer.from('no-marker-here', 'ascii'))).toThrow(
      /expected at least one Tauri bundle marker occurrence/,
    )
  })
})
