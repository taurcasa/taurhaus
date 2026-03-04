import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  upsertTeamPreset: vi.fn(),
  normalizeProjectOption: (p) => ({ id: p.id, label: p.name }),
}))

import * as ipc from '../ipc.js'
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

describe('TeamCustomizerPanel - Save as Preset', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ipc.upsertTeamPreset.mockResolvedValue({ presetId: 'new-preset', name: 'New Preset' })
  })

  it('shows "Save as New Preset" button and opens dialog', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
      },
    })

    const savePresetBtn = await screen.findByTestId('team-customizer-save-preset-trigger')
    expect(savePresetBtn).toBeInTheDocument()
    expect(savePresetBtn).toBeEnabled()

    await fireEvent.click(savePresetBtn)

    expect(screen.getByTestId('save-preset-dialog')).toBeInTheDocument()
    expect(screen.getByTestId('save-preset-name-input')).toBeInTheDocument()
    expect(screen.getByTestId('save-preset-description-input')).toBeInTheDocument()
  })

  it('validates preset name is required', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
      },
    })

    await fireEvent.click(await screen.findByTestId('team-customizer-save-preset-trigger'))

    const saveBtn = screen.getByTestId('save-preset-confirm')
    // Clear it because it pre-fills with team name
    await fireEvent.input(screen.getByTestId('save-preset-name-input'), { target: { value: '' } })
    expect(saveBtn).toBeDisabled()

    await fireEvent.input(screen.getByTestId('save-preset-name-input'), { target: { value: 'My Cool Preset' } })
    expect(saveBtn).not.toBeDisabled()
  })

  it('calls upsertTeamPreset with correct payload', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
      },
    })

    await fireEvent.click(await screen.findByTestId('team-customizer-save-preset-trigger'))
    await fireEvent.input(screen.getByTestId('save-preset-name-input'), { target: { value: 'My Cool Preset' } })
    await fireEvent.input(screen.getByTestId('save-preset-description-input'), { target: { value: 'Preset desc' } })
    
    await fireEvent.click(screen.getByTestId('save-preset-confirm'))

    expect(ipc.upsertTeamPreset).toHaveBeenCalledWith(expect.objectContaining({
      name: 'My Cool Preset',
      description: 'Preset desc',
      leadRoleId: null, // Lead doesn't have roleId in current baseTeamConfig, we'll see how it's handled
      agentSlots: [
        expect.objectContaining({ count: 1 })
      ]
    }))
  })

  it('disables save preset button when team config has errors', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: { ...baseTeamConfig(), teamName: '' }, // Error: team name required
        projectPath: '/projects/taurhaus',
      },
    })

    const savePresetBtn = await screen.findByTestId('team-customizer-save-preset-trigger')
    expect(savePresetBtn).toBeDisabled()
  })
})
