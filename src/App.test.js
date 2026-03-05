import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./lib/ipc.js', () => ({
  isTauri: vi.fn(() => true),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('./Shell.svelte', () => ({
  default: function MockShell(target) {
    const el = document.createElement('div')
    el.setAttribute('data-testid', 'mock-shell')
    el.textContent = 'Shell ready'
    if (target.nodeType === Node.ELEMENT_NODE) {
      target.appendChild(el)
    } else {
      target.parentNode.insertBefore(el, target)
    }
    return {
      $set() {},
      $destroy() {
        el.remove()
      },
    }
  },
}))

vi.mock('./lib/SplashScreen.svelte', () => ({
  default: function MockSplashScreen(target, props) {
    let currentProps = props || {}
    const root = document.createElement('div')
    root.setAttribute('data-testid', 'mock-splash-screen')

    const completeBtn = document.createElement('button')
    completeBtn.setAttribute('data-testid', 'splash-complete')
    completeBtn.textContent = 'Complete'
    completeBtn.onclick = () => currentProps.onComplete?.()

    const retryBtn = document.createElement('button')
    retryBtn.setAttribute('data-testid', 'splash-retry')
    retryBtn.textContent = 'Retry'
    retryBtn.onclick = () => currentProps.onRetry?.()

    const continueBtn = document.createElement('button')
    continueBtn.setAttribute('data-testid', 'splash-continue')
    continueBtn.textContent = 'Continue'
    continueBtn.onclick = () => currentProps.onContinue?.()

    root.append(completeBtn, retryBtn, continueBtn)

    if (target.nodeType === Node.ELEMENT_NODE) {
      target.appendChild(root)
    } else {
      target.parentNode.insertBefore(root, target)
    }

    return {
      $set(nextProps) {
        currentProps = { ...currentProps, ...(nextProps || {}) }
      },
      $destroy() {
        root.remove()
      },
    }
  },
}))

const { isTauri } = await import('./lib/ipc.js')
const { invoke } = await import('@tauri-apps/api/core')
import App from './App.svelte'

describe('App orchestration', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-03-05T00:00:00.000Z'))
    vi.clearAllMocks()
    isTauri.mockReturnValue(true)
    invoke.mockResolvedValue(undefined)
  })

  afterEach(() => {
    vi.runOnlyPendingTimers()
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  it('transitions from splash to shell after splash completion', async () => {
    render(App)

    expect(screen.getByTestId('mock-splash-screen')).toBeInTheDocument()
    expect(screen.queryByTestId('mock-shell')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('splash-complete'))

    expect(screen.getByTestId('mock-shell')).toBeInTheDocument()
    expect(screen.getByTestId('mock-splash-screen')).toBeInTheDocument()

    await vi.advanceTimersByTimeAsync(300)

    await waitFor(() => {
      expect(screen.queryByTestId('mock-splash-screen')).not.toBeInTheDocument()
    })
  })

  it('shows a retry error banner when daemon restart fails', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    invoke.mockRejectedValueOnce(new Error('daemon unavailable'))
    render(App)

    await fireEvent.click(screen.getByTestId('splash-retry'))

    await waitFor(() => {
      expect(screen.getByTestId('daemon-retry-error-banner')).toHaveTextContent(
        'Retry failed: daemon unavailable'
      )
    })
    expect(errorSpy).toHaveBeenCalledWith(
      '[splash] daemon retry failed:',
      expect.any(Error)
    )

    errorSpy.mockRestore()
  })

  it('allows dismissing the retry error banner', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    invoke.mockRejectedValueOnce(new Error('still down'))
    render(App)

    await fireEvent.click(screen.getByTestId('splash-retry'))

    await waitFor(() => {
      expect(screen.getByTestId('daemon-retry-error-banner')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByRole('button', { name: 'Dismiss retry error' }))

    await waitFor(() => {
      expect(screen.queryByTestId('daemon-retry-error-banner')).not.toBeInTheDocument()
    })

    errorSpy.mockRestore()
  })
})
