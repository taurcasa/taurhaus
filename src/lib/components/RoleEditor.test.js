import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import RoleEditor from './RoleEditor.svelte'

describe('RoleEditor', () => {
  it('renders all required fields', () => {
    render(RoleEditor, {
      props: {
        open: true,
      },
    })

    expect(screen.getByTestId('role-editor-name-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-id-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-tool-select')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-model-select')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-instructions-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-add-rule-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-add-capability-input')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-save')).toBeInTheDocument()
    expect(screen.getByTestId('role-editor-cancel')).toBeInTheDocument()
  })

  it('generates role id from name', async () => {
    render(RoleEditor, {
      props: {
        open: true,
      },
    })

    const nameInput = screen.getByTestId('role-editor-name-input')
    const idInput = screen.getByTestId('role-editor-id-input')

    await fireEvent.input(nameInput, { target: { value: 'Frontend Developer' } })
    expect(idInput.value).toBe('frontend-developer')
  })

  it('can add and remove rules in behavioral contract', async () => {
    render(RoleEditor, {
      props: {
        open: true,
      },
    })

    const addRuleInput = screen.getByTestId('role-editor-add-rule-input')
    const addRuleButton = screen.getByTestId('role-editor-add-rule-button')

    await fireEvent.input(addRuleInput, { target: { value: 'Always use Tailwind v4' } })
    await fireEvent.click(addRuleButton)

    expect(screen.getByText('Always use Tailwind v4')).toBeInTheDocument()
    expect(screen.getByTestId('role-rule-0-checkbox')).toBeInTheDocument()

    const removeButton = screen.getByTestId('role-rule-0-remove')
    await fireEvent.click(removeButton)

    expect(screen.queryByText('Always use Tailwind v4')).not.toBeInTheDocument()
  })

  it('can add and remove capability tags', async () => {
    render(RoleEditor, {
      props: {
        open: true,
      },
    })

    const addCapInput = screen.getByTestId('role-editor-add-capability-input')
    const addCapButton = screen.getByTestId('role-editor-add-capability-button')

    await fireEvent.input(addCapInput, { target: { value: 'Svelte 5' } })
    await fireEvent.click(addCapButton)

    expect(screen.getByText('Svelte 5')).toBeInTheDocument()

    const removeButton = screen.getByTestId('role-capability-0-remove')
    await fireEvent.click(removeButton)

    expect(screen.queryByText('Svelte 5')).not.toBeInTheDocument()
  })

  it('save button is disabled until name, tool, and model are filled', async () => {
    render(RoleEditor, {
      props: {
        open: true,
      },
    })

    const saveButton = screen.getByTestId('role-editor-save')
    expect(saveButton).toBeDisabled()

    await fireEvent.input(screen.getByTestId('role-editor-name-input'), { target: { value: 'Frontend' } })
    await fireEvent.change(screen.getByTestId('role-editor-tool-select'), { target: { value: 'codex' } })
    await fireEvent.change(screen.getByTestId('role-editor-model-select'), { target: { value: 'gpt-5.4-high' } })

    expect(saveButton).not.toBeDisabled()
  })

  it('emits onSave with full role object', async () => {
    const onSave = vi.fn()
    render(RoleEditor, {
      props: {
        open: true,
        onSave,
      },
    })

    await fireEvent.input(screen.getByTestId('role-editor-name-input'), { target: { value: 'Frontend' } })
    await fireEvent.change(screen.getByTestId('role-editor-tool-select'), { target: { value: 'codex' } })
    await fireEvent.change(screen.getByTestId('role-editor-model-select'), { target: { value: 'gpt-5.4-high' } })
    await fireEvent.input(screen.getByTestId('role-editor-instructions-input'), { target: { value: 'Work hard' } })
    
    // Add a rule
    await fireEvent.input(screen.getByTestId('role-editor-add-rule-input'), { target: { value: 'Be fast' } })
    await fireEvent.click(screen.getByTestId('role-editor-add-rule-button'))
    
    // Add a capability
    await fireEvent.input(screen.getByTestId('role-editor-add-capability-input'), { target: { value: 'JS' } })
    await fireEvent.click(screen.getByTestId('role-editor-add-capability-button'))

    await fireEvent.click(screen.getByTestId('role-editor-save'))

    expect(onSave).toHaveBeenCalledWith({
      roleId: 'frontend',
      name: 'Frontend',
      tool: 'codex',
      model: 'gpt-5.4-high',
      instructions: 'Work hard',
      behavioralContract: [
        { rule: 'Be fast', enabled: true }
      ],
      capabilities: ['JS']
    })
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
