import { describe, it, expect, beforeEach, vi } from 'vitest'

describe('navHistory', () => {
  let nav

  beforeEach(async () => {
    vi.resetModules()
    nav = await import('./navHistory.svelte.js')
    nav.reset()
  })

  it('push adds entries and goBack returns them', () => {
    nav.push({ tab: 'overview' })
    nav.push({ tab: 'git' })
    nav.push({ tab: 'files' })

    expect(nav.canGoBack()).toBe(true)
    const entry = nav.goBack()
    expect(entry.tab).toBe('git')
  })

  it('goBack returns null when at start', () => {
    nav.push({ tab: 'overview' })
    expect(nav.goBack()).toBeNull()
  })

  it('goBack returns null on empty stack', () => {
    expect(nav.goBack()).toBeNull()
    expect(nav.canGoBack()).toBe(false)
  })

  it('goForward returns null when at end', () => {
    nav.push({ tab: 'overview' })
    nav.push({ tab: 'git' })
    expect(nav.goForward()).toBeNull()
    expect(nav.canGoForward()).toBe(false)
  })

  it('goForward returns next entry after goBack', () => {
    nav.push({ tab: 'overview' })
    nav.push({ tab: 'git' })
    nav.push({ tab: 'files' })

    nav.goBack() // → git
    const entry = nav.goForward()
    expect(entry.tab).toBe('files')
  })

  it('push after goBack truncates forward history', () => {
    nav.push({ tab: 'overview' })
    nav.push({ tab: 'git' })
    nav.push({ tab: 'files' })

    nav.goBack() // → git
    nav.push({ tab: 'tasks' }) // should truncate 'files'

    expect(nav.canGoForward()).toBe(false)
    expect(nav.goForward()).toBeNull()

    const back = nav.goBack()
    expect(back.tab).toBe('git')
  })

  it('deduplicates identical consecutive entries', () => {
    nav.push({ tab: 'git' })
    nav.push({ tab: 'git' })
    nav.push({ tab: 'git' })

    expect(nav.canGoBack()).toBe(false)
  })

  it('deduplicates entries with matching file and lineNumber', () => {
    nav.push({ tab: 'files', file: 'src/main.rs', lineNumber: 10 })
    nav.push({ tab: 'files', file: 'src/main.rs', lineNumber: 10 })

    expect(nav.canGoBack()).toBe(false)
  })

  it('does not deduplicate entries with different fields', () => {
    nav.push({ tab: 'files', file: 'a.rs' })
    nav.push({ tab: 'files', file: 'b.rs' })

    expect(nav.canGoBack()).toBe(true)
  })

  it('deduplicates entries with matching rangeFilter', () => {
    nav.push({ tab: 'git', rangeFilter: { after: 'abc', before: 'def' } })
    nav.push({ tab: 'git', rangeFilter: { after: 'abc', before: 'def' } })

    expect(nav.canGoBack()).toBe(false)
  })

  it('reset clears all history', () => {
    nav.push({ tab: 'overview' })
    nav.push({ tab: 'git' })
    nav.push({ tab: 'files' })

    nav.reset()

    expect(nav.canGoBack()).toBe(false)
    expect(nav.canGoForward()).toBe(false)
    expect(nav.goBack()).toBeNull()
  })

  it('withSuppressed prevents push from recording', () => {
    nav.push({ tab: 'overview' })
    nav.push({ tab: 'git' })

    nav.withSuppressed(() => {
      nav.push({ tab: 'files' })
      nav.push({ tab: 'tasks' })
    })

    // Should still be at 'git', no new entries
    expect(nav.canGoForward()).toBe(false)
    const back = nav.goBack()
    expect(back.tab).toBe('overview')
  })

  it('caps stack at 50 entries', () => {
    for (let i = 0; i < 60; i++) {
      nav.push({ tab: 'git', commit: `hash-${i}` })
    }

    // Should be able to go back at most 49 times (50 entries, cursor at 49)
    let count = 0
    while (nav.goBack()) count++
    expect(count).toBe(49)
  })

  it('full back-forward round trip preserves entries', () => {
    nav.push({ tab: 'overview' })
    nav.push({ tab: 'git', commit: 'abc' })
    nav.push({ tab: 'files', file: 'src/main.rs' })

    // Go all the way back
    const b1 = nav.goBack()
    expect(b1).toEqual({ tab: 'git', commit: 'abc' })
    const b2 = nav.goBack()
    expect(b2).toEqual({ tab: 'overview' })
    expect(nav.goBack()).toBeNull()

    // Go all the way forward
    const f1 = nav.goForward()
    expect(f1).toEqual({ tab: 'git', commit: 'abc' })
    const f2 = nav.goForward()
    expect(f2).toEqual({ tab: 'files', file: 'src/main.rs' })
    expect(nav.goForward()).toBeNull()
  })
})
