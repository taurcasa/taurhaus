import { beforeEach, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import HostedThread from './HostedThread.svelte'
import { coordinationHosted } from '../ipc/coordination.js'
vi.mock('../ipc/coordination.js', () => ({ coordinationHosted: vi.fn() }))
beforeEach(() => vi.clearAllMocks())

it('round-trips operator input with the transcript attachment generation', async () => {
  let text = 'Initial transcript'
  coordinationHosted.mockImplementation(async (_team, _member, operation, params) => {
    if (operation === 'input') { text = params.text; return { turn: { id: 'turn-2' } } }
    return { attachmentGeneration: 7, thread: { turns: [{ items: [{ type: 'agentMessage', text }] }] }, requests: [] }
  })
  const { unmount } = render(HostedThread, { teamName: 'team', memberName: 'seat' })
  expect(await screen.findByText('Initial transcript')).toBeVisible()
  await fireEvent.input(screen.getByLabelText('Message hosted member'), { target: { value: 'Operator marker' } })
  await fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  await waitFor(() => expect(coordinationHosted).toHaveBeenCalledWith('team', 'seat', 'input', { generation: 7, text: 'Operator marker' }))
  expect(await screen.findByText('Operator marker')).toBeVisible()
  unmount()
})

it('explains older daemons without offering an input control', async () => {
  coordinationHosted.mockRejectedValue(new Error('Remote("Unknown method: coordination.hosted_transcript")'))
  const { unmount } = render(HostedThread, { teamName: 'team', memberName: 'seat' })
  expect(await screen.findByRole('alert')).toHaveTextContent('Hosted controls require a daemon update')
  expect(screen.queryByRole('button', { name: 'Send' })).not.toBeInTheDocument()
  unmount()
})

it('clears the previous member before the next transcript is available', async () => {
  // Regression: a9c8109b reused a member detail component with the old draft and attachment.
  coordinationHosted.mockResolvedValueOnce({ attachmentGeneration: 7, thread: { turns: [{ items: [{ text: 'First member' }] }] } })
  coordinationHosted.mockImplementation(() => new Promise(() => {}))
  const { rerender, unmount } = render(HostedThread, { teamName: 'team', memberName: 'first' })
  await screen.findByText('First member')
  await fireEvent.input(screen.getByLabelText('Message hosted member'), { target: { value: 'Private draft' } })
  await rerender({ teamName: 'team', memberName: 'second' })
  await waitFor(() => expect(screen.queryByText('First member')).not.toBeInTheDocument())
  expect(screen.queryByRole('button', { name: 'Send' })).not.toBeInTheDocument()
  unmount()
})

it.each(['NOT_HOSTED', 'Unknown method: coordination.hosted_transcript'])('stops polling after %s', async (message) => {
  // Regression: a9c8109b rescheduled polls after terminal unavailable responses.
  vi.useFakeTimers()
  try {
    coordinationHosted.mockRejectedValue(new Error(message))
    const { unmount } = render(HostedThread, { teamName: 'team', memberName: 'seat' })
    await vi.advanceTimersByTimeAsync(10000)
    expect(coordinationHosted).toHaveBeenCalledTimes(1)
    unmount()
  } finally { vi.useRealTimers() }
})

it('keeps deferred input editable without claiming an unknown outcome', async () => {
  // Regression: a9c8109b marked pre-input contention as outcome-unknown.
  coordinationHosted.mockImplementation(async (_team, _member, operation) => {
    if (operation === 'input') throw new Error('host operation deferred: lock busy')
    return { attachmentGeneration: 7, thread: { turns: [] }, requests: [] }
  })
  const { unmount } = render(HostedThread, { teamName: 'team', memberName: 'seat' })
  const input = await screen.findByLabelText('Message hosted member')
  await fireEvent.input(input, { target: { value: 'Keep this draft' } })
  await fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('deferred'))
  expect(input).toBeEnabled()
  expect(input).toHaveValue('Keep this draft')
  expect(screen.queryByText(/Previous input has an unknown outcome/)).not.toBeInTheDocument()
  unmount()
})
