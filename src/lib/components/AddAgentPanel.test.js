import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  listRoleTemplates: vi.fn(),
  getRoleTemplate: vi.fn(),
}))

const { listRoleTemplates, getRoleTemplate } = await import('../ipc.js')

import AddAgentPanel from './AddAgentPanel.svelte'

describe('AddAgentPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    listRoleTemplates.mockResolvedValue([
      {
        roleId: 'custom-doc-writer',
        name: 'Documentation Writer',
        cliTool: 'gemini',
        model: 'gemini-2.5-pro',
        capabilities: ['documentation'],
      },
    ])
    getRoleTemplate.mockResolvedValue({
      roleId: 'custom-doc-writer',
      instructions: 'Write release notes and docs updates.',
    })
  })

  function renderPanel(props = {}) {
    return render(AddAgentPanel, {
      props: {
        open: true,
        dark: false,
        availableProjects: [
          { id: '/projects/taurhaus', name: 'taurhaus' },
          { id: '/projects/docs', name: 'docs' },
        ],
        ...props,
      },
    })
  }

  it('renders From Template / Manual toggle', async () => {
    renderPanel()
    await waitFor(() => {
      expect(screen.getByTestId('add-agent-tab-template')).toBeInTheDocument()
    })
    expect(screen.getByTestId('add-agent-tab-manual')).toBeInTheDocument()
  })

  it('manual path validates required fields and disables Add when invalid', async () => {
    renderPanel()
    await fireEvent.click(screen.getByTestId('add-agent-tab-manual'))

    const submit = screen.getByTestId('add-agent-submit')
    expect(submit).toBeDisabled()

    await fireEvent.input(screen.getByTestId('add-agent-name-input'), {
      target: { value: 'frontend-dev' },
    })
    await fireEvent.change(screen.getByTestId('add-agent-project-select'), {
      target: { value: '/projects/taurhaus' },
    })

    expect(submit).toBeEnabled()

    await fireEvent.input(screen.getByTestId('add-agent-name-input'), {
      target: { value: '' },
    })
    expect(submit).toBeDisabled()
  })

  it('calls onAddAgent with correct config on submit', async () => {
    const onAddAgent = vi.fn().mockResolvedValue({})
    renderPanel({ onAddAgent })
    await fireEvent.click(screen.getByTestId('add-agent-tab-manual'))

    await fireEvent.input(screen.getByTestId('add-agent-name-input'), {
      target: { value: 'backend-dev' },
    })
    await fireEvent.change(screen.getByTestId('add-agent-tool-select'), {
      target: { value: 'codex' },
    })
    await fireEvent.change(screen.getByTestId('add-agent-model-select'), {
      target: { value: 'gpt-5.3-codex' },
    })
    await fireEvent.change(screen.getByTestId('add-agent-project-select'), {
      target: { value: '/projects/docs' },
    })
    await fireEvent.input(screen.getByTestId('add-agent-description-input'), {
      target: { value: 'Own backend changes' },
    })

    await fireEvent.click(screen.getByTestId('add-agent-submit'))

    expect(onAddAgent).toHaveBeenCalledTimes(1)
    expect(onAddAgent).toHaveBeenCalledWith({
      name: 'backend-dev',
      tool: 'codex',
      model: 'gpt-5.3-codex',
      projectId: '/projects/docs',
      description: 'Own backend changes',
    })
  })

  it('from template shows role list and template selection auto-fills fields', async () => {
    renderPanel()

    await waitFor(() => {
      expect(screen.getByTestId('add-agent-template-list')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('add-agent-template-custom-doc-writer'))

    await waitFor(() => {
      expect(screen.getByTestId('add-agent-name-input')).toHaveValue('Documentation Writer')
    })
    expect(screen.getByTestId('add-agent-tool-select')).toHaveValue('gemini')
    expect(screen.getByTestId('add-agent-model-select')).toHaveValue('gemini-2.5-pro')
    expect(screen.getByTestId('add-agent-description-input')).toHaveValue(
      'Write release notes and docs updates.'
    )
    expect(screen.getByTestId('add-agent-template-preview')).toBeInTheDocument()
  })
})
