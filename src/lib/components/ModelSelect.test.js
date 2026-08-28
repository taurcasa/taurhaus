import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import ModelSelect from './ModelSelect.svelte'

const CATALOG = {
  claude: [
    {
      id: 'opus',
      label: 'Opus 5',
      efforts: ['low', 'medium', 'high'],
      defaultEffort: null,
      deprecated: false,
      replacement: null,
    },
  ],
  codex: [
    {
      id: 'gpt-5.6-terra',
      label: 'GPT-5.6-Terra',
      efforts: ['low', 'medium', 'high', 'xhigh'],
      defaultEffort: 'high',
      deprecated: false,
      replacement: null,
    },
    {
      id: 'gpt-5.4',
      label: 'GPT-5.4',
      efforts: ['low', 'medium', 'high', 'xhigh'],
      defaultEffort: 'medium',
      deprecated: true,
      replacement: 'gpt-5.6-terra',
    },
  ],
  agy: [
    {
      id: 'gemini-3.7-flash-high',
      label: 'Gemini 3.7 Flash (High)',
      efforts: [],
      defaultEffort: null,
      deprecated: false,
      replacement: null,
    },
  ],
}

function optionValues(select) {
  return Array.from(select.querySelectorAll('option')).map((option) => option.value)
}

describe('ModelSelect', () => {
  it('lists the catalog entries for the tool by label', () => {
    render(ModelSelect, {
      props: { tool: 'codex', model: 'gpt-5.6-terra', catalog: CATALOG },
    })

    const select = screen.getByTestId('model-select')
    expect(optionValues(select)).toEqual(['gpt-5.6-terra', 'gpt-5.4'])
    expect(select).toHaveValue('gpt-5.6-terra')
    expect(select).toHaveTextContent('GPT-5.6-Terra')
  })

  // Regression: the roster <select> could not show a model that only existed in a
  // YAML template, so opening a team silently rewrote it to the first list entry.
  it('injects an unknown current value as a custom option', () => {
    render(ModelSelect, {
      props: { tool: 'codex', model: 'gpt-6-preview', catalog: CATALOG },
    })

    const select = screen.getByTestId('model-select')
    expect(optionValues(select)).toEqual(['gpt-6-preview', 'gpt-5.6-terra', 'gpt-5.4'])
    expect(select).toHaveValue('gpt-6-preview')
  })

  it('renders the effort select only when the model declares efforts', () => {
    const { unmount } = render(ModelSelect, {
      props: { tool: 'codex', model: 'gpt-5.6-terra', reasoningEffort: 'xhigh', catalog: CATALOG },
    })

    expect(screen.getByTestId('model-select-effort')).toHaveValue('xhigh')
    unmount()

    render(ModelSelect, {
      props: { tool: 'agy', model: 'gemini-3.7-flash-high', catalog: CATALOG },
    })

    expect(screen.queryByTestId('model-select-effort')).not.toBeInTheDocument()
  })

  // Regression: b345de1 (PR 5c) preselected the entry's `defaultEffort` whenever
  // the value was unset, so a member that deliberately inherits the CLI's global
  // effort looked as if it had picked one — and `resolveMemberModel` then shipped
  // that synthesized effort in the initialize payload.
  it('shows the inherited default state when no effort is set', () => {
    render(ModelSelect, {
      props: { tool: 'codex', model: 'gpt-5.6-terra', catalog: CATALOG },
    })

    const effort = screen.getByTestId('model-select-effort')
    expect(effort).toHaveValue('')
    expect(optionValues(effort)).toEqual(['', 'low', 'medium', 'high', 'xhigh'])
    expect(effort).toHaveTextContent('default')
  })

  it('emits a null effort when the inherited option is chosen again', async () => {
    const onchange = vi.fn()
    render(ModelSelect, {
      props: {
        tool: 'codex',
        model: 'gpt-5.6-terra',
        reasoningEffort: 'low',
        catalog: CATALOG,
        onchange,
      },
    })

    await fireEvent.change(screen.getByTestId('model-select-effort'), { target: { value: '' } })

    expect(onchange).toHaveBeenCalledWith({ model: 'gpt-5.6-terra', reasoningEffort: null })
  })

  // The backend validates an effort against the tool-wide vocabulary for models
  // it does not know (`ModelCatalog::supports_effort`), so a custom model id must
  // still offer the tool's efforts instead of hiding the control.
  it('offers the tool-wide efforts for a model outside the catalog', () => {
    render(ModelSelect, {
      props: { tool: 'codex', model: 'gpt-6-preview', catalog: CATALOG },
    })

    expect(optionValues(screen.getByTestId('model-select-effort'))).toEqual([
      '',
      'low',
      'medium',
      'high',
      'xhigh',
    ])
  })

  it('resets the effort to the new entry default when the model changes', async () => {
    const onchange = vi.fn()
    render(ModelSelect, {
      props: {
        tool: 'codex',
        model: 'gpt-5.6-terra',
        reasoningEffort: 'xhigh',
        catalog: CATALOG,
        onchange,
      },
    })

    await fireEvent.change(screen.getByTestId('model-select'), { target: { value: 'gpt-5.4' } })

    expect(onchange).toHaveBeenCalledWith({ model: 'gpt-5.4', reasoningEffort: 'medium' })
  })

  it('emits the selected effort without changing the model', async () => {
    const onchange = vi.fn()
    render(ModelSelect, {
      props: {
        tool: 'codex',
        model: 'gpt-5.6-terra',
        reasoningEffort: 'high',
        catalog: CATALOG,
        onchange,
      },
    })

    await fireEvent.change(screen.getByTestId('model-select-effort'), {
      target: { value: 'low' },
    })

    expect(onchange).toHaveBeenCalledWith({ model: 'gpt-5.6-terra', reasoningEffort: 'low' })
  })

  it('renders the deprecation hint with its replacement', () => {
    render(ModelSelect, {
      props: { tool: 'codex', model: 'gpt-5.4', catalog: CATALOG },
    })

    expect(screen.getByTestId('model-select-deprecated')).toHaveTextContent('→ gpt-5.6-terra')
  })

  it('keeps an effort that is not in the catalog list', () => {
    render(ModelSelect, {
      props: { tool: 'codex', model: 'gpt-5.6-terra', reasoningEffort: 'ultra', catalog: CATALOG },
    })

    const effort = screen.getByTestId('model-select-effort')
    expect(optionValues(effort)).toEqual(['', 'ultra', 'low', 'medium', 'high', 'xhigh'])
    expect(effort).toHaveValue('ultra')
  })

  it('honours a custom testId and the disabled flag', () => {
    render(ModelSelect, {
      props: {
        tool: 'codex',
        model: 'gpt-5.6-terra',
        catalog: CATALOG,
        disabled: true,
        testId: 'mesh-builder-lead-model-input',
      },
    })

    expect(screen.getByTestId('mesh-builder-lead-model-input')).toBeDisabled()
    expect(screen.getByTestId('mesh-builder-lead-model-input-effort')).toBeDisabled()
  })

  // Regression: b345de1 (PR 5c) offered an empty "default" effort option on every
  // roster row and emitted null for it. For a member bound to a role that declares
  // an effort the backend refills that null from the role template
  // (`apply_role_template_defaults`, request_normalization.rs), so choosing
  // "default" still launched at the role's effort. A role-declared effort is now
  // shown as the inherited value and the misleading empty option is withheld.
  it('withholds the misleading empty option when a role effort is inherited', () => {
    render(ModelSelect, {
      props: {
        tool: 'codex',
        model: 'gpt-5.6-terra',
        reasoningEffort: null,
        inheritedEffort: 'high',
        catalog: CATALOG,
      },
    })

    const effort = screen.getByTestId('model-select-effort')
    expect(optionValues(effort)).toEqual(['low', 'medium', 'high', 'xhigh'])
    expect(effort).toHaveValue('high')
  })

  it('keeps an inherited effort the model does not list', () => {
    render(ModelSelect, {
      props: {
        tool: 'codex',
        model: 'gpt-5.6-terra',
        inheritedEffort: 'ultra',
        catalog: CATALOG,
      },
    })

    const effort = screen.getByTestId('model-select-effort')
    expect(optionValues(effort)).toEqual(['ultra', 'low', 'medium', 'high', 'xhigh'])
    expect(effort).toHaveValue('ultra')
  })

  it('lets an explicit member effort win over the inherited one', () => {
    render(ModelSelect, {
      props: {
        tool: 'codex',
        model: 'gpt-5.6-terra',
        reasoningEffort: 'low',
        inheritedEffort: 'high',
        catalog: CATALOG,
      },
    })

    expect(screen.getByTestId('model-select-effort')).toHaveValue('low')
  })

  it('still offers the empty option when nothing is inherited', () => {
    render(ModelSelect, {
      props: { tool: 'codex', model: 'gpt-5.6-terra', inheritedEffort: null, catalog: CATALOG },
    })

    expect(optionValues(screen.getByTestId('model-select-effort'))).toEqual([
      '',
      'low',
      'medium',
      'high',
      'xhigh',
    ])
  })

  it('shows an inherited effort even when the model declares none', () => {
    render(ModelSelect, {
      props: {
        tool: 'agy',
        model: 'gemini-3.7-flash-high',
        inheritedEffort: 'medium',
        catalog: CATALOG,
      },
    })

    expect(screen.getByTestId('model-select-effort')).toHaveValue('medium')
  })

  it('falls back to the catalog default when no model is set', () => {
    render(ModelSelect, {
      props: { tool: 'codex', catalog: CATALOG },
    })

    expect(screen.getByTestId('model-select')).toHaveValue('gpt-5.6-terra')
  })
})
