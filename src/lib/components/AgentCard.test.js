import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import { readFileSync } from 'node:fs'

import AgentCard from './AgentCard.svelte'
import { TEST_MODEL_CATALOG } from '../../test/fixtures/modelCatalog.js'

describe('AgentCard', () => {
  it('renders name, tool, and model in read mode', () => {
    render(AgentCard, {
      props: {
        testId: 'agent-card',
        name: 'dev-1',
        tool: 'codex',
        model: 'gpt-5.4',
        reasoningEffort: 'high',
        modelCatalog: TEST_MODEL_CATALOG,
        projectId: '/projects/taurhaus',
      },
    })

    expect(screen.getByTestId('agent-card-name')).toHaveTextContent('dev-1')
    expect(screen.getByTestId('agent-card-tool-model')).toHaveTextContent('Codex · gpt-5.4 · high')
    expect(screen.getByTestId('agent-card-project')).toHaveTextContent('/projects/taurhaus')
  })

  it('edit mode shows input fields', async () => {
    render(AgentCard, {
      props: {
        testId: 'agent-card',
      },
    })

    await fireEvent.click(screen.getByTestId('agent-card-edit'))

    expect(screen.getByTestId('agent-card-edit-form')).toBeInTheDocument()
    expect(screen.getByTestId('agent-card-name-input')).toBeInTheDocument()
    expect(screen.getByTestId('agent-card-tool-select')).toBeInTheDocument()
    expect(screen.getByTestId('agent-card-model-select')).toBeInTheDocument()
    expect(screen.getByTestId('agent-card-project-input')).toBeInTheDocument()
    expect(screen.getByTestId('agent-card-description-input')).toBeInTheDocument()
  })

  it('lead cards do not show remove button', () => {
    render(AgentCard, {
      props: {
        testId: 'lead-card',
        role: 'lead',
      },
    })

    expect(screen.queryByTestId('lead-card-remove')).not.toBeInTheDocument()
  })

  it('save callback fires with edited config', async () => {
    const onSave = vi.fn()
    render(AgentCard, {
      props: {
        testId: 'agent-card',
        name: 'dev-1',
        tool: 'codex',
        model: 'gpt-5.4',
        reasoningEffort: 'high',
        modelCatalog: TEST_MODEL_CATALOG,
        projectId: '/projects/taurhaus',
        description: 'Initial',
        onSave,
      },
    })

    await fireEvent.click(screen.getByTestId('agent-card-edit'))
    await fireEvent.input(screen.getByTestId('agent-card-name-input'), {
      target: { value: 'api-dev' },
    })
    await fireEvent.change(screen.getByTestId('agent-card-tool-select'), {
      target: { value: 'gemini' },
    })
    await fireEvent.change(screen.getByTestId('agent-card-model-select'), {
      target: { value: 'gemini-3.1-pro' },
    })
    await fireEvent.input(screen.getByTestId('agent-card-project-input'), {
      target: { value: '/projects/api' },
    })
    await fireEvent.input(screen.getByTestId('agent-card-description-input'), {
      target: { value: 'Own API tasks' },
    })
    await fireEvent.click(screen.getByTestId('agent-card-save'))

    expect(onSave).toHaveBeenCalledTimes(1)
    expect(onSave).toHaveBeenCalledWith({
      name: 'api-dev',
      tool: 'gemini',
      model: 'gemini-3.1-pro',
      reasoningEffort: null,
      projectId: '/projects/api',
      description: 'Own API tasks',
    })
  })

  it('cancel returns to read mode', async () => {
    render(AgentCard, {
      props: {
        testId: 'agent-card',
      },
    })

    await fireEvent.click(screen.getByTestId('agent-card-edit'))
    expect(screen.getByTestId('agent-card-edit-form')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('agent-card-cancel'))
    expect(screen.queryByTestId('agent-card-edit-form')).not.toBeInTheDocument()
  })

  // Regression: c1603fe (PR 5c review round 2) taught ModelSelect to withhold the
  // empty "default" effort option where a role declares one, but this card passed
  // no `inheritedEffort`, so the advanced preset editor kept offering a clear the
  // backend undoes by refilling the role's effort.
  it('withholds the effort default when the bound role declares one', async () => {
    render(AgentCard, {
      props: {
        testId: 'agent-card',
        tool: 'codex',
        model: 'gpt-5.6-terra',
        reasoningEffort: null,
        inheritedEffort: 'high',
        modelCatalog: TEST_MODEL_CATALOG,
      },
    })

    await fireEvent.click(screen.getByTestId('agent-card-edit'))

    const effortSelect = screen.getByTestId('agent-card-model-select-effort')
    expect(effortSelect).toHaveValue('high')
    expect(Array.from(effortSelect.options).map((option) => option.value)).not.toContain('')
  })

  it('uses tokenized color styles instead of hardcoded literals', () => {
    const source = readFileSync(`${process.cwd()}/src/lib/components/AgentCard.svelte`, 'utf8')
    expect(source).toContain('var(--agent-card-border-light)')
    expect(source).toContain('var(--agent-card-bg-light-to)')
    expect(source).toContain('var(--agent-card-shadow-light)')
    expect(source).not.toContain('#b2d8d0')
    expect(source).not.toContain('#e6f7f4')
    expect(source).not.toMatch(/\brgba?\(/)
  })
})
