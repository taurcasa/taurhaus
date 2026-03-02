import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  coordinationInitializeTeam: vi.fn(),
  onCoordinationStepProgress: vi.fn(),
}))

const { coordinationInitializeTeam, onCoordinationStepProgress } = await import('../ipc.js')

import MeshInitProgress from './MeshInitProgress.svelte'

function deferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('MeshInitProgress', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    onCoordinationStepProgress.mockResolvedValue(() => {})
  })

  it('renders pending steps initially', async () => {
    const pending = deferred()
    coordinationInitializeTeam.mockReturnValueOnce(pending.promise)

    render(MeshInitProgress, {
      props: {
        dark: false,
        request: { teamName: 'arch-team' },
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-progress')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-init-icon-validate_configuration')).toHaveTextContent('○')
  })

  it('updates step status on progress event', async () => {
    const pending = deferred()
    coordinationInitializeTeam.mockReturnValueOnce(pending.promise)
    let progressHandler = null
    onCoordinationStepProgress.mockImplementationOnce(async (callback) => {
      progressHandler = callback
      return () => {}
    })

    render(MeshInitProgress, {
      props: {
        dark: false,
        request: { teamName: 'arch-team' },
      },
    })

    await waitFor(() => {
      expect(progressHandler).toBeTypeOf('function')
    })

    progressHandler({
      payload: {
        teamName: 'arch-team',
        operation: 'initialize_team',
        progress: {
          step: 'validate_configuration',
          status: 'running',
          message: null,
        },
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-icon-validate_configuration')).toHaveTextContent('●')
    })
  })

  it('shows success state when all steps succeed', async () => {
    coordinationInitializeTeam.mockResolvedValueOnce({
      teamName: 'arch-team',
      failedStep: null,
      retryable: false,
      message: 'team initialized',
      steps: [
        { step: 'validate_configuration', status: 'succeeded', message: 'ok' },
        { step: 'create_team', status: 'succeeded', message: 'ok' },
      ],
    })

    render(MeshInitProgress, {
      props: {
        dark: false,
        request: { teamName: 'arch-team' },
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-success')).toHaveTextContent('Team initialized!')
    })
    expect(screen.getByTestId('mesh-init-runtime-button')).toBeInTheDocument()
  })

  it('shows failure with retry button when a step fails', async () => {
    coordinationInitializeTeam.mockResolvedValueOnce({
      teamName: 'arch-team',
      failedStep: 'create_team',
      retryable: true,
      message: 'team already exists',
      steps: [
        { step: 'validate_configuration', status: 'succeeded', message: 'ok' },
        { step: 'create_team', status: 'failed', message: 'conflict' },
      ],
    })

    render(MeshInitProgress, {
      props: {
        dark: false,
        request: { teamName: 'arch-team' },
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-failure')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-init-retry-button')).toBeInTheDocument()
  })

  it('retry button triggers re-initialization', async () => {
    coordinationInitializeTeam
      .mockResolvedValueOnce({
        teamName: 'arch-team',
        failedStep: 'create_team',
        retryable: true,
        message: 'boom',
        steps: [{ step: 'create_team', status: 'failed', message: 'boom' }],
      })
      .mockResolvedValueOnce({
        teamName: 'arch-team',
        failedStep: null,
        retryable: false,
        message: 'ok',
        steps: [{ step: 'create_team', status: 'succeeded', message: 'ok' }],
      })

    render(MeshInitProgress, {
      props: {
        dark: false,
        request: { teamName: 'arch-team' },
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-retry-button')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-init-retry-button'))

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalledTimes(2)
    })
  })
})
