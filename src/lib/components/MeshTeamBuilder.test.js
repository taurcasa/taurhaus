import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshTeamBuilder from './MeshTeamBuilder.svelte'

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
})
