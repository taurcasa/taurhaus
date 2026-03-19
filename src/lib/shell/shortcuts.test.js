import { describe, expect, it, vi } from 'vitest'
import { setupHistoryNavigation, setupSearchShortcut } from './shortcuts.svelte.js'

describe('shell shortcuts', () => {
  it('toggles search on ctrl/cmd+k', () => {
    const addEventListener = vi.fn()
    const removeEventListener = vi.fn()
    const doc = { addEventListener, removeEventListener }
    const onToggleSearch = vi.fn()

    const cleanup = setupSearchShortcut({ doc, onToggleSearch })
    const handler = addEventListener.mock.calls[0][1]
    const event = {
      key: 'k',
      ctrlKey: true,
      metaKey: false,
      preventDefault: vi.fn(),
    }

    handler(event)

    expect(event.preventDefault).toHaveBeenCalledTimes(1)
    expect(onToggleSearch).toHaveBeenCalledTimes(1)

    cleanup()
    expect(removeEventListener).toHaveBeenCalledWith('keydown', handler)
  })

  it('routes mouse back/forward buttons through navigation callbacks', () => {
    const addEventListener = vi.fn()
    const removeEventListener = vi.fn()
    const doc = { addEventListener, removeEventListener }
    const onGoBack = vi.fn()
    const onGoForward = vi.fn()

    const cleanup = setupHistoryNavigation({ doc, onGoBack, onGoForward })
    const mouseHandler = addEventListener.mock.calls.find(([eventName]) => eventName === 'mousedown')[1]

    const backEvent = { button: 3, preventDefault: vi.fn() }
    mouseHandler(backEvent)
    expect(backEvent.preventDefault).toHaveBeenCalledTimes(1)
    expect(onGoBack).toHaveBeenCalledTimes(1)

    const forwardEvent = { button: 4, preventDefault: vi.fn() }
    mouseHandler(forwardEvent)
    expect(forwardEvent.preventDefault).toHaveBeenCalledTimes(1)
    expect(onGoForward).toHaveBeenCalledTimes(1)

    cleanup()
    expect(removeEventListener).toHaveBeenCalledTimes(2)
  })

  it('routes alt+arrow keys through navigation callbacks', () => {
    const addEventListener = vi.fn()
    const doc = { addEventListener, removeEventListener: vi.fn() }
    const onGoBack = vi.fn()
    const onGoForward = vi.fn()

    setupHistoryNavigation({ doc, onGoBack, onGoForward })
    const keyHandler = addEventListener.mock.calls.find(([eventName]) => eventName === 'keydown')[1]

    const leftEvent = { altKey: true, key: 'ArrowLeft', preventDefault: vi.fn() }
    keyHandler(leftEvent)
    expect(leftEvent.preventDefault).toHaveBeenCalledTimes(1)
    expect(onGoBack).toHaveBeenCalledTimes(1)

    const rightEvent = { altKey: true, key: 'ArrowRight', preventDefault: vi.fn() }
    keyHandler(rightEvent)
    expect(rightEvent.preventDefault).toHaveBeenCalledTimes(1)
    expect(onGoForward).toHaveBeenCalledTimes(1)
  })
})
