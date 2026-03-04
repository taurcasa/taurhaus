export function collectDuplicateNames(values) {
  const counts = new Map()
  for (const value of values ?? []) {
    const normalized = String(value ?? '').trim().toLowerCase()
    if (!normalized) continue
    counts.set(normalized, (counts.get(normalized) ?? 0) + 1)
  }

  return Array.from(counts.entries())
    .filter(([, count]) => count > 1)
    .map(([name]) => name)
}
