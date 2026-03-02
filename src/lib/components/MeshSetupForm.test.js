import { describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshSetupForm from './MeshSetupForm.svelte'

describe('MeshSetupForm', () => {
  const availableProjects = [
    { id: 'proj-web', name: 'Web UI' },
    { id: 'proj-api', name: 'API Service' },
  ]

  it('renders lead card with Claude fixed', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-lead-card')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-lead-tool-fixed')).toHaveTextContent('Claude')
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

  it('uses a single-line input for team description', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    const description = screen.getByTestId('mesh-team-description-input')
    expect(description.tagName).toBe('INPUT')
    expect(description).toHaveAttribute('placeholder', "Optional — describe the team's purpose")
  })

  it('auto-selects project only when exactly one project exists', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects: [{ id: 'proj-only', name: 'Only Project' }],
      },
    })

    expect(screen.getByTestId('mesh-agent-project-select-0')).toHaveValue('proj-only')

    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    expect(screen.getByTestId('mesh-agent-project-select-1')).toHaveValue('proj-only')
  })

  it('renders separator border on agent cards after the first', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    const cards = screen.getAllByTestId('mesh-agent-card')
    expect(cards[0].className).not.toContain('border-t')
    expect(cards[1].className).toContain('border-t')
  })

  it('quick-add buttons use compact labels and tooltip descriptions', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    const frontendQuickAdd = screen.getByTestId('mesh-quick-add-frontend')
    expect(frontendQuickAdd).toHaveTextContent(/^Frontend$/)
    expect(frontendQuickAdd).toHaveAttribute('title', 'Owns UI implementation')
  })

  it('review panel lists agent names with selected tools', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await fireEvent.input(screen.getByTestId('mesh-agent-name-input-0'), {
      target: { value: 'frontend-dev' },
    })
    await fireEvent.change(screen.getByTestId('mesh-agent-tool-select-0'), {
      target: { value: 'gemini' },
    })

    expect(screen.getByTestId('mesh-review-agents-detail')).toHaveTextContent('frontend-dev (Gemini)')
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

  it('initialize button is disabled when required fields are empty', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getByTestId('mesh-create-team-button')).toBeDisabled()
  })

  it('initialize emits correct payload shape', async () => {
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

    const button = screen.getByTestId('mesh-create-team-button')
    await waitFor(() => {
      expect(button).not.toBeDisabled()
    })

    await fireEvent.click(button)

    expect(onInitialize).toHaveBeenCalledTimes(1)
    expect(onInitialize).toHaveBeenCalledWith(
      expect.objectContaining({
        teamName: 'taurhaus-team',
        teamDescription: null,
        leadMode: 'attach_existing',
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
