import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
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
})
