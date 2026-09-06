import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { clickUntil } from './clickUntil.js'

// Regression: 430e09ee introduced runtime single-click flows; the 2s status
// poll can replace the clicked element without opening anything (673dac42
// fixed only template-crud-ui). Drive lost delivery without a real WebDriver.
describe('clickUntil', () => {
  let opened
  let clicks
  let queries
  const wait = { timeout: 300, interval: 100, timeoutMsg: 'Form did not open' }

  beforeEach(() => {
    opened = false
    clicks = []
    queries = 0
    vi.stubGlobal('$', vi.fn(async (selector) => {
      if (selector === '[data-testid="form"]') {
        return { isExisting: async () => opened }
      }
      const generation = ++queries
      return {
        isExisting: async () => true,
        scrollIntoView: async () => {},
        click: async () => {
          clicks.push(generation)
          // First click silently lands on a just-replaced node. The next
          // must use a fresh query; clicking again after open would close it.
          if (generation > 1) opened = !opened
        },
      }
    }))
    vi.stubGlobal('browser', {
      waitUntil: vi.fn(async (condition, options) => {
        for (let elapsed = 0; elapsed <= options.timeout; elapsed += options.interval) {
          if (await condition()) return true
        }
        throw new Error(options.timeoutMsg)
      }),
    })
  })

  afterEach(() => vi.unstubAllGlobals())

  it('re-queries and retries a silently lost click until the target exists', async () => {
    await clickUntil('open', 'form', wait)
    expect(clicks).toEqual([1, 2])
    expect(opened).toBe(true)
    expect(browser.waitUntil).toHaveBeenCalledWith(expect.any(Function), wait)
  })

  it('does not toggle an already-open target', async () => {
    opened = true
    await clickUntil('open', 'form', wait)
    expect(clicks).toEqual([])
  })

  it('checks the target before retrying when it appears between polls', async () => {
    let checks = 0
    const click = vi.fn()
    await clickUntil(click, async () => ++checks >= 3, wait)
    expect(click).toHaveBeenCalledTimes(1)
  })

  it('supports an exact named-detail predicate without accepting another member', async () => {
    let detailName = 'other-agent'
    const click = vi.fn(async () => {
      if (click.mock.calls.length === 2) detailName = 'requested-agent'
    })
    await clickUntil(click, async () => detailName === 'requested-agent', wait)
    expect(click).toHaveBeenCalledTimes(2)
    expect(detailName).toBe('requested-agent')
  })

  it('fails with the caller diagnostic when clicks never open the target', async () => {
    const click = vi.fn()
    await expect(clickUntil(click, async () => false, wait)).rejects.toThrow(wait.timeoutMsg)
    expect(click).toHaveBeenCalledTimes(4)
  })
})
