import { readdirSync, readFileSync } from 'node:fs'
import { relative, resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const SOURCE_ROOT = resolve(process.cwd(), 'src/lib')
const ALLOWED_FILES = new Set(['toolRegistry.js', 'ipc/mocks/tasks.js'])
const TOOL_COMPARISON = /(?:[!=]==\s*['"](?:claude|codex|agy|grok)['"]|case\s+['"](?:claude|codex|agy|grok)['"]|includes\(\s*['"](?:claude|codex|agy|grok)['"]|['"](?:claude|codex|agy|grok)['"]\s*:)/g

function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(directory, entry.name)
    if (entry.isDirectory()) return sourceFiles(path)
    if (!/\.(?:js|svelte)$/.test(entry.name) || entry.name.endsWith('.test.js')) return []
    return [path]
  })
}

describe('tool registry module boundary', () => {
  it('keeps tool-name comparisons inside the registry and test fixtures', () => {
    // Regression: commit 9a66d1c distributed frontend tool identity branches
    // across UI consumers; labels, aliases, accents, and capabilities now come
    // from the terminal contract registry.
    const violations = []
    let allowedComparisonCount = 0

    for (const path of sourceFiles(SOURCE_ROOT)) {
      const source = readFileSync(path, 'utf8')
      const matches = source.match(TOOL_COMPARISON) ?? []
      if (matches.length === 0) continue
      const file = relative(SOURCE_ROOT, path).replaceAll('\\', '/')
      if (ALLOWED_FILES.has(file)) allowedComparisonCount += matches.length
      else violations.push(`${file}: ${matches.length}`)
    }

    expect(violations).toEqual([])
    expect(allowedComparisonCount).toBe(2)
  })
})
