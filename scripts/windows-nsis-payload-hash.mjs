import crypto from 'node:crypto'
import fs from 'node:fs'

export const TAURI_UNKNOWN_BUNDLE_MARKER = '__TAURI_BUNDLE_TYPE_VAR_UNK'
export const TAURI_NSIS_BUNDLE_MARKER = '__TAURI_BUNDLE_TYPE_VAR_NSS'

const UNKNOWN_MARKER_BYTES = Buffer.from(TAURI_UNKNOWN_BUNDLE_MARKER, 'ascii')
const NSIS_MARKER_BYTES = Buffer.from(TAURI_NSIS_BUNDLE_MARKER, 'ascii')

function findAllMarkerOffsets(buffer, marker) {
  const offsets = []
  let start = 0
  while (start <= buffer.length - marker.length) {
    const index = buffer.indexOf(marker, start)
    if (index === -1) {
      break
    }
    offsets.push(index)
    start = index + marker.length
  }
  return offsets
}

export function materializeNsisPayloadBytes(input) {
  const buffer = Buffer.from(input)
  const unknownOffsets = findAllMarkerOffsets(buffer, UNKNOWN_MARKER_BYTES)
  const nsisOffsets = findAllMarkerOffsets(buffer, NSIS_MARKER_BYTES)

  if (unknownOffsets.length === 1 && nsisOffsets.length === 0) {
    NSIS_MARKER_BYTES.copy(buffer, unknownOffsets[0])
    return buffer
  }

  if (unknownOffsets.length === 0 && nsisOffsets.length === 1) {
    return buffer
  }

  throw new Error(
    `expected exactly one Tauri bundle marker occurrence, found UNK=${unknownOffsets.length} NSS=${nsisOffsets.length}`,
  )
}

export function hashNsisPayloadBytes(input) {
  return crypto.createHash('sha256').update(materializeNsisPayloadBytes(input)).digest('hex').toUpperCase()
}

function main() {
  const [, , exePath] = process.argv
  if (!exePath) {
    console.error('usage: bun windows-nsis-payload-hash.mjs <built-exe-path>')
    process.exit(1)
  }

  const bytes = fs.readFileSync(exePath)
  process.stdout.write(`${hashNsisPayloadBytes(bytes)}\n`)
}

if (import.meta.main) {
  main()
}
