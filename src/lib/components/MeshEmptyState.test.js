import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshEmptyState from './MeshEmptyState.svelte'

describe('MeshEmptyState', () => {
  const presets = [
    {
      presetId: 'fullstack-dev',
      name: 'Fullstack Dev',
      description: 'Lead + implementation and review agents.',
      leadCount: 1,
      agentCount: 3,
      tools: ['claude', 'codex'],
      builtIn: true,
    },
    {
      presetId: 'research-dev',
      name: 'Research Dev',
      description: 'Research-focused team composition.',
      leadCount: 1,
      agentCount: 2,
      tools: ['gemini'],
      builtIn: true,
    },
  ]

  it('renders preset cards when presets are provided', () => {
    render(MeshEmptyState, {
      props: {
        presets,
      },
    })

    expect(screen.getByTestId('mesh-empty-state')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-template-preset-fullstack-dev')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-template-preset-research-dev')).toBeInTheDocument()
  })

  it('calls onSelectPreset when a preset card is clicked', async () => {
    const onSelectPreset = vi.fn()
    render(MeshEmptyState, {
      props: {
        presets,
        onSelectPreset,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-fullstack-dev'))
    expect(onSelectPreset).toHaveBeenCalledTimes(1)
    expect(onSelectPreset).toHaveBeenCalledWith(
      expect.objectContaining({ presetId: 'fullstack-dev' })
    )
  })

  it('shows browse and scratch actions', () => {
    render(MeshEmptyState, {
      props: {
        presets,
      },
    })

    expect(screen.getByRole('button', { name: 'Browse Catalog' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Build Custom' })).toBeInTheDocument()
  })

  it('renders browse and scratch actions when presets are empty', () => {
    render(MeshEmptyState, {
      props: {
        presets: [],
      },
    })

    expect(screen.queryByTestId('mesh-template-preset-fullstack-dev')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Browse Catalog' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Build Custom' })).toBeInTheDocument()
  })
})
