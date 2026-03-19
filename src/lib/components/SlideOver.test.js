import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import SlideOverHarness from './SlideOverHarness.svelte'

describe('SlideOver', () => {
  it('renders when open=true', () => {
    render(SlideOverHarness, {
      props: {
        open: true,
      },
    })

    expect(screen.getByTestId('slideover-panel')).toBeInTheDocument()
  })

  it('does not render when open=false', () => {
    render(SlideOverHarness, {
      props: {
        open: false,
      },
    })

    expect(screen.queryByTestId('slideover-panel')).not.toBeInTheDocument()
  })

  it('displays title text', () => {
    render(SlideOverHarness, {
      props: {
        open: true,
        title: 'Session details',
      },
    })

    expect(screen.getByText('Session details')).toBeInTheDocument()
  })

  it('calls onClose when close button clicked', async () => {
    const onClose = vi.fn()
    render(SlideOverHarness, {
      props: {
        open: true,
        onClose,
      },
    })

    await fireEvent.click(screen.getByTestId('slideover-close'))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('calls onClose when Escape pressed', async () => {
    const onClose = vi.fn()
    render(SlideOverHarness, {
      props: {
        open: true,
        onClose,
      },
    })

    await fireEvent.keyDown(window, { key: 'Escape', code: 'Escape' })
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('calls onClose when backdrop clicked', async () => {
    const onClose = vi.fn()
    render(SlideOverHarness, {
      props: {
        open: true,
        onClose,
      },
    })

    await fireEvent.click(screen.getByTestId('slideover-backdrop'))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('renders children content', () => {
    render(SlideOverHarness, {
      props: {
        open: true,
      },
    })

    expect(screen.getByTestId('slideover-child')).toHaveTextContent('Child content')
  })

  it('focus trap cycles with Tab', async () => {
    render(SlideOverHarness, {
      props: {
        open: true,
      },
    })

    const closeButton = screen.getByTestId('slideover-close')
    const lastButton = screen.getByTestId('slideover-focus-2')

    lastButton.focus()
    expect(lastButton).toHaveFocus()

    await fireEvent.keyDown(window, { key: 'Tab', code: 'Tab' })
    expect(closeButton).toHaveFocus()

    await fireEvent.keyDown(window, { key: 'Tab', code: 'Tab', shiftKey: true })
    expect(lastButton).toHaveFocus()
  })

  it('restores focus to the previously focused element when closed', async () => {
    const trigger = document.createElement('button')
    trigger.textContent = 'Open panel'
    document.body.appendChild(trigger)
    trigger.focus()

    const { rerender } = render(SlideOverHarness, {
      props: {
        open: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('slideover-close')).toHaveFocus()
    })

    await rerender({ open: false })

    await waitFor(() => {
      expect(trigger).toHaveFocus()
    })
    trigger.remove()
  })
})
