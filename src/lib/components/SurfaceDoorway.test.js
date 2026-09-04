import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import { createRawSnippet } from 'svelte'

import SurfaceDoorway from './SurfaceDoorway.svelte'
import { RAIL_ICONS } from '../railIcons.js'

describe('SurfaceDoorway', () => {
  it('renders the key echo, labeled back affordance, and Esc hint', async () => {
    const onBack = vi.fn()
    render(SurfaceDoorway, {
      props: {
        title: 'Settings',
        icon: RAIL_ICONS.settings,
        backTestid: 'settings-back',
        onBack,
      },
    })

    const doorway = screen.getByTestId('surface-doorway')
    expect(doorway).toBeInTheDocument()
    // Key echo: the exact icon+name key that opened the surface.
    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument()
    expect(doorway.querySelector('path')?.getAttribute('d')).toBe('M15.75 19.5 8.25 12l7.5-7.5')
    expect(doorway.querySelectorAll('svg').length).toBe(2)
    // Labeled back: chevron + "Back", and the caller's testid survives.
    const back = screen.getByTestId('settings-back')
    expect(back).toHaveTextContent('Back')
    await fireEvent.click(back)
    expect(onBack).toHaveBeenCalled()
    // The Esc hint is part of the doorway grammar.
    expect(doorway.querySelector('kbd')).toHaveTextContent('Esc')
  })

  it('renders the optional meta slot and ghost action', () => {
    render(SurfaceDoorway, {
      props: {
        title: 'Accounts',
        icon: RAIL_ICONS.accounts,
        meta: createRawSnippet(() => ({
          render: () => '<span data-testid="doorway-meta">Usage as of 14:32</span>',
        })),
        action: createRawSnippet(() => ({
          render: () => '<button data-testid="doorway-action">Refresh</button>',
        })),
      },
    })

    expect(screen.getByTestId('doorway-meta')).toHaveTextContent('Usage as of 14:32')
    expect(screen.getByTestId('doorway-action')).toHaveTextContent('Refresh')
  })
})
