import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshActionBar from './MeshActionBar.svelte'

describe('MeshActionBar', () => {
  it('shows the provided team name', () => {
    render(MeshActionBar, {
      props: {
        teamName: 'taurhaus-team',
      },
    })

    expect(screen.getByTestId('mesh-action-team-name')).toHaveTextContent('taurhaus-team')
  })

  it('initialize is disabled when canInitialize is false', () => {
    render(MeshActionBar, {
      props: {
        canInitialize: false,
      },
    })

    expect(screen.getByTestId('mesh-action-initialize')).toBeDisabled()
  })

  it('initialize is enabled when canInitialize is true', () => {
    render(MeshActionBar, {
      props: {
        canInitialize: true,
      },
    })

    expect(screen.getByTestId('mesh-action-initialize')).not.toBeDisabled()
  })

  it('calls initialize/customize/reset callbacks', async () => {
    const onInitialize = vi.fn()
    const onOpenCustomizer = vi.fn()
    const onReset = vi.fn()

    render(MeshActionBar, {
      props: {
        canInitialize: true,
        onInitialize,
        onOpenCustomizer,
        onReset,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-action-customize'))
    await fireEvent.click(screen.getByTestId('mesh-action-initialize'))
    await fireEvent.click(screen.getByTestId('mesh-action-reset'))

    expect(onOpenCustomizer).toHaveBeenCalledTimes(1)
    expect(onInitialize).toHaveBeenCalledTimes(1)
    expect(onReset).toHaveBeenCalledTimes(1)
  })
})
