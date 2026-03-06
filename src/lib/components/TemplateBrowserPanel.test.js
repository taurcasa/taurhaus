import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  listRoleTemplates: vi.fn(),
  getRoleTemplate: vi.fn(),
  upsertRoleTemplate: vi.fn(),
  deleteRoleTemplate: vi.fn(),
  listTeamPresets: vi.fn(),
  getTeamPreset: vi.fn(),
  upsertTeamPreset: vi.fn(),
  deleteTeamPreset: vi.fn(),
  getTemplateStorageStatus: vi.fn(),
  getTemplateHistory: vi.fn(),
  getTemplateDiff: vi.fn(),
  revertTemplateVersion: vi.fn(),
}))

const {
  listRoleTemplates,
  getRoleTemplate,
  upsertRoleTemplate,
  deleteRoleTemplate,
  listTeamPresets,
  getTeamPreset,
  upsertTeamPreset,
  deleteTeamPreset,
  getTemplateStorageStatus,
  getTemplateHistory,
  getTemplateDiff,
  revertTemplateVersion,
} = await import('../ipc.js')

import TemplateBrowserPanel from './TemplateBrowserPanel.svelte'

function deferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

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
        focusArea: 'Team orchestration',
        contextSummary: 'Keeps the whole team aligned on sequencing and delivery risks.',
        behaviorSummary: 'Coordinates work and escalates blockers instead of doing specialist implementation.',
        capabilities: ['planning', 'coordination'],
        builtIn: true,
        readOnly: true,
      },
      {
        roleId: 'custom-doc-writer',
        name: 'Documentation Writer',
        kind: 'agent',
        cliTool: 'gemini',
        model: 'gemini-2.5-pro',
        focusArea: 'Documentation systems',
        contextSummary: 'Maintains operational docs and architecture-facing explanations.',
        behaviorSummary: 'Writes and clarifies docs without taking over code ownership lanes.',
        capabilities: ['documentation', 'research'],
        builtIn: false,
        readOnly: false,
      },
    ])

    getRoleTemplate.mockImplementation(async (id) => ({
      roleId: id,
      name: id === 'claude-orchestrator' ? 'Claude Orchestrator' : 'Documentation Writer',
      focusArea: id === 'claude-orchestrator' ? 'Team orchestration' : 'Documentation systems',
      contextSummary: 'Detailed context summary',
      behaviorSummary: 'Detailed behavior boundary',
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
        builtIn: true,
      },
      {
        presetId: 'backend-sprint-team',
        name: 'Backend Sprint Team',
        description: 'Lead plus two backend agents',
        leadRoleId: 'claude-orchestrator',
        roleCount: 1,
        agentCount: 2,
        tools: ['claude', 'codex'],
        builtIn: false,
      },
    ])

    getTeamPreset.mockImplementation(async (id) => {
      if (id === 'backend-sprint-team') {
        return {
          presetId: id,
          name: 'Backend Sprint Team',
          description: 'Preset details',
          leadRoleId: 'claude-orchestrator',
          agentSlots: [{ roleId: 'custom-doc-writer', count: 2 }],
          defaults: {
            teamNamePattern: '{project}-team',
            tmuxLayout: 'tiled',
          },
        }
      }
      return {
        presetId: id,
        name: 'Review Team',
        description: 'Preset details',
        leadRoleId: 'claude-orchestrator',
        agentSlots: [{ roleId: 'custom-doc-writer', count: 2 }],
        defaults: {
          teamNamePattern: '{project}-team',
          tmuxLayout: 'tiled',
        },
      }
    })

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
    upsertRoleTemplate.mockResolvedValue({
      roleId: 'custom-doc-writer',
      name: 'Documentation Writer',
      kind: 'agent',
      builtIn: false,
      readOnly: false,
    })
    deleteRoleTemplate.mockResolvedValue({
      roleId: 'custom-doc-writer',
      deleted: true,
    })
    upsertTeamPreset.mockResolvedValue({
      presetId: 'backend-sprint-team',
    })
    deleteTeamPreset.mockResolvedValue({
      presetId: 'backend-sprint-team',
      deleted: true,
    })
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

  it('shows preset CRUD actions only for custom presets', async () => {
    render(TemplateBrowserPanel, { props: { open: true } })

    await fireEvent.click(screen.getByTestId('catalog-tab-presets'))
    await waitFor(() => {
      expect(screen.getByTestId('template-preset-list')).toBeInTheDocument()
    })

    expect(screen.getByTestId('template-preset-edit-backend-sprint-team')).toBeInTheDocument()
    expect(screen.getByTestId('template-preset-duplicate-backend-sprint-team')).toBeInTheDocument()
    expect(screen.getByTestId('template-preset-delete-backend-sprint-team')).toBeInTheDocument()

    expect(screen.queryByTestId('template-preset-edit-review-team')).not.toBeInTheDocument()
    expect(screen.queryByTestId('template-preset-duplicate-review-team')).not.toBeInTheDocument()
    expect(screen.queryByTestId('template-preset-delete-review-team')).not.toBeInTheDocument()
  })

  it('opens TeamCustomizerPanel from the + Create preset action', async () => {
    render(TemplateBrowserPanel, { props: { open: true } })

    await fireEvent.click(screen.getByTestId('catalog-tab-presets'))
    await waitFor(() => {
      expect(screen.getByTestId('template-preset-create')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('template-preset-create'))
    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-panel')).toBeInTheDocument()
    })
  })

  it('duplicate preset opens editor with copy name', async () => {
    render(TemplateBrowserPanel, { props: { open: true } })

    await fireEvent.click(screen.getByTestId('catalog-tab-presets'))
    await waitFor(() => {
      expect(screen.getByTestId('template-preset-duplicate-backend-sprint-team')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('template-preset-duplicate-backend-sprint-team'))

    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-name-input')).toHaveValue('Copy of Backend Sprint Team')
    })
  })

  it('delete preset requires confirm and refreshes list after delete', async () => {
    render(TemplateBrowserPanel, { props: { open: true } })

    await fireEvent.click(screen.getByTestId('catalog-tab-presets'))
    await waitFor(() => {
      expect(screen.getByTestId('template-preset-delete-backend-sprint-team')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('template-preset-delete-backend-sprint-team'))

    await waitFor(() => {
      expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))

    await waitFor(() => {
      expect(deleteTeamPreset).toHaveBeenCalledWith('backend-sprint-team')
      expect(listTeamPresets.mock.calls.length).toBeGreaterThanOrEqual(2)
    })
  })

  it('edit preset save calls upsert and refreshes presets', async () => {
    render(TemplateBrowserPanel, { props: { open: true } })

    await fireEvent.click(screen.getByTestId('catalog-tab-presets'))
    await waitFor(() => {
      expect(screen.getByTestId('template-preset-edit-backend-sprint-team')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('template-preset-edit-backend-sprint-team'))

    await waitFor(() => {
      expect(screen.getByTestId('team-customizer-save')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('team-customizer-save'))

    await waitFor(() => {
      expect(upsertTeamPreset).toHaveBeenCalledTimes(1)
      expect(listTeamPresets.mock.calls.length).toBeGreaterThanOrEqual(2)
    })
  })

  it('shows create/edit/delete actions only for custom roles', async () => {
    render(TemplateBrowserPanel, { props: { open: true } })

    await waitFor(() => {
      expect(screen.getByTestId('role-template-card-claude-orchestrator')).toBeInTheDocument()
      expect(screen.getByTestId('role-template-card-custom-doc-writer')).toBeInTheDocument()
    })

    expect(screen.getByTestId('role-create-button')).toBeInTheDocument()
    expect(screen.getByTestId('role-inspect-claude-orchestrator')).toBeInTheDocument()
    expect(screen.getByTestId('role-use-claude-orchestrator')).toBeInTheDocument()
    expect(screen.queryByTestId('role-edit-claude-orchestrator')).not.toBeInTheDocument()
    expect(screen.queryByTestId('role-delete-claude-orchestrator')).not.toBeInTheDocument()

    expect(screen.getByTestId('role-inspect-custom-doc-writer')).toBeInTheDocument()
    expect(screen.getByTestId('role-use-custom-doc-writer')).toBeInTheDocument()
    expect(screen.getByTestId('role-edit-custom-doc-writer')).toBeInTheDocument()
    expect(screen.getByTestId('role-delete-custom-doc-writer')).toBeInTheDocument()
  })

  it('opens role editor from create and edit actions and saves through upsertRoleTemplate', async () => {
    getRoleTemplate.mockImplementation(async (id) => {
      if (id === 'frontend-dev') {
        return {
          roleId: 'frontend-dev',
          name: 'Frontend Developer',
          tool: 'codex',
          model: 'gpt-5.4 high',
          instructions: 'Frontend role details',
          capabilities: ['ui'],
        }
      }
      return {
        roleId: id,
        name: 'Claude Orchestrator',
        instructions: 'Detailed role instructions',
      }
    })

    listRoleTemplates
      .mockResolvedValueOnce([
        {
          roleId: 'claude-orchestrator',
          name: 'Claude Orchestrator',
          kind: 'lead',
          cliTool: 'claude',
          model: 'claude-opus-4-6',
          capabilities: ['planning'],
          builtIn: true,
          readOnly: true,
        },
      ])
      .mockResolvedValue([
        {
          roleId: 'claude-orchestrator',
          name: 'Claude Orchestrator',
          kind: 'lead',
          cliTool: 'claude',
          model: 'claude-opus-4-6',
          capabilities: ['planning'],
          builtIn: true,
          readOnly: true,
        },
        {
          roleId: 'frontend-dev',
          name: 'Frontend Developer',
          kind: 'agent',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          capabilities: ['ui'],
          builtIn: false,
          readOnly: false,
        },
      ])

    render(TemplateBrowserPanel, { props: { open: true } })

    await waitFor(() => {
      expect(screen.getByTestId('role-create-button')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('role-create-button'))
    await waitFor(() => {
      expect(screen.getByTestId('role-editor-container')).toBeInTheDocument()
    })

    await fireEvent.input(screen.getByTestId('role-editor-name-input'), {
      target: { value: 'Frontend Developer' },
    })
    await fireEvent.click(screen.getByTestId('role-editor-save'))

    await waitFor(() => {
      expect(upsertRoleTemplate).toHaveBeenCalled()
    })
    expect(listRoleTemplates.mock.calls.length).toBeGreaterThanOrEqual(2)

    await waitFor(() => {
      expect(screen.getByTestId('role-edit-frontend-dev')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('role-edit-frontend-dev'))

    await waitFor(() => {
      expect(screen.getByDisplayValue('Frontend Developer')).toBeInTheDocument()
    })
  })

  it('requires delete confirmation before calling deleteRoleTemplate and refreshes roles', async () => {
    listRoleTemplates
      .mockResolvedValueOnce([
        {
          roleId: 'custom-doc-writer',
          name: 'Documentation Writer',
          kind: 'agent',
          cliTool: 'gemini',
          model: 'gemini-2.5-pro',
          capabilities: ['documentation'],
          builtIn: false,
          readOnly: false,
        },
      ])
      .mockResolvedValue([])

    render(TemplateBrowserPanel, { props: { open: true } })

    await waitFor(() => {
      expect(screen.getByTestId('role-delete-custom-doc-writer')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('role-delete-custom-doc-writer'))
    expect(deleteRoleTemplate).not.toHaveBeenCalled()

    await fireEvent.click(screen.getByTestId('confirm-dialog-confirm'))

    await waitFor(() => {
      expect(deleteRoleTemplate).toHaveBeenCalledWith('custom-doc-writer')
    })
    expect(listRoleTemplates.mock.calls.length).toBeGreaterThanOrEqual(2)
  })

  it('shows empty custom roles message when no custom roles exist', async () => {
    listRoleTemplates.mockResolvedValue([
      {
        roleId: 'claude-orchestrator',
        name: 'Claude Orchestrator',
        kind: 'lead',
        cliTool: 'claude',
        model: 'claude-opus-4-6',
        capabilities: ['planning'],
        builtIn: true,
        readOnly: true,
      },
    ])

    render(TemplateBrowserPanel, { props: { open: true } })

    await waitFor(() => {
      expect(screen.getByTestId('role-custom-empty-state')).toBeInTheDocument()
    })
    expect(
      screen.getByText('No custom roles yet. Create one or capture from a live team.')
    ).toBeInTheDocument()
  })

  it('renders tool, focus area, and behavior summary on role cards', async () => {
    render(TemplateBrowserPanel, { props: { open: true } })

    await waitFor(() => {
      expect(screen.getByTestId('role-tool-badge-custom-doc-writer')).toBeInTheDocument()
    })
    expect(screen.getByTestId('role-model-badge-custom-doc-writer')).toBeInTheDocument()
    expect(screen.getByTestId('role-focus-area-custom-doc-writer')).toHaveTextContent('Documentation systems')
    expect(screen.getByTestId('role-behavior-summary-custom-doc-writer')).toHaveTextContent(
      'Writes and clarifies docs without taking over code ownership lanes.'
    )
    expect(screen.queryByText('documentation')).not.toBeInTheDocument()
  })

  it('ignores stale catalog responses when panel is reopened quickly', async () => {
    const oldRoles = deferred()
    const oldPresets = deferred()
    const newRoles = deferred()
    const newPresets = deferred()

    listRoleTemplates
      .mockReturnValueOnce(oldRoles.promise)
      .mockReturnValueOnce(newRoles.promise)
    listTeamPresets
      .mockReturnValueOnce(oldPresets.promise)
      .mockReturnValueOnce(newPresets.promise)

    const view = render(TemplateBrowserPanel, { props: { open: true } })
    await view.rerender({ open: false })
    await view.rerender({ open: true })

    newRoles.resolve([
      {
        roleId: 'new-role',
        name: 'New Role',
        kind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.4 high',
        capabilities: [],
        builtIn: false,
      },
    ])
    newPresets.resolve([
      {
        presetId: 'new-preset',
        name: 'New Preset',
        description: 'fresh',
        roleCount: 1,
        agentCount: 0,
        tools: ['codex'],
      },
    ])

    await waitFor(() => {
      expect(screen.getByTestId('role-template-card-new-role')).toBeInTheDocument()
    })

    oldRoles.resolve([
      {
        roleId: 'old-role',
        name: 'Old Role',
        kind: 'agent',
        cliTool: 'gemini',
        model: 'gemini-2.5-pro',
        capabilities: [],
        builtIn: false,
      },
    ])
    oldPresets.resolve([
      {
        presetId: 'old-preset',
        name: 'Old Preset',
        description: 'stale',
        roleCount: 1,
        agentCount: 0,
        tools: ['gemini'],
      },
    ])

    await waitFor(() => {
      expect(screen.getByTestId('role-template-card-new-role')).toBeInTheDocument()
    })
    expect(screen.queryByTestId('role-template-card-old-role')).not.toBeInTheDocument()
  })
})
