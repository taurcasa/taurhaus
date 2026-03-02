import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  coordinationGetLiveTeamStatus: vi.fn(),
  coordinationReonboard: vi.fn(),
}))

const { coordinationGetLiveTeamStatus, coordinationReonboard } = await import('../ipc.js')

import MeshTeamRoster from './MeshTeamRoster.svelte'

const sampleRoster = {
  teamName: 'architecture-final',
  leadName: 'team-lead',
  members: [
    {
      name: 'team-lead',
      role: 'lead',
      cliTool: 'Claude',
      model: 'opus',
      projectId: 'taurhaus',
      sessionStatus: 'active',
      paneId: '%1',
    },
    {
      name: 'frontend-dev',
      role: 'member',
      cliTool: 'Codex',
      model: 'gpt-5.3',
      projectId: 'taurhaus-web',
      sessionStatus: 'idle',
      paneId: '%2',
    },
    {
      name: 'docs-writer',
      role: 'member',
      cliTool: 'Gemini',
      model: 'gemini-2.5-pro',
      projectId: 'taurhaus-docs',
      sessionStatus: 'offline',
      paneId: null,
    },
  ],
}

describe('MeshTeamRoster', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    coordinationGetLiveTeamStatus.mockResolvedValue(sampleRoster)
    coordinationReonboard.mockResolvedValue({ delivered: true, method: 'tmux_injection' })
  })

  it('renders team name in header', async () => {
    render(MeshTeamRoster, { props: { teamName: 'architecture-final' } })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('Team: architecture-final')
    })
  })

  it('renders lead with star and members without', async () => {
    render(MeshTeamRoster, { props: { teamName: 'architecture-final' } })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-role-indicator-team-lead')).toHaveTextContent('★')
    })
    expect(screen.getByTestId('mesh-role-indicator-frontend-dev')).toHaveTextContent('◦')
  })

  it('shows correct status dots per session_status', async () => {
    render(MeshTeamRoster, { props: { teamName: 'architecture-final' } })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-status-dot-team-lead')).toHaveTextContent('● Active')
    })
    expect(screen.getByTestId('mesh-status-dot-frontend-dev')).toHaveTextContent('● Idle')
    expect(screen.getByTestId('mesh-status-dot-docs-writer')).toHaveTextContent('○ Offline')
  })

  it('focus pane button calls onFocusPane with pane_id', async () => {
    const onFocusPane = vi.fn()
    render(MeshTeamRoster, { props: { teamName: 'architecture-final', onFocusPane } })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-focus-pane-frontend-dev')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-focus-pane-frontend-dev'))
    expect(onFocusPane).toHaveBeenCalledWith('%2')
  })

  it('add agent button calls onAddAgent', async () => {
    const onAddAgent = vi.fn()
    render(MeshTeamRoster, { props: { teamName: 'architecture-final', onAddAgent } })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    expect(onAddAgent).toHaveBeenCalledTimes(1)
  })

  it('disband button calls onDisband', async () => {
    const onDisband = vi.fn()
    render(MeshTeamRoster, { props: { teamName: 'architecture-final', onDisband } })
    await fireEvent.click(screen.getByTestId('mesh-disband-button'))
    expect(onDisband).toHaveBeenCalledTimes(1)
  })

  it('re-onboard button triggers coordinationReonboard for non-lead members', async () => {
    render(MeshTeamRoster, { props: { teamName: 'architecture-final' } })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-reonboard-frontend-dev')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-reonboard-frontend-dev'))
    await waitFor(() => {
      expect(coordinationReonboard).toHaveBeenCalledWith('architecture-final', 'frontend-dev')
    })
  })
})
