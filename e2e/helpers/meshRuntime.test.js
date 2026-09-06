import { afterEach, describe, expect, it, vi } from 'vitest'
import { clickRuntimeAddAgent } from './meshRuntime.js'

// Regression: 275d42d6 retried the primary action without checking whether
// it still meant Add Agent; a stopped team would receive repeated resumes.
describe('runtime Add Agent opener', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    document.body.innerHTML = ''
  })

  it.each(['Resume Team', 'Resume Stopped (1)', 'Resuming Team...'])('never clicks %s', async (label) => {
    document.body.innerHTML = `<button data-testid="mesh-runtime-primary-action">${label}</button>`
    const click = vi.fn()
    document.querySelector('button').onclick = click
    vi.stubGlobal('$', async () => ({ isExisting: async () => false }))
    vi.stubGlobal('browser', {
      execute: async (fn) => fn(),
      waitUntil: async (condition, options) => {
        for (let poll = 0; poll < 3; poll++) await condition()
        throw new Error(options.timeoutMsg)
      },
    })
    await expect(clickRuntimeAddAgent({ timeoutMsg: 'Add Agent did not open' }))
      .rejects.toThrow('Add Agent did not open')
    expect(click).not.toHaveBeenCalled()
  })

  it('rechecks a replaced control and clicks only an enabled Add Agent action', async () => {
    let opened = false
    const labels = []
    const click = vi.fn((event) => {
      labels.push(event.currentTarget.textContent)
      opened = true
    })
    vi.stubGlobal('$', async () => ({ isExisting: async () => opened }))
    vi.stubGlobal('browser', {
      execute: async (fn) => fn(),
      waitUntil: async (condition) => {
        for (const markup of [
          '<button data-testid="mesh-runtime-primary-action">Resume Team</button>',
          '<button data-testid="mesh-runtime-primary-action" disabled>Add Agent</button>',
          '<button data-testid="mesh-runtime-primary-action">Add Agent</button>',
        ]) {
          document.body.innerHTML = markup
          document.querySelector('button').onclick = click
          if (await condition()) break
        }
        expect(await condition()).toBe(true) // target-first: no second click
      },
    })
    await clickRuntimeAddAgent({})
    expect(click).toHaveBeenCalledTimes(1)
    expect(labels).toEqual(['Add Agent'])
    expect(opened).toBe(true)
  })
})
