import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshRuntimeBar from './MeshRuntimeBar.svelte'

const agents = [
  { id: 'a1', status: 'active' },
  { id: 'a2', status: 'active' },
  { id: 'a3', status: 'idle' },
  { id: 'a4', status: 'offline' },
]

describe('MeshRuntimeBar', () => {
  it('shows team name and status summary counts', () => {
    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        agents,
      },
    })

    expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    expect(screen.getByTestId('mesh-runtime-summary')).toHaveTextContent('2 active, 1 idle, 1 offline')
  })

  it('calls onAddAgent when add button is clicked', async () => {
    const onAddAgent = vi.fn()

    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        agents,
        onAddAgent,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-add-agent'))
    expect(onAddAgent).toHaveBeenCalledTimes(1)
  })

  it('shows overflow menu and disband callback', async () => {
    const onDisband = vi.fn()

    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        agents,
        onDisband,
      },
    })

    expect(screen.queryByTestId('mesh-runtime-disband')).not.toBeInTheDocument()
    await fireEvent.click(screen.getByTestId('mesh-runtime-overflow-button'))
    expect(screen.getByTestId('mesh-runtime-disband')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-runtime-disband'))
    expect(onDisband).toHaveBeenCalledTimes(1)
  })
})
