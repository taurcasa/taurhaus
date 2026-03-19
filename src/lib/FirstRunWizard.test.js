import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
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

describe('FirstRunWizard', () => {
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
})
