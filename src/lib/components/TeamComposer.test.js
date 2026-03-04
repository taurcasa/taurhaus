import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  listRoleTemplates: vi.fn(),
  composeTeam: vi.fn(),
}))

const { listRoleTemplates, composeTeam } = await import('../ipc.js')

import TeamComposer from './TeamComposer.svelte'

function mockCompositionResponse() {
  return {
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
      {
        name: 'dev-1',
        roleId: 'codex-developer',
        roleKind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.3-codex',
        instructions: 'Agent instructions',
        projectBinding: 'lead_project',
        projectId: '/projects/taurhaus',
      },
    ],
    warnings: [],
    validationErrors: [],
  }
}

describe('TeamComposer', () => {
  beforeEach(() => {
    vi.clearAllMocks()

    listRoleTemplates.mockResolvedValue([
      {
        roleId: 'claude-orchestrator',
        name: 'Claude Orchestrator',
        kind: 'lead',
        cliTool: 'claude',
        model: 'claude-opus-4-6',
        capabilities: ['planning'],
      },
      {
        roleId: 'codex-developer',
        name: 'Codex Developer',
        kind: 'agent',
        cliTool: 'codex',
        model: 'gpt-5.3-codex',
        capabilities: ['implementation'],
      },
      {
        roleId: 'claude-reviewer',
        name: 'Claude Reviewer',
        kind: 'agent',
        cliTool: 'claude',
        model: 'claude-opus-4-6',
        capabilities: ['review'],
      },
    ])

    composeTeam.mockResolvedValue(mockCompositionResponse())
  })

  it('renders composer sections', async () => {
    render(TeamComposer, {
      props: {
        dark: false,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('lead-role-picker')).toBeInTheDocument()
    })

    expect(screen.getByTestId('agent-role-selector')).toBeInTheDocument()
    expect(screen.getByTestId('roster-review')).toBeInTheDocument()
    expect(screen.getByTestId('composition-validator')).toBeInTheDocument()
    expect(screen.getByTestId('save-as-preset')).toBeInTheDocument()
  })

  it('pre-populates lead and agent quantities from initial preset', async () => {
    render(TeamComposer, {
      props: {
        initialPreset: {
          presetId: 'docs-sprint',
          leadRoleId: 'claude-orchestrator',
          agentSlots: [{ roleId: 'codex-developer', count: 2 }],
        },
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('composer-lead-select')).toHaveValue('claude-orchestrator')
    })

    expect(screen.getByTestId('agent-count-codex-developer')).toHaveTextContent('2')
  })

  it('shows validation errors and warnings from composition', async () => {
    composeTeam.mockResolvedValue({
      ...mockCompositionResponse(),
      warnings: ['Constraint warning from composition'],
      validationErrors: ['Backend validation failure'],
    })

    render(TeamComposer, {
      props: {
        initialPreset: {
          presetId: 'review-team',
          leadRoleId: 'claude-orchestrator',
          agentSlots: [{ roleId: 'claude-reviewer', count: 1 }],
        },
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('composer-validation-errors')).toBeInTheDocument()
    })

    expect(screen.getByText('Backend validation failure')).toBeInTheDocument()
    expect(screen.getByTestId('composer-validation-warnings')).toHaveTextContent(
      'Constraint warning from composition'
    )
  })

  it('supports roster inline edits and applies mesh-init payload', async () => {
    const onApply = vi.fn()

    render(TeamComposer, {
      props: {
        projectPath: '/projects/taurhaus',
        initialPreset: {
          presetId: 'fullstack-dev',
          leadRoleId: 'claude-orchestrator',
          agentSlots: [{ roleId: 'codex-developer', count: 1 }],
        },
        onApply,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('composer-roster-card-1')).toBeInTheDocument()
    })

    await fireEvent.input(screen.getByTestId('composer-roster-name-1'), {
      target: { value: 'api-dev' },
    })
    await fireEvent.change(screen.getByTestId('composer-roster-tool-1'), {
      target: { value: 'gemini' },
    })
    await fireEvent.input(screen.getByTestId('composer-roster-model-1'), {
      target: { value: 'gemini-2.5-pro' },
    })
    await fireEvent.input(screen.getByTestId('composer-roster-instructions-1'), {
      target: { value: 'Write API docs and tests.' },
    })

    await fireEvent.click(screen.getByTestId('composer-apply'))

    expect(onApply).toHaveBeenCalledTimes(1)
    const payload = onApply.mock.calls[0][0]
    expect(payload.leadMode).toBe('launch_new')
    expect(payload.lead.name).toBe('lead-project')
    expect(payload.agents[0]).toMatchObject({
      name: 'api-dev',
      cliTool: 'gemini',
      model: 'gemini-2.5-pro',
    })
    expect(payload.roster[1].instructions).toBe('Write API docs and tests.')
  })

  it('keeps apply enabled when roster edits introduce duplicate names', async () => {
    const onApply = vi.fn()

    render(TeamComposer, {
      props: {
        projectPath: '/projects/taurhaus',
        initialPreset: {
          presetId: 'fullstack-dev',
          leadRoleId: 'claude-orchestrator',
          agentSlots: [{ roleId: 'codex-developer', count: 1 }],
        },
        onApply,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('composer-roster-card-1')).toBeInTheDocument()
    })

    await fireEvent.input(screen.getByTestId('composer-roster-name-1'), {
      target: { value: 'lead-project' },
    })

    await waitFor(() => {
      expect(screen.getByTestId('composer-validation-warnings')).toHaveTextContent(
        'Name collisions: lead-project.'
      )
    })

    expect(screen.getByTestId('composer-apply')).not.toBeDisabled()
    await fireEvent.click(screen.getByTestId('composer-apply'))
    expect(onApply).toHaveBeenCalledTimes(1)
  })
})
