import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import RoleEditor from './RoleEditor.svelte'
import { TEST_MODEL_CATALOG } from '../../test/fixtures/modelCatalog.js'
import { normalizeRoleTemplate } from './templateBrowserUtils.js'

describe('RoleEditor', () => {
  it('renders context-steering fields and removes capability editing', () => {
    render(RoleEditor, {
      props: {
        open: true,
        dark: true,
      },
    })

    expect(screen.getByTestId('role-editor-name-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-id-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-tool-select')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-model-select')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-focus-area-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-context-summary-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-behavior-summary-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-instructions-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-add-rule-input')).toBeInTheDocument()
    expect(screen.queryByTestId('role-editor-add-capability-input')).not.toBeInTheDocument()
    expect(screen.getByTestId('role-editor-save')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-cancel')).toBeInTheDocument()
  })

  it('generates role id from name', async () => {
    render(RoleEditor, {
      props: {
        open: true,
        modelCatalog: TEST_MODEL_CATALOG,
      },
    })

    const nameInput = screen.getByTestId('role-editor-name-input')
    const idInput = screen.getByTestId('role-editor-id-input')

    await fireEvent.input(nameInput, { target: { value: 'Frontend Developer' } })
    expect(idInput.value).toBe('frontend-developer')
  })

  it('hydrates context-steering fields for an existing role', () => {
    render(RoleEditor, {
      props: {
        open: true,
        role: {
          roleId: 'frontend-dev',
          name: 'Frontend Developer',
          cliTool: 'codex',
          model: 'gpt-5.4 high',
          focusArea: 'Frontend UI and component architecture',
          contextSummary: 'Owns patterns, tokens, and accessibility context.',
          behaviorSummary: 'Implements UI work independently and escalates architecture changes.',
          instructions: 'Stay in the frontend lane.',
          behavioralContract: [{ rule: 'Protect accessibility quality.', enabled: true }],
          builtIn: false,
        },
      },
    })

    expect(screen.getByTestId('role-editor-focus-area-input')).toHaveValue(
      'Frontend UI and component architecture'
    )
    expect(screen.getByTestId('role-editor-context-summary-input')).toHaveValue(
      'Owns patterns, tokens, and accessibility context.'
    )
    expect(screen.getByTestId('role-editor-behavior-summary-input')).toHaveValue(
      'Implements UI work independently and escalates architecture changes.'
    )
    expect(screen.getByText('Protect accessibility quality.')).toBeInTheDocument()
  })

  it('hydrates backend-shaped behavioral contracts into rendered editor rules', () => {
    const role = normalizeRoleTemplate({
      roleId: 'mesh-expert',
      name: 'Mesh Expert',
      cliTool: 'codex',
      model: 'gpt-5.4 high',
      behavioralContract: {
        communication: ['Confirm scope first.'],
        execution: ['Stay within the assigned lane.'],
        escalation: ['Escalate cross-project changes immediately.'],
      },
    })

    render(RoleEditor, {
      props: {
        open: true,
        role: {
          ...role,
          tool: role.cliTool,
        },
      },
    })

    expect(screen.getByText('Confirm scope first.')).toBeInTheDocument()
    expect(screen.getByText('Stay within the assigned lane.')).toBeInTheDocument()
    expect(screen.getByText('Escalate cross-project changes immediately.')).toBeInTheDocument()
  })

  it('can add and remove rules in operational boundaries', async () => {
    render(RoleEditor, {
      props: {
        open: true,
        modelCatalog: TEST_MODEL_CATALOG,
      },
    })

    const addRuleInput = screen.getByTestId('role-editor-add-rule-input')
    const addRuleButton = screen.getByTestId('role-editor-add-rule-button')

    await fireEvent.input(addRuleInput, { target: { value: 'Escalate product direction changes' } })
    await fireEvent.click(addRuleButton)

    expect(screen.getByText('Escalate product direction changes')).toBeInTheDocument()
    expect(screen.getByTestId('role-rule-0-checkbox')).toBeInTheDocument()

    const removeButton = screen.getByTestId('role-rule-0-remove')
    await fireEvent.click(removeButton)

    expect(screen.queryByText('Escalate product direction changes')).not.toBeInTheDocument()
  })

  it('save button is disabled until name, tool, and model are filled', async () => {
    render(RoleEditor, {
      props: {
        open: true,
        modelCatalog: TEST_MODEL_CATALOG,
      },
    })

    const saveButton = screen.getByTestId('role-editor-save')
    expect(saveButton).toBeDisabled()

    await fireEvent.input(screen.getByTestId('role-editor-name-input'), {
      target: { value: 'Frontend' },
    })
    await fireEvent.change(screen.getByTestId('role-editor-tool-select'), {
      target: { value: 'codex' },
    })
    await fireEvent.change(screen.getByTestId('role-editor-model-select'), {
      target: { value: 'gpt-5.6-terra' },
    })

    expect(saveButton).not.toBeDisabled()
  })

  it('emits onSave with context-steering fields and behavioral contract', async () => {
    const onSave = vi.fn()
    render(RoleEditor, {
      props: {
        open: true,
        modelCatalog: TEST_MODEL_CATALOG,
        onSave,
      },
    })

    await fireEvent.input(screen.getByTestId('role-editor-name-input'), {
      target: { value: 'Frontend' },
    })
    await fireEvent.change(screen.getByTestId('role-editor-tool-select'), {
      target: { value: 'codex' },
    })
    await fireEvent.change(screen.getByTestId('role-editor-model-select'), {
      target: { value: 'gpt-5.6-terra' },
    })
    await fireEvent.input(screen.getByTestId('role-editor-focus-area-input'), {
      target: { value: 'Frontend UI and component architecture' },
    })
    await fireEvent.input(screen.getByTestId('role-editor-context-summary-input'), {
      target: { value: 'Maintains component patterns, tokens, and accessibility context.' },
    })
    await fireEvent.input(screen.getByTestId('role-editor-behavior-summary-input'), {
      target: { value: 'Implements UI work independently; escalates architecture changes.' },
    })
    await fireEvent.input(screen.getByTestId('role-editor-instructions-input'), {
      target: { value: 'Stay close to the UI implementation lane.' },
    })
    await fireEvent.input(screen.getByTestId('role-editor-add-rule-input'), {
      target: { value: 'Flag cross-team dependency changes immediately.' },
    })
    await fireEvent.click(screen.getByTestId('role-editor-add-rule-button'))

    await fireEvent.click(screen.getByTestId('role-editor-save'))

    expect(onSave).toHaveBeenCalledWith({
      roleId: 'frontend',
      name: 'Frontend',
      tool: 'codex',
      model: 'gpt-5.6-terra',
      reasoningEffort: 'medium',
      focusArea: 'Frontend UI and component architecture',
      contextSummary: 'Maintains component patterns, tokens, and accessibility context.',
      behaviorSummary: 'Implements UI work independently; escalates architecture changes.',
      instructions: 'Stay close to the UI implementation lane.',
      behavioralContract: [
        { rule: 'Flag cross-team dependency changes immediately.', enabled: true }
      ],
    })
  })

  it('normalizes blank context-steering fields to null on save', async () => {
    const onSave = vi.fn()
    render(RoleEditor, {
      props: {
        open: true,
        modelCatalog: TEST_MODEL_CATALOG,
        onSave,
      },
    })

    await fireEvent.input(screen.getByTestId('role-editor-name-input'), {
      target: { value: 'Reviewer' },
    })

    await fireEvent.click(screen.getByTestId('role-editor-save'))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        focusArea: null,
        contextSummary: null,
        behaviorSummary: null,
      })
    )
  })

  it('shows delete button only for existing custom roles', () => {
    const { rerender } = render(RoleEditor, {
      props: {
        open: true,
        role: null,
      },
    })

    expect(screen.queryByTestId('role-editor-delete')).not.toBeInTheDocument()

    rerender({
      open: true,
      role: { roleId: 'custom-1', builtIn: false },
    })

    expect(screen.getByTestId('role-editor-delete')).toBeInTheDocument()

    rerender({
      open: true,
      role: { roleId: 'builtin-1', builtIn: true },
    })

    expect(screen.queryByTestId('role-editor-delete')).not.toBeInTheDocument()
  })
})
