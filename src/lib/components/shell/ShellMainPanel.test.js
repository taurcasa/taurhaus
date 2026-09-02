import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import ShellMainPanel from './ShellMainPanel.svelte'

describe('ShellMainPanel', () => {
  it('renders Accounts as a global takeover independently of Settings', () => {
    render(ShellMainPanel, {
      props: {
        accountsOpen: true,
        accountStates: {},
        projects: [],
      },
    })

    expect(screen.getByTestId('accounts-home')).toBeInTheDocument()
    expect(screen.queryByText('Select a project')).not.toBeInTheDocument()
  })

  // Regression: e28881d copied the Settings takeover but left project and
  // shell notices visible above Accounts, stacking project chrome into the hub.
  it('suppresses project-scoped notices during the Accounts takeover', () => {
    render(ShellMainPanel, {
      props: {
        accountsOpen: true,
        accountStates: {},
        projects: [],
        projectLoadIssues: [{ section: 'README', message: 'boom' }],
        shellNotice: 'Project details changed.',
      },
    })

    expect(screen.queryByTestId('project-load-degraded-banner')).not.toBeInTheDocument()
    expect(screen.queryByTestId('shell-notice-banner')).not.toBeInTheDocument()
  })

  it('shows a restart action once daemon recovery has stalled', async () => {
    const onRestartDaemon = vi.fn()

    render(ShellMainPanel, {
      props: {
        dark: false,
        daemonStatus: 'disconnected',
        daemonRecoveryEscalated: true,
        daemonRestarting: false,
        onRestartDaemon,
      },
    })

    expect(screen.getByTestId('daemon-connecting-banner')).toHaveTextContent(
      'Still trying to reconnect to the helper service. You can keep working, or restart it now.'
    )

    await fireEvent.click(screen.getByTestId('daemon-restart-button'))
    expect(onRestartDaemon).toHaveBeenCalledTimes(1)
  })

  it('summarizes degraded project loads in plain language', () => {
    render(ShellMainPanel, {
      props: {
        dark: false,
        projectLoadIssues: [
          { section: 'Recent commits', message: 'boom' },
          { section: 'README', message: 'boom' },
        ],
      },
    })

    expect(screen.getByTestId('project-load-degraded-message')).toHaveTextContent(
      'Some project details could not be loaded: Recent commits, README.'
    )
  })

  it('uses helper service wording in the update banner', () => {
    render(ShellMainPanel, {
      props: {
        dark: false,
        daemonUpdateAvailable: {
          version: '0.2.8',
          bundled_version: '0.3.0',
        },
      },
    })

    expect(screen.getByTestId('daemon-update-banner')).toHaveTextContent(
      'Helper service update available: v0.2.8 → v0.3.0'
    )
  })

  it('announces dynamic status banners to assistive technology', () => {
    render(ShellMainPanel, {
      props: {
        dark: false,
        daemonStatus: 'failed',
        daemonUpdateAvailable: {
          version: '0.2.8',
          bundled_version: '0.3.0',
        },
        projectLoadIssues: [
          { section: 'Recent commits', message: 'boom' },
        ],
        shellNotice: 'Saved, but some details need attention.',
      },
    })

    expect(screen.getByTestId('daemon-connecting-banner')).toHaveAttribute('role', 'status')
    expect(screen.getByTestId('daemon-connecting-banner')).toHaveAttribute('aria-live', 'polite')
    expect(screen.getByTestId('daemon-update-banner')).toHaveAttribute('role', 'status')
    expect(screen.getByTestId('project-load-degraded-banner')).toHaveAttribute('role', 'status')
    expect(screen.getByTestId('shell-notice-banner')).toHaveAttribute('role', 'status')
  })
})
