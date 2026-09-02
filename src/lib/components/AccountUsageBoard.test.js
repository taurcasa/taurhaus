import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import AccountUsageBoard from './AccountUsageBoard.svelte'

describe('AccountUsageBoard', () => {
  it('uses shared account-menu rows and opens account management', async () => {
    const onManage = vi.fn()
    render(AccountUsageBoard, {
      props: {
        states: {
          claude: {
            accounts: [
              {
                id: 'personal',
                label: 'personal@example.com',
                display_name: 'Personal',
                logged_in: true,
                usage: {
                  windows: [{ title: 'Weekly', used_percentage: 82 }],
                },
              },
            ],
          },
          codex: {
            accounts: [
              { id: 'work', label: 'work@example.com', logged_in: false, usage: null },
            ],
          },
        },
        x: 220,
        y: 700,
        onManage,
      },
    })

    expect(screen.getByTestId('context-menu')).toBeInTheDocument()
    expect(screen.getByText('Personal')).toBeInTheDocument()
    expect(screen.getByText('Weekly 82%')).toBeInTheDocument()
    expect(screen.getByText('not logged in')).toBeInTheDocument()

    await fireEvent.mouseDown(screen.getByText('Manage accounts →'))
    expect(onManage).toHaveBeenCalledTimes(1)
  })

  // Regression: e28881d reused a keyboard-owning ContextMenu for a hover-only
  // board, so merely crossing the Accounts button stole focus and arrow keys.
  it('does not take focus or keyboard ownership when opened by hover', async () => {
    const input = document.createElement('input')
    document.body.append(input)
    input.focus()

    render(AccountUsageBoard, { props: { states: {}, x: 220, y: 700 } })
    await Promise.resolve()

    expect(input).toHaveFocus()
    const event = new KeyboardEvent('keydown', {
      key: 'ArrowDown',
      bubbles: true,
      cancelable: true,
    })
    input.dispatchEvent(event)
    expect(event.defaultPrevented).toBe(false)
    input.remove()
  })
})
