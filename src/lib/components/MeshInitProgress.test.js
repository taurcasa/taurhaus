import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  coordinationDisbandTeam: vi.fn(),
  coordinationInitializeTeam: vi.fn(),
  onCoordinationStepProgress: vi.fn(),
}))

const {
  coordinationDisbandTeam,
  coordinationInitializeTeam,
  onCoordinationStepProgress,
} = await import('../ipc.js')

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
    coordinationDisbandTeam.mockResolvedValue({
      teamName: 'arch-team',
      disbanded: true,
      alreadyDisbanded: false,
      message: 'team disbanded',
    })
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
    expect(screen.getByTestId('mesh-init-step-validate_configuration')).toHaveTextContent(
      'Validating configuration'
    )
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
    expect(screen.getByTestId('mesh-init-desc-validate_configuration')).toHaveTextContent(
      'Checking team name, agent tools, and project assignments'
    )
  })

  it('shows elapsed seconds while initialization is running', async () => {
    vi.useFakeTimers()
    const pending = deferred()
    coordinationInitializeTeam.mockReturnValueOnce(pending.promise)

    render(MeshInitProgress, {
      props: {
        dark: false,
        request: { teamName: 'arch-team' },
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-elapsed')).toHaveTextContent('Elapsed: 0s')
    })

    vi.advanceTimersByTime(2000)

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-elapsed')).toHaveTextContent('Elapsed: 2s')
    })
    vi.useRealTimers()
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
    expect(screen.getByTestId('mesh-init-failure-details')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-init-open-existing-button')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-init-disband-existing-button')).toBeInTheDocument()
  })

  it('open existing team action invokes onSuccess with openedExisting flag', async () => {
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
    const onSuccess = vi.fn()

    render(MeshInitProgress, {
      props: {
        dark: false,
        request: { teamName: 'arch-team' },
        onSuccess,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-open-existing-button')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-init-open-existing-button'))

    expect(onSuccess).toHaveBeenCalledWith({
      teamName: 'arch-team',
      openedExisting: true,
    })
  })

  it('disband existing team action disbands then retries initialization', async () => {
    coordinationInitializeTeam
      .mockResolvedValueOnce({
        teamName: 'arch-team',
        failedStep: 'create_team',
        retryable: true,
        message: 'team already exists',
        steps: [{ step: 'create_team', status: 'failed', message: 'conflict' }],
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
      expect(screen.getByTestId('mesh-init-disband-existing-button')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-init-disband-existing-button'))
    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))

    await waitFor(() => {
      expect(coordinationDisbandTeam).toHaveBeenCalledWith('arch-team')
      expect(coordinationInitializeTeam).toHaveBeenCalledTimes(2)
    })
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

    const onRetry = vi.fn()
    render(MeshInitProgress, {
      props: {
        dark: false,
        request: { teamName: 'arch-team' },
        onRetry,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-retry-button')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('mesh-init-retry-button'))

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalledTimes(2)
    })
    expect(onRetry).toHaveBeenCalledTimes(1)
  })

  it('does not auto-run the same request again after remount', async () => {
    coordinationInitializeTeam.mockResolvedValueOnce({
      teamName: 'arch-team',
      failedStep: null,
      retryable: false,
      message: 'ok',
      steps: [{ step: 'create_team', status: 'succeeded', message: 'ok' }],
    })

    const request = { teamName: 'arch-team' }
    const first = render(MeshInitProgress, {
      props: {
        dark: false,
        request,
      },
    })

    await waitFor(() => {
      expect(coordinationInitializeTeam).toHaveBeenCalledTimes(1)
      expect(screen.getByTestId('mesh-init-success')).toBeInTheDocument()
    })

    first.unmount()

    render(MeshInitProgress, {
      props: {
        dark: false,
        request,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-init-progress')).toBeInTheDocument()
    })
    expect(coordinationInitializeTeam).toHaveBeenCalledTimes(1)
  })
})
