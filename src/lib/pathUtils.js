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
