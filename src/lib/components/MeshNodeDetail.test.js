import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import MeshNodeDetail from './MeshNodeDetail.svelte'

function renderDetail(props = {}) {
  return render(MeshNodeDetail, {
    props: {
      name: 'frontend-dev',
      role: 'agent',
      tool: 'codex',
      model: 'gpt-5.3-codex',
      status: 'active',
      projectId: 'taurhaus-web',
      description: 'Implements UI surface details for the mesh canvas.',
      mode: 'setup',
      dark: true,
      ...props,
    },
  })
}

describe('MeshNodeDetail', () => {
  it('renders name, tool, model, and status', () => {
    renderDetail()

    expect(screen.getByTestId('mesh-node-detail-name')).toHaveTextContent('frontend-dev')
    expect(screen.getByTestId('mesh-node-detail-tool-model')).toHaveTextContent('Codex · gpt-5.3-codex')
    expect(screen.getByTestId('mesh-node-detail-status')).toHaveTextContent('Active')
    expect(screen.getByTestId('status-badge-active')).toBeInTheDocument()
  })

  it('animates in using mesh-detail-enter keyframe class', () => {
    renderDetail()
    expect(screen.getByTestId('mesh-node-detail').className).toContain('mesh-detail-enter')
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
      onEdit,
      onRemove,
      onClose,
    })

    await fireEvent.click(screen.getByTestId('mesh-node-detail-edit'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-remove'))
    await fireEvent.click(screen.getByTestId('mesh-node-detail-close'))
    expect(onEdit).toHaveBeenCalledTimes(1)
    expect(onRemove).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)

    view.rerender({
      name: 'frontend-dev',
      role: 'agent',
      tool: 'codex',
      model: 'gpt-5.3-codex',
      status: 'active',
      projectId: 'taurhaus-web',
      description: 'Implements UI surface details for the mesh canvas.',
      mode: 'runtime',
      dark: true,
      onResume,
      onStop,
      onFocusPane,
      onCapture,
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
      name: 'frontend-dev',
      role: 'agent',
      tool: 'codex',
      model: 'gpt-5.3-codex',
      status: 'active',
      projectId: 'taurhaus-web',
      description: '',
      mode: 'setup',
      dark: true,
    })

    expect(screen.queryByTestId('mesh-node-detail-description')).not.toBeInTheDocument()
  })
})
