import { describe, it, expect } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import ValidationBar from './ValidationBar.svelte'

describe('ValidationBar', () => {
  const warningIssues = [
    { severity: 'warning', member: 'dev-1', message: 'Project ID is empty.' },
    { severity: 'warning', member: 'dev-2', message: 'Project ID is empty.' },
  ]

  it('shows issue count when collapsed', () => {
    render(ValidationBar, {
      props: {
        issues: warningIssues,
      },
    })

    expect(screen.getByTestId('validation-bar-summary')).toHaveTextContent('2 issues')
    expect(screen.queryByTestId('validation-bar-list')).not.toBeInTheDocument()
  })

  it('auto-expands when errors are present', () => {
    render(ValidationBar, {
      props: {
        issues: [{ severity: 'error', member: 'team', message: 'Team name is required.' }],
      },
    })

    expect(screen.getByTestId('validation-bar-list')).toBeInTheDocument()
    expect(screen.getByTestId('validation-bar-error-badge')).toHaveTextContent('1 error')
  })

  it('toggle expand/collapse works', async () => {
    render(ValidationBar, {
      props: {
        issues: warningIssues,
      },
    })

    expect(screen.queryByTestId('validation-bar-list')).not.toBeInTheDocument()
    await fireEvent.click(screen.getByTestId('validation-bar-toggle'))
    expect(screen.getByTestId('validation-bar-list')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('validation-bar-toggle'))
    expect(screen.queryByTestId('validation-bar-list')).not.toBeInTheDocument()
  })

  it('shows severity icons for each issue', async () => {
    render(ValidationBar, {
      props: {
        issues: [
          { severity: 'error', member: 'Team', message: 'Team name is required.' },
          { severity: 'warning', member: 'dev-1', message: 'Project ID is empty.' },
        ],
      },
    })

    const rows = screen.getAllByTestId(/validation-issue-/)
    expect(rows).toHaveLength(2)
    expect(rows[0]).toHaveTextContent('●')
    expect(rows[0]).toHaveTextContent('Team:')
    expect(rows[1]).toHaveTextContent('●')
    expect(rows[1]).toHaveTextContent('dev-1:')
  })
})
