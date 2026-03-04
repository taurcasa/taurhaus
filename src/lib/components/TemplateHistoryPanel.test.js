import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  getTemplateStorageStatus: vi.fn(),
  getTemplateHistory: vi.fn(),
  getTemplateDiff: vi.fn(),
  revertTemplateVersion: vi.fn(),
}))

const { getTemplateStorageStatus, getTemplateHistory, getTemplateDiff, revertTemplateVersion } =
  await import('../ipc.js')

import TemplateHistoryPanel from './TemplateHistoryPanel.svelte'

function makeHistoryPage() {
  return {
    commits: [
      {
        commitId: 'a1b2c3d4aa0011223344556677889900ffeeccdd',
        shortId: 'a1b2c3d4',
        message: 'Update claude reviewer contract',
        author: 'dev-a',
        timestamp: 1_706_000_000,
        changedPaths: ['roles/claude-reviewer.yaml'],
      },
      {
        commitId: 'de11ab008d1ca69f3f7a0b98b7f7c4d0f7d98322',
        shortId: 'de11ab00',
        message: 'Add docs preset',
        author: 'dev-b',
        timestamp: 1_705_999_000,
        changedPaths: ['presets/docs-sprint.yaml'],
      },
    ],
    nextCursor: null,
  }
}

function makeDiff(commitId) {
  return {
    commitId,
    files: [
      {
        path: 'roles/claude-reviewer.yaml',
        status: 'modified',
        hunks: [
          {
            old_start: 10,
            old_lines: 1,
            new_start: 10,
            new_lines: 2,
            lines: [
              {
                origin: '-',
                old_lineno: 10,
                new_lineno: null,
                content: 'execution: [focus on correctness]',
              },
              {
                origin: '+',
                old_lineno: null,
                new_lineno: 10,
                content: 'execution: [focus on correctness and regression risk]',
              },
            ],
          },
        ],
      },
    ],
    stats: { filesChanged: 1, insertions: 1, deletions: 1 },
  }
}

describe('TemplateHistoryPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getTemplateStorageStatus.mockResolvedValue({
      mode: 'git',
      repoInitialized: true,
      dirty: true,
      pendingActions: [],
      lastCommit: 1_706_000_000,
    })
    getTemplateHistory.mockResolvedValue(makeHistoryPage())
    getTemplateDiff.mockImplementation(async (commitId) => makeDiff(commitId))
    revertTemplateVersion.mockResolvedValue(undefined)
  })

  it('renders global history and dirty status indicator', async () => {
    render(TemplateHistoryPanel, {
      props: { dark: false },
    })

    await waitFor(() => {
      expect(screen.getByTestId('template-history-commit-a1b2c3d4')).toBeInTheDocument()
    })

    expect(screen.getByTestId('template-history-commit-de11ab00')).toBeInTheDocument()
    expect(screen.getByTestId('template-history-dirty-indicator')).toHaveTextContent('Dirty')
  })

  it('filters commit list to selected template when template scope is chosen', async () => {
    render(TemplateHistoryPanel, {
      props: {
        selectedTemplateId: 'claude-reviewer',
        selectedTemplateKind: 'role',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('template-history-commit-a1b2c3d4')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('template-history-scope-template'))

    expect(screen.getByTestId('template-history-commit-a1b2c3d4')).toBeInTheDocument()
    expect(screen.queryByTestId('template-history-commit-de11ab00')).not.toBeInTheDocument()
  })

  it('shows diff lines for selected commit', async () => {
    render(TemplateHistoryPanel, {
      props: { selectedTemplateId: 'claude-reviewer', selectedTemplateKind: 'role' },
    })

    await waitFor(() => {
      expect(screen.getByTestId('template-history-commit-a1b2c3d4')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('template-history-commit-de11ab00'))

    await waitFor(() => {
      expect(screen.getAllByTestId('template-history-diff-line').length).toBeGreaterThan(0)
    })
    expect(screen.getByTestId('template-history-detail-panel')).toHaveTextContent('Add docs preset')
    expect(getTemplateDiff).toHaveBeenCalledWith('de11ab008d1ca69f3f7a0b98b7f7c4d0f7d98322')
  })

  it('reverts selected template version from selected commit', async () => {
    const onReverted = vi.fn()

    render(TemplateHistoryPanel, {
      props: {
        selectedTemplateId: 'claude-reviewer',
        selectedTemplateKind: 'role',
        onReverted,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('template-history-revert-button')).toBeEnabled()
    })

    await fireEvent.click(screen.getByTestId('template-history-revert-button'))

    await waitFor(() => {
      expect(revertTemplateVersion).toHaveBeenCalledWith(
        'claude-reviewer',
        'a1b2c3d4aa0011223344556677889900ffeeccdd'
      )
    })
    expect(onReverted).toHaveBeenCalledTimes(1)
  })
})
