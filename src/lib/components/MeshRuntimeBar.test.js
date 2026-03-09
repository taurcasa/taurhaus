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
        lead: { id: 'lead', status: 'active' },
        agents,
        teamRuntimeState: 'degraded',
      },
    })

    expect(screen.getByTestId('mesh-runtime-title')).toHaveTextContent('architecture-final')
    expect(screen.getByTestId('mesh-runtime-summary-line')).toHaveTextContent(
      '5 members • 3 active • 1 idle • 1 stopped'
    )
  })

  it('calls onAddAgent when add button is clicked for active teams', async () => {
    const onAddAgent = vi.fn()

    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        lead: { id: 'lead', status: 'active' },
        agents,
        teamRuntimeState: 'active',
        onAddAgent,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-runtime-primary-action'))
    expect(onAddAgent).toHaveBeenCalledTimes(1)
  })

  it('shows disband inside the overflow menu, not as a primary action', async () => {
    const onDisband = vi.fn()

    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        lead: { id: 'lead', status: 'offline' },
        agents,
        teamRuntimeState: 'coldResume',
        onDisband,
      },
    })

    expect(screen.queryByTestId('mesh-runtime-disband')).not.toBeInTheDocument()
    await fireEvent.click(screen.getByTestId('mesh-runtime-more-toggle'))
    expect(screen.getByTestId('mesh-runtime-stop-all')).toBeDisabled()
    await fireEvent.click(screen.getByTestId('mesh-runtime-disband'))
    expect(onDisband).toHaveBeenCalledTimes(1)
  })
})
