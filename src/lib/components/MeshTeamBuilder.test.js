import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshTeamBuilder from './MeshTeamBuilder.svelte'

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
    },
    {
      roleId: 'lead-codex',
      name: 'Codex Product Lead',
      kind: 'lead',
      cliTool: 'codex',
      model: 'gpt-5.4 high',
      behaviorSummary: 'Owns execution planning.',
    },
    {
      roleId: 'agent-codex',
      name: 'Codex Developer',
      kind: 'agent',
      cliTool: 'codex',
      model: 'gpt-5.4 high',
      behaviorSummary: 'Implements scoped changes.',
    },
    {
      roleId: 'agent-gemini',
      name: 'Gemini Researcher',
      kind: 'agent',
      cliTool: 'gemini',
      model: 'gemini-2.5-pro',
      behaviorSummary: 'Finds source material.',
    },
  ]

  for (let index = 0; index < extraAgentCount; index += 1) {
    roles.push({
      roleId: `agent-extra-${index + 1}`,
      name: `Extra Agent ${index + 1}`,
      kind: 'agent',
      cliTool: index % 2 === 0 ? 'codex' : 'gemini',
      model: index % 2 === 0 ? 'gpt-5.4 high' : 'gemini-2.5-pro',
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
      tools: ['gemini', 'claude'],
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

function renderBuilder(props = {}) {
  return render(MeshTeamBuilder, {
    props: {
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
      availableProjects: [],
      onBuildCustom: vi.fn(),
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
      onInitialize: vi.fn(),
      onReset: vi.fn(),
      onSavePreset: vi.fn(),
      ...props,
    },
  })
}

describe('MeshTeamBuilder', () => {
  beforeEach(() => {
    window.localStorage.clear()
  })

  it('filters roles by tool icon toggle', async () => {
    renderBuilder()

    expect(screen.getByTestId('mesh-builder-role-lead-claude')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-codex')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-filter-tool-codex'))

    expect(screen.queryByTestId('mesh-builder-role-lead-claude')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-lead-codex')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-codex')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-builder-role-agent-gemini')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('mesh-builder-filter-tool-codex'))

    expect(screen.getByTestId('mesh-builder-role-lead-claude')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-gemini')).toBeInTheDocument()
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
    expect(screen.queryByText('Implements scoped changes.')).not.toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-role-agent-codex')).toHaveAttribute(
      'title',
      'Implements scoped changes.'
    )

    await fireEvent.click(screen.getByTestId('mesh-builder-role-agent-codex'))

    expect(onAppendAgentRole).toHaveBeenCalledWith('agent-codex')
  })

  it('defaults to expanded density when eight or fewer roles are visible', () => {
    renderBuilder()

    expect(screen.getByTestId('mesh-builder-density-expanded')).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByText('Implements scoped changes.')).toBeInTheDocument()
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
    expect(screen.getByTestId('mesh-builder-pinned-chip-agent-codex')).toBeInTheDocument()
    expect(window.localStorage.getItem(PINNED_ROLE_IDS_STORAGE_KEY)).toBe(
      JSON.stringify(['agent-codex'])
    )

    await fireEvent.click(screen.getByTestId('mesh-builder-pin-agent-codex'))

    expect(screen.queryByTestId('mesh-builder-pinned-strip')).not.toBeInTheDocument()
    expect(window.localStorage.getItem(PINNED_ROLE_IDS_STORAGE_KEY)).toBe(JSON.stringify([]))
  })

  it('restores pinned roles from localStorage on remount', () => {
    window.localStorage.setItem(
      PINNED_ROLE_IDS_STORAGE_KEY,
      JSON.stringify(['lead-codex', 'agent-gemini'])
    )

    renderBuilder()

    expect(screen.getByTestId('mesh-builder-pinned-chip-lead-codex')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-pinned-chip-agent-gemini')).toBeInTheDocument()
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

    await fireEvent.click(screen.getByTestId('mesh-builder-pinned-chip-lead-codex'))
    await fireEvent.click(screen.getByTestId('mesh-builder-pinned-chip-agent-codex'))

    expect(onAssignLeadRole).toHaveBeenCalledWith('lead-codex')
    expect(onAppendAgentRole).toHaveBeenCalledWith('agent-codex')
  })

  it('uses the updated empty roster copy and empty agent dropzone treatment', () => {
    renderBuilder({
      teamConfig: {
        description: '',
        lead: null,
        agents: [],
      },
    })

    expect(screen.getByTestId('mesh-builder-lead-empty')).toHaveTextContent('Pick a lead role')
    expect(screen.getByTestId('mesh-builder-lead-empty')).toHaveTextContent(
      'Click a lead in the catalog or drag one here.'
    )
    expect(screen.getByTestId('mesh-builder-agent-dropzone')).toHaveAttribute(
      'data-dropzone-mode',
      'empty'
    )
    expect(screen.getByTestId('mesh-builder-agent-dropzone')).toHaveTextContent(
      'Drag or click a role to add'
    )
  })

  it('shows compact roster summary rows and a compact add row when roles are assigned', () => {
    renderBuilder({
      teamConfig: sampleRosterConfig(),
    })

    expect(screen.getByTestId('mesh-builder-lead-section')).toHaveTextContent('1 assigned')
    expect(screen.getByTestId('mesh-builder-lead-summary')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-agents-section')).toHaveTextContent('1 assigned')
    expect(screen.getByTestId('mesh-builder-agent-summary-agent-codex-1')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-builder-agent-dropzone')).toHaveAttribute(
      'data-dropzone-mode',
      'compact'
    )
  })

  it('renders presets as compact rows with summaries, tool medallions, and built-in badges', async () => {
    const onApplyPreset = vi.fn()

    renderBuilder({
      presets: samplePresets(),
      onApplyPreset,
    })

    const fullTeamPreset = screen.getByTestId('mesh-template-preset-full-team')
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
