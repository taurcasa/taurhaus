/**
 * Formatting utilities — pure functions for display values.
 */

/**
 * Format a duration in milliseconds to a human-readable string.
 *
 * @param {number} ms — duration in milliseconds
 * @returns {string}
 *   < 60s:  "< 1m"
 *   < 1h:   "23m"
 *   < 24h:  "1h 23m"
 *   >= 24h: "1d 3h"
 */
export function formatDuration(ms) {
  if (ms < 60_000) return '< 1m'
  const totalMinutes = Math.floor(ms / 60_000)
  const hours = Math.floor(totalMinutes / 60)
  const minutes = totalMinutes % 60
  const days = Math.floor(hours / 24)
  const remainingHours = hours % 24

  if (days > 0) return `${days}d ${remainingHours}h`
  if (hours > 0) return `${hours}h ${minutes}m`
  return `${minutes}m`
}
