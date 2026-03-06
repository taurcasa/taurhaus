import { render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import { describe, expect, it } from 'vitest'

import MeshConnection from './MeshConnection.svelte'

function explicitRoute(overrides = {}) {
  return {
    start: { x: 120, y: 84 },
    control1: { x: 132, y: 164 },
    control2: { x: 208, y: 244 },
    end: { x: 240, y: 324 },
    ...overrides,
  }
}

describe('MeshConnection', () => {
  it('renders a cubic bezier directly from explicit control points', () => {
    const route = explicitRoute()

    render(MeshConnection, {
      props: {
        ...route,
        status: 'active',
      },
    })

    const connection = screen.getByTestId('mesh-connection')
    expect(connection.getAttribute('d')).toBe(
      'M 120,84 C 132,164 208,244 240,324'
    )
  })

  it('does not let a legacy bend prop alter an explicit route', () => {
    const route = explicitRoute()

    render(MeshConnection, {
      props: {
        ...route,
        // Legacy callers may still pass this during migration; it must be ignored.
        bend: 999,
      },
    })

    expect(screen.getByTestId('mesh-connection').getAttribute('d')).toBe(
      'M 120,84 C 132,164 208,244 240,324'
    )
  })

  it('renders a straight vertical route as an explicit cubic path when control points are aligned', () => {
    render(MeshConnection, {
      props: {
        start: { x: 180, y: 96 },
        control1: { x: 180, y: 152 },
        control2: { x: 180, y: 208 },
        end: { x: 180, y: 264 },
      },
    })

    const connection = screen.getByTestId('mesh-connection')
    expect(connection.getAttribute('d')).toBe(
      'M 180,96 C 180,152 180,208 180,264'
    )
    expect(connection.getAttribute('d')).not.toContain(' L ')
  })

  it('preserves status styling and glow filter behavior', () => {
    render(MeshConnection, {
      props: {
        ...explicitRoute(),
        status: 'offline',
        dark: true,
        glowFilterId: 'mesh-glow',
      },
    })

    const connection = screen.getByTestId('mesh-connection')
    const style = connection.getAttribute('style') || ''
    const className = connection.getAttribute('class') || ''

    expect(className).toContain('mesh-connection-offline')
    expect(style).toContain('stroke-dasharray: 6,4')
    expect(style).toContain('opacity: 0.28')
    expect(style).toContain('filter: url("#mesh-glow")')
  })

  it('renders cross-project runtime connections as dashed and slightly dimmed', () => {
    render(MeshConnection, {
      props: {
        ...explicitRoute(),
        status: 'active',
        isCrossProject: true,
      },
    })

    const style = screen.getByTestId('mesh-connection').getAttribute('style') || ''
    expect(style).toContain('stroke-dasharray: 6,4')
    expect(style).toContain('opacity: 0.8')
  })
})
