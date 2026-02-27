import { describe, it, expect, beforeEach } from 'vitest'
import * as assetCache from './assetCache.js'

beforeEach(() => {
  assetCache.clear()
})

describe('assetCache', () => {
  it('returns null for uncached entries', () => {
    expect(assetCache.get('proj-1', 'logo.png')).toBe(null)
  })

  it('stores and retrieves a value', () => {
    assetCache.set('proj-1', 'logo.png', 'data:image/png;base64,abc')
    expect(assetCache.get('proj-1', 'logo.png')).toBe('data:image/png;base64,abc')
  })

  it('isolates entries by project', () => {
    assetCache.set('proj-1', 'logo.png', 'data-1')
    assetCache.set('proj-2', 'logo.png', 'data-2')
    expect(assetCache.get('proj-1', 'logo.png')).toBe('data-1')
    expect(assetCache.get('proj-2', 'logo.png')).toBe('data-2')
  })

  it('isolates entries by path', () => {
    assetCache.set('proj-1', 'a.png', 'data-a')
    assetCache.set('proj-1', 'b.png', 'data-b')
    expect(assetCache.get('proj-1', 'a.png')).toBe('data-a')
    expect(assetCache.get('proj-1', 'b.png')).toBe('data-b')
  })

  it('invalidates a single entry', () => {
    assetCache.set('proj-1', 'logo.png', 'data-1')
    assetCache.set('proj-1', 'icon.png', 'data-2')
    assetCache.invalidate('proj-1', 'logo.png')
    expect(assetCache.get('proj-1', 'logo.png')).toBe(null)
    expect(assetCache.get('proj-1', 'icon.png')).toBe('data-2')
  })

  it('invalidates all entries for a project', () => {
    assetCache.set('proj-1', 'a.png', 'data-a')
    assetCache.set('proj-1', 'b.png', 'data-b')
    assetCache.set('proj-2', 'c.png', 'data-c')
    assetCache.invalidateProject('proj-1')
    expect(assetCache.get('proj-1', 'a.png')).toBe(null)
    expect(assetCache.get('proj-1', 'b.png')).toBe(null)
    expect(assetCache.get('proj-2', 'c.png')).toBe('data-c')
  })

  it('clears the entire cache', () => {
    assetCache.set('proj-1', 'a.png', 'data-a')
    assetCache.set('proj-2', 'b.png', 'data-b')
    assetCache.clear()
    expect(assetCache.size()).toBe(0)
    expect(assetCache.get('proj-1', 'a.png')).toBe(null)
  })

  it('reports correct size', () => {
    expect(assetCache.size()).toBe(0)
    assetCache.set('proj-1', 'a.png', 'data')
    expect(assetCache.size()).toBe(1)
    assetCache.set('proj-1', 'b.png', 'data')
    expect(assetCache.size()).toBe(2)
    assetCache.invalidate('proj-1', 'a.png')
    expect(assetCache.size()).toBe(1)
  })

  it('overwrites existing entry on set', () => {
    assetCache.set('proj-1', 'logo.png', 'old-data')
    assetCache.set('proj-1', 'logo.png', 'new-data')
    expect(assetCache.get('proj-1', 'logo.png')).toBe('new-data')
    expect(assetCache.size()).toBe(1)
  })
})
