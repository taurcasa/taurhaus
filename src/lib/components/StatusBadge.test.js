import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import StatusBadge from './StatusBadge.svelte'

describe('StatusBadge', () => {
  it('renders correct color class for active/idle/offline', () => {
    const active = render(StatusBadge, { props: { status: 'active' } })
    expect(screen.getByTestId('status-badge-active').className).toContain('bg-success-400')
    active.unmount()

    const idle = render(StatusBadge, { props: { status: 'idle' } })
    expect(screen.getByTestId('status-badge-idle').className).toContain('bg-warning-400')
    idle.unmount()

    render(StatusBadge, { props: { status: 'offline' } })
    expect(screen.getByTestId('status-badge-offline').className).toContain('bg-zinc-500')
  })

  it('sm and md sizes render different dimensions', () => {
    const sm = render(StatusBadge, { props: { status: 'idle', size: 'sm' } })
    expect(screen.getByTestId('status-badge-idle').className).toContain('h-1.5')
    expect(screen.getByTestId('status-badge-idle').className).toContain('w-1.5')
    sm.unmount()

    render(StatusBadge, { props: { status: 'idle', size: 'md' } })
    expect(screen.getByTestId('status-badge-idle').className).toContain('h-2')
    expect(screen.getByTestId('status-badge-idle').className).toContain('w-2')
  })

  it('active status has animation class', () => {
    render(StatusBadge, { props: { status: 'active' } })
    const badge = screen.getByTestId('status-badge-active')
    expect(badge.className).toContain('activepulse')
  })
})
