import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./ipc.js', () => ({
  scanDirectory: vi.fn(),
  registerProjectsBatch: vi.fn(),
  checkDaemonInstallStatus: vi.fn(),
  installDaemon: vi.fn(),
  getPlatform: vi.fn(),
  isTauri: vi.fn(() => false),
  openExternalUrl: vi.fn(),
}))

vi.mock('./DirectoryBrowser.svelte', () => ({
  default: function MockDirectoryBrowser(target) {
    const root = document.createElement('div')
    root.setAttribute('data-testid', 'mock-directory-browser')

    if (target.nodeType === Node.ELEMENT_NODE) {
      target.appendChild(root)
    } else {
      target.parentNode.insertBefore(root, target)
    }

    return {
      $destroy() {
        root.remove()
      },
    }
  },
}))

import FirstRunWizard from './FirstRunWizard.svelte'
const { checkDaemonInstallStatus, getPlatform, installDaemon } = await import('./ipc.js')

function deferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('FirstRunWizard', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getPlatform.mockResolvedValue('linux')
    checkDaemonInstallStatus.mockResolvedValue({
      installed: false,
      needs_update: false,
      wsl_available: true,
    })
    installDaemon.mockResolvedValue({
      success: true,
      message: 'installed',
    })
  })

  it('renders as a modal dialog and closes on Escape', async () => {
    const onDismiss = vi.fn()

    render(FirstRunWizard, {
      props: {
        dark: false,
        onDismiss,
      },
    })

    expect(screen.getByRole('dialog', { name: 'First run setup' })).toBeInTheDocument()

    await fireEvent.keyDown(window, { key: 'Escape' })

    expect(onDismiss).toHaveBeenCalledTimes(1)
  })

  it('uses helper service copy while checking and after install status is confirmed', async () => {
    const pendingCheck = deferred()
    checkDaemonInstallStatus.mockReturnValueOnce(pendingCheck.promise)

    render(FirstRunWizard, {
      props: {
        dark: false,
      },
    })

    await fireEvent.click(screen.getByTestId('get-started-button'))

    expect(screen.getByTestId('daemon-checking')).toHaveTextContent('Checking...')

    pendingCheck.resolve({
      installed: true,
      version: '0.3.0',
      needs_update: false,
      wsl_available: true,
    })

    await waitFor(() => {
      expect(screen.getByTestId('daemon-installed')).toHaveTextContent('Helper service ready')
    })
  })

  it('uses helper service install copy when setup is still needed', async () => {
    const installPending = deferred()
    installDaemon.mockReturnValueOnce(installPending.promise)

    render(FirstRunWizard, {
      props: {
        dark: false,
      },
    })

    await fireEvent.click(screen.getByTestId('get-started-button'))

    await waitFor(() => {
      expect(screen.getByTestId('daemon-not-installed')).toHaveTextContent(
        'Helper service not installed'
      )
    })

    await fireEvent.click(screen.getByTestId('daemon-install-button'))
    expect(screen.getByTestId('daemon-installing')).toHaveTextContent('Installing...')

    checkDaemonInstallStatus.mockResolvedValueOnce({
      installed: true,
      version: '0.3.0',
      needs_update: false,
      wsl_available: true,
    })
    installPending.resolve({ success: true, message: 'installed' })

    await waitFor(() => {
      expect(screen.getByTestId('daemon-installed')).toHaveTextContent('Helper service ready')
    })
  })
})
