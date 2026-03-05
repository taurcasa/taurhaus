/**
 * Path utilities for resolving relative paths in rendered content.
 */

/**
 * Resolve a relative path against a file's directory.
 *
 * Given a base file path and a relative reference (image src, link href),
 * returns a project-root-relative path with ".." segments resolved.
 *
 * @param {string|null} filePath — path of the file containing the reference (e.g. "docs/architecture/daemon-protocol.md")
 * @param {string} relativeSrc — the relative reference (e.g. "../daemon-protocol.jpg")
 * @returns {string} — resolved project-root-relative path (e.g. "docs/daemon-protocol.jpg")
 *
 * @example
 * resolveRelativePath("docs/architecture/foo.md", "../img.jpg")  // "docs/img.jpg"
 * resolveRelativePath("docs/architecture/foo.md", "bar.jpg")     // "docs/architecture/bar.jpg"
 * resolveRelativePath(null, "docs/img.jpg")                      // "docs/img.jpg"  (passthrough)
 * resolveRelativePath("foo.md", "img.jpg")                       // "img.jpg"
 */
export function resolveRelativePath(filePath, relativeSrc) {
  if (!filePath || relativeSrc.startsWith('/')) return relativeSrc
  const dir = filePath.replace(/[^/]*$/, '')
  const parts = (dir + relativeSrc).split('/')
  const resolved = []
  for (const p of parts) {
    if (p === '..') resolved.pop()
    else if (p && p !== '.') resolved.push(p)
  }
  return resolved.join('/')
}

function normalizeLinuxPath(path) {
  let value = String(path || '').trim()
  if (!value) return ''
  value = value.replace(/\\/g, '/')
  value = value.replace(/\/+/g, '/')
  while (value.length > 1 && value.endsWith('/')) {
    value = value.slice(0, -1)
  }
  return value
}

function wslUncToLinux(path) {
  const normalized = String(path || '').trim().replace(/\//g, '\\')
  const lower = normalized.toLowerCase()

  let prefix = ''
  if (lower.startsWith('\\\\wsl$\\')) {
    prefix = '\\\\wsl$\\'
  } else if (lower.startsWith('\\\\wsl.localhost\\')) {
    prefix = '\\\\wsl.localhost\\'
  } else {
    return null
  }

  const remainder = normalized.slice(prefix.length)
  const firstSeparator = remainder.indexOf('\\')
  if (firstSeparator === -1) return null

  const afterDistro = remainder.slice(firstSeparator)
  if (!afterDistro || afterDistro === '\\') return '/'
  return normalizeLinuxPath(afterDistro)
}

function windowsDriveToLinux(path) {
  const match = String(path || '').trim().match(/^([a-zA-Z]):[\\/](.*)$/)
  if (!match) return null
  const [, drive, rest] = match
  return normalizeLinuxPath(`/mnt/${drive.toLowerCase()}/${rest}`)
}

/**
 * Normalize project paths for cross-platform matching.
 *
 * This mirrors backend path conversion semantics for project identity:
 * - WSL UNC path (`\\wsl$\\...`, `\\wsl.localhost\\...`) -> Linux path
 * - Windows drive path (`D:\\...`) -> Linux mount path (`/mnt/d/...`)
 * - Native/relative paths -> slash-normalized Linux-style form
 * - Trailing separators are stripped (except `/`)
 */
export function normalizeProjectPath(path) {
  const raw = String(path || '').trim()
  if (!raw) return ''
  return wslUncToLinux(raw) ?? windowsDriveToLinux(raw) ?? normalizeLinuxPath(raw)
}
