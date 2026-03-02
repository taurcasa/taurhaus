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
