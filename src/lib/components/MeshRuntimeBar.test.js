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

  it('supports keyboard and outside-close behavior for the More menu', async () => {
    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        lead: { id: 'lead', status: 'active' },
        agents,
        teamRuntimeState: 'active',
      },
    })

    const toggle = screen.getByTestId('mesh-runtime-more-toggle')
    toggle.focus()

    await fireEvent.keyDown(toggle, { key: 'ArrowDown' })

    const disbandItem = screen.getByRole('menuitem', { name: 'Disband Team...' })
    expect(disbandItem).toHaveFocus()

    await fireEvent.keyDown(disbandItem, { key: 'Escape' })
    expect(screen.queryByTestId('mesh-runtime-more-menu')).not.toBeInTheDocument()
    expect(toggle).toHaveFocus()

    await fireEvent.click(toggle)
    expect(screen.getByRole('menuitem', { name: 'Stop All Members' })).toBeDisabled()
    await fireEvent.mouseDown(document.body)
    expect(screen.queryByTestId('mesh-runtime-more-menu')).not.toBeInTheDocument()
  })

  it('uses stopped copy and resume-first actions for non-active teams', () => {
    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        lead: { id: 'lead', status: 'stopped' },
        agents: [{ id: 'a1', status: 'terminated' }],
        teamRuntimeState: 'coldResume',
      },
    })

    expect(screen.getByTestId('mesh-runtime-summary-line')).toHaveTextContent(
      '2 members • 0 active • 2 stopped'
    )
    expect(screen.getByTestId('mesh-runtime-state-copy')).toHaveTextContent('All members stopped')
    expect(screen.getByTestId('mesh-runtime-primary-action')).toHaveTextContent('Resume Team')
    expect(screen.queryByTestId('mesh-runtime-add-agent')).not.toBeInTheDocument()
  })

  it('uses stopped wording for degraded teams', () => {
    render(MeshRuntimeBar, {
      props: {
        teamName: 'architecture-final',
        lead: { id: 'lead', status: 'active' },
        agents: [{ id: 'a1', status: 'stopped' }],
        teamRuntimeState: 'degraded',
      },
    })

    expect(screen.getByTestId('mesh-runtime-state-copy')).toHaveTextContent('1 member stopped')
    expect(screen.getByTestId('mesh-runtime-primary-action')).toHaveTextContent('Resume Stopped (1)')
    expect(screen.queryByTestId('mesh-runtime-add-agent')).not.toBeInTheDocument()
  })
})
