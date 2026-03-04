import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  listRoleTemplates: vi.fn(),
  getRoleTemplate: vi.fn(),
  listTeamPresets: vi.fn(),
  getTeamPreset: vi.fn(),
  getTemplateStorageStatus: vi.fn(),
  getTemplateHistory: vi.fn(),
  getTemplateDiff: vi.fn(),
  revertTemplateVersion: vi.fn(),
}))

const {
  listRoleTemplates,
  getRoleTemplate,
  listTeamPresets,
  getTeamPreset,
  getTemplateStorageStatus,
  getTemplateHistory,
  getTemplateDiff,
  revertTemplateVersion,
} = await import('../ipc.js')

import TemplateBrowserPanel from './TemplateBrowserPanel.svelte'

describe('TemplateBrowserPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()

    listRoleTemplates.mockResolvedValue([
      {
        roleId: 'claude-orchestrator',
        name: 'Claude Orchestrator',
        kind: 'lead',
        cliTool: 'claude',
        model: 'claude-opus-4-6',
        capabilities: ['planning', 'coordination'],
      },
      {
        roleId: 'custom-doc-writer',
        name: 'Documentation Writer',
        kind: 'agent',
        cliTool: 'gemini',
        model: 'gemini-2.5-pro',
        capabilities: ['documentation', 'research'],
      },
    ])

    getRoleTemplate.mockImplementation(async (id) => ({
      roleId: id,
      name: id === 'claude-orchestrator' ? 'Claude Orchestrator' : 'Documentation Writer',
      instructions: 'Detailed role instructions',
    }))

    listTeamPresets.mockResolvedValue([
      {
        presetId: 'review-team',
        name: 'Review Team',
        description: 'Lead plus two reviewers',
        leadRoleId: 'claude-orchestrator',
        roleCount: 1,
        agentCount: 2,
        tools: ['claude'],
      },
    ])

    getTeamPreset.mockImplementation(async (id) => ({
      presetId: id,
      name: 'Review Team',
      description: 'Preset details',
      agentSlots: [],
    }))

    getTemplateStorageStatus.mockResolvedValue({
      mode: 'git',
      repoInitialized: true,
      dirty: false,
      pendingActions: [],
      lastCommit: 1_706_000_000,
    })
    getTemplateHistory.mockResolvedValue({
      commits: [],
      nextCursor: null,
    })
    getTemplateDiff.mockResolvedValue({
      commitId: '',
      files: [],
      stats: { filesChanged: 0, insertions: 0, deletions: 0 },
    })
    revertTemplateVersion.mockResolvedValue()
  })

  it('renders in SlideOver when open=true and hidden when open=false', async () => {
    render(TemplateBrowserPanel, {
      props: {
        open: true,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('slideover-panel')).toBeInTheDocument()
    })
    expect(screen.getByTestId('slideover-title')).toHaveTextContent('Templates')
    expect(screen.getByTestId('template-browser-panel')).toBeInTheDocument()
    expect(listRoleTemplates).toHaveBeenCalled()
    expect(listTeamPresets).toHaveBeenCalled()
  })

  it('is not visible when open=false', () => {
    render(TemplateBrowserPanel, {
      props: {
        open: false,
      },
    })

    expect(screen.queryByTestId('slideover-panel')).not.toBeInTheDocument()
  })

  it('shows Roles/Presets/History tabs and tab switching works', async () => {
    render(TemplateBrowserPanel, { props: { open: true, dark: true } })

    await waitFor(() => {
      expect(screen.getByTestId('catalog-tab-roles')).toBeInTheDocument()
    })
    expect(screen.getByTestId('catalog-tab-presets')).toBeInTheDocument()
    expect(screen.getByTestId('catalog-tab-history')).toBeInTheDocument()

    await waitFor(() => {
      expect(screen.getByTestId('template-role-list')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('catalog-tab-presets'))
    await waitFor(() => {
      expect(screen.getByTestId('template-preset-list')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('catalog-tab-history'))
    await waitFor(() => {
      expect(screen.getByTestId('template-history-panel')).toBeInTheDocument()
    })
  })

  it('calls onSelectPreset when preset selected', async () => {
    const onSelectPreset = vi.fn()
    render(TemplateBrowserPanel, {
      props: {
        open: true,
        onSelectPreset,
      },
    })

    await fireEvent.click(screen.getByTestId('catalog-tab-presets'))

    await waitFor(() => {
      expect(screen.getByTestId('template-browser-preset-review-team')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('template-browser-preset-review-team'))

    expect(onSelectPreset).toHaveBeenCalledTimes(1)
    expect(onSelectPreset.mock.calls[0][0].presetId).toBe('review-team')
  })

  it('calls onSelectRole when role selected', async () => {
    const onSelectRole = vi.fn()
    render(TemplateBrowserPanel, {
      props: {
        open: true,
        onSelectRole,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('role-inspect-custom-doc-writer')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('role-inspect-custom-doc-writer'))

    await waitFor(() => {
      expect(screen.getByTestId('role-select-custom-doc-writer')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('role-select-custom-doc-writer'))

    expect(onSelectRole).toHaveBeenCalledTimes(1)
    expect(onSelectRole.mock.calls[0][0].roleId).toBe('custom-doc-writer')
  })

  it('calls onClose when SlideOver close button clicked', async () => {
    const onClose = vi.fn()
    render(TemplateBrowserPanel, {
      props: {
        open: true,
        onClose,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('slideover-close')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('slideover-close'))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('renders with 420px panel width', async () => {
    render(TemplateBrowserPanel, { props: { open: true } })

    await waitFor(() => {
      expect(screen.getByTestId('slideover-panel')).toBeInTheDocument()
    })
    expect(screen.getByTestId('slideover-panel')).toHaveStyle({ width: '420px' })
  })
})
