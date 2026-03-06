import { beforeEach, describe, expect, it } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import MeshCacheHarness from './MeshCacheHarness.svelte'
import {
  clearMeshCache,
  getMeshCache,
  resetMeshCache,
  setMeshCache,
} from './meshCache.svelte.js'

describe('meshCache', () => {
  beforeEach(() => {
    resetMeshCache()
  })

  it('returns null for unknown projects and stores snapshots round-trip', () => {
    expect(getMeshCache('/projects/unknown')).toBeNull()

    const snapshot = {
      teamName: 'alpha-team',
      mode: 'runtime',
      members: [{ name: 'lead' }],
    }

    setMeshCache('/projects/alpha', snapshot)

    expect(getMeshCache('/projects/alpha')).toEqual(snapshot)
  })

  it('clear removes the cached snapshot', () => {
    setMeshCache('/projects/alpha', {
      teamName: 'alpha-team',
      mode: 'runtime',
    })
    expect(getMeshCache('/projects/alpha')).not.toBeNull()

    clearMeshCache('/projects/alpha')

    expect(getMeshCache('/projects/alpha')).toBeNull()
  })

  it('stores multiple projects independently', () => {
    setMeshCache('/projects/alpha', { teamName: 'alpha-team', mode: 'runtime' })
    setMeshCache('/projects/beta', { teamName: 'beta-team', mode: 'setup' })

    expect(getMeshCache('/projects/alpha')).toEqual({
      teamName: 'alpha-team',
      mode: 'runtime',
    })
    expect(getMeshCache('/projects/beta')).toEqual({
      teamName: 'beta-team',
      mode: 'setup',
    })
  })

  it('is reactive when components read a cached snapshot', async () => {
    render(MeshCacheHarness, {
      props: {
        projectPath: '/projects/alpha',
      },
    })

    expect(screen.getByTestId('mesh-cache-team-name')).toHaveTextContent('empty')

    setMeshCache('/projects/alpha', {
      teamName: 'alpha-team',
      mode: 'runtime',
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-cache-team-name')).toHaveTextContent('alpha-team')
    })
  })
})
