import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  checkMeshInstallStatus: vi.fn(),
  coordinationPreflightCheck: vi.fn(),
  installMesh: vi.fn(),
}))

const { checkMeshInstallStatus, coordinationPreflightCheck, installMesh } = await import('../ipc.js')
import MeshAvailabilityGateHarness from './MeshAvailabilityGateHarness.svelte'

describe('MeshAvailabilityGate', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    checkMeshInstallStatus.mockResolvedValue({
      installed: true,
      version: '0.1.0',
      bundled_version: '0.1.0',
      needs_update: false,
      environment_available: true,
      error: null,
    })
    installMesh.mockResolvedValue('Mesh installed successfully: mesh 0.1.0')
  })

  it('shows setup form content when all required tools are available', async () => {
    coordinationPreflightCheck.mockResolvedValueOnce({
      canInitialize: true,
      blockingErrors: [],
      agentWarnings: [],
    })

    render(MeshAvailabilityGateHarness, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-gate-child')).toBeInTheDocument()
    })
    expect(screen.queryByTestId('mesh-availability-blocking')).not.toBeInTheDocument()
  })

  it('shows mesh-missing guidance when mesh binary is not found', async () => {
    coordinationPreflightCheck.mockResolvedValueOnce({
      canInitialize: false,
      blockingErrors: ['Mesh CLI not found. Install it to enable multi-agent collaboration.'],
      agentWarnings: [],
    })

    render(MeshAvailabilityGateHarness, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-availability-blocking')).toBeInTheDocument()
    })
    expect(screen.getByText('Mesh CLI not found. Install it to enable multi-agent collaboration.')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-availability-mesh-help')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-install-button')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-gate-child')).not.toBeInTheDocument()
  })

  it('shows tmux-missing message when tmux is not found', async () => {
    coordinationPreflightCheck.mockResolvedValueOnce({
      canInitialize: false,
      blockingErrors: ['tmux is required for multi-agent sessions.'],
      agentWarnings: [],
    })

    render(MeshAvailabilityGateHarness, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-availability-blocking')).toBeInTheDocument()
    })
    expect(screen.getByText('tmux is required for multi-agent sessions.')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-availability-mesh-help')).not.toBeInTheDocument()
    expect(screen.queryByTestId('mesh-install-button')).not.toBeInTheDocument()
  })

  it('passes agent warnings to children through slot props', async () => {
    coordinationPreflightCheck.mockResolvedValueOnce({
      canInitialize: true,
      blockingErrors: [],
      agentWarnings: [
        {
          agentName: 'codex-check',
          cliTool: 'codex',
          message: "Codex CLI not found - agent 'codex-check' cannot be launched.",
        },
      ],
    })

    render(MeshAvailabilityGateHarness, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        captureWarnings: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-gate-warning-count')).toHaveTextContent('1')
    })
    expect(screen.getByTestId('mesh-gate-warning-message')).toHaveTextContent(
      "Codex CLI not found - agent 'codex-check' cannot be launched."
    )
  })

  it('surfaces version mismatch and recovers after installing bundled mesh', async () => {
    checkMeshInstallStatus
      .mockResolvedValueOnce({
        installed: true,
        version: '0.0.9',
        bundled_version: '0.1.0',
        needs_update: true,
        environment_available: true,
        error: null,
      })
      .mockResolvedValueOnce({
        installed: true,
        version: '0.1.0',
        bundled_version: '0.1.0',
        needs_update: false,
        environment_available: true,
        error: null,
      })
    coordinationPreflightCheck
      .mockResolvedValueOnce({
        canInitialize: true,
        blockingErrors: [],
        agentWarnings: [],
      })
      .mockResolvedValueOnce({
        canInitialize: true,
        blockingErrors: [],
        agentWarnings: [],
      })
    installMesh.mockResolvedValueOnce('Mesh installed successfully: mesh 0.1.0')

    render(MeshAvailabilityGateHarness, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-availability-blocking')).toBeInTheDocument()
    })
    expect(screen.getByTestId('mesh-install-button')).toBeInTheDocument()
    expect(screen.getByText(/taurhaus requires 0.1.0/i)).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-install-button'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-gate-child')).toBeInTheDocument()
    })
    expect(installMesh).toHaveBeenCalledTimes(1)
  })
})
