import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./ipc.js', () => ({
  refreshAccountsUsage: vi.fn(() => Promise.resolve(true)),
  navigateToSession: vi.fn(),
  launchCliSession: vi.fn(),
  stopClaudeSession: vi.fn(),
  removeProject: vi.fn(),
  listAccounts: vi.fn(() =>
    Promise.resolve({ accounts: [], source: 'native', degraded: false, error: null })
  ),
  setProjectAccount: vi.fn(() => Promise.resolve()),
  listAccountRelationships: vi.fn(() => Promise.resolve({ byAccount: {} })),
  resolveLaunchAccount: vi.fn(() => Promise.resolve({ needsChoice: true })),
  getSettings: vi.fn(() => Promise.resolve({ terminal: {} })),
}))

vi.mock('./sessionStore.svelte.js', () => ({
  getSessionForProject: vi.fn(() => null),
  getSessionsForProject: vi.fn(() => []),
}))

vi.mock('./sessionIndicator.js', () => ({
  hasLiveSession: vi.fn(() => false),
  rowTintForSessions: vi.fn(() => ''),
  toolIndicators: vi.fn(() => []),
}))

import Sidebar from './Sidebar.svelte'

// The Shell body geometry the driver would measure in the real app. JSDOM
// has no layout, so the rects are answered by element identity; the row's
// right edge decides flush (257, against the rail's inner border) vs a
// scrollbar lane (249).
const RAIL = { left: 6, top: 46, right: 258, bottom: 634, width: 252, height: 588 }
const LIST = { left: 7, top: 92, right: 257, bottom: 590, width: 250, height: 498 }

function mockLayout({ rowRight = 257 } = {}) {
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function () {
    if (this.classList?.contains('sidebar-row-pulled')) {
      return { left: 13, top: 200, right: rowRight, bottom: 236, width: rowRight - 13, height: 36 }
    }
    if (this.dataset?.testid === 'sidebar-project-scroll') return { ...LIST }
    if (this.tagName === 'ASIDE') return { ...RAIL }
    return { left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0 }
  })
}

function makeProjects(count) {
  return Array.from({ length: count }, (_, index) => ({
    id: `project-${index}`,
    name: `Project ${index}`,
    path: `/projects/project-${index}`,
    activityState: 'active',
    branch: 'main',
    isDirty: false,
  }))
}

describe('Sidebar pulled-row bridge — show/hide', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockLayout()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('bridges the pulled row into the panel gutter', async () => {
    const projects = makeProjects(3)
    render(Sidebar, { props: { projects, selectedProject: projects[1] } })

    const bridge = screen.getByTestId('sidebar-bridge')
    await waitFor(() => expect(bridge).toHaveAttribute('data-bridge-active'))

    // The clip box spans rail border → panel edge + 2px hairline cover,
    // sized to the list viewport.
    expect(bridge.style.left).toBe('257px')
    expect(bridge.style.width).toBe('9px')
    expect(bridge.style.top).toBe('92px')
    expect(bridge.style.height).toBe('498px')

    // The strip flares 8px beyond the row (content offset 200-92-8).
    const strip = bridge.querySelector('.sidebar-bridge-strip')
    expect(strip.style.top).toBe('100px')
    expect(strip.style.height).toBe('52px')

    // Flush row: no scrollbar lane to cover.
    expect(screen.getByTestId('sidebar-bridge-lane')).not.toHaveAttribute('data-bridge-active')
  })

  it('shows no bridge when nothing is selected', async () => {
    render(Sidebar, { props: { projects: makeProjects(3), selectedProject: null } })

    await waitFor(() =>
      expect(screen.getByTestId('sidebar-bridge')).not.toHaveAttribute('data-bridge-active')
    )
  })

  it('shows no bridge in the held state (utility surface open)', async () => {
    const projects = makeProjects(3)
    render(Sidebar, {
      props: { projects, selectedProject: projects[1], settingsOpen: true },
    })

    // The row demotes to held (no .sidebar-row-pulled), so the bridge has
    // nothing to continue; the pulled footer key deliberately does not
    // bridge (see the rail-key-pulled ruling in app.css).
    await waitFor(() =>
      expect(screen.getByTestId('sidebar-bridge')).not.toHaveAttribute('data-bridge-active')
    )
  })

  it('shows no bridge while the virtualizer has the pulled row unmounted', async () => {
    const projects = makeProjects(60)
    render(Sidebar, { props: { projects, selectedProject: projects[59] } })

    // Row 59 sits far below the 480px viewport + 220px overscan window: it
    // is not in the DOM, and material must never float beside empty rail.
    await waitFor(() => {
      expect(document.querySelector('.sidebar-row-pulled')).toBe(null)
      expect(screen.getByTestId('sidebar-bridge')).not.toHaveAttribute('data-bridge-active')
    })
  })

  it('covers the scrollbar lane when the pulled row stops short of the rail edge', async () => {
    mockLayout({ rowRight: 249 })
    const projects = makeProjects(3)
    render(Sidebar, { props: { projects, selectedProject: projects[1] } })

    const lane = screen.getByTestId('sidebar-bridge-lane')
    await waitFor(() => expect(lane).toHaveAttribute('data-bridge-active'))

    // Rail PADDING-box coordinates (the aside's 1px border makes the
    // padding box the containing block for the absolute lane): from the
    // row's right edge (viewport 249 -> 249-6-1) to the inner border.
    expect(lane.style.left).toBe('242px')
    expect(lane.style.width).toBe('8px')
    expect(lane.style.top).toBe('45px')
    expect(lane.style.height).toBe('498px')

    // The gutter bridge still runs, sharing the strip's vertical base.
    const bridge = screen.getByTestId('sidebar-bridge')
    expect(bridge).toHaveAttribute('data-bridge-active')
    expect(lane.querySelector('.sidebar-bridge-strip').style.top).toBe('100px')
  })

  it('hides the bridge while the list is reloading', async () => {
    // Regression: loadProjects flips sidebarLoading with projects and
    // selectedProject untouched, replacing the rows with the skeleton; the
    // strip must not keep painting across the gutter beside it.
    const projects = makeProjects(3)
    const { rerender } = render(Sidebar, {
      props: { projects, selectedProject: projects[1] },
    })
    const bridge = screen.getByTestId('sidebar-bridge')
    await waitFor(() => expect(bridge).toHaveAttribute('data-bridge-active'))

    await rerender({ projects, selectedProject: projects[1], sidebarLoading: true })
    await waitFor(() => {
      expect(screen.getByTestId('sidebar-skeleton')).toBeInTheDocument()
      expect(bridge).not.toHaveAttribute('data-bridge-active')
    })
  })

  it('hides the bridge while the list shows its error state', async () => {
    // Regression: the error branch is sticky until a retry; a bridge left
    // floating beside it would paint material next to empty rail for good.
    const projects = makeProjects(3)
    const { rerender } = render(Sidebar, {
      props: { projects, selectedProject: projects[1] },
    })
    const bridge = screen.getByTestId('sidebar-bridge')
    await waitFor(() => expect(bridge).toHaveAttribute('data-bridge-active'))

    await rerender({
      projects,
      selectedProject: projects[1],
      sidebarError: 'Could not load projects',
    })
    await waitFor(() => {
      expect(screen.getByTestId('sidebar-error')).toBeInTheDocument()
      expect(bridge).not.toHaveAttribute('data-bridge-active')
      expect(screen.getByTestId('sidebar-bridge-lane')).not.toHaveAttribute('data-bridge-active')
    })
  })

  it('drops the bridge when the selection is cleared', async () => {
    const projects = makeProjects(3)
    const { rerender } = render(Sidebar, {
      props: { projects, selectedProject: projects[1] },
    })
    const bridge = screen.getByTestId('sidebar-bridge')
    await waitFor(() => expect(bridge).toHaveAttribute('data-bridge-active'))

    await rerender({ projects, selectedProject: null })
    await waitFor(() => expect(bridge).not.toHaveAttribute('data-bridge-active'))
  })
})
