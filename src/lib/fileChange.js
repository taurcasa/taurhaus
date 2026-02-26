/**
 * File change event utilities.
 *
 * Centralizes path matching logic for the `project-files-changed` Tauri event.
 * Used by Shell.svelte (central listener) and any component that needs to
 * check whether a specific file was among the changed paths.
 */

/**
 * Check whether a relative file path matches any of the changed paths.
 *
 * Handles both forward-slash (Linux/daemon) and backslash (Windows/local)
 * path separators. Matches by filename suffix so it works regardless of
 * whether the event paths are absolute, relative, or UNC.
 *
 * @param {string[]} changedPaths — paths from the event payload
 * @param {string} relativePath — the file to check (e.g., "README.md", "src/lib.rs")
 * @returns {boolean}
 */
export function pathWasChanged(changedPaths, relativePath) {
  if (!changedPaths?.length || !relativePath) return false

  // Normalize the relative path to use forward slashes for matching
  const needle = '/' + relativePath.replace(/\\/g, '/')

  return changedPaths.some(p => {
    // Normalize the event path to forward slashes
    const normalized = p.replace(/\\/g, '/')
    return normalized.endsWith(needle) || normalized === relativePath
  })
}

/**
 * Check whether any of the changed paths match a pattern.
 *
 * @param {string[]} changedPaths — paths from the event payload
 * @param {RegExp} pattern — regex to test against each path
 * @returns {boolean}
 */
export function anyPathMatches(changedPaths, pattern) {
  if (!changedPaths?.length) return false
  return changedPaths.some(p => pattern.test(p))
}
