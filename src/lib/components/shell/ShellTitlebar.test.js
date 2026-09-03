import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import ShellTitlebar from './ShellTitlebar.svelte'

describe('ShellTitlebar', () => {
  it('renders the project sections as tabs', () => {
    render(ShellTitlebar, {
      props: {
        dark: true,
        activeTab: 'overview',
      },
    })

    expect(screen.getByRole('tablist', { name: 'Project sections' })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: 'Overview' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('tab', { name: 'Files' })).toHaveAttribute('aria-selected', 'false')
  })

  // Regression: e28881d added the Accounts main-panel takeover without giving
  // the titlebar its takeover state, leaving every project tab as a dead control.
  it('replaces project tabs with the Accounts takeover label', () => {
    render(ShellTitlebar, {
      props: {
        dark: true,
        activeTab: 'overview',
        accountsOpen: true,
      },
    })

    expect(screen.getByText('Accounts')).toBeInTheDocument()
    expect(screen.queryByRole('tab')).not.toBeInTheDocument()
  })

  it('supports arrow-key navigation between tabs', async () => {
    const onSwitchTab = vi.fn()

    render(ShellTitlebar, {
      props: {
        dark: true,
        activeTab: 'overview',
        onSwitchTab,
      },
    })

    const overviewTab = screen.getByRole('tab', { name: 'Overview' })
    overviewTab.focus()

    await fireEvent.keyDown(overviewTab, { key: 'ArrowRight' })

    expect(onSwitchTab).toHaveBeenCalledWith('files')
    expect(screen.getByRole('tab', { name: 'Files' })).toHaveFocus()
  })

  it('keeps aria-labels on icon-only window controls', () => {
    render(ShellTitlebar, {
      props: {
        dark: true,
        activeTab: 'overview',
      },
    })

    expect(screen.getByRole('button', { name: 'Open search' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Minimize window' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Maximize window' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Close window' })).toBeInTheDocument()
  })

  it('marks decorative titlebar svg icons as hidden from assistive tech', () => {
    const { container } = render(ShellTitlebar, {
      props: {
        dark: true,
        activeTab: 'overview',
      },
    })

    for (const button of screen.getAllByRole('button')) {
      const icon = button.querySelector('svg')
      if (!icon) continue
      expect(icon).toHaveAttribute('aria-hidden', 'true')
    }
  })
})
