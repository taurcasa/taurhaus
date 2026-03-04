import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  listRoleTemplates: vi.fn(),
  getRoleTemplate: vi.fn(),
  listTeamPresets: vi.fn(),
  getTeamPreset: vi.fn(),
  composeTeam: vi.fn(),
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
  composeTeam,
  getTemplateStorageStatus,
  getTemplateHistory,
  getTemplateDiff,
} = await import('../ipc.js')

import TemplateCatalog from './TemplateCatalog.svelte'

describe('TemplateCatalog', () => {
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
        builtIn: true,
        readOnly: true,
      },
      {
        roleId: 'custom-doc-writer',
        name: 'Documentation Writer',
        kind: 'agent',
        cliTool: 'gemini',
        model: 'gemini-2.5-pro',
        capabilities: ['documentation', 'research'],
        builtIn: false,
        readOnly: false,
      },
    ])

    getRoleTemplate.mockImplementation(async (id) => ({
      roleId: id,
      name: id === 'claude-orchestrator' ? 'Claude Orchestrator' : 'Documentation Writer',
      behavioralContract: {
        communication: ['One'],
        execution: ['Two'],
        escalation: ['Three'],
      },
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
        capabilities: ['review'],
        builtIn: true,
        readOnly: true,
      },
      {
        presetId: 'docs-sprint',
        name: 'Docs Sprint',
        description: 'Lead plus one doc writer',
        leadRoleId: 'claude-orchestrator',
        roleCount: 1,
        agentCount: 1,
        tools: ['gemini'],
        capabilities: ['documentation', 'research'],
        builtIn: false,
        readOnly: false,
      },
    ])

    getTeamPreset.mockImplementation(async (id) => ({
      presetId: id,
      name: id === 'review-team' ? 'Review Team' : 'Docs Sprint',
      description: 'Preset details',
      agentSlots: [],
      leadRoleId: 'claude-orchestrator',
    }))

    composeTeam.mockResolvedValue({
      roster: [
        {
          name: 'lead-project',
          roleId: 'claude-orchestrator',
          roleKind: 'lead',
          cliTool: 'claude',
          model: 'claude-opus-4-6',
          instructions: 'Lead instructions',
          projectBinding: 'lead_project',
          projectId: '/projects/taurhaus',
        },
      ],
      warnings: [],
      validationErrors: [],
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
  })

  it('renders role and preset sections from IPC data', async () => {
    render(TemplateCatalog, { props: { dark: true } })

    await waitFor(() => {
      expect(screen.getByTestId('role-template-card-claude-orchestrator')).toBeInTheDocument()
    })

    expect(screen.getByTestId('team-preset-card-review-team')).toBeInTheDocument()
    expect(screen.getByTestId('template-catalog-title')).toHaveTextContent('Template Catalog')
  })

  it('filters templates by tool and capability', async () => {
    render(TemplateCatalog, { props: { dark: false } })

    await waitFor(() => {
      expect(screen.getByTestId('role-template-card-custom-doc-writer')).toBeInTheDocument()
    })

    await fireEvent.change(screen.getByTestId('template-tool-filter'), {
      target: { value: 'gemini' },
    })

    expect(screen.getByTestId('role-template-card-custom-doc-writer')).toBeInTheDocument()
    expect(screen.queryByTestId('role-template-card-claude-orchestrator')).not.toBeInTheDocument()

    await fireEvent.change(screen.getByTestId('template-capability-filter'), {
      target: { value: 'documentation' },
    })

    expect(screen.getByTestId('team-preset-card-docs-sprint')).toBeInTheDocument()
    expect(screen.queryByTestId('team-preset-card-review-team')).not.toBeInTheDocument()
  })

  it('shows edit/delete only for user templates and opens composer from preset preview', async () => {
    const onComposePreview = vi.fn()

    render(TemplateCatalog, {
      props: {
        onComposePreview,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('role-template-card-custom-doc-writer')).toBeInTheDocument()
    })

    expect(screen.getByTestId('role-readonly-claude-orchestrator')).toBeInTheDocument()
    expect(screen.queryByTestId('role-edit-claude-orchestrator')).not.toBeInTheDocument()
    expect(screen.getByTestId('role-edit-custom-doc-writer')).toBeInTheDocument()
    expect(screen.getByTestId('role-delete-custom-doc-writer')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('preset-preview-docs-sprint'))

    await waitFor(() => {
      expect(screen.getByTestId('team-composer')).toBeInTheDocument()
    })
    expect(screen.getByTestId('composer-lead-select')).toHaveValue('claude-orchestrator')
    expect(composeTeam).toHaveBeenCalled()
    expect(onComposePreview).toHaveBeenCalledTimes(1)
  })
})
