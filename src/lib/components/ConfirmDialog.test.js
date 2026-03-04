import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import ConfirmDialog from './ConfirmDialog.svelte'

describe('ConfirmDialog', () => {
  it('renders title and message when open', async () => {
    render(ConfirmDialog, {
      props: {
        open: true,
        title: 'Delete team',
        message: 'This action cannot be undone.',
      },
    })

    expect(screen.getByText('Delete team')).toBeInTheDocument()
    expect(screen.getByText('This action cannot be undone.')).toBeInTheDocument()
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toHaveAttribute('open')
    })
  })

  it('calls confirm callback when confirm button is clicked', async () => {
    const onconfirm = vi.fn()
    render(ConfirmDialog, {
      props: {
        open: true,
        onconfirm,
      },
    })

    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))
    expect(onconfirm).toHaveBeenCalledTimes(1)
  })

  it('calls cancel callback when cancel button is clicked', async () => {
    const oncancel = vi.fn()
    render(ConfirmDialog, {
      props: {
        open: true,
        oncancel,
      },
    })

    await fireEvent.click(screen.getByTestId('confirm-dialog-cancel'))
    expect(oncancel).toHaveBeenCalledTimes(1)
  })

  it('supports Enter keyboard shortcut for confirm', async () => {
    const onconfirm = vi.fn()
    render(ConfirmDialog, {
      props: {
        open: true,
        onconfirm,
      },
    })

    const dialog = screen.getByTestId('confirm-dialog')

    await fireEvent.keyDown(dialog, { key: 'Enter', code: 'Enter' })
    expect(onconfirm).toHaveBeenCalledTimes(1)
  })

  it('supports Escape keyboard shortcut for cancel', async () => {
    const oncancel = vi.fn()
    render(ConfirmDialog, {
      props: {
        open: true,
        oncancel,
      },
    })

    const dialog = screen.getByTestId('confirm-dialog')
    await fireEvent.keyDown(dialog, { key: 'Escape', code: 'Escape' })
    expect(oncancel).toHaveBeenCalledTimes(1)
  })
})
