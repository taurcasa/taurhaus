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

  it('hydrates a default lead from selected role context when opened without team config', async () => {
    const onSave = vi.fn()

    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: null,
        projectPath: '/projects/taurhaus',
        context: {
          selectedRole: {
            roleId: 'codex-product-lead',
            name: 'Codex Product Lead',
            cliTool: 'codex',
            model: 'gpt-5.4 high',
          },
        },
        onSave,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-lead')).toBeInTheDocument()
      expect(screen.getByTestId('team-customizer-lead-tool-model')).toHaveTextContent('Codex')
    })

    await fireEvent.input(screen.getByTestId('team-customizer-name-input'), {
      target: { value: 'taurhaus-team' },
    })
    await fireEvent.click(screen.getByTestId('team-customizer-save'))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        teamName: 'taurhaus-team',
        lead: expect.objectContaining({
          name: 'team-lead',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          roleId: 'codex-product-lead',
        }),
      })
    )
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

  it('preserves a non-Claude lead selection when saving', async () => {
    const onSave = vi.fn()

    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: {
          ...baseTeamConfig(),
          lead: {
            ...baseTeamConfig().lead,
            tool: 'codex',
            model: 'gpt-5.4 high',
            roleId: 'codex-orchestrator',
          },
        },
        projectPath: '/projects/taurhaus',
        onSave,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-save')).toBeEnabled()
    })

    await fireEvent.click(screen.getByTestId('team-customizer-save'))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        lead: expect.objectContaining({
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          roleId: 'codex-orchestrator',
        }),
      })
    )
  })

  it('rejects case-insensitive duplicate member names and disables save', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-save')).toBeEnabled()
    })

    await fireEvent.click(screen.getByTestId('team-customizer-lead-edit'))
    await fireEvent.input(screen.getByTestId('team-customizer-lead-name-input'), {
      target: { value: '  DEV-1  ' },
    })
    await fireEvent.click(screen.getByTestId('team-customizer-lead-save'))

    await waitFor(() => {
      expect(screen.getByTestId('validation-bar-list')).toBeInTheDocument()
      expect(screen.getByTestId('validation-bar-error-badge')).toHaveTextContent('1 error')
      expect(screen.getByTestId('validation-bar-list')).toHaveTextContent('Duplicate member name.')
      expect(screen.getByTestId('team-customizer-save')).toBeDisabled()
    })
  })

  it('rejects whitespace-only lead name and renders validation error', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
      },
    })

    await fireEvent.click(screen.getByTestId('team-customizer-lead-edit'))
    await fireEvent.input(screen.getByTestId('team-customizer-lead-name-input'), {
      target: { value: '   ' },
    })
    await fireEvent.click(screen.getByTestId('team-customizer-lead-save'))

    await waitFor(() => {
      expect(screen.getByTestId('validation-bar-list')).toBeInTheDocument()
      expect(screen.getByTestId('validation-bar-list')).toHaveTextContent('Lead')
      expect(screen.getByTestId('validation-bar-list')).toHaveTextContent('Lead name is required.')
    })
  })

  it('keeps save disabled until validation issues are fixed', async () => {
    render(TeamCustomizerPanel, {
      props: {
        open: true,
        teamConfig: baseTeamConfig(),
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-save')).toBeEnabled()
    })

    await fireEvent.click(screen.getByTestId('team-customizer-lead-edit'))
    await fireEvent.input(screen.getByTestId('team-customizer-lead-name-input'), {
      target: { value: 'dev-1' },
    })
    await fireEvent.click(screen.getByTestId('team-customizer-lead-save'))

    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-save')).toBeDisabled()
    })

    await fireEvent.click(screen.getByTestId('team-customizer-lead-edit'))
    await fireEvent.input(screen.getByTestId('team-customizer-lead-name-input'), {
      target: { value: 'team-lead' },
    })
    await fireEvent.click(screen.getByTestId('team-customizer-lead-save'))

    await waitFor(() => {
      expect(screen.queryByTestId('validation-bar-list')).not.toBeInTheDocument()
      expect(screen.getByTestId('team-customizer-save')).toBeEnabled()
    })
  })
})
