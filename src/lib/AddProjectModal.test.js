import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./ipc.js', () => ({
  scanDirectory: vi.fn(),
  registerProjectsBatch: vi.fn(),
  listProjects: vi.fn(),
  removeProject: vi.fn(),
  validateProjectPath: vi.fn(),
}))

const { listProjects } = await import('./ipc.js')
import AddProjectModal from './AddProjectModal.svelte'

describe('AddProjectModal timer cleanup', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    listProjects.mockResolvedValue([
      {
        id: 'p1',
        name: 'Project One',
        path: '/projects/one',
        activity_state: 'active',
      },
    ])
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('clears pending remove-confirm timer on unmount', async () => {
    vi.useFakeTimers()

    const { unmount } = render(AddProjectModal, {
      props: {
        dark: false,
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('remove-p1')).toBeInTheDocument()
    })

    const baselineTimers = vi.getTimerCount()
    await fireEvent.click(screen.getByTestId('remove-p1'))
    expect(vi.getTimerCount()).toBeGreaterThan(baselineTimers)

    unmount()
    expect(vi.getTimerCount()).toBe(baselineTimers)
  })
})
