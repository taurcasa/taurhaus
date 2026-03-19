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
    const onConfirm = vi.fn()
    render(ConfirmDialog, {
      props: {
        open: true,
        onConfirm,
      },
    })

    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))
    expect(onConfirm).toHaveBeenCalledTimes(1)
  })

  it('calls cancel callback when cancel button is clicked', async () => {
    const onCancel = vi.fn()
    render(ConfirmDialog, {
      props: {
        open: true,
        onCancel,
      },
    })

    await fireEvent.click(screen.getByTestId('confirm-dialog-cancel'))
    expect(onCancel).toHaveBeenCalledTimes(1)
  })

  it('supports Enter keyboard shortcut for confirm', async () => {
    const onConfirm = vi.fn()
    render(ConfirmDialog, {
      props: {
        open: true,
        onConfirm,
      },
    })

    const dialog = screen.getByTestId('confirm-dialog')

    await fireEvent.keyDown(dialog, { key: 'Enter', code: 'Enter' })
    expect(onConfirm).toHaveBeenCalledTimes(1)
  })

  it('supports Escape keyboard shortcut for cancel', async () => {
    const onCancel = vi.fn()
    render(ConfirmDialog, {
      props: {
        open: true,
        onCancel,
      },
    })

    const dialog = screen.getByTestId('confirm-dialog')
    await fireEvent.keyDown(dialog, { key: 'Escape', code: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(1)
  })

  it('moves initial focus to the first action button when opened', async () => {
    render(ConfirmDialog, {
      props: {
        open: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog-cancel')).toHaveFocus()
    })
  })
})
