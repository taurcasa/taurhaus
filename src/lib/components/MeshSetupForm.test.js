import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshSetupForm from './MeshSetupForm.svelte'

async function openCustomize() {
  await fireEvent.click(screen.getByTestId('mesh-advanced-toggle'))
  await waitFor(() => {
    expect(screen.getByTestId('mesh-team-basics')).toBeInTheDocument()
  })
}

describe('MeshSetupForm', () => {
  const availableProjects = [
    { id: 'proj-web', name: 'Web UI' },
    { id: 'proj-api', name: 'API Service' },
  ]

  beforeEach(() => {
    localStorage.clear()
  })

  it('shows roster-style team preview with lead and default agent', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getByTestId('mesh-roster-preview')).toBeInTheDocument()
    expect(screen.getByText('You')).toBeInTheDocument()
    expect(screen.getByText('Lead')).toBeInTheDocument()
    expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(1)
    expect(screen.queryByTestId('mesh-lead-card')).not.toBeInTheDocument()
    expect(screen.queryByTestId('mesh-review-panel')).not.toBeInTheDocument()
  })

  it('agent rows are always visible without opening customize', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(1)
    expect(screen.getByTestId('mesh-agent-name-input-0')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-agent-tool-select-0')).toBeInTheDocument()
  })

  it('add agent creates a new agent card', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(1)
    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(2)
  })

  it('renders onboarding banner and allows dismissing it', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getByTestId('mesh-onboarding-banner')).toBeInTheDocument()
    await fireEvent.click(screen.getByTestId('mesh-onboarding-dismiss'))

    await waitFor(() => {
      expect(screen.queryByTestId('mesh-onboarding-banner')).not.toBeInTheDocument()
    })
    expect(localStorage.getItem('mesh-onboarding-dismissed')).toBe('true')
  })

  it('shows plain tool labels without icon prefixes', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    const select = screen.getByTestId('mesh-agent-tool-select-0')
    const labels = Array.from(select.querySelectorAll('option')).map((option) => option.textContent)
    expect(labels).toEqual(['Claude', 'Codex', 'Gemini'])
  })

  it('customize toggle reveals team name and description fields', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.queryByTestId('mesh-team-name-input')).not.toBeInTheDocument()
    await openCustomize()
    const description = screen.getByTestId('mesh-team-description-input')
    expect(description.tagName).toBe('INPUT')
    expect(description).toHaveAttribute('placeholder', "Optional — describe the team's purpose")
  })

  it('auto-selects project when current project matches an option', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects: [
          { id: '/projects/taurhaus', name: 'taurhaus' },
          { id: '/projects/other', name: 'Other' },
        ],
      },
    })

    expect(screen.getByTestId('mesh-agent-project-select-0')).toHaveValue('/projects/taurhaus')

    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    expect(screen.getByTestId('mesh-agent-project-select-1')).toHaveValue('/projects/taurhaus')
  })

  it('auto-selects project when only one project exists', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects: [{ id: 'proj-only', name: 'Only Project' }],
      },
    })

    expect(screen.getByTestId('mesh-agent-project-select-0')).toHaveValue('proj-only')
  })

  it('all agent rows have border-t separator', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    const cards = screen.getAllByTestId('mesh-agent-card')
    expect(cards[0].className).toContain('border-t')
    expect(cards[1].className).toContain('border-t')
  })

  it('surfaces multiple warnings as one subtle banner', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
        preflightWarnings: [{ message: 'MESH_DAEMON_NOT_RUNNING' }, { message: 'TMUX_MISSING' }],
      },
    })

    expect(screen.getByTestId('mesh-setup-warnings')).toHaveTextContent(
      'Some tools may need installation. You can still start \u2014 agents will report issues.'
    )
  })

  it('duplicate name shows inline error', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await fireEvent.input(screen.getByTestId('mesh-agent-name-input-0'), {
      target: { value: 'dupe' },
    })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    await fireEvent.input(screen.getByTestId('mesh-agent-name-input-1'), {
      target: { value: 'dupe' },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-duplicate-name-error')).toBeInTheDocument()
    })
  })

  it('start team button is always enabled with defaults', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getByTestId('mesh-create-team-button')).not.toBeDisabled()
  })

  it('infers team name from UNC project path', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '\\\\wsl$\\Ubuntu\\home\\mstie\\projects\\taurhaus',
        availableProjects,
      },
    })

    await openCustomize()
    await waitFor(() => {
      expect(screen.getByTestId('mesh-team-name-input')).toHaveValue('taurhaus-team')
    })
  })

  it('start team emits expected payload with auto-generated agent name', async () => {
    const onInitialize = vi.fn()
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
        oninitialize: onInitialize,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-create-team-button'))

    expect(onInitialize).toHaveBeenCalledTimes(1)
    expect(onInitialize).toHaveBeenCalledWith({
      teamName: 'taurhaus-team',
      teamDescription: null,
      leadMode: 'launch_new',
      lead: {
        name: 'team-lead',
        cliTool: 'claude',
        model: 'opus',
        projectId: '/projects/taurhaus',
        description: 'Team lead',
      },
      agents: [
        {
          name: 'taurhaus-dev',
          cliTool: 'codex',
          model: 'gpt-5.3',
          projectId: '/projects/taurhaus',
          description: null,
        },
      ],
    })
  })

  it('initialize emits correct payload with customized agent', async () => {
    const onInitialize = vi.fn()
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
        oninitialize: onInitialize,
      },
    })

    await fireEvent.input(screen.getByTestId('mesh-agent-name-input-0'), {
      target: { value: 'frontend-dev' },
    })
    await fireEvent.change(screen.getByTestId('mesh-agent-project-select-0'), {
      target: { value: 'proj-web' },
    })

    await fireEvent.click(screen.getByTestId('mesh-create-team-button'))

    expect(onInitialize).toHaveBeenCalledTimes(1)
    expect(onInitialize).toHaveBeenCalledWith(
      expect.objectContaining({
        teamName: 'taurhaus-team',
        teamDescription: null,
        leadMode: 'launch_new',
        lead: expect.objectContaining({
          name: 'team-lead',
          cliTool: 'claude',
          model: 'opus',
          projectId: '/projects/taurhaus',
        }),
        agents: [
          expect.objectContaining({
            name: 'frontend-dev',
            cliTool: 'codex',
            model: 'gpt-5.3',
            projectId: 'proj-web',
          }),
        ],
      })
    )
  })
})
