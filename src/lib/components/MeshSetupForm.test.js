import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  listTeamPresets: vi.fn(),
  getTeamPreset: vi.fn(),
  composeTeam: vi.fn(),
  listRoleTemplates: vi.fn(),
  getRoleTemplate: vi.fn(),
  getTemplateStorageStatus: vi.fn(),
  getTemplateHistory: vi.fn(),
  getTemplateDiff: vi.fn(),
  revertTemplateVersion: vi.fn(),
}))

const {
  listTeamPresets,
  getTeamPreset,
  composeTeam,
  listRoleTemplates,
  getRoleTemplate,
  getTemplateStorageStatus,
  getTemplateHistory,
  getTemplateDiff,
} =
  await import('../ipc.js')

import MeshSetupForm from './MeshSetupForm.svelte'

async function openCustomize() {
  await fireEvent.click(screen.getByTestId('mesh-advanced-toggle'))
  await waitFor(() => {
    expect(screen.getByTestId('mesh-team-basics')).toBeInTheDocument()
  })
}

function createDeferred() {
  /** @type {(value: any) => void} */
  let resolve
  /** @type {(reason?: any) => void} */
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('MeshSetupForm', () => {
  const availableProjects = [
    { id: 'proj-web', name: 'Web UI' },
    { id: 'proj-api', name: 'API Service' },
  ]

  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()

    const roleCatalog = {
      'claude-orchestrator': {
        roleId: 'claude-orchestrator',
        kind: 'lead',
        cliTool: 'claude',
        model: 'claude-opus-4-6',
        instructions: 'Lead instructions',
      },
      'codex-developer': {
        roleId: 'codex-developer',
        kind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.3-codex',
        instructions: 'Implement code',
      },
      'claude-reviewer': {
        roleId: 'claude-reviewer',
        kind: 'agent',
        cliTool: 'claude',
        model: 'claude-opus-4-6',
        instructions: 'Review changes',
      },
      'research-dev': {
        roleId: 'research-dev',
        kind: 'agent',
        cliTool: 'gemini',
        model: 'gemini-2.5-pro',
        instructions: 'Research and summarize',
      },
    }

    const presets = [
      {
        presetId: 'fullstack-dev',
        name: 'Fullstack Dev',
        leadRoleId: 'claude-orchestrator',
        agentSlots: [{ roleId: 'codex-developer', count: 2, projectBinding: 'lead_project' }],
      },
      {
        presetId: 'review-team',
        name: 'Review Team',
        leadRoleId: 'claude-orchestrator',
        agentSlots: [{ roleId: 'claude-reviewer', count: 2, projectBinding: 'lead_project' }],
      },
      {
        presetId: 'research-dev',
        name: 'Research Dev',
        leadRoleId: 'claude-orchestrator',
        agentSlots: [{ roleId: 'research-dev', count: 1, projectBinding: 'lead_project' }],
      },
    ]

    listTeamPresets.mockResolvedValue(
      presets.map((preset) => ({
        presetId: preset.presetId,
        name: preset.name,
        leadRoleId: preset.leadRoleId,
      }))
    )
    getTeamPreset.mockImplementation(async (presetId) => {
      const preset = presets.find((entry) => entry.presetId === presetId)
      return preset ? { ...preset } : null
    })

    composeTeam.mockImplementation(async (request) => {
      const leadRole = roleCatalog[request?.leadRoleId] ?? roleCatalog['claude-orchestrator']
      const roster = [
        {
          name: 'team-lead',
          roleId: leadRole.roleId,
          roleKind: 'lead',
          cliTool: leadRole.cliTool,
          model: leadRole.model,
          instructions: leadRole.instructions,
          projectId: '/projects/taurhaus',
        },
      ]

      for (const slot of request?.agentSlots ?? []) {
        const role = roleCatalog[slot.roleId] ?? roleCatalog['codex-developer']
        const count = Number(slot.count ?? 0)
        for (let i = 0; i < count; i += 1) {
          roster.push({
            name: `${role.roleId}-${i + 1}`,
            roleId: role.roleId,
            roleKind: 'agent',
            cliTool: role.cliTool,
            model: role.model,
            instructions: role.instructions,
            projectId: '/projects/taurhaus',
          })
        }
      }

      return {
        roster,
        warnings: [],
        validationErrors: [],
      }
    })

    listRoleTemplates.mockResolvedValue(
      Object.values(roleCatalog).map((role) => ({
        roleId: role.roleId,
        name: role.roleId,
        kind: role.kind,
        cliTool: role.cliTool,
        model: role.model,
        capabilities: [],
      }))
    )
    getRoleTemplate.mockResolvedValue({
      roleId: 'claude-orchestrator',
      name: 'claude-orchestrator',
      instructions: 'Lead instructions',
      behavioralContract: { communication: [], execution: [], escalation: [] },
    })

    getTemplateStorageStatus.mockResolvedValue({
      mode: 'git',
      repoInitialized: true,
      dirty: false,
      pendingActions: [],
      lastCommit: 1_706_000_000,
    })
    getTemplateHistory.mockResolvedValue({ commits: [], nextCursor: null })
    getTemplateDiff.mockResolvedValue({
      commitId: '',
      files: [],
      stats: { filesChanged: 0, insertions: 0, deletions: 0 },
    })
  })

  it('renders template picker controls', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-template-picker')).toBeInTheDocument()
    })

    expect(screen.getByTestId('mesh-template-preset-fullstack-dev')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-template-browse-catalog')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-template-build-custom')).toBeInTheDocument()
  })

  it('preset quick-select populates the editable roster', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-template-preset-fullstack-dev')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-fullstack-dev'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-template-notice')).toHaveTextContent('Applied preset')
    })
    expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(2)
    expect(screen.getByTestId('mesh-agent-tool-select-0')).toHaveValue('codex')
  })

  it('blank slate resets roster after template application', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-template-preset-review-team')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-review-team'))
    await waitFor(() => {
      expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(2)
    })

    await fireEvent.click(screen.getByTestId('mesh-template-blank-slate'))
    await waitFor(() => {
      expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(1)
    })
  })

  it('ignores stale preset responses when switching presets quickly', async () => {
    const firstPreset = createDeferred()
    getTeamPreset.mockImplementationOnce(() => firstPreset.promise)

    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-template-preset-fullstack-dev')).toBeInTheDocument()
      expect(screen.getByTestId('mesh-template-preset-review-team')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-template-preset-fullstack-dev'))
    await fireEvent.click(screen.getByTestId('mesh-template-preset-review-team'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-template-notice')).toHaveTextContent(
        'Applied preset: Review Team'
      )
    })
    expect(screen.getByTestId('mesh-agent-name-input-0')).toHaveValue('claude-reviewer-1')

    firstPreset.resolve({
      presetId: 'fullstack-dev',
      name: 'Fullstack Dev',
      leadRoleId: 'claude-orchestrator',
      agentSlots: [{ roleId: 'codex-developer', count: 2, projectBinding: 'lead_project' }],
    })

    await Promise.resolve()
    await Promise.resolve()
    expect(screen.getByTestId('mesh-agent-name-input-0')).toHaveValue('claude-reviewer-1')
  })

  it('build custom team opens TeamComposer flow', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-template-build-custom'))

    await waitFor(() => {
      expect(screen.getByTestId('team-composer')).toBeInTheDocument()
    })
  })

  it('shows roster-style team preview with lead and default agent', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getByTestId('mesh-roster-preview')).toBeInTheDocument()
    expect(screen.getByText('team-lead')).toBeInTheDocument()
    expect(screen.getByText('Lead')).toBeInTheDocument()
    expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(1)
    expect(screen.queryByTestId('mesh-lead-card')).not.toBeInTheDocument()
    expect(screen.queryByTestId('mesh-review-panel')).not.toBeInTheDocument()
  })

  it('agent rows are always visible without opening customize', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(1)
    expect(screen.getByTestId('mesh-agent-name-input-0')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-agent-tool-select-0')).toBeInTheDocument()
  })

  it('add agent creates a new agent card', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(1)
    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    expect(screen.getAllByTestId('mesh-agent-card')).toHaveLength(2)
  })

  it('renders onboarding banner and allows dismissing it', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getByTestId('mesh-onboarding-banner')).toBeInTheDocument()
    await fireEvent.click(screen.getByTestId('mesh-onboarding-dismiss'))

    await waitFor(() => {
      expect(screen.queryByTestId('mesh-onboarding-banner')).not.toBeInTheDocument()
    })
    expect(localStorage.getItem('mesh-onboarding-dismissed')).toBe('true')
  })

  it('shows plain tool labels without icon prefixes', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    const select = screen.getByTestId('mesh-agent-tool-select-0')
    const labels = Array.from(select.querySelectorAll('option')).map((option) => option.textContent)
    expect(labels).toEqual(['Claude', 'Codex', 'Gemini'])
  })

  it('customize toggle reveals team name and description fields', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.queryByTestId('mesh-team-name-input')).not.toBeInTheDocument()
    await openCustomize()
    const description = screen.getByTestId('mesh-team-description-input')
    expect(description.tagName).toBe('INPUT')
    expect(description).toHaveAttribute('placeholder', "Optional — describe the team's purpose")
  })

  it('auto-selects project when current project matches an option', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects: [
          { id: '/projects/taurhaus', name: 'taurhaus' },
          { id: '/projects/other', name: 'Other' },
        ],
      },
    })

    expect(screen.getByTestId('mesh-agent-project-select-0')).toHaveValue('/projects/taurhaus')

    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    expect(screen.getByTestId('mesh-agent-project-select-1')).toHaveValue('/projects/taurhaus')
  })

  it('auto-selects project when only one project exists', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects: [{ id: 'proj-only', name: 'Only Project' }],
      },
    })

    expect(screen.getByTestId('mesh-agent-project-select-0')).toHaveValue('proj-only')
  })

  it('all agent rows have border-t separator', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    const cards = screen.getAllByTestId('mesh-agent-card')
    expect(cards[0].className).toContain('border-t')
    expect(cards[1].className).toContain('border-t')
  })

  it('surfaces multiple warnings as one subtle banner', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
        preflightWarnings: [{ message: 'MESH_DAEMON_NOT_RUNNING' }, { message: 'TMUX_MISSING' }],
      },
    })

    expect(screen.getByTestId('mesh-setup-warnings')).toHaveTextContent(
      'Some tools may need installation. You can still start \u2014 agents will report issues.'
    )
  })

  it('duplicate names show warning but still submit initialization request', async () => {
    const onInitialize = vi.fn()
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
        oninitialize: onInitialize,
      },
    })

    await fireEvent.input(screen.getByTestId('mesh-agent-name-input-0'), {
      target: { value: 'dupe' },
    })
    await fireEvent.click(screen.getByTestId('mesh-add-agent-button'))
    await fireEvent.input(screen.getByTestId('mesh-agent-name-input-1'), {
      target: { value: 'dupe' },
    })

    await waitFor(() => {
      expect(screen.getByTestId('mesh-duplicate-name-error')).toBeInTheDocument()
    })

    expect(screen.getByTestId('mesh-create-team-button')).not.toBeDisabled()
    await fireEvent.click(screen.getByTestId('mesh-create-team-button'))
    expect(onInitialize).toHaveBeenCalledTimes(1)
  })

  it('start team button is always enabled with defaults', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
      },
    })

    expect(screen.getByTestId('mesh-create-team-button')).not.toBeDisabled()
  })

  it('prevents double-submit on rapid start team clicks', async () => {
    const onInitialize = vi.fn()
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
        oninitialize: onInitialize,
      },
    })

    const startButton = screen.getByTestId('mesh-create-team-button')
    await fireEvent.click(startButton)
    await fireEvent.click(startButton)

    expect(onInitialize).toHaveBeenCalledTimes(1)
    expect(screen.getByTestId('mesh-create-team-button')).toBeDisabled()
    expect(screen.getByTestId('mesh-create-team-button')).toHaveTextContent('Starting')
  })

  it('infers team name from UNC project path', async () => {
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '\\\\wsl$\\Ubuntu\\home\\mstie\\projects\\taurhaus',
        availableProjects,
      },
    })

    await openCustomize()
    await waitFor(() => {
      expect(screen.getByTestId('mesh-team-name-input')).toHaveValue('taurhaus-team')
    })
  })

  it('start team emits expected payload with auto-generated agent name', async () => {
    const onInitialize = vi.fn()
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
        oninitialize: onInitialize,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-create-team-button'))

    expect(onInitialize).toHaveBeenCalledTimes(1)
    expect(onInitialize).toHaveBeenCalledWith({
      teamName: 'taurhaus-team',
      teamDescription: null,
      leadMode: 'launch_new',
      lead: {
        name: 'team-lead',
        cliTool: 'claude',
        model: 'opus',
        projectId: '/projects/taurhaus',
        description: 'Team lead',
        roleId: null,
        instructions: null,
        behavioralContract: null,
        capabilities: null,
      },
      agents: [
        {
          name: 'taurhaus-dev',
          cliTool: 'codex',
          model: 'gpt-5.3-codex',
          projectId: '/projects/taurhaus',
          description: null,
          roleId: null,
          instructions: null,
          behavioralContract: null,
          capabilities: null,
        },
      ],
    })
  })

  it('initialize emits correct payload with customized agent', async () => {
    const onInitialize = vi.fn()
    render(MeshSetupForm, {
      props: {
        dark: false,
        projectPath: '/projects/taurhaus',
        availableProjects,
        oninitialize: onInitialize,
      },
    })

    await fireEvent.input(screen.getByTestId('mesh-agent-name-input-0'), {
      target: { value: 'frontend-dev' },
    })
    await fireEvent.change(screen.getByTestId('mesh-agent-project-select-0'), {
      target: { value: 'proj-web' },
    })

    await fireEvent.click(screen.getByTestId('mesh-create-team-button'))

    expect(onInitialize).toHaveBeenCalledTimes(1)
    expect(onInitialize).toHaveBeenCalledWith(
      expect.objectContaining({
        teamName: 'taurhaus-team',
        teamDescription: null,
        leadMode: 'launch_new',
        lead: expect.objectContaining({
          name: 'team-lead',
          cliTool: 'claude',
          model: 'opus',
          projectId: '/projects/taurhaus',
        }),
        agents: [
          expect.objectContaining({
            name: 'frontend-dev',
            cliTool: 'codex',
            model: 'gpt-5.3-codex',
            projectId: 'proj-web',
          }),
        ],
      })
    )
  })
})
