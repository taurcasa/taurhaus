import { expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/svelte'
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
