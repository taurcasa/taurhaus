import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  coordinationCreateTeam: vi.fn(),
  coordinationDisbandTeam: vi.fn(),
  coordinationAddMember: vi.fn(),
  coordinationRemoveMember: vi.fn(),
  coordinationListTeams: vi.fn(),
  coordinationGetTeamStatus: vi.fn(),
}))

const {
  coordinationCreateTeam,
  coordinationDisbandTeam,
  coordinationAddMember,
  coordinationRemoveMember,
  coordinationListTeams,
  coordinationGetTeamStatus,
} = await import('../ipc.js')

import CoordinationPanel from './CoordinationPanel.svelte'

describe('CoordinationPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    coordinationCreateTeam.mockResolvedValue(undefined)
    coordinationDisbandTeam.mockResolvedValue(undefined)
    coordinationAddMember.mockResolvedValue(undefined)
    coordinationRemoveMember.mockResolvedValue(undefined)
    coordinationListTeams.mockResolvedValue([])
    coordinationGetTeamStatus.mockResolvedValue({
      teamName: 'architecture-final',
      members: ['codex-reviewer'],
    })
  })

  it('renders without crashing and loads team list', async () => {
    render(CoordinationPanel, { props: { dark: false } })

    await waitFor(() => {
      expect(coordinationListTeams).toHaveBeenCalledTimes(1)
    })

    expect(screen.getByTestId('coordination-panel')).toBeInTheDocument()
    expect(screen.getByTestId('coordination-create-team-button')).toBeInTheDocument()
  })

  it('shows an error banner when team listing fails', async () => {
    coordinationListTeams.mockRejectedValueOnce(new Error('coordination not initialized'))
    render(CoordinationPanel, { props: { dark: false } })

    await waitFor(() => {
      expect(screen.getByTestId('coordination-error').textContent).toContain(
        'coordination not initialized'
      )
    })
  })

  it('renders team list items from IPC data', async () => {
    coordinationListTeams.mockResolvedValueOnce([
      { teamName: 'architecture-final' },
      { team_name: 'ops' },
    ])

    render(CoordinationPanel, { props: { dark: false } })

    await waitFor(() => {
      expect(screen.getByTestId('coordination-team-list')).toBeInTheDocument()
    })
    expect(screen.getByTestId('coordination-team-architecture-final')).toBeInTheDocument()
    expect(screen.getByTestId('coordination-team-ops')).toBeInTheDocument()
  })

  it('create team flow calls IPC with trimmed name', async () => {
    coordinationListTeams.mockResolvedValue([])
    render(CoordinationPanel, { props: { dark: false } })

    const input = screen.getByTestId('coordination-create-team-input')
    const button = screen.getByTestId('coordination-create-team-button')
    await fireEvent.input(input, { target: { value: '  architecture-final  ' } })
    await fireEvent.click(button)

    await waitFor(() => {
      expect(coordinationCreateTeam).toHaveBeenCalledWith('architecture-final')
      expect(coordinationGetTeamStatus).toHaveBeenCalledWith('architecture-final')
    })
  })

  it('disband uses confirm dialog and respects cancel path', async () => {
    coordinationListTeams.mockResolvedValue([{ teamName: 'architecture-final' }])
    render(CoordinationPanel, { props: { dark: false } })

    await fireEvent.click(await screen.findByTestId('coordination-team-architecture-final'))
    await waitFor(() => {
      expect(coordinationGetTeamStatus).toHaveBeenCalledWith('architecture-final')
    })

    await fireEvent.click(screen.getByTestId('coordination-disband-team-button'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('confirm-dialog-cancel'))
    expect(coordinationDisbandTeam).not.toHaveBeenCalled()

    await fireEvent.click(screen.getByTestId('coordination-disband-team-button'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))
    await waitFor(() => {
      expect(coordinationDisbandTeam).toHaveBeenCalledWith('architecture-final')
    })
  })
})
