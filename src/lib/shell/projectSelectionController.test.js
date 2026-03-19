import { describe, expect, it, vi, beforeEach } from 'vitest'

vi.mock('../projectSelection.js', () => ({
  classifyProjectLoadResults: vi.fn((results) => ({
    issues: results.filter((result) => !result.ok),
    pendingRetry: false,
    visibleIssues: [],
  })),
  loadDeferredProjectSelectionData: vi.fn(),
  prefetchProjectSelectionData: vi.fn(() => Promise.resolve(null)),
}))

import {
  loadDeferredProjectSelectionData,
  prefetchProjectSelectionData,
} from '../projectSelection.js'
import { createShellProjectSelectionController } from './projectSelection.svelte.js'

function createState() {
  return {
    projects: [],
    selectedProject: null,
    sidebarLoading: false,
    sidebarError: null,
    detailLoading: false,
    activeTab: 'overview',
    visitedTabs: new Set(['overview']),
    recentCommits: [{ hash: 'stale' }],
    commitsLoading: false,
    showAllCommits: false,
    filesNavTarget: null,
    latestSession: null,
    sessionHistory: [],
    sessionLoading: false,
    readmeContent: { path: 'README.md' },
    relationships: [],
    relationshipsLoading: false,
    projectLoadIssues: [],
    pendingProjectLoadRetry: false,
    gitNavTarget: null,
    taskNavTarget: null,
  }
}

describe('createShellProjectSelectionController', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('applies deferred selection results onto the critical shell state', async () => {
    const state = createState()
    const nav = {
      push: vi.fn(),
      reset: vi.fn(),
      withSuppressed: (fn) => fn(),
    }

    loadDeferredProjectSelectionData.mockResolvedValue({
      detail: { ok: true, value: { branch: 'main' } },
      commits: { ok: true, value: [{ hash: 'fresh' }] },
      latest: { ok: true, value: { id: 'latest-session' } },
      sessionList: { ok: true, value: [{ id: 'session-1' }] },
      readme: { ok: true, value: { path: 'docs/README.md' } },
      rels: { ok: true, value: [{ id: 'rel-1' }] },
    })

    const controller = createShellProjectSelectionController({
      state,
      positions: { files: null, git: null, task: null },
      nav,
      ipc: {
        listProjects: vi.fn(),
        getProject: vi.fn(),
        getRecentCommits: vi.fn(),
        getAllCommits: vi.fn(),
        getReadme: vi.fn(),
        getLatestSession: vi.fn(),
        listSessions: vi.fn(),
        getRelationships: vi.fn(),
        dismissRelationship: vi.fn(),
      },
      getDaemonRecoveryPending: () => false,
      getDaemonStatus: () => 'connected',
      getSessionBridgeLive: () => false,
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      doc: { hidden: false },
    })

    await controller.selectProject({
      id: 'proj-1',
      name: 'taurhaus',
      path: '/projects/taurhaus',
    })

    expect(nav.reset).toHaveBeenCalledTimes(1)
    expect(nav.push).toHaveBeenCalledWith({ tab: 'overview', file: undefined })
    expect(state.selectedProject).toEqual({
      id: 'proj-1',
      name: 'taurhaus',
      path: '/projects/taurhaus',
      branch: 'main',
    })
    expect(state.recentCommits).toEqual([{ hash: 'fresh' }])
    expect(state.latestSession).toEqual({ id: 'latest-session' })
    expect(state.sessionHistory).toEqual([{ id: 'session-1' }])
    expect(state.readmeContent).toEqual({ path: 'docs/README.md' })
    expect(state.relationships).toEqual([{ id: 'rel-1' }])
    expect(state.detailLoading).toBe(false)
    expect(state.sessionLoading).toBe(false)
    expect(state.commitsLoading).toBe(false)
  })

  it('prefetches hovered projects without touching the active selection', () => {
    const state = createState()
    state.selectedProject = { id: 'proj-1', path: '/projects/taurhaus' }

    const controller = createShellProjectSelectionController({
      state,
      positions: { files: null, git: null, task: null },
      nav: {
        push: vi.fn(),
        reset: vi.fn(),
        withSuppressed: (fn) => fn(),
      },
      ipc: {
        listProjects: vi.fn(),
        getProject: vi.fn(),
        getRecentCommits: vi.fn(),
        getAllCommits: vi.fn(),
        getReadme: vi.fn(),
        getLatestSession: vi.fn(),
        listSessions: vi.fn(),
        getRelationships: vi.fn(),
        dismissRelationship: vi.fn(),
      },
      getDaemonRecoveryPending: () => false,
      getDaemonStatus: () => 'connected',
      getSessionBridgeLive: () => false,
      logger: console,
      doc: { hidden: false },
    })

    controller.prefetchProjectSelection({
      id: 'proj-2',
      path: '/projects/mesh',
    })

    expect(prefetchProjectSelectionData).toHaveBeenCalledWith(
      'proj-2',
      expect.any(Object),
      expect.objectContaining({
        projectPath: '/projects/mesh',
        daemonStatus: 'connected',
        batchKind: 'deferred',
      })
    )
    expect(state.selectedProject.id).toBe('proj-1')
  })
})
