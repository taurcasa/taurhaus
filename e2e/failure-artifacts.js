import { basename, extname, resolve } from 'node:path'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'

const DEFAULT_TAIL_LINES = 200
const MAX_DRIVER_STDERR_CHARS = 64_000

function defaultTimestamp() {
  return new Date().toISOString().replace(/[:.]/g, '-')
}

function sanitizeSegment(value) {
  const raw = String(value || '').trim()
  if (!raw) return 'unknown-spec'
  return raw.replace(/[^a-zA-Z0-9._-]+/g, '-')
}

function tailLines(text, limit = DEFAULT_TAIL_LINES) {
  const lines = String(text || '').split('\n')
  if (lines.length <= limit) return lines.join('\n')
  return lines.slice(-limit).join('\n')
}

function extractRunIdFromJsonl(text) {
  const lines = String(text || '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .reverse()

  for (const line of lines) {
    try {
      const entry = JSON.parse(line)
      if (typeof entry?.run_id === 'string' && entry.run_id.trim()) {
        return entry.run_id
      }
    } catch {
      // ignore non-JSON lines
    }
  }
  return null
}

function readFirstTail(paths, lineLimit = DEFAULT_TAIL_LINES, deps = {}) {
  const io = {
    existsSync: deps.existsSync ?? existsSync,
    readFileSync: deps.readFileSync ?? readFileSync,
  }

  for (const candidate of paths || []) {
    if (!candidate) continue
    if (!io.existsSync(candidate)) continue
    const raw = io.readFileSync(candidate, 'utf8')
    return {
      path: candidate,
      tail: tailLines(raw, lineLimit),
    }
  }
  return null
}

export function appendDriverStderr(buffer, chunk, maxChars = MAX_DRIVER_STDERR_CHARS) {
  const next = `${buffer || ''}${chunk || ''}`
  if (next.length <= maxChars) return next
  return next.slice(next.length - maxChars)
}

export async function collectFailureArtifacts(options, deps = {}) {
  const io = {
    mkdirSync: deps.mkdirSync ?? mkdirSync,
    writeFileSync: deps.writeFileSync ?? writeFileSync,
    existsSync: deps.existsSync ?? existsSync,
    readFileSync: deps.readFileSync ?? readFileSync,
  }

  const specFile = options?.specFile || 'unknown-spec.js'
  const specBase = basename(specFile, extname(specFile))
  const specDirName = sanitizeSegment(specBase)
  const timestamp = options?.timestamp || defaultTimestamp()
  const artifactDir = resolve(options.outputDir, specDirName, timestamp)
  const metadata = {
    spec_name: specBase,
    spec_file: specFile,
    test_title: options?.testTitle || '',
    group_index: options?.groupIndex ?? null,
    captured_at: timestamp,
    run_id: null,
    app_log_source: null,
    daemon_log_source: null,
    files: [],
  }

  io.mkdirSync(artifactDir, { recursive: true })

  const appTail = readFirstTail(options?.appLogPaths || [], options?.tailLines, io)
  if (appTail) {
    const appLogOut = resolve(artifactDir, 'app-log.tail.log')
    io.writeFileSync(appLogOut, `${appTail.tail}\n`, 'utf8')
    metadata.app_log_source = appTail.path
    metadata.run_id = extractRunIdFromJsonl(appTail.tail)
    metadata.files.push('app-log.tail.log')
  }

  const daemonTail = readFirstTail(options?.daemonLogPaths || [], options?.tailLines, io)
  if (daemonTail) {
    const daemonLogOut = resolve(artifactDir, 'daemon-log.tail.log')
    io.writeFileSync(daemonLogOut, `${daemonTail.tail}\n`, 'utf8')
    metadata.daemon_log_source = daemonTail.path
    metadata.files.push('daemon-log.tail.log')
  }

  const driverStderr = String(options?.driverStderr || '').trim()
  if (driverStderr) {
    const driverLogOut = resolve(artifactDir, 'driver-stderr.log')
    io.writeFileSync(driverLogOut, `${driverStderr}\n`, 'utf8')
    metadata.files.push('driver-stderr.log')
  }

  if (typeof options?.saveScreenshot === 'function') {
    const screenshotPath = resolve(artifactDir, 'failure.png')
    await options.saveScreenshot(screenshotPath)
    metadata.files.push('failure.png')
  }

  io.writeFileSync(
    resolve(artifactDir, 'metadata.json'),
    `${JSON.stringify(metadata, null, 2)}\n`,
    'utf8'
  )
  metadata.files.push('metadata.json')

  return {
    artifactDir,
    metadata,
  }
}
