import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./ipc.js', () => ({
  getProjectActivity: vi.fn(),
  getRecentCommits: vi.fn(),
}))

vi.mock('./sessionIndicator.js', () => ({
  sessionBadge: vi.fn((session) => ({ toolLabel: session.toolLabel || 'Codex' })),
  hasLiveSession: vi.fn((session) => Boolean(session.live)),
  toolIcon: vi.fn(() => ({ viewBox: '0 0 10 10', path: 'M0 0h10v10z' })),
}))

vi.mock('./format.js', () => ({
  formatDuration: vi.fn((ms) => `${Math.round(ms)}ms`),
}))

const { getProjectActivity, getRecentCommits } = await import('./ipc.js')

import HoverCard from './HoverCard.svelte'

function createProject(overrides = {}) {
  return {
    id: 'proj-1',
    path: '/projects/taurhaus',
    name: 'taurhaus',
    branch: 'main',
    activityState: 'active',
    isDirty: false,
    ...overrides,
  }
}

describe('HoverCard', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getProjectActivity.mockResolvedValue({
      total_active_ms: 8_000,
      session_count: 2,
    })
    getRecentCommits.mockResolvedValue([
      { hash: 'abc1234', message: 'Fix tests', date: 'today' },
      { hash: 'def5678', message: 'Refactor UI', date: 'yesterday' },
    ])
  })

  it('does not render when project is missing', () => {
    render(HoverCard, {
      props: {
        project: null,
        sessions: [],
      },
    })

    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument()
  })

  it('renders project metadata, dirty state, commits, and no-sessions message', async () => {
    render(HoverCard, {
      props: {
        project: createProject({ isDirty: true }),
        sessions: [{ live: false }],
      },
    })

    await waitFor(() => {
      expect(getProjectActivity).toHaveBeenCalledWith('/projects/taurhaus')
      expect(getRecentCommits).toHaveBeenCalledWith('proj-1', 3)
    })

    expect(screen.getByText('taurhaus')).toBeInTheDocument()
    expect(screen.getByText('main')).toBeInTheDocument()
    expect(screen.getByText('Active')).toBeInTheDocument()
    expect(screen.getByText('Dirty')).toBeInTheDocument()
    expect(screen.getByText('No active sessions')).toBeInTheDocument()
    expect(screen.getByText('abc1234')).toBeInTheDocument()
    expect(screen.getByText('Fix tests')).toBeInTheDocument()
  })

  it('renders live sessions with status variants and technical metadata', async () => {
    const now = Date.now()

    render(HoverCard, {
      props: {
        project: createProject({ activityState: 'unknown-state', branch: '' }),
        sessions: [
          {
            live: true,
            state: 'active',
            _duration: 4_000,
            _activeMs: 3_000,
            _activePercent: 75,
            toolLabel: 'Codex',
            session_id: 'session-abc-123456',
            tmux_session: 'mesh',
            tmux_window: 1,
            pid: 42,
          },
          {
            live: true,
            state: 'idle',
            project_unattributed_active: true,
            _duration: 10_000,
            _activeMs: 4_000,
            _activePercent: 40,
            _lastTransition: now - 500,
            toolLabel: 'Gemini',
          },
          {
            live: true,
            state: 'idle',
            project_unattributed_active: false,
            _duration: null,
            toolLabel: 'Claude',
          },
        ],
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('tooltip')).toBeInTheDocument()
    })

    expect(screen.getByText('Unknown')).toBeInTheDocument()
    expect(screen.queryByText('main')).not.toBeInTheDocument()

    expect(screen.getByText('working')).toBeInTheDocument()
    expect(screen.getByText('project active (unattributed)')).toBeInTheDocument()
    expect(screen.getByText('waiting for input')).toBeInTheDocument()

    expect(screen.getByText('Active 3000ms (75%)')).toBeInTheDocument()
    expect(screen.getByText(/^idle \d+ms$/)).toBeInTheDocument()

    expect(screen.getByText('session-')).toBeInTheDocument()
    expect(screen.getByText('mesh:1')).toBeInTheDocument()
    expect(screen.getByText('pid 42')).toBeInTheDocument()
  })

  it('handles stats/commit fetch errors by hiding optional sections', async () => {
    getProjectActivity.mockRejectedValueOnce(new Error('stats failed'))
    getRecentCommits.mockRejectedValueOnce(new Error('commits failed'))

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [{ live: false }],
      },
    })

    await waitFor(() => {
      expect(getProjectActivity).toHaveBeenCalled()
      expect(getRecentCommits).toHaveBeenCalled()
    })

    expect(screen.queryByText(/across .* session/)).not.toBeInTheDocument()
    expect(screen.queryByText('Fix tests')).not.toBeInTheDocument()
    expect(screen.getByText('No active sessions')).toBeInTheDocument()
  })

  it('positions card to stay in viewport when anchor would overflow right/bottom', async () => {
    const anchorEl = {
      getBoundingClientRect: () => ({
        left: 760,
        right: 790,
        top: 580,
        height: 24,
      }),
    }

    const previousWidth = window.innerWidth
    const previousHeight = window.innerHeight
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 800 })
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 600 })

    const rectSpy = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function getRect() {
        if (this.getAttribute && this.getAttribute('role') === 'tooltip') {
          return {
            left: 0,
            top: 0,
            right: 280,
            bottom: 220,
            width: 280,
            height: 220,
            x: 0,
            y: 0,
            toJSON() {},
          }
        }
        return {
          left: 0,
          top: 0,
          right: 0,
          bottom: 0,
          width: 0,
          height: 0,
          x: 0,
          y: 0,
          toJSON() {},
        }
      })

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [],
        anchorEl,
      },
    })

    await waitFor(() => {
      const tooltip = screen.getByRole('tooltip')
      expect(tooltip.style.left).toBe('472px')
      expect(tooltip.style.top).toBe('372px')
    })

    rectSpy.mockRestore()
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: previousWidth })
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: previousHeight })
  })

  it('clamps top position to minimum edge padding', async () => {
    const anchorEl = {
      getBoundingClientRect: () => ({
        left: 10,
        right: 40,
        top: -40,
        height: 20,
      }),
    }

    const rectSpy = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function getRect() {
        if (this.getAttribute && this.getAttribute('role') === 'tooltip') {
          return {
            left: 0,
            top: 0,
            right: 220,
            bottom: 260,
            width: 220,
            height: 260,
            x: 0,
            y: 0,
            toJSON() {},
          }
        }
        return {
          left: 0,
          top: 0,
          right: 0,
          bottom: 0,
          width: 0,
          height: 0,
          x: 0,
          y: 0,
          toJSON() {},
        }
      })

    render(HoverCard, {
      props: {
        project: createProject(),
        sessions: [],
        anchorEl,
      },
    })

    await waitFor(() => {
      expect(screen.getByRole('tooltip').style.top).toBe('8px')
    })

    rectSpy.mockRestore()
  })
})
