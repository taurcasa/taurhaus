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

/**
 * Whether an account has nothing left to spend, and why.
 *
 * The one question the launch path asks of a usage snapshot: is the
 * subscription this project remembers still able to run a session? A `stale`
 * snapshot answers it — it is the last thing known, and a limit that was spent
 * an hour ago is not refilled by the poller falling behind. `unauthorized`
 * outranks the windows: an account that cannot be read cannot be used either,
 * whatever its last numbers said. A provider that does not measure usage
 * (`unsupported`) reports nothing, which is not the same as reporting headroom.
 *
 * `null` means "nothing to say"; callers must not read that as "healthy".
 */
export function exhaustedUsage(usage) {
  if (!usage) return null
  const status = String(usage.status ?? 'ok')
  if (status === 'unsupported') return null
  if (status === 'unauthorized') return { kind: 'unauthorized' }
  const windows = Array.isArray(usage.windows) ? usage.windows : []
  const spent = windows.find((window) => Number(window?.used_percentage) >= 100)
  return spent ? { kind: 'exhausted', window: spent } : null
}
