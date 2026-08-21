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
  gemini: [
    {
      id: 'gemini-3.1-pro',
      label: 'Gemini 3.1 Pro',
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
      props: { tool: 'gemini', model: 'gemini-3.1-pro', catalog: CATALOG },
    })

    expect(screen.queryByTestId('model-select-effort')).not.toBeInTheDocument()
  })

  it('defaults the effort select to the catalog default effort', () => {
    render(ModelSelect, {
      props: { tool: 'codex', model: 'gpt-5.6-terra', catalog: CATALOG },
    })

    expect(screen.getByTestId('model-select-effort')).toHaveValue('high')
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
    expect(optionValues(effort)).toEqual(['ultra', 'low', 'medium', 'high', 'xhigh'])
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

  it('falls back to the catalog default when no model is set', () => {
    render(ModelSelect, {
      props: { tool: 'codex', catalog: CATALOG },
    })

    expect(screen.getByTestId('model-select')).toHaveValue('gpt-5.6-terra')
  })
})
