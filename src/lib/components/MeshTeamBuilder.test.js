import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  deleteRoleTemplate: vi.fn(),
  exportRoleToFile: vi.fn(),
  getRoleTemplate: vi.fn(),
  importRoleFromFile: vi.fn(),
  isTauri: vi.fn(),
  upsertRoleTemplate: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-fs', () => ({
  writeTextFile: vi.fn(),
}))

const {
  deleteRoleTemplate,
  exportRoleToFile,
  getRoleTemplate,
  importRoleFromFile,
  isTauri,
  upsertRoleTemplate,
} = await import('../ipc.js')
const { open, save } = await import('@tauri-apps/plugin-dialog')
const { writeTextFile } = await import('@tauri-apps/plugin-fs')

import MeshTeamBuilder from './MeshTeamBuilder.svelte'
import { TEST_MODEL_CATALOG } from '../../test/fixtures/modelCatalog.js'

const ROLE_VERSION_VISIBILITY_STORAGE_KEY =
  'taurhaus.mesh.builder.show-all-role-versions'
const PINNED_ROLE_IDS_STORAGE_KEY = 'taurhaus.mesh.pinnedRoleIds'

function sampleRoles(extraAgentCount = 0) {
  const roles = [
    {
      roleId: 'lead-claude',
      name: 'Claude Orchestrator',
      kind: 'lead',
      cliTool: 'claude',
      model: 'claude-opus-4.5',
      behaviorSummary: 'Routes the team.',
      mode: 'coordination',
    },
    {
      roleId: 'lead-codex',
      name: 'Codex Product Lead',
      kind: 'lead',
      cliTool: 'codex',
      model: 'gpt-5.4 high',
      behaviorSummary: 'Owns execution planning.',
      mode: 'planning',
    },
    {
      roleId: 'agent-codex',
      name: 'Codex Developer',
      kind: 'agent',
      cliTool: 'codex',
      model: 'gpt-5.4 high',
      behaviorSummary: 'Implements scoped changes.',
      communicationStyle: 'Minimal and concrete.',
      qualityGates: ['Run the scoped test lane.'],
      definitionOfDone: ['Ready for review.'],
      phaseScope: ['implementation', 'verification'],
      mode: 'implementation',
    },
    {
      roleId: 'agent-antigravity',
      name: 'Antigravity Researcher',
      kind: 'agent',
      cliTool: 'agy',
      model: 'gemini-3.7-flash-high',
      behaviorSummary: 'Finds source material.',
      mode: 'research',
    },
  ]

  for (let index = 0; index < extraAgentCount; index += 1) {
    roles.push({
      roleId: `agent-extra-${index + 1}`,
      name: `Extra Agent ${index + 1}`,
      kind: 'agent',
      cliTool: index % 2 === 0 ? 'codex' : 'agy',
      model: index % 2 === 0 ? 'gpt-5.4 high' : 'gemini-3.7-flash-high',
      behaviorSummary: `Extra agent summary ${index + 1}.`,
    })
  }

  return roles
}

function sampleVersionedRoles() {
  return [
    {
      roleId: 'lead-claude',
      name: 'Claude Orchestrator',
      kind: 'lead',
      cliTool: 'claude',
      model: 'claude-opus-4.5',
      behaviorSummary: 'Routes the team.',
    },
    {
      roleId: 'v2-codex-developer',
      name: 'Codex Developer',
      kind: 'agent',
      cliTool: 'codex',
      model: 'gpt-5.4 medium',
      behaviorSummary: 'Older implementation lane.',
      provenance: { importedAt: '2026-03-10T09:00:00Z' },
    },
    {
      roleId: 'v3-codex-developer',
      name: 'Codex Developer',
      kind: 'agent',
      cliTool: 'codex',
      model: 'gpt-5.4 high',
      behaviorSummary: 'Current implementation lane.',
      provenance: { importedAt: '2026-03-12T09:00:00Z' },
    },
    {
      roleId: 'claude-reviewer-legacy',
      name: 'Claude Reviewer',
      kind: 'agent',
      cliTool: 'claude',
      model: 'claude-opus-4.5',
      behaviorSummary: 'Older review lane.',
      provenance: { importedAt: '2026-03-09T08:00:00Z' },
    },
    {
      roleId: 'claude-reviewer-current',
      name: 'Claude Reviewer',
      kind: 'agent',
      cliTool: 'claude',
      model: 'claude-opus-4.6',
      behaviorSummary: 'Current review lane.',
      provenance: { importedAt: '2026-03-11T08:00:00Z' },
    },
  ]
}

function samplePresets() {
  return [
    {
      presetId: 'full-team',
      name: 'Full Team',
      description: 'Lead, architect, and two developers.',
      roleCount: 4,
      agentCount: 3,
      tools: ['claude', 'codex'],
      builtIn: true,
    },
    {
      presetId: 'research-pod',
      name: 'Research Pod',
      description: 'Lean research and validation crew.',
      leadCount: 1,
      agentCount: 2,
      tools: ['agy', 'claude'],
      builtIn: false,
    },
  ]
}

function sampleRosterConfig() {
  return {
    description: 'Delivery team',
    lead: {
      id: 'lead',
      name: 'team-lead',
      roleId: 'lead-claude',
      roleName: 'Claude Orchestrator',
      tool: 'claude',
      model: 'claude-opus-4.5',
      projectId: '/projects/taurhaus',
    },
    agents: [
      {
        id: 'agent-codex-1',
        name: 'builder-1',
        roleId: 'agent-codex',
        roleName: 'Codex Developer',
        tool: 'codex',
        model: 'gpt-5.4 high',
        projectId: '/projects/taurhaus',
      },
    ],
  }
}

function sampleAvailableProjects() {
  return [
    { id: '/projects/taurhaus', path: '/projects/taurhaus', name: 'taurhaus' },
    { id: '/projects/mesh', path: '/projects/mesh', name: 'mesh' },
  ]
}

function builderProps(props = {}) {
  return {
    dark: false,
    mode: 'setup',
    teamName: 'taurhaus-team',
    teamConfig: {
      description: '',
      lead: null,
      agents: [],
    },
    roleTemplates: sampleRoles(),
    presets: [],
    availableProjects: sampleAvailableProjects(),
    modelCatalog: TEST_MODEL_CATALOG,
    onBrowseCatalog: vi.fn(),
    onTeamNameChange: vi.fn(),
    onDescriptionChange: vi.fn(),
    onApplyPreset: vi.fn(),
    onAssignLeadRole: vi.fn(),
    onClearLead: vi.fn(),
    onAppendAgentRole: vi.fn(),
    onUpdateLead: vi.fn(),
    onUpdateAgent: vi.fn(),
    onRemoveAgent: vi.fn(),
    onReorderAgent: vi.fn(),
    onMoveAgentToEnd: vi.fn(),
    onRefreshRoleTemplates: vi.fn(),
    onInitialize: vi.fn(),
    onReset: vi.fn(),
    onSavePreset: vi.fn(),
    ...props,
  }
}

function renderBuilder(props = {}) {
  return render(MeshTeamBuilder, {
    props: builderProps(props),
  })
}

describe('MeshTeamBuilder', () => {
  beforeEach(() => {
    window.localStorage.clear()
    vi.clearAllMocks()
    isTauri.mockReturnValue(true)
    getRoleTemplate.mockImplementation(async (roleId) => (
      sampleRoles().find((role) => role.roleId === roleId) ?? null
    ))
    upsertRoleTemplate.mockResolvedValue(undefined)
    deleteRoleTemplate.mockResolvedValue(undefined)
    exportRoleToFile.mockResolvedValue({
      targetFormat: 'yaml',
      fileContent: 'schema:\n  kind: role_template\n',
      lossyFields: [],
    })
    importRoleFromFile.mockResolvedValue({
      success: true,
      role: {
        roleId: 'imported-role',
        name: 'Imported Role',
      },
      conflict: null,
    })
    open.mockResolvedValue(null)
    save.mockResolvedValue('/tmp/exported-role.yaml')
    writeTextFile.mockResolvedValue(undefined)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('uses inline roster header text and opens the team name editor on click', async () => {
    const onTeamNameChange = vi.fn()

    renderBuilder({
      onTeamNameChange,
    })

    expect(screen.getByTestId('mesh-builder-team-name-display')).toHaveTextContent('taurhaus-team')
    expect(screen.queryByTestId('mesh-builder-team-name-input')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-team-name-display'))

    const input = screen.getByTestId('mesh-builder-team-name-input')
    expect(input).toBeInTheDocument()

    await fireEvent.input(input, { target: { value: 'mesh-redesign-team' } })

    expect(onTeamNameChange).toHaveBeenCalledWith('mesh-redesign-team')
  })

  it('filters roles by tool icon toggle', async () => {
    renderBuilder()

    expect(screen.getByTestId('mesh-builder-role-lead-claude')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-codex')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-filter-tool-codex'))

    expect(screen.queryByTestId('mesh-builder-role-lead-claude')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-lead-codex')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-codex')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-builder-role-agent-antigravity')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-filter-tool-codex'))

    expect(screen.getByTestId('mesh-builder-role-lead-claude')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-antigravity')).toBeInTheDocument()

    // Regression: 9a66d1c made the third tool filter a retired-CLI-only branch.
    await fireEvent.click(screen.getByTestId('mesh-builder-filter-tool-agy'))
    expect(screen.getByTestId('mesh-builder-role-agent-antigravity')).toBeInTheDocument()
  })

  it('filters roles by kind chips', async () => {
    renderBuilder()

    await fireEvent.click(screen.getByTestId('mesh-builder-filter-kind-agent'))

    expect(screen.queryByTestId('mesh-builder-role-section-leads')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-section-agents')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-codex')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-filter-kind-lead'))

    expect(screen.getByTestId('mesh-builder-role-section-leads')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-builder-role-section-agents')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-lead-claude')).toBeInTheDocument()
  })

  it('filters roles by mode pills', async () => {
    renderBuilder()

    expect(screen.getByTestId('mesh-builder-filter-modes')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-filter-mode-implementation'))

    expect(screen.queryByTestId('mesh-builder-role-lead-claude')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-codex')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-builder-role-agent-antigravity')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-filter-mode-implementation'))

    expect(screen.getByTestId('mesh-builder-role-lead-claude')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-antigravity')).toBeInTheDocument()
  })

  it('shows an empty-results state when filters remove every role', async () => {
    renderBuilder()

    await fireEvent.input(screen.getByTestId('mesh-builder-role-search'), {
      target: { value: 'nonexistent-role' },
    })

    expect(screen.getByTestId('mesh-builder-empty-results')).toBeInTheDocument()
  })

  it('defaults to compact density when more than eight roles are visible and still assigns on click', async () => {
    const onAppendAgentRole = vi.fn()
    renderBuilder({
      roleTemplates: sampleRoles(6),
      onAppendAgentRole,
    })

    expect(screen.getByTestId('mesh-builder-density-compact')).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByTestId('mesh-builder-density-expanded')).toHaveAttribute('aria-pressed', 'false')
    expect(screen.getByText('Implements scoped changes.')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-codex')).not.toHaveAttribute('title')
    expect(screen.getByTestId('mesh-builder-role-agent-codex')).not.toHaveTextContent('gpt-5.4 high')

    await fireEvent.click(screen.getByTestId('mesh-builder-role-agent-codex'))

    expect(onAppendAgentRole).toHaveBeenCalledWith('agent-codex')
  })

  it('flashes the source role row and swaps the add button to a checkmark briefly on add', async () => {
    vi.useFakeTimers()
    const onAppendAgentRole = vi.fn()

    renderBuilder({ onAppendAgentRole })

    const roleRow = screen.getByTestId('mesh-builder-role-row-agent-codex')
    const addButton = screen.getByTestId('mesh-builder-add-agent-codex')

    await fireEvent.click(addButton)

    expect(onAppendAgentRole).toHaveBeenCalledWith('agent-codex')
    expect(roleRow).toHaveClass('mesh-builder-role-row-active')
    expect(addButton).toHaveTextContent('✓')

    await vi.advanceTimersByTimeAsync(401)

    expect(roleRow).not.toHaveClass('mesh-builder-role-row-active')
    expect(addButton).toHaveTextContent('+')
  })

  it('defaults to expanded density when eight or fewer roles are visible', () => {
    renderBuilder()

    expect(screen.getByTestId('mesh-builder-density-expanded')).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByText('Implements scoped changes.')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-codex')).not.toHaveTextContent('gpt-5.4 high')
  })

  it('persists a manual density toggle in localStorage across remounts', async () => {
    const { unmount } = renderBuilder({
      roleTemplates: sampleRoles(6),
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-density-expanded'))

    expect(window.localStorage.getItem('taurhaus.mesh.roleCatalogDensity')).toBe('expanded')
    expect(screen.getByText('Implements scoped changes.')).toBeInTheDocument()

    unmount()

    renderBuilder({
      roleTemplates: sampleRoles(6),
    })

    expect(screen.getByTestId('mesh-builder-density-expanded')).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByText('Implements scoped changes.')).toBeInTheDocument()
  })

  it('hides superseded role versions by default', () => {
    renderBuilder({ roleTemplates: sampleVersionedRoles() })

    expect(screen.queryByTestId('mesh-builder-role-v2-codex-developer')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-v3-codex-developer')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-builder-role-claude-reviewer-legacy')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-claude-reviewer-current')).toBeInTheDocument()
    expect(screen.getByText('Show all versions')).toBeInTheDocument()
  })

  it('persists the show-all-versions toggle across remounts', async () => {
    const { unmount } = renderBuilder({ roleTemplates: sampleVersionedRoles() })

    await fireEvent.click(screen.getByTestId('mesh-builder-version-visibility-toggle'))

    expect(screen.getByTestId('mesh-builder-role-v2-codex-developer')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-claude-reviewer-legacy')).toBeInTheDocument()
    expect(window.localStorage.getItem(ROLE_VERSION_VISIBILITY_STORAGE_KEY)).toBe('true')

    unmount()

    renderBuilder({ roleTemplates: sampleVersionedRoles() })

    expect(screen.getByTestId('mesh-builder-role-v2-codex-developer')).toBeInTheDocument()
    expect(screen.getByText('Latest only')).toBeInTheDocument()
  })

  it('pins and unpins roles in expanded mode and hides the strip when empty', async () => {
    renderBuilder()

    expect(screen.queryByTestId('mesh-builder-pinned-strip')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-pin-agent-codex'))

    expect(screen.getByTestId('mesh-builder-pinned-strip')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-pinned-row-agent-codex')).toHaveClass(
      'mesh-builder-role-row'
    )
    expect(screen.getByTestId('mesh-builder-pinned-chip-agent-codex')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-builder-role-row-agent-codex')).not.toBeInTheDocument()
    expect(window.localStorage.getItem(PINNED_ROLE_IDS_STORAGE_KEY)).toBe(
      JSON.stringify(['agent-codex'])
    )

    await fireEvent.click(screen.getByTestId('mesh-builder-pin-agent-codex'))

    expect(screen.queryByTestId('mesh-builder-pinned-strip')).not.toBeInTheDocument()
    expect(window.localStorage.getItem(PINNED_ROLE_IDS_STORAGE_KEY)).toBe(JSON.stringify([]))
  })

  it('bounces the pin toggle briefly when starring a role', async () => {
    vi.useFakeTimers()

    renderBuilder()

    const pinButton = screen.getByTestId('mesh-builder-pin-agent-codex')

    await fireEvent.click(pinButton)

    expect(screen.getByTestId('mesh-builder-pin-agent-codex')).toHaveClass('mesh-builder-pin-bounce')

    await vi.advanceTimersByTimeAsync(201)

    expect(pinButton).not.toHaveClass('mesh-builder-pin-bounce')
  })

  it('restores pinned roles from localStorage on remount', () => {
    window.localStorage.setItem(
      PINNED_ROLE_IDS_STORAGE_KEY,
      JSON.stringify(['lead-codex', 'agent-antigravity'])
    )

    renderBuilder()

    expect(screen.getByTestId('mesh-builder-pinned-chip-lead-codex')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-pinned-chip-agent-antigravity')).toBeInTheDocument()
  })

  it('pins roles in compact mode and lets pinned chips assign the same role callbacks', async () => {
    const onAssignLeadRole = vi.fn()
    const onAppendAgentRole = vi.fn()

    renderBuilder({
      roleTemplates: sampleRoles(6),
      onAssignLeadRole,
      onAppendAgentRole,
    })

    expect(screen.getByTestId('mesh-builder-density-compact')).toHaveAttribute('aria-pressed', 'true')

    await fireEvent.click(screen.getByTestId('mesh-builder-pin-lead-codex'))
    await fireEvent.click(screen.getByTestId('mesh-builder-pin-agent-codex'))

    expect(screen.queryByTestId('mesh-builder-role-row-lead-codex')).not.toBeInTheDocument()
    expect(screen.queryByTestId('mesh-builder-role-row-agent-codex')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-pinned-chip-lead-codex'))
    await fireEvent.click(screen.getByTestId('mesh-builder-pinned-chip-agent-codex'))

    expect(onAssignLeadRole).toHaveBeenCalledWith('lead-codex')
    expect(onAppendAgentRole).toHaveBeenCalledWith('agent-codex')
  })

  it('uses the same compact row treatment for favorites and suppresses native role tooltips', async () => {
    renderBuilder()

    expect(screen.getByTestId('mesh-builder-role-agent-codex')).not.toHaveAttribute('title')

    await fireEvent.click(screen.getByTestId('mesh-builder-pin-agent-codex'))

    expect(screen.getByTestId('mesh-builder-pinned-strip')).toHaveTextContent('Favorites')
    expect(screen.getByTestId('mesh-builder-pinned-row-agent-codex')).toHaveClass(
      'mesh-builder-role-row'
    )
    expect(screen.getByTestId('mesh-builder-pinned-add-agent-codex')).toHaveTextContent('+')
  })

  it('opens the shared role detail overlay from the catalog and adds the selected role from there', async () => {
    const onAppendAgentRole = vi.fn()

    renderBuilder({ onAppendAgentRole })

    await fireEvent.click(screen.getByTestId('mesh-builder-role-info-agent-codex'))

    expect(screen.getByRole('dialog', { name: 'Codex Developer' })).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-add')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-node-detail-add'))

    expect(onAppendAgentRole).toHaveBeenCalledWith('agent-codex')
    expect(screen.queryByTestId('mesh-node-detail')).not.toBeInTheDocument()
  })

  it('creates a new custom role from the builder catalog', async () => {
    const onRefreshRoleTemplates = vi.fn().mockResolvedValue(undefined)

    renderBuilder({ onRefreshRoleTemplates })

    await fireEvent.click(screen.getByTestId('mesh-builder-create-role'))

    expect(screen.getByRole('dialog', { name: 'Create Role' })).toBeInTheDocument()

    await fireEvent.input(screen.getByTestId('mesh-role-editor-name-input'), {
      target: { value: 'QA Specialist' },
    })
    await fireEvent.input(screen.getByTestId('mesh-role-editor-context-summary-input'), {
      target: { value: 'Owns verification context for scoped releases.' },
    })
    await fireEvent.click(screen.getByTestId('mesh-role-editor-save'))

    await waitFor(() => expect(upsertRoleTemplate).toHaveBeenCalledTimes(1))
    expect(upsertRoleTemplate).toHaveBeenCalledWith(
      expect.objectContaining({
        roleId: 'qa-specialist',
        name: 'QA Specialist',
        kind: 'agent',
        defaults: expect.objectContaining({
          cliTool: 'codex',
        }),
        contextSummary: 'Owns verification context for scoped releases.',
      })
    )
    expect(onRefreshRoleTemplates).toHaveBeenCalledTimes(1)
  })

  // Regression: b345de1 (PR 5c) added an effort control to the inline role editor
  // but left `reasoningEffort` out of `serializeRoleDetailDraft`, so changing only
  // the effort never flipped the draft to dirty and the unsaved marker stayed
  // hidden.
  it('marks the role detail dirty when only the reasoning effort changes', async () => {
    getRoleTemplate.mockResolvedValue({
      ...sampleRoles().find((role) => role.roleId === 'agent-codex'),
      model: 'gpt-5.4',
      defaults: {
        cliTool: 'codex',
        model: 'gpt-5.4',
        reasoningEffort: 'high',
        defaultNamePattern: 'agent-codex-{n}',
      },
    })

    renderBuilder()

    await fireEvent.click(screen.getByTestId('mesh-builder-role-info-agent-codex'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-edit'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-model-input-effort')).toHaveValue('high')
    })
    expect(screen.queryByTestId('mesh-node-detail-unsaved-dot')).not.toBeInTheDocument()

    await fireEvent.change(screen.getByTestId('mesh-node-detail-model-input-effort'), {
      target: { value: 'xhigh' },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-node-detail-unsaved-dot')).toBeInTheDocument()
    })
  })

  it('switches from role detail into edit mode and saves role changes', async () => {
    const onRefreshRoleTemplates = vi.fn().mockResolvedValue(undefined)
    getRoleTemplate.mockResolvedValue({
      ...sampleRoles().find((role) => role.roleId === 'agent-codex'),
      instructions: 'Keep implementation scoped.',
      communicationStyle: 'Short progress updates.',
      qualityGates: ['Run targeted tests'],
      definitionOfDone: ['Handoff sent'],
      phaseScope: ['implementation'],
      mode: 'implementation',
      inheritsFrom: 'shared-codex-dev',
      requiredArtifacts: ['notes.md'],
      defaults: {
        cliTool: 'codex',
        model: 'gpt-5.4 high',
        defaultNamePattern: 'agent-codex-{n}',
      },
      behavioralContract: {
        communication: ['Confirm handoffs clearly.'],
        execution: ['Implement the scoped change.'],
        escalation: ['Escalate dependency issues.'],
      },
      constraints: {
        minInstances: 0,
        maxInstances: 8,
        requiresLeadTool: null,
        allowedProjectBinding: 'lead_project',
      },
    })

    renderBuilder({ onRefreshRoleTemplates })

    await fireEvent.click(screen.getByTestId('mesh-builder-role-info-agent-codex'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-edit'))

    expect(await screen.findByRole('dialog', { name: 'Codex Developer' })).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-name-input')).toHaveValue('Codex Developer')
    expect(screen.getByTestId('mesh-node-detail-save')).toHaveTextContent('Save Changes')

    await fireEvent.input(screen.getByTestId('mesh-node-detail-context-input'), {
      target: { value: 'Carries implementation context across PR-sized changes.' },
    })
    await fireEvent.input(screen.getByTestId('mesh-node-detail-communication-style-input'), {
      target: { value: 'Short updates with exact file paths.' },
    })
    await fireEvent.input(screen.getByTestId('mesh-node-detail-quality-gates-input-0'), {
      target: { value: 'Run targeted tests and typecheck' },
    })
    await fireEvent.input(screen.getByTestId('mesh-node-detail-phase-scope-input'), {
      target: { value: 'implementation, verification' },
    })
    await fireEvent.change(screen.getByTestId('mesh-node-detail-mode-input'), {
      target: { value: 'review' },
    })
    await fireEvent.input(screen.getByTestId('mesh-node-detail-inherits-from-input'), {
      target: { value: 'shared-review-codex' },
    })
    await fireEvent.input(screen.getByTestId('mesh-node-detail-required-artifacts-input-0'), {
      target: { value: 'verification-notes.md' },
    })
    await fireEvent.click(screen.getByTestId('mesh-node-detail-save'))

    await waitFor(() => expect(upsertRoleTemplate).toHaveBeenCalledTimes(1))
    expect(upsertRoleTemplate).toHaveBeenCalledWith(
      expect.objectContaining({
        roleId: 'agent-codex',
        contextSummary: 'Carries implementation context across PR-sized changes.',
        communicationStyle: 'Short updates with exact file paths.',
        qualityGates: ['Run targeted tests and typecheck'],
        phaseScope: ['implementation', 'verification'],
        mode: 'review',
        inheritsFrom: 'shared-review-codex',
        requiredArtifacts: ['verification-notes.md'],
        behavioralContract: expect.objectContaining({
          execution: ['Implement the scoped change.'],
        }),
      })
    )
    expect(onRefreshRoleTemplates).toHaveBeenCalledTimes(1)
    expect(screen.queryByTestId('mesh-node-detail-name-input')).not.toBeInTheDocument()
  })

  it('exports role detail as yaml from the shared detail view', async () => {
    renderBuilder()

    await fireEvent.click(screen.getByTestId('mesh-builder-role-info-agent-codex'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-export'))

    await waitFor(() => expect(exportRoleToFile).toHaveBeenCalledWith('agent-codex', 'yaml'))
    await waitFor(() => expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        defaultPath: 'codex-developer.yaml',
      })
    ))
    await waitFor(() => expect(writeTextFile).toHaveBeenCalledWith(
      '/tmp/exported-role.yaml',
      'schema:\n  kind: role_template\n'
    ))
  })

  it('imports yaml into the catalog and refreshes the role list', async () => {
    const onRefreshRoleTemplates = vi.fn().mockResolvedValue(undefined)
    open.mockResolvedValue('/tmp/imported-role.yaml')

    renderBuilder({ onRefreshRoleTemplates })

    await fireEvent.click(screen.getByTestId('mesh-builder-import-yaml'))

    await waitFor(() => expect(importRoleFromFile).toHaveBeenCalledWith('/tmp/imported-role.yaml'))
    expect(onRefreshRoleTemplates).toHaveBeenCalledTimes(1)
    expect(screen.getByTestId('mesh-builder-role-status')).toHaveTextContent("Imported 'Imported Role'.")
  })

  it('confirms and deletes a custom role from the detail view', async () => {
    const onRefreshRoleTemplates = vi.fn().mockResolvedValue(undefined)

    renderBuilder({ onRefreshRoleTemplates })

    await fireEvent.click(screen.getByTestId('mesh-builder-role-info-agent-codex'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-delete'))
    await fireEvent.click(
      screen
        .getAllByTestId('confirm-dialog-confirm')
        .find((element) => element.textContent?.includes('Delete'))
    )

    await waitFor(() => expect(deleteRoleTemplate).toHaveBeenCalledWith('agent-codex'))
    expect(onRefreshRoleTemplates).toHaveBeenCalledTimes(1)
  })

  it('keeps the catalog visible for both empty and populated rosters', () => {
    const { unmount } = renderBuilder()

    expect(screen.getByTestId('mesh-builder-catalog')).toHaveAttribute('data-collapsed', 'false')
    expect(screen.getByTestId('mesh-builder-catalog-content')).toBeInTheDocument()

    unmount()

    renderBuilder({
      teamConfig: sampleRosterConfig(),
    })

    expect(screen.getByTestId('mesh-builder-catalog')).toHaveAttribute('data-collapsed', 'false')
    expect(screen.getByTestId('mesh-builder-catalog-content')).toBeInTheDocument()
  })

  it('uses the updated empty roster copy and add guidance', () => {
    renderBuilder({
      presets: samplePresets(),
      teamConfig: {
        description: '',
        lead: null,
        agents: [],
      },
    })

    expect(screen.getByTestId('mesh-builder-catalog')).toHaveTextContent('Available Roles')
    expect(screen.getByTestId('mesh-builder-team-panel')).toHaveTextContent('Your Team')
    expect(screen.getByTestId('mesh-builder-lead-empty')).toHaveTextContent(
      'Choose a lead role to anchor the team.'
    )
    expect(screen.getByTestId('mesh-builder-lead-empty')).toHaveTextContent(
      'Use the + button next to any lead on the left.'
    )
    expect(screen.getByTestId('mesh-builder-agent-dropzone')).toHaveAttribute(
      'data-dropzone-mode',
      'empty'
    )
    expect(screen.getByTestId('mesh-builder-agent-dropzone')).toHaveTextContent(
      '+ Add from catalog'
    )
    expect(screen.getByTestId('mesh-builder-agent-dropzone')).toHaveTextContent(
      'Start with a developer, researcher, or reviewer to flesh out the team.'
    )
    expect(screen.getByTestId('mesh-builder-preset-section')).toHaveTextContent('Quick start')
    expect(
      screen.getByText('Search roles, pin favorites, and build the lineup from left to right.')
    ).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-template-build-custom')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-action-initialize')).toBeDisabled()
    expect(screen.getByTestId('mesh-action-initialize-hint')).toHaveAttribute(
      'title',
      'Lead role is required.'
    )
    expect(screen.queryByText('1 issue')).not.toBeInTheDocument()
    expect(screen.queryByText('Lead required')).not.toBeInTheDocument()
    expect(screen.queryByText('Required')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-team-lead-group')).toHaveTextContent('Lead')
    expect(screen.getByTestId('mesh-builder-team-agents-group')).toHaveTextContent('Agents')
    expect(screen.getByTestId('mesh-builder-team-summary-card')).toHaveClass('bg-brand-50/65')
    expect(screen.getByTestId('mesh-builder-lead-section')).toHaveClass('border-brand-200/80')
    expect(screen.getByTestId('mesh-builder-agents-section')).toHaveClass('bg-brand-50/55')
    expect(screen.getByTestId('mesh-builder-agent-dropzone')).toHaveClass('bg-white/82')
  })

  it('uses a medium-screen breakpoint for the live two-column roster layout', () => {
    renderBuilder()

    const rosterShell = screen.getByTestId('mesh-builder-shell')

    expect(rosterShell).toHaveClass('md:grid-cols-[minmax(0,1.22fr)_minmax(340px,0.94fr)]')
    expect(rosterShell).toHaveClass('md:h-[calc(100vh-10.75rem)]')
    expect(rosterShell).not.toHaveClass('xl:grid-cols-[minmax(0,1.22fr)_minmax(340px,0.94fr)]')
  })

  it('keeps roster chrome fixed and only makes the role list scroll', () => {
    renderBuilder({
      roleTemplates: sampleRoles(14),
      teamConfig: sampleRosterConfig(),
    })

    expect(screen.getByTestId('mesh-builder-shell')).toHaveClass('overflow-hidden')
    expect(screen.getByTestId('mesh-builder-catalog')).toHaveClass('overflow-hidden')
    expect(screen.getByTestId('mesh-builder-role-scroll')).toHaveClass('md:overflow-y-auto')
    expect(screen.getByTestId('mesh-builder-team-panel')).toHaveClass('overflow-hidden')
    expect(screen.getByTestId('mesh-builder-team-scroll')).toHaveClass('md:overflow-y-auto')
  })

  it('shows collapsed roster summary rows by default and reveals member fields on demand', async () => {
    renderBuilder({
      teamConfig: sampleRosterConfig(),
    })

    expect(screen.getByTestId('mesh-builder-team-panel')).toHaveTextContent('2 members')
    expect(screen.getByTestId('mesh-builder-team-meta')).toHaveTextContent(
      '1 agent supporting the lead.'
    )
    expect(screen.getByTestId('mesh-builder-lead-summary')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-team-summary-card')).toHaveClass('bg-brand-50/65')
    expect(screen.getByTestId('mesh-builder-lead-section')).toHaveClass('bg-brand-50/55')
    expect(screen.getByTestId('mesh-builder-lead-card')).toHaveClass('bg-amber-50')
    expect(screen.queryByTestId('mesh-builder-lead-name-input')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-agents-section')).toHaveTextContent('Codex Developer')
    expect(screen.getByTestId('mesh-builder-agent-summary-agent-codex-1')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-agent-card-agent-codex-1')).toHaveClass('bg-sky-50')
    expect(screen.getByTestId('mesh-builder-agent-summary-agent-codex-1')).toHaveTextContent(
      'builder-1'
    )
    expect(screen.queryByTestId('mesh-builder-agent-name-input-agent-codex-1')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-agent-dropzone')).toHaveAttribute(
      'data-dropzone-mode',
      'compact'
    )
    expect(screen.getByTestId('mesh-builder-agent-dropzone')).toHaveTextContent(
      'Keep building with developer, reviewer, and research roles from the left.'
    )

    await fireEvent.click(screen.getByTestId('mesh-builder-lead-edit-toggle'))
    await fireEvent.click(screen.getByTestId('mesh-builder-agent-edit-toggle-agent-codex-1'))

    expect(await screen.findByTestId('mesh-builder-lead-name-input')).toBeInTheDocument()
    expect(await screen.findByTestId('mesh-builder-agent-name-input-agent-codex-1')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-lead-project-input').tagName).toBe('SELECT')
    expect(screen.getByTestId('mesh-builder-agent-project-input-agent-codex-1').tagName).toBe(
      'SELECT'
    )
    expect(screen.getByTestId('mesh-builder-lead-project-input')).toHaveDisplayValue('taurhaus')
  })

  it('animates new roster entries when the team gains members', async () => {
    vi.useFakeTimers()
    const view = renderBuilder()

    await view.rerender(
      builderProps({
        teamConfig: sampleRosterConfig(),
      })
    )

    const leadCard = screen.getByTestId('mesh-builder-lead-card')
    const agentCard = screen.getByTestId('mesh-builder-agent-card-agent-codex-1')

    expect(leadCard).toHaveClass('content-enter')
    expect(leadCard).toHaveClass('mesh-builder-roster-entry')
    expect(agentCard).toHaveClass('content-enter')
    expect(agentCard).toHaveClass('mesh-builder-roster-entry')

    await vi.advanceTimersByTimeAsync(601)

    expect(leadCard).not.toHaveClass('mesh-builder-roster-entry')
    expect(agentCard).not.toHaveClass('mesh-builder-roster-entry')
  })

  it('uses the shared account picker for Codex members and a truth chip for Claude', async () => {
    const onUpdateAgent = vi.fn()
    renderBuilder({
      teamConfig: sampleRosterConfig(),
      accountStates: {
        claude: {
          accounts: [{ id: 'claude-default', label: 'Claude Default', logged_in: true, is_default: true }],
          defaultAccountId: 'claude-default',
        },
        codex: {
          accounts: [
            { id: 'personal', label: 'Personal', logged_in: true, is_default: true },
            { id: 'work', label: 'Work', logged_in: true, is_default: false },
          ],
          defaultAccountId: 'personal',
        },
      },
      onUpdateAgent,
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-lead-edit-toggle'))
    await fireEvent.click(screen.getByTestId('mesh-builder-agent-edit-toggle-agent-codex-1'))

    expect(screen.getByTestId('mesh-builder-member-account-lead')).toHaveTextContent(
      'Team account · Claude Default'
    )
    await fireEvent.click(screen.getByTestId('mesh-builder-member-account-agent-codex-1'))
    await fireEvent.click(screen.getByTestId('account-option-work'))
    expect(onUpdateAgent).toHaveBeenCalledWith('agent-codex-1', { accountId: 'work' })
  })

  // Regression: 0bc79ceb made the Claude team truth chip prefer the app-launch
  // global default even though managed teams still launch from the registry home.
  it('names the registry-home Claude account in the team truth chip', async () => {
    renderBuilder({
      teamConfig: sampleRosterConfig(),
      accountStates: {
        claude: {
          accounts: [
            { id: 'claude-home', label: 'Claude Home', logged_in: true, is_default: true },
            { id: 'claude-work', label: 'Claude Work', logged_in: true, is_default: false },
          ],
          defaultAccountId: 'claude-work',
        },
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-lead-edit-toggle'))

    expect(screen.getByTestId('mesh-builder-member-account-lead')).toHaveTextContent(
      'Team account · Claude Home'
    )
  })

  // Regression: 0bc79ceb made a new Codex member's account row prefer the
  // app-launch global default even though managed teams launch from the registry home.
  it('names the registry-home Codex account when the member has no account choice', async () => {
    renderBuilder({
      teamConfig: sampleRosterConfig(),
      accountStates: {
        codex: {
          accounts: [
            { id: 'personal', label: 'Personal', logged_in: true, is_default: true },
            { id: 'work', label: 'Work', logged_in: true, is_default: false },
          ],
          defaultAccountId: 'work',
        },
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-agent-edit-toggle-agent-codex-1'))

    expect(screen.getByTestId('mesh-builder-member-account-agent-codex-1')).toHaveTextContent(
      'Personal'
    )
  })

  // Regression: 95848682 fixed the row label to mirror the registry home but
  // left the popover preselecting the app-launch global default, which
  // `managed_member_account` never consults. The row said Personal while the
  // picker opened on Work, so confirming without moving pinned Work.
  it('opens the member account popover on the account the row names', async () => {
    renderBuilder({
      teamConfig: sampleRosterConfig(),
      accountStates: {
        codex: {
          accounts: [
            { id: 'personal', label: 'Personal', logged_in: true, is_default: true },
            { id: 'work', label: 'Work', logged_in: true, is_default: false },
          ],
          defaultAccountId: 'work',
        },
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-builder-agent-edit-toggle-agent-codex-1'))
    await fireEvent.click(screen.getByTestId('mesh-builder-member-account-agent-codex-1'))

    expect(screen.getByTestId('account-option-personal')).toHaveAttribute(
      'data-preselected',
      'true'
    )
    expect(screen.getByTestId('account-option-work')).toHaveAttribute('data-preselected', 'false')
  })

  it('waits for the exit animation before removing an agent card', async () => {
    vi.useFakeTimers()
    const onRemoveAgent = vi.fn()

    renderBuilder({
      teamConfig: sampleRosterConfig(),
      onRemoveAgent,
    })

    const agentCard = screen.getByTestId('mesh-builder-agent-card-agent-codex-1')

    await fireEvent.click(screen.getByTestId('mesh-builder-agent-remove-agent-codex-1'))

    expect(agentCard).toHaveClass('mesh-builder-roster-exit')
    expect(onRemoveAgent).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(119)

    expect(onRemoveAgent).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(1)

    expect(onRemoveAgent).toHaveBeenCalledWith('agent-codex-1')
    expect(agentCard).not.toHaveClass('mesh-builder-roster-exit')
  })

  it('keeps reset, save, and initialize actions in the sticky footer bar', () => {
    renderBuilder({
      presets: samplePresets(),
      teamConfig: sampleRosterConfig(),
    })

    const actionBar = screen.getByTestId('mesh-action-bar')

    expect(actionBar).toContainElement(screen.getByTestId('mesh-builder-save-preset'))
    expect(actionBar).toContainElement(screen.getByTestId('mesh-action-reset'))
    expect(actionBar).toContainElement(screen.getByTestId('mesh-action-initialize'))
    expect(screen.queryByTestId('mesh-template-build-custom')).not.toBeInTheDocument()
  })

  it('renders presets as compact rows with summaries, tool medallions, and built-in badges', async () => {
    const onApplyPreset = vi.fn()

    renderBuilder({
      presets: samplePresets(),
      onApplyPreset,
    })

    const fullTeamPreset = screen.getByTestId('mesh-template-preset-full-team')
    expect(fullTeamPreset).toHaveClass('active:scale-[0.98]')
    expect(fullTeamPreset).toHaveAttribute(
      'title',
      'Lead, architect, and two developers.'
    )
    expect(screen.getByTestId('mesh-template-preset-summary-full-team')).toHaveTextContent(
      '3 agents · 1 lead'
    )
    expect(screen.getByTestId('mesh-builder-preset-section')).toHaveTextContent('2 presets')
    expect(screen.getByTestId('mesh-template-preset-tool-full-team-claude')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-template-preset-tool-full-team-codex')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-template-preset-built-in-full-team')).toBeInTheDocument()

    await fireEvent.click(fullTeamPreset)

    expect(onApplyPreset).toHaveBeenCalledWith(samplePresets()[0])
    expect(screen.getByTestId('mesh-template-preset-summary-research-pod')).toHaveTextContent(
      '2 agents · 1 lead'
    )
    expect(screen.queryByTestId('mesh-template-preset-built-in-research-pod')).not.toBeInTheDocument()
  })
})

describe('MeshTeamBuilder role-inherited reasoning effort', () => {
  function optionValues(select) {
    return Array.from(select.querySelectorAll('option')).map((option) => option.value)
  }

  function roleBoundTeamConfig() {
    return {
      description: '',
      lead: {
        id: 'lead',
        name: 'team-lead',
        roleId: 'lead-codex',
        roleName: 'Codex Product Lead',
        tool: 'codex',
        model: 'gpt-5.6-terra',
        reasoningEffort: null,
        projectId: '/projects/taurhaus',
      },
      agents: [
        {
          id: 'agent-codex-1',
          name: 'builder-1',
          roleId: 'agent-codex',
          roleName: 'Codex Developer',
          tool: 'codex',
          model: 'gpt-5.6-terra',
          reasoningEffort: null,
          projectId: '/projects/taurhaus',
        },
      ],
    }
  }

  // Regression: b345de1 (PR 5c) offered an empty "default" effort option on every
  // roster row. Picking it sent reasoning_effort: null, which the backend refills
  // from the member's role template (`apply_role_template_defaults`,
  // request_normalization.rs), so a role declaring "high" still launched at high
  // while the row claimed the CLI global applied.
  it('shows the role-declared effort instead of a misleading default option', async () => {
    renderBuilder({ teamConfig: roleBoundTeamConfig() })

    await fireEvent.click(screen.getByLabelText('Edit builder-1 details'))

    const effort = await screen.findByTestId('mesh-builder-agent-model-input-agent-codex-1-effort')
    expect(effort).toHaveValue('high')
    expect(optionValues(effort)).not.toContain('')
  })

  it('shows the lead role-declared effort the same way', async () => {
    renderBuilder({ teamConfig: roleBoundTeamConfig() })

    await fireEvent.click(screen.getByLabelText('Edit lead details'))

    const effort = await screen.findByTestId('mesh-builder-lead-model-input-effort')
    expect(effort).toHaveValue('high')
    expect(optionValues(effort)).not.toContain('')
  })

  it('keeps the inherit-global option for a member whose role declares no effort', async () => {
    const config = roleBoundTeamConfig()
    config.agents[0].roleId = 'agent-antigravity'
    config.agents[0].tool = 'codex'
    renderBuilder({ teamConfig: config })

    await fireEvent.click(screen.getByLabelText('Edit builder-1 details'))

    const effort = await screen.findByTestId('mesh-builder-agent-model-input-agent-codex-1-effort')
    expect(optionValues(effort)).toContain('')
    expect(effort).toHaveValue('')
  })
})
