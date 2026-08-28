/**
 * Which usage windows a compact surface (chip, menu row) shows.
 *
 * `compact` is a provider preference, not a gate: a provider that flags nothing
 * still has headroom worth showing, and an empty chip reads as "no
 * subscription". Shared by `UsageMeter` and the account menu so both surfaces
 * agree on what a row means.
 */
function isSessionWindow(window) {
  const key = String(window?.key ?? '')
  const title = String(window?.title ?? '')
  return key === 'session' || key === 'five_hour' || title.startsWith('Current session')
}

/** Flagged compact windows, else every non-session window, else the first two. */
export function compactSelection(windows) {
  const flagged = windows.filter((window) => window?.compact === true)
  if (flagged.length) return flagged
  const nonSession = windows.filter((window) => !isSessionWindow(window))
  if (nonSession.length) return nonSession
  return windows.slice(0, 2)
}
