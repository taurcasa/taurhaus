import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import TeamCustomizerPanel from './TeamCustomizerPanel.svelte'

function baseTeamConfig() {
  return {
    teamName: 'taurhaus-team',
    description: 'Mesh team config',
    presetId: 'fullstack-dev',
    lead: {
      id: 'lead',
      name: 'team-lead',
      tool: 'claude',
      model: 'opus',
      projectId: '/projects/taurhaus',
      description: 'Lead agent',
    },
    agents: [
      {
        id: 'agent-1',
        name: 'dev-1',
        tool: 'codex',
        model: 'gpt-5.3-codex',
        projectId: '/projects/taurhaus',
        description: 'Agent one',
      },
    ],
  }
}

describe('TeamCustomizerPanel', () => {
  it('renders inside SlideOver when open', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('slideover-panel')).toBeInTheDocument()
    })
    expect(screen.getByTestId('team-customizer-panel')).toBeInTheDocument()
    expect(screen.getByTestId('team-customizer-reset')).toBeInTheDocument()
    expect(screen.getByTestId('team-customizer-save')).toBeInTheDocument()
    expect(screen.getByTestId('team-customizer-lead')).toBeInTheDocument()
  })

  it('shows selected role context hint when provided', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
        context: {
          selectedRole: {
            roleId: 'codex-developer',
            name: 'Codex Developer',
          },
        },
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-selected-role')).toHaveTextContent(
        'Codex Developer'
      )
    })
  })

  it('save and reset callbacks fire', async () => {
    const onSave = vi.fn()
    const onReset = vi.fn()

    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
        onSave,
        onReset,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-save')).toBeEnabled()
    })

    await fireEvent.click(screen.getByTestId('team-customizer-save'))
    expect(onSave).toHaveBeenCalledTimes(1)
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        lead: expect.objectContaining({
          name: 'team-lead',
          cliTool: 'claude',
        }),
      })
    )

    await fireEvent.click(screen.getByTestId('team-customizer-reset'))
    expect(onReset).toHaveBeenCalledTimes(1)
  })
})
