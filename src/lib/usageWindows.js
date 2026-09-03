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
 * How hard one window presses, as every surface must read it.
 *
 * A provider's `severity` and its percentage are independent claims: Claude
 * passes an API severity through whatever the reading says, and other providers
 * send no severity at all. Neither alone answers "is this account under
 * pressure", so the bar, the account row's health dot and the ambient badge
 * take the worse of the two rather than each picking one.
 */
export function windowPressure(window) {
  const used = Number(window?.used_percentage ?? window?.usedPercentage)
  const severity = String(window?.severity ?? '')
  if (severity === 'critical' || used >= 100) return 'critical'
  if (severity === 'warning' || used >= 80) return 'warning'
  return 'normal'
}

/** The rule the meters draw by: a window past its own reset is no longer live. */
function hasReset(window, now) {
  const resetsAt = window?.resets_at ?? window?.resetsAt
  if (resetsAt == null) return false
  const reset = Number(resetsAt)
  return Number.isFinite(reset) && reset * 1000 <= now
}

/** Windows with a numeric reading whose reset has not passed. */
export function liveUsageWindows(windows, now = Date.now()) {
  return (Array.isArray(windows) ? windows : []).filter((window) => {
    const used = Number(window?.used_percentage ?? window?.usedPercentage)
    return Number.isFinite(used) && !hasReset(window, now)
  })
}

/** Remaining percentage across the tightest provider window, or unknown. */
export function accountHeadroom(usage) {
  const readings = (Array.isArray(usage?.windows) ? usage.windows : [])
    .map((window) => Number(window?.used_percentage ?? window?.usedPercentage))
    .filter(Number.isFinite)
  if (readings.length === 0) return null
  return Math.max(0, Math.min(100, 100 - Math.max(...readings)))
}

/**
 * Whether an account has nothing left to spend, and why.
 *
 * The one question the launch path asks of a usage snapshot: is the
 * subscription this project remembers still able to run a session? A `stale`
 * snapshot answers it — it is the last thing known, and a limit that was spent
 * an hour ago is not refilled by the poller falling behind. It is refilled by
 * its own reset, though, and a reading is read against the clock the caller
 * hands in: a window whose `resets_at` has passed has come back, whatever
 * percentage the last reading recorded, exactly as `UsageMeter` stops drawing
 * it. `unauthorized` outranks the windows: an account that cannot be read
 * cannot be used either, whatever its last numbers said. A provider that does
 * not measure usage (`unsupported`) reports nothing, which is not the same as
 * reporting headroom.
 *
 * `null` means "nothing to say"; callers must not read that as "healthy".
 */
export function exhaustedUsage(usage, now = Date.now()) {
  if (!usage) return null
  const status = String(usage.status ?? 'ok')
  if (status === 'unsupported') return null
  if (status === 'unauthorized') return { kind: 'unauthorized' }
  const spent = liveUsageWindows(usage.windows, now).find(
    (window) => Number(window?.used_percentage ?? window?.usedPercentage) >= 100
  )
  return spent ? { kind: 'exhausted', window: spent } : null
}

/**
 * When a window comes back, in the reader's own locale.
 *
 * A reset further out than a day names the weekday as well — "Tue 00:00" is an
 * answer, "00:00" alone is not. Shared so the meters and the chooser's
 * explanation say it the same way.
 */
export function resetLabel(resetsAt, now = Date.now()) {
  if (resetsAt == null) return null
  const date = new Date(Number(resetsAt) * 1000)
  if (!Number.isFinite(date.getTime())) return null
  const moreThanDay = date.getTime() - now > 24 * 60 * 60 * 1000
  return new Intl.DateTimeFormat(undefined, {
    ...(moreThanDay ? { weekday: 'short' } : {}),
    hour: 'numeric',
    minute: '2-digit',
  }).format(date)
}
