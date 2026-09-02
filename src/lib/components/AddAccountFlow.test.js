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
})
