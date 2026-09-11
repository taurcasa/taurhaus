import { configureToolRegistry, FALLBACK_TOOLS } from '../toolRegistry.js'
import { expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import MeshRuntimeView from './MeshRuntimeView.svelte'
import { coordinationHosted } from '../ipc/coordination.js'
vi.mock('../ipc/coordination.js', () => ({ coordinationHosted: vi.fn() }))

it('preserves hosted authority through the runtime detail projection', async () => {
  // Regression: fdfb3d2c gated controls on hosted, but the runtime projection dropped it.
  coordinationHosted.mockResolvedValue({ attachmentGeneration: 7, thread: { turns: [] } })
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  const view = render(MeshRuntimeView, { teamName: 'team', selectedNode: { name: 'seat', tool: 'codex', hosted: true } })
  try {
    expect(await screen.findByRole('region', { name: 'Hosted conversation' })).toBeVisible()
    expect(screen.getByLabelText('Message hosted member')).toBeEnabled()
    expect(coordinationHosted).toHaveBeenCalledWith('team', 'seat', 'transcript', {})
    await view.rerender({ teamName: 'team', selectedNode: { name: 'pane', tool: 'codex' } })
    expect(screen.queryByRole('region', { name: 'Hosted conversation' })).not.toBeInTheDocument()
  } finally { view.unmount(); vi.unstubAllGlobals() }
})

it('offers delivery on add-agent only for a backend-hostable Codex seat', async () => {
  // Regression: c36b1900 omitted the hosted gate and its explanation from add-agent.
  configureToolRegistry(FALLBACK_TOOLS.map(tool => ({ ...tool, hostingSupported: tool.id === 'codex' })))
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  const onUpdateAddAgentField = vi.fn()
  const view = render(MeshRuntimeView, { teamName: 'team', addAgentOpen: true,
    addAgentDraft: { tool: 'codex' }, onUpdateAddAgentField })
  try {
    const select = screen.getByRole('combobox', { name: 'Delivery' })
    expect(select).toHaveValue('tmux')
    const reason = 'Native delivery requires Mesh 0.3.0 or newer'
    await view.rerender({ meshStatus: { hosted_delivery_supported: false, hosted_delivery_reason: reason } })
    expect(screen.getByRole('option', { name: /native \(app-server/ })).toBeDisabled()
    expect(screen.getByText(reason)).toBeVisible()
    await view.rerender({ meshStatus: { hosted_delivery_supported: true } })
    expect(screen.getByRole('option', { name: /native \(app-server/ })).toBeEnabled()
    await fireEvent.change(select, { target: { value: 'app_server' } })
    expect(onUpdateAddAgentField).toHaveBeenCalledWith('delivery', 'app_server')
    await view.rerender({ addAgentDraft: { tool: 'claude' } })
    expect(screen.queryByRole('combobox', { name: 'Delivery' })).not.toBeInTheDocument()
    configureToolRegistry(null)
    await view.rerender({ addAgentDraft: { tool: 'codex' } })
    expect(screen.queryByRole('combobox', { name: 'Delivery' })).not.toBeInTheDocument()
  } finally { view.unmount(); configureToolRegistry(null); vi.unstubAllGlobals() }
})
