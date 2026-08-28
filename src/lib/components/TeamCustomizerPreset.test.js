import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  upsertTeamPreset: vi.fn(),
  normalizeProjectOption: (p) => ({ id: p.id, label: p.name }),
}))

import * as ipc from '../ipc.js'
import TeamCustomizerPanel from './TeamCustomizerPanel.svelte'
import { TEST_MODEL_CATALOG } from '../../test/fixtures/modelCatalog.js'
import {
  FALLBACK_TOOLS,
  configureToolRegistry,
  resetToolRegistry,
} from '../toolRegistry.js'

function baseTeamConfig() {
  return {
    teamName: 'taurhaus-team',
    description: 'Mesh team config',
    presetId: 'dev-team',
    lead: {
      id: 'lead',
      name: 'team-lead',
      tool: 'claude',
      model: 'opus',
      projectId: '/projects/taurhaus',
      description: 'Lead agent',
      roleId: 'claude-orchestrator',
    },
    agents: [
      {
        id: 'agent-1',
        name: 'dev-1',
        tool: 'codex',
        model: 'gpt-5.4 high',
        projectId: '/projects/taurhaus',
        description: 'Agent one',
      },
    ],
  }
}

describe('TeamCustomizerPanel - Save as Preset', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    resetToolRegistry()
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

    expect(ipc.upsertTeamPreset).toHaveBeenCalledWith(
      expect.objectContaining({
        schema: {
          kind: 'team_preset',
          version: 1,
        },
        presetId: 'my-cool-preset',
        name: 'My Cool Preset',
        description: 'Preset desc',
        version: '1.0.0',
        leadRoleId: 'claude-orchestrator',
        defaults: {
          teamNamePattern: '{project}-team',
          tmuxLayout: 'tiled',
        },
        agentSlots: [
          expect.objectContaining({
            roleId: 'codex-developer',
            count: 1,
            projectBinding: 'lead_project',
            projectId: null,
            overrides: { model: 'gpt-5.4', reasoningEffort: 'high' },
          }),
        ],
      })
    )
  })

  it('uses the registry role default independently of the presentation accent', async () => {
    // Regression: 91f4d3f replaced tool identity with accent identity, so a
    // harmless palette change silently changed the persisted role template.
    const contract = structuredClone(FALLBACK_TOOLS)
    contract.find((entry) => entry.id === 'codex').accent = 'emerald'
    contract.find((entry) => entry.id === 'codex').defaultAgentRoleId = 'codex-developer'
    configureToolRegistry(contract)

    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
      },
    })

    await fireEvent.click(await screen.findByTestId('team-customizer-save-preset-trigger'))
    await fireEvent.input(screen.getByTestId('save-preset-name-input'), {
      target: { value: 'Semantic Registry' },
    })
    await fireEvent.click(screen.getByTestId('save-preset-confirm'))

    expect(ipc.upsertTeamPreset).toHaveBeenCalledWith(
      expect.objectContaining({
        agentSlots: [expect.objectContaining({ roleId: 'codex-developer' })],
      })
    )
  })

  it('still assigns the codex default role to an agent whose tool is empty or unknown', async () => {
    // Regression: 91f4d3f (PR 15) looked the fallback role up in the tool
    // registry only, so an agent row with an empty or unrecognized tool saved
    // `roleId: ""` and the backend rejected the preset; main's default arm had
    // always answered `codex-developer` for anything that was not claude/gemini.
    const config = baseTeamConfig()
    config.agents = [
      { id: 'agent-1', name: 'dev-1', tool: '', model: 'gpt-5.4', projectId: '/projects/taurhaus' },
      { id: 'agent-2', name: 'dev-2', tool: 'mystery-cli', projectId: '/projects/taurhaus' },
    ]

    render(TeamCustomizerPanel, {
      props: { open: true, teamConfig: config, projectPath: '/projects/taurhaus' },
    })

    await fireEvent.click(await screen.findByTestId('team-customizer-save-preset-trigger'))
    await fireEvent.input(screen.getByTestId('save-preset-name-input'), {
      target: { value: 'Unknown Tools' },
    })
    await fireEvent.click(screen.getByTestId('save-preset-confirm'))

    expect(ipc.upsertTeamPreset).toHaveBeenCalledWith(
      expect.objectContaining({
        agentSlots: [
          expect.objectContaining({ roleId: 'codex-developer' }),
          expect.objectContaining({ roleId: 'codex-developer' }),
        ],
      })
    )
  })

  // Regression: b345de1 (PR 5c) taught this panel to edit the model and the
  // reasoning effort but still serialized every slot with `overrides: null`, so
  // reloading a saved preset restored the role defaults and threw the selection
  // away.
  it('carries an edited model and effort into the slot overrides', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
        modelCatalog: TEST_MODEL_CATALOG,
      },
    })

    await fireEvent.click(await screen.findByTestId('team-customizer-agent-agent-1-edit'))
    await fireEvent.change(screen.getByTestId('team-customizer-agent-agent-1-model-select'), {
      target: { value: 'gpt-5.6-terra' },
    })
    await fireEvent.change(
      screen.getByTestId('team-customizer-agent-agent-1-model-select-effort'),
      { target: { value: 'xhigh' } }
    )
    await fireEvent.click(screen.getByTestId('team-customizer-agent-agent-1-save'))

    await fireEvent.click(await screen.findByTestId('team-customizer-save-preset-trigger'))
    await fireEvent.input(screen.getByTestId('save-preset-name-input'), {
      target: { value: 'Edited Team' },
    })
    await fireEvent.click(screen.getByTestId('save-preset-confirm'))

    await waitFor(() => {
      expect(ipc.upsertTeamPreset).toHaveBeenCalledWith(
        expect.objectContaining({
          agentSlots: [
            expect.objectContaining({
              overrides: { model: 'gpt-5.6-terra', reasoningEffort: 'xhigh' },
            }),
          ],
        })
      )
    })
  })

  it('preserves an explicit non-Claude lead role id when saving a preset', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: {
          ...baseTeamConfig(),
          lead: {
            ...baseTeamConfig().lead,
            tool: 'agy',
            model: 'gemini-3.7-flash-high',
            roleId: 'antigravity-orchestrator',
          },
        },
        projectPath: '/projects/taurhaus',
      },
    })

    await fireEvent.click(await screen.findByTestId('team-customizer-save-preset-trigger'))
    await fireEvent.input(screen.getByTestId('save-preset-name-input'), { target: { value: 'Antigravity Team' } })
    await fireEvent.click(screen.getByTestId('save-preset-confirm'))

    expect(ipc.upsertTeamPreset).toHaveBeenCalledWith(
      expect.objectContaining({
        leadRoleId: 'antigravity-orchestrator',
      })
    )
  })

  it('shows danger feedback class when save fails', async () => {
    ipc.upsertTeamPreset.mockRejectedValueOnce(new Error('save failed'))

    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
      },
    })

    await fireEvent.click(await screen.findByTestId('team-customizer-save-preset-trigger'))
    await fireEvent.input(screen.getByTestId('save-preset-name-input'), { target: { value: 'Broken Preset' } })
    await fireEvent.click(screen.getByTestId('save-preset-confirm'))

    await waitFor(() => {
      expect(screen.getByTestId('save-preset-feedback')).toHaveClass('text-danger-500')
    })
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
