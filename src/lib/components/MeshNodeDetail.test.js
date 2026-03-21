import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../MarkdownRenderer.svelte', () => ({
  default: function MockMarkdownRenderer(target, props) {
    const element = document.createElement('div')
    element.setAttribute('data-testid', 'mock-markdown')
    element.textContent = String(props.source ?? '')
    target.parentNode?.insertBefore(element, target)
    return {
      update(nextProps) {
        element.textContent = String(nextProps.source ?? '')
      },
      destroy() {
        element.remove()
      },
    }
  },
}))

import MeshNodeDetail from './MeshNodeDetail.svelte'

function renderDetail(props = {}) {
  const node = {
    name: 'frontend-dev',
    roleName: 'Codex Architect',
    role: 'agent',
    tool: 'codex',
    model: 'gpt-5.4 high',
    status: 'idle',
    projectId: 'taurhaus-web',
    focusArea: 'Architecture decisions and structural review',
    contextSummary: 'Carries long-lived context around module boundaries and reviews.',
    behaviorSummary: 'Escalates direction changes before implementation.\nProtects long-lived system boundaries.',
    instructions: '## Operating notes\n\nKeep implementation scoped.',
    paneId: '%9',
    sessionId: 'sess-123',
    sessionState: 'warming',
    capabilities: ['planning', 'review'],
    ...props.node,
  }

  return render(MeshNodeDetail, {
    props: {
      node,
      mode: 'runtime',
      dark: true,
      actions: {},
      ...props,
    },
  })
}

describe('MeshNodeDetail', () => {
  it('renders as a full dialog with pinned runtime actions and markdown-backed sections', () => {
    renderDetail({
      actions: {
        onResume: vi.fn(),
        onStop: vi.fn(),
        onFocusPane: vi.fn(),
        onCapture: vi.fn(),
        onClose: vi.fn(),
      },
    })

    expect(screen.getByRole('dialog', { name: 'Codex Architect' })).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-name')).toHaveTextContent('Codex Architect')
    expect(screen.getAllByText('Idle').length).toBeGreaterThan(0)
    expect(screen.getByTestId('mesh-node-detail-toolbar')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-resume')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-stop')).toHaveTextContent('Stop')
    expect(screen.getByTestId('mesh-node-detail-focus')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-capture')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-focus-area')).toHaveTextContent(
      'Architecture decisions and structural review'
    )
    expect(screen.getByTestId('mesh-node-detail-context-summary')).toHaveTextContent(
      'Carries long-lived context around module boundaries and reviews.'
    )
    expect(screen.getByTestId('mesh-node-detail-behavior-summary')).toHaveTextContent(
      'Escalates direction changes before implementation.'
    )
    expect(screen.getByTestId('mesh-node-detail-description')).toHaveTextContent(
      'Keep implementation scoped.'
    )
    expect(screen.getByTestId('mesh-node-detail-pane')).toHaveTextContent('%9')
    expect(screen.getByTestId('mesh-node-detail-session')).toHaveTextContent('sess-123')
    expect(screen.getByTestId('mesh-node-detail-session-state')).toHaveTextContent('warming')
  })

  it('renders the roster context with an Add to Team action instead of runtime controls', () => {
    renderDetail({
      mode: 'builder',
      node: {
        status: '',
        paneId: '',
        sessionId: '',
        sessionState: '',
      },
      actions: {
        onAdd: vi.fn(),
        onClose: vi.fn(),
      },
    })

    expect(screen.getByTestId('mesh-node-detail-add')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-node-detail-resume')).not.toBeInTheDocument()
    expect(screen.queryByTestId('mesh-node-detail-focus')).not.toBeInTheDocument()
    expect(screen.getByText('Template')).toBeInTheDocument()
  })

  it('uses normal light-theme surfaces instead of forcing the dark shell', () => {
    renderDetail({
      dark: false,
    })

    expect(screen.getByTestId('mesh-node-detail').className).toContain('bg-white/98')
    expect(screen.getByTestId('mesh-node-detail').className).not.toContain('bg-brand-950/98')
    expect(screen.getByTestId('mesh-node-detail-header').className).toContain('bg-[linear-gradient(180deg,rgba(255,255,255,0.98),rgba(244,244,245,0.96))]')
    expect(screen.getByTestId('mesh-node-detail-focus-card').className).toContain('bg-brand-50/75')
    expect(screen.getByTestId('mesh-node-detail-focus-card').className).toContain('border-brand-200/80')
  })

  it('renders builder detail management actions when provided', () => {
    renderDetail({
      mode: 'builder',
      node: {
        status: '',
        paneId: '',
        sessionId: '',
        sessionState: '',
      },
      actions: {
        onAdd: vi.fn(),
        onEdit: vi.fn(),
        onExport: vi.fn(),
        onDelete: vi.fn(),
        onClose: vi.fn(),
      },
    })

    expect(screen.getByTestId('mesh-node-detail-edit')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-export')).toHaveTextContent('Export YAML')
    expect(screen.getByTestId('mesh-node-detail-delete')).toBeInTheDocument()
  })

  it('renders in-place editing controls in the shared detail shell', () => {
    renderDetail({
      mode: 'builder',
      editing: true,
      editDraft: {
        name: 'Engineering Delivery Lead',
        kind: 'lead',
        tool: 'claude',
        model: 'opus',
        focusArea: 'Team sequencing, delivery coordination, and blocker escalation',
        contextSummary: 'Maintains delivery flow and unblocks specialists.',
        behaviorSummary: '- Escalate blockers quickly',
        instructions: '',
        showInstructions: false,
      },
      dirty: true,
      actions: {
        onCancelEdit: vi.fn(),
        onSaveEdit: vi.fn(),
        onEditChange: vi.fn(),
        onAddSection: vi.fn(),
      },
    })

    expect(screen.getByTestId('mesh-node-detail-name-input')).toHaveValue('Engineering Delivery Lead')
    expect(screen.getByTestId('mesh-node-detail-tool-input')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-model-input')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-kind-input')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-focus-input')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-context-input')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-behavior-input')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-markdown-hint')).toHaveTextContent('Supports markdown')
    expect(screen.getByTestId('mesh-node-detail-cancel')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-save')).toHaveTextContent('Save Changes')
    expect(screen.getByTestId('mesh-node-detail-unsaved-dot')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-add-section')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-node-detail-close')).not.toBeInTheDocument()
  })

  it('keeps edit mode typography borderless so the document barely changes when editing', () => {
    renderDetail({
      mode: 'builder',
      editing: true,
      editDraft: {
        name: 'Engineering Delivery Lead',
        kind: 'lead',
        tool: 'claude',
        model: 'opus',
        focusArea: 'Team sequencing, delivery coordination, and blocker escalation',
        contextSummary: 'Maintains delivery flow and unblocks specialists.',
        behaviorSummary: '- Escalate blockers quickly',
        instructions: 'Keep the team moving.',
        showInstructions: true,
      },
      actions: {
        onCancelEdit: vi.fn(),
        onSaveEdit: vi.fn(),
        onEditChange: vi.fn(),
        onAddSection: vi.fn(),
      },
    })

    expect(screen.getByTestId('mesh-node-detail-context-input').className).toContain('border-0')
    expect(screen.getByTestId('mesh-node-detail-context-input').className).toContain('bg-transparent')
    expect(screen.getByTestId('mesh-node-detail-context-input').className).toContain('text-[15px]')
    expect(screen.getByTestId('mesh-node-detail-context-input').className).toContain('leading-[1.6]')
    expect(screen.getByTestId('mesh-node-detail-behavior-input').className).toContain('border-0')
    expect(screen.getByTestId('mesh-node-detail-behavior-input').className).toContain('text-[15px]')
    expect(screen.getByTestId('mesh-node-detail-instructions-input').className).toContain('border-0')
    expect(screen.getByTestId('mesh-node-detail-focus-input').className).toContain('border-0')
    expect(screen.getByTestId('mesh-node-detail-focus-input').className).toContain('text-[17px]')
  })

  it('invokes runtime actions and close affordances', async () => {
    const onResume = vi.fn()
    const onStop = vi.fn()
    const onFocusPane = vi.fn()
    const onCapture = vi.fn()
    const onClose = vi.fn()

    renderDetail({
      actions: {
        onResume,
        onStop,
        onFocusPane,
        onCapture,
        onClose,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-resume'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-stop'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-focus'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-capture'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-close'))

    expect(onResume).toHaveBeenCalledTimes(1)
    expect(onStop).toHaveBeenCalledTimes(1)
    expect(onFocusPane).toHaveBeenCalledTimes(1)
    expect(onCapture).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('closes on Escape and backdrop click', async () => {
    const onClose = vi.fn()

    renderDetail({
      actions: { onClose },
    })

    await fireEvent.keyDown(window, { key: 'Escape' })
    await fireEvent.click(screen.getByTestId('mesh-node-detail-host'))

    expect(onClose).toHaveBeenCalledTimes(2)
  })

  it('uses Escape to cancel editing instead of closing the overlay', async () => {
    const onCancelEdit = vi.fn()
    const onClose = vi.fn()

    renderDetail({
      mode: 'builder',
      editing: true,
      editDraft: {
        name: 'Engineering Delivery Lead',
        kind: 'lead',
        tool: 'claude',
        model: 'opus',
        focusArea: '',
        contextSummary: '',
        behaviorSummary: '',
        instructions: '',
        showInstructions: false,
      },
      actions: {
        onCancelEdit,
        onClose,
      },
    })

    await fireEvent.keyDown(window, { key: 'Escape' })

    expect(onCancelEdit).toHaveBeenCalledTimes(1)
    expect(onClose).not.toHaveBeenCalled()
  })

  it('returns focus to the opener when closed', async () => {
    const opener = document.createElement('button')
    opener.textContent = 'Open detail'
    document.body.appendChild(opener)
    opener.focus()

    const onClose = vi.fn()
    const view = renderDetail({
      actions: { onClose },
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-close'))

    expect(onClose).toHaveBeenCalledTimes(1)

    view.unmount()
    expect(opener).toHaveFocus()
    opener.remove()
  })
})
