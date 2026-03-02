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
      cliTool: 'claude',
      model: 'opus',
      projectId: 'taurhaus',
      description: 'Own orchestration',
      sessionStatus: 'active',
      paneId: '%1',
    },
    {
      name: 'frontend-dev',
      role: 'member',
      cliTool: 'codex',
      model: 'gpt-5.3',
      projectId: 'taurhaus-web',
      description: 'UI implementation',
      sessionStatus: 'idle',
      paneId: '%2',
    },
    {
      name: 'docs-writer',
      role: 'member',
      cliTool: 'gemini',
      model: 'gemini-2.5-pro',
      projectId: 'taurhaus-docs',
      description: null,
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
      expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    })
  })

  it('renders member names with star indicator for lead', async () => {
    render(MeshTeamRoster, { props: { teamName: 'architecture-final' } })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-role-indicator-team-lead')).toHaveTextContent('★ team-lead')
    })
    expect(screen.getByTestId('mesh-role-indicator-frontend-dev')).toHaveTextContent('frontend-dev')
  })

  it('shows status badges with correct text labels and tones', async () => {
    render(MeshTeamRoster, { props: { teamName: 'architecture-final' } })
    await waitFor(() => {
      const activeBadge = screen.getByTestId('mesh-status-badge-team-lead')
      expect(activeBadge).toHaveTextContent('Active')
      expect(activeBadge.className).toContain('text-success-')
    })
    const idleBadge = screen.getByTestId('mesh-status-badge-frontend-dev')
    expect(idleBadge).toHaveTextContent('Idle')
    expect(idleBadge.className).toContain('text-warning-')
    const offlineBadge = screen.getByTestId('mesh-status-badge-docs-writer')
    expect(offlineBadge).toHaveTextContent('Offline')
    expect(offlineBadge.className).toContain('text-zinc-')
  })

  it('renders tool, model, project metadata and optional description', async () => {
    render(MeshTeamRoster, { props: { teamName: 'architecture-final' } })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-member-meta-frontend-dev')).toHaveTextContent(
        'Codex · gpt-5.3 · taurhaus-web'
      )
    })
    expect(screen.getByTestId('mesh-member-description-frontend-dev')).toHaveTextContent(
      'UI implementation'
    )
    expect(screen.queryByTestId('mesh-member-description-docs-writer')).not.toBeInTheDocument()
  })

  it('shows action buttons without hover gating', async () => {
    render(MeshTeamRoster, { props: { teamName: 'architecture-final' } })
    await waitFor(() => {
      expect(screen.getByTestId('mesh-focus-pane-frontend-dev')).toBeVisible()
      expect(screen.getByTestId('mesh-reonboard-frontend-dev')).toBeVisible()
    })
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
