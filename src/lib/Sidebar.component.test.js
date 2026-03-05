import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./ipc.js', () => ({
  navigateToSession: vi.fn(),
  launchClaudeSession: vi.fn(),
  stopClaudeSession: vi.fn(),
  removeProject: vi.fn(),
}))

vi.mock('./sessionStore.svelte.js', () => ({
  getSessionForProject: vi.fn(() => null),
  getSessionsForProject: vi.fn(() => []),
}))

vi.mock('./sessionIndicator.js', () => ({
  rowTintForSessions: vi.fn(() => ''),
  toolIndicators: vi.fn(() => []),
}))

import Sidebar from './Sidebar.svelte'

function makeProjects(count) {
  const activityStates = ['active', 'recent', 'stale', 'dormant']
  return Array.from({ length: count }, (_, index) => ({
    id: `project-${index}`,
    name: `Project ${index}`,
    path: `/projects/project-${index}`,
    activity_state: activityStates[index % activityStates.length],
    branch: null,
    is_dirty: false,
  }))
}

describe('Sidebar virtualization + timer cleanup', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('virtualizes large project lists and limits rendered project rows', async () => {
    render(Sidebar, {
      props: {
        projects: makeProjects(220),
      },
    })

    await waitFor(() => {
      expect(screen.getAllByTestId('project-item').length).toBeGreaterThan(0)
    })

    expect(screen.getAllByTestId('project-item').length).toBeLessThan(150)
  })

  it('clears pending hover timers on unmount', async () => {
    vi.useFakeTimers()
    const clearTimeoutSpy = vi.spyOn(globalThis, 'clearTimeout')

    const { unmount } = render(Sidebar, {
      props: {
        projects: makeProjects(1),
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('project-item')).toBeInTheDocument()
    })

    await fireEvent.mouseEnter(screen.getByTestId('project-item'))

    unmount()
    expect(clearTimeoutSpy).toHaveBeenCalled()
  })
})
