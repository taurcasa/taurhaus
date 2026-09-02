import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  prepareAccountDirectory: vi.fn(),
  launchAccountLogin: vi.fn(),
}))

vi.mock('../accounts.svelte.js', () => ({
  accountState: vi.fn(() => ({ accounts: [] })),
  refreshAccounts: vi.fn(() => Promise.resolve()),
  setGlobalDefault: vi.fn(() => Promise.resolve()),
}))

import AddAccountFlow from './AddAccountFlow.svelte'
import { launchAccountLogin, prepareAccountDirectory } from '../ipc.js'
import { refreshAccounts } from '../accounts.svelte.js'

describe('AddAccountFlow', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('creates the sibling and opens the registry login in a managed terminal', async () => {
    prepareAccountDirectory.mockResolvedValue('/home/user/.codex-work')
    launchAccountLogin.mockResolvedValue({ tmux_pane: '%4' })
    render(AddAccountFlow, { props: { open: true, tool: 'codex', projectId: 'p1' } })

    await fireEvent.input(screen.getByLabelText('Account name'), { target: { value: 'work' } })
    await fireEvent.click(screen.getByText('Open sign-in terminal'))

    await waitFor(() => {
      expect(prepareAccountDirectory).toHaveBeenCalledWith('codex', 'work')
      expect(launchAccountLogin).toHaveBeenCalledWith('p1', 'codex', '/home/user/.codex-work')
    })
    expect(screen.getByTestId('account-login-waiting')).toHaveTextContent(
      'Waiting for Codex to finish sign-in'
    )
  })

  // Regression: 186f19a2 promised a resumable signed-out row for an abandoned
  // run, but a prepared directory with no credentials in it is invisible to
  // detection, so no such row exists — only re-opening the flow resumes it.
  it('names the recovery an abandoned run actually has', async () => {
    prepareAccountDirectory.mockResolvedValue('/home/user/.codex-work')
    launchAccountLogin.mockResolvedValue({ tmux_pane: '%3' })
    render(AddAccountFlow, { props: { open: true, tool: 'codex', projectId: 'p1' } })

    await fireEvent.input(screen.getByLabelText('Account name'), { target: { value: 'work' } })
    await fireEvent.click(screen.getByText('Open sign-in terminal'))

    const waiting = await screen.findByTestId('account-login-waiting')
    expect(waiting).toHaveTextContent(
      'You can close this panel; re-open Add account with the same name to resume'
    )
    expect(waiting).not.toHaveTextContent('a signed-out row remains resumable')
  })

  it('resumes sign-in for an existing signed-out row without recreating its directory', async () => {
    launchAccountLogin.mockResolvedValue({ tmux_pane: '%5' })
    render(AddAccountFlow, {
      props: {
        open: true,
        tool: 'claude',
        projectId: 'p1',
        existingAccount: { id: 'work', label: 'work@example.com', dir: '/home/user/.claude-work' },
      },
    })

    await fireEvent.click(screen.getByText('Open sign-in terminal'))

    expect(prepareAccountDirectory).not.toHaveBeenCalled()
    expect(launchAccountLogin).toHaveBeenCalledWith('p1', 'claude', '/home/user/.claude-work')
  })

  it('refreshes detection after a terminal error so the created directory stays resumable', async () => {
    prepareAccountDirectory.mockResolvedValue('/home/user/.codex-work')
    launchAccountLogin.mockRejectedValue(new Error('terminal unavailable'))
    render(AddAccountFlow, { props: { open: true, tool: 'codex', projectId: 'p1' } })

    await fireEvent.input(screen.getByLabelText('Account name'), { target: { value: 'work' } })
    await fireEvent.click(screen.getByText('Open sign-in terminal'))

    await waitFor(() => {
      expect(screen.getByRole('status')).toHaveTextContent('terminal unavailable')
      expect(refreshAccounts).toHaveBeenCalledWith('codex', { force: true })
    })
    expect(screen.getByText('Open sign-in terminal')).toBeEnabled()
  })

  // Regression: faffe345 polled forced daemon detection every two seconds for
  // as long as an unfinished sign-in panel stayed mounted.
  it('stops polling an unfinished sign-in after a bounded backoff window', async () => {
    vi.useFakeTimers()
    try {
      prepareAccountDirectory.mockResolvedValue('/home/user/.codex-work')
      launchAccountLogin.mockResolvedValue({ tmux_pane: '%4' })
      const { unmount } = render(AddAccountFlow, {
        props: { open: true, tool: 'codex', projectId: 'p1' },
      })

      await fireEvent.input(screen.getByLabelText('Account name'), { target: { value: 'work' } })
      await fireEvent.click(screen.getByText('Open sign-in terminal'))
      await vi.advanceTimersByTimeAsync(5 * 60 * 1000 + 1)

      expect(refreshAccounts.mock.calls.length).toBeLessThan(20)
      expect(screen.getByText('Open sign-in terminal')).toBeEnabled()
      unmount()
    } finally {
      vi.useRealTimers()
    }
  })
})
