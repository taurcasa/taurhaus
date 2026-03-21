import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
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
