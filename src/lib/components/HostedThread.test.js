import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import HostedThread from './HostedThread.svelte'
import { coordinationHosted } from '../ipc/coordination.js'
vi.mock('../ipc/coordination.js', () => ({ coordinationHosted: vi.fn() }))
beforeEach(() => vi.clearAllMocks())
afterEach(cleanup)

it('round-trips operator input with the transcript attachment generation', async () => {
  let text = 'Initial transcript'
  coordinationHosted.mockImplementation(async (_team, _member, operation, params) => {
    if (operation === 'input') { text = params.text; return { turn: { id: 'turn-2' } } }
    return { attachmentGeneration: 7, thread: { turns: [{ items: [{ type: 'agentMessage', text }] }] }, requests: [] }
  })
  render(HostedThread, { teamName: 'team', memberName: 'seat' })
  expect(await screen.findByText('Initial transcript')).toBeVisible()
  await fireEvent.input(screen.getByLabelText('Message hosted member'), { target: { value: 'Operator marker' } })
  await fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  await waitFor(() => expect(coordinationHosted).toHaveBeenCalledWith('team', 'seat', 'input', { generation: 7, text: 'Operator marker' }))
  expect(await screen.findByText('Operator marker')).toBeVisible()
})

it('explains older daemons without offering an input control', async () => {
  coordinationHosted.mockRejectedValue(new Error('Remote("Unknown method: coordination.hosted_transcript")'))
  render(HostedThread, { teamName: 'team', memberName: 'seat' })
  expect(await screen.findByRole('alert')).toHaveTextContent('Hosted controls require a daemon update')
  expect(screen.queryByRole('button', { name: 'Send' })).not.toBeInTheDocument()
})

it.each([
  [{ stopped: true }, /Member is stopped/],
  [{ orphanProcessId: 123 }, /Orphaned host process 123/],
])('disables unavailable seats: %j', async (state, message) => {
  // Regression: a9c8109b left stopped input enabled and hid orphan cleanup guidance.
  coordinationHosted.mockResolvedValue({ ...state, outcomeUnknown: false, attachmentGeneration: 7 })
  render(HostedThread, { teamName: 'team', memberName: 'seat' })
  expect(await screen.findByRole('status')).toHaveTextContent(message)
  expect(screen.getByLabelText('Message hosted member')).toBeDisabled()
  expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled()
  expect(screen.getByRole('button', { name: 'Stop turn' })).toBeDisabled()
})

it('clears the previous member before the next transcript is available', async () => {
  // Regression: a9c8109b reused a member detail component with the old draft and attachment.
  coordinationHosted.mockResolvedValueOnce({ attachmentGeneration: 7, thread: { turns: [{ items: [{ text: 'First member' }] }] } })
  coordinationHosted.mockImplementation(() => new Promise(() => {}))
  const { rerender } = render(HostedThread, { teamName: 'team', memberName: 'first' })
  await screen.findByText('First member')
  await fireEvent.input(screen.getByLabelText('Message hosted member'), { target: { value: 'Private draft' } })
  await rerender({ teamName: 'team', memberName: 'second' })
  await waitFor(() => expect(screen.queryByText('First member')).not.toBeInTheDocument())
  expect(screen.queryByRole('button', { name: 'Send' })).not.toBeInTheDocument()
})

it.each(['NOT_HOSTED', 'Unknown method: coordination.hosted_transcript'])('stops polling after %s', async (message) => {
  // Regression: a9c8109b rescheduled polls after terminal unavailable responses.
  vi.useFakeTimers()
  try {
    coordinationHosted.mockRejectedValue(new Error(message))
    render(HostedThread, { teamName: 'team', memberName: 'seat' })
    await vi.advanceTimersByTimeAsync(10000)
    expect(coordinationHosted).toHaveBeenCalledTimes(1)
  } finally { vi.useRealTimers() }
})

it('keeps deferred input editable without claiming an unknown outcome', async () => {
  // Regression: a9c8109b marked pre-input contention as outcome-unknown.
  coordinationHosted.mockImplementation(async (_team, _member, operation) => {
    if (operation === 'input') throw new Error('host operation deferred: lock busy')
    return { attachmentGeneration: 7, thread: { turns: [] }, requests: [] }
  })
  render(HostedThread, { teamName: 'team', memberName: 'seat' })
  const input = await screen.findByLabelText('Message hosted member')
  await fireEvent.input(input, { target: { value: 'Keep this draft' } })
  await fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('deferred'))
  expect(input).toBeEnabled()
  expect(input).toHaveValue('Keep this draft')
  expect(screen.queryByText(/Previous input has an unknown outcome/)).not.toBeInTheDocument()
})

it.each(['pending: rollout is empty', 'temporary disconnect'])('recovers transcript polling after %s', async (message) => {
  // Regression: 04128879 exposed transient reads to a9c8109b's latched recovery alarm.
  vi.useFakeTimers()
  try {
    coordinationHosted.mockRejectedValueOnce(new Error(message)).mockResolvedValue({ thread: { status: { type: 'idle' }, turns: [] } })
    render(HostedThread, { teamName: 'team', memberName: 'seat' })
    await vi.advanceTimersByTimeAsync(0)
    if (message.startsWith('pending:')) {
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
      expect(screen.getByRole('status')).toHaveTextContent('Conversation is updating')
    }
    await vi.advanceTimersByTimeAsync(2000)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
  } finally { vi.useRealTimers() }
})

it('does not report accepted input as deferred when its transcript is pending', async () => {
  // Regression: 04128879's transient read made a9c8109b invite duplicate sends after a receipt.
  coordinationHosted.mockResolvedValueOnce({ attachmentGeneration: 7, thread: { turns: [] } })
    .mockResolvedValueOnce({ turn: { id: 'accepted' } }).mockRejectedValue(new Error('pending: rollout is empty'))
  render(HostedThread, { teamName: 'team', memberName: 'seat' })
  const input = await screen.findByLabelText('Message hosted member')
  await fireEvent.input(input, { target: { value: 'Send once' } })
  await fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Conversation is updating'))
  expect(input).toHaveValue('')
  expect(screen.queryByText(/draft is saved|Input deferred/)).not.toBeInTheDocument()
  expect(coordinationHosted.mock.calls.filter(call => call[2] === 'input')).toHaveLength(1)
})

it('shows the compaction boundary and its recovery turn', async () => {
  // Regression: 6f61f611, attempt-8 continuation: contextCompaction has no text and was dropped.
  coordinationHosted.mockResolvedValue({ thread: { turns: [
    { items: [{ type: 'contextCompaction', id: 'compact-item' }] },
    { items: [{ type: 'userMessage', content: [{ type: 'text', text: '[taurhaus] recovery_card boundary' }] }] },
  ] } })
  render(HostedThread, { teamName: 'team', memberName: 'seat' })
  // Regression: e56953e4 rendered the attempt-8 boundary identically to conversation text.
  expect(await screen.findByRole('separator', { name: 'Context compacted' })).toBeVisible()
  expect(screen.getByText('[taurhaus] recovery_card boundary')).toBeVisible()
})

it('preserves the transcript and silently retries a busy hosted read on the next tick', async () => {
  // Regression: 3000bc3e (#161) background refresh made a9c8109b's poller surface seat contention.
  vi.useFakeTimers()
  try {
    const transcript = text => ({ thread: { turns: [{ items: [{ text }] }] } })
    coordinationHosted.mockRejectedValueOnce(new Error('HOST_OPERATION_FAILED: host member busy'))
      .mockResolvedValueOnce(transcript('Last transcript'))
      .mockRejectedValueOnce(new Error('HOST_OPERATION_FAILED: host member busy'))
      .mockResolvedValue(transcript('Updated transcript'))
    render(HostedThread, { teamName: 'team', memberName: 'seat' })
    await vi.advanceTimersByTimeAsync(0)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
    await vi.advanceTimersByTimeAsync(2000)
    expect(screen.getByText('Last transcript')).toBeVisible()
    await vi.advanceTimersByTimeAsync(2000)
    expect(screen.getByText('Last transcript')).toBeVisible()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
    expect(coordinationHosted).toHaveBeenCalledTimes(3)
    await vi.advanceTimersByTimeAsync(2000)
    expect(screen.getByText('Updated transcript')).toBeVisible()
    expect(coordinationHosted).toHaveBeenCalledTimes(4)
  } finally { cleanup(); vi.useRealTimers() }
})
