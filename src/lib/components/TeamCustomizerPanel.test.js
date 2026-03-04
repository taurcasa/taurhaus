import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  listRoleTemplates: vi.fn(),
  composeTeam: vi.fn(),
}))

const { listRoleTemplates, composeTeam } = await import('../ipc.js')

import TeamCustomizerPanel from './TeamCustomizerPanel.svelte'

function baseTeamConfig() {
  return {
    teamName: 'taurhaus-team',
    description: 'Mesh team config',
    presetId: 'fullstack-dev',
    composition: {
      presetId: 'fullstack-dev',
      leadRoleId: 'claude-orchestrator',
      agentSlots: [{ roleId: 'codex-developer', count: 1 }],
    },
    lead: {
      name: 'team-lead',
      tool: 'claude',
      model: 'opus',
      projectId: '/projects/taurhaus',
      description: 'Lead agent',
    },
    agents: [
      {
        name: 'dev-1',
        tool: 'codex',
        model: 'gpt-5.3-codex',
        projectId: '/projects/taurhaus',
        description: 'Agent one',
      },
    ],
  }
}

function mockCompositionResponse() {
  return {
    roster: [
      {
        name: 'lead-project',
        roleId: 'claude-orchestrator',
        roleKind: 'lead',
        cliTool: 'claude',
        model: 'claude-opus-4-6',
        instructions: 'Lead instructions',
        projectBinding: 'lead_project',
        projectId: '/projects/taurhaus',
      },
      {
        name: 'dev-1',
        roleId: 'codex-developer',
        roleKind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.3-codex',
        instructions: 'Agent instructions',
        projectBinding: 'lead_project',
        projectId: '/projects/taurhaus',
      },
    ],
    warnings: [],
    validationErrors: [],
  }
}

describe('TeamCustomizerPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()

    listRoleTemplates.mockResolvedValue([
      {
        roleId: 'claude-orchestrator',
        name: 'Claude Orchestrator',
        kind: 'lead',
        cliTool: 'claude',
        model: 'claude-opus-4-6',
        capabilities: ['planning'],
      },
      {
        roleId: 'codex-developer',
        name: 'Codex Developer',
        kind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.3-codex',
        capabilities: ['implementation'],
      },
    ])

    composeTeam.mockResolvedValue(mockCompositionResponse())
  })

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
    expect(screen.getByTestId('team-composer')).toBeInTheDocument()
    expect(screen.getByTestId('team-customizer-reset')).toBeInTheDocument()
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
      expect(screen.getByTestId('composer-apply')).not.toBeDisabled()
    })

    await fireEvent.click(screen.getByTestId('composer-apply'))
    expect(onSave).toHaveBeenCalledTimes(1)
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        lead: expect.objectContaining({
          name: 'lead-project',
        }),
      })
    )

    await fireEvent.click(screen.getByTestId('team-customizer-reset'))
    expect(onReset).toHaveBeenCalledTimes(1)
  })
})
