import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshNodeDetail from './MeshNodeDetail.svelte'

function renderDetail(props = {}) {
  const legacyNode = {}
  if (props.name !== undefined) legacyNode.name = props.name
  if (props.role !== undefined) legacyNode.role = props.role
  if (props.tool !== undefined) legacyNode.tool = props.tool
  if (props.model !== undefined) legacyNode.model = props.model
  if (props.status !== undefined) legacyNode.status = props.status
  if (props.projectId !== undefined) legacyNode.projectId = props.projectId
  if (props.description !== undefined) legacyNode.description = props.description
  const node = {
    name: 'frontend-dev',
    role: 'agent',
    tool: 'codex',
    model: 'gpt-5.4-high',
    status: 'active',
    projectId: 'taurhaus-web',
    description: 'Implements UI surface details for the mesh canvas.',
    ...legacyNode,
    ...(props.node || {}),
  }
  const actions = {
    ...(props.actions || {}),
  }
  const passthrough = { ...props }
  delete passthrough.name
  delete passthrough.role
  delete passthrough.tool
  delete passthrough.model
  delete passthrough.status
  delete passthrough.projectId
  delete passthrough.description
  delete passthrough.node
  delete passthrough.actions
  return render(MeshNodeDetail, {
    props: {
      node,
      mode: 'setup',
      dark: true,
      actions,
      ...passthrough,
    },
  })
}

describe('MeshNodeDetail', () => {
  it('renders name, tool, model, and status', () => {
    renderDetail()

    expect(screen.getByTestId('mesh-node-detail-name')).toHaveTextContent('frontend-dev')
    expect(screen.getByTestId('mesh-node-detail-tool-model')).toHaveTextContent('Codex · gpt-5.4-high')
    expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Active')
    expect(screen.getByTestId('status-badge-active')).toBeInTheDocument()
  })

  it('renders lead role chip and runtime diagnostics when supplied', () => {
    renderDetail({
      mode: 'runtime',
      role: 'lead',
      status: 'idle',
      node: {
        paneId: '%9',
        sessionId: 'sess-123',
        sessionState: 'warming',
      },
    })

    expect(screen.getByText('Lead')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-runtime')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-pane')).toHaveTextContent('%9')
    expect(screen.getByTestId('mesh-node-detail-session')).toHaveTextContent('sess-123')
    expect(screen.getByTestId('mesh-node-detail-session-state')).toHaveTextContent('warming')
  })

  it('animates in using mesh-detail-enter keyframe class', () => {
    renderDetail()
    expect(screen.getByTestId('mesh-node-detail').className).toContain('mesh-detail-enter')
  })

  it('defaults to bottom placement when no anchor is provided', () => {
    renderDetail()
    const detail = screen.getByTestId('mesh-node-detail')
    expect(detail).toHaveAttribute('data-placement', 'bottom')
    expect(detail.getAttribute('style') || '').toContain('transform: translateX(-50%)')
  })

  it('uses anchored top/left coordinates when provided', () => {
    renderDetail({
      anchor: {
        left: 88,
        top: 24,
        placement: 'top',
        cardWidth: 220,
      },
    })

    const detail = screen.getByTestId('mesh-node-detail')
    const style = detail.getAttribute('style') || ''
    expect(detail).toHaveAttribute('data-placement', 'top')
    expect(style).toContain('left: 88px')
    expect(style).toContain('top: 24px')
    expect(style).toContain('width: 220px')
  })

  it('shows Edit and Remove buttons in setup mode', () => {
    renderDetail({ mode: 'setup', role: 'agent' })
    expect(screen.getByTestId('mesh-node-detail-edit')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-remove')).toBeInTheDocument()
  })

  it("doesn't show Remove for lead nodes in setup mode", () => {
    renderDetail({ mode: 'setup', role: 'lead' })
    expect(screen.getByTestId('mesh-node-detail-edit')).toBeInTheDocument()
    expect(screen.queryByTestId('mesh-node-detail-remove')).not.toBeInTheDocument()
  })

  it('shows Resume, Stop, and Focus buttons in runtime mode', () => {
    renderDetail({ mode: 'runtime' })
    expect(screen.getByTestId('mesh-node-detail-resume')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-stop')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-focus')).toBeInTheDocument()
    expect(screen.getByTestId('mesh-node-detail-capture')).toBeInTheDocument()
  })

  it('disables Focus button when no focus callback is provided', () => {
    renderDetail({
      mode: 'runtime',
      actions: {
        onResume: vi.fn(),
        onStop: vi.fn(),
        onCapture: vi.fn(),
      },
    })

    expect(screen.getByTestId('mesh-node-detail-focus')).toBeDisabled()
  })

  it('renders contrast-safe surfaces in dark and light modes', () => {
    const view = renderDetail({ mode: 'runtime', dark: true })
    expect(screen.getByTestId('mesh-node-detail').className).toContain('text-zinc-100')

    view.rerender({
      node: {
        name: 'frontend-dev',
        role: 'agent',
        tool: 'codex',
        model: 'gpt-5.4-high',
        status: 'active',
        projectId: 'taurhaus-web',
        description: 'Implements UI surface details for the mesh canvas.',
      },
      mode: 'runtime',
      dark: false,
      actions: {},
    })
    expect(screen.getByTestId('mesh-node-detail').className).toContain('text-zinc-900')
  })

  it('calls action callbacks', async () => {
    const onEdit = vi.fn()
    const onRemove = vi.fn()
    const onResume = vi.fn()
    const onStop = vi.fn()
    const onFocusPane = vi.fn()
    const onCapture = vi.fn()
    const onClose = vi.fn()

    const view = renderDetail({
      mode: 'setup',
      actions: {
        onEdit,
        onRemove,
        onClose,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-edit'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-remove'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-close'))
    expect(onEdit).toHaveBeenCalledTimes(1)
    expect(onRemove).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)

    view.rerender({
      node: {
        name: 'frontend-dev',
        role: 'agent',
        tool: 'codex',
        model: 'gpt-5.4-high',
        status: 'active',
        projectId: 'taurhaus-web',
        description: 'Implements UI surface details for the mesh canvas.',
      },
      mode: 'runtime',
      dark: true,
      actions: {
        onResume,
        onStop,
        onFocusPane,
        onCapture,
      },
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-resume'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-stop'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-focus'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-capture'))
    expect(onResume).toHaveBeenCalledTimes(1)
    expect(onStop).toHaveBeenCalledTimes(1)
    expect(onFocusPane).toHaveBeenCalledTimes(1)
    expect(onCapture).toHaveBeenCalledTimes(1)
  })

  it('shows description when provided and hides when empty', () => {
    const view = renderDetail({ description: 'Node details for handoff.' })
    expect(screen.getByTestId('mesh-node-detail-description')).toHaveTextContent('Node details for handoff.')

    view.rerender({
      node: {
        name: 'frontend-dev',
        role: 'agent',
        tool: 'codex',
        model: 'gpt-5.4-high',
        status: 'active',
        projectId: 'taurhaus-web',
        description: '',
      },
      mode: 'setup',
      dark: true,
    })

    expect(screen.queryByTestId('mesh-node-detail-description')).not.toBeInTheDocument()
  })
})
