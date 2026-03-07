import { describe, expect, it } from 'vitest'

import {
  applyNavEntryState,
  buildPlatformRouteUrl,
  buildProjectSelectionState,
  classifyMarkdownNavigateAction,
  createProjectPosition,
  normalizeMarkdownTarget,
  switchTabState,
} from './navigation.svelte.js'

describe('normalizeMarkdownTarget', () => {
  it('resolves relative path from context directory', () => {
    expect(normalizeMarkdownTarget('foo.md', 'docs/design-brief.md')).toEqual({
      resolvedPath: 'docs/foo.md',
      escapedAboveRoot: false,
      platformSegments: [],
    })
  })

  it('normalizes parent traversal inside repo paths', () => {
    expect(normalizeMarkdownTarget('../README.md', 'docs/sessions/session.md')).toEqual({
      resolvedPath: 'docs/README.md',
      escapedAboveRoot: false,
      platformSegments: [],
    })
  })

  it('tracks above-root traversal as platform segments', () => {
    expect(normalizeMarkdownTarget('../../../issues/12', 'docs/architecture/daemon.md')).toEqual({
      resolvedPath: '',
      escapedAboveRoot: true,
      platformSegments: ['issues', '12'],
    })
  })
})

describe('buildPlatformRouteUrl', () => {
  it('joins route segments onto the remote base', () => {
    expect(buildPlatformRouteUrl('https://example.com/repo/', ['issues', '12'])).toBe('https://example.com/repo/issues/12')
  })

  it('returns the base when no route segments remain', () => {
    expect(buildPlatformRouteUrl('https://example.com/repo/', [])).toBe('https://example.com/repo')
  })
})

describe('classifyMarkdownNavigateAction', () => {
  it('returns an external action for above-root routes with remote URL', () => {
    expect(classifyMarkdownNavigateAction({
      relativePath: '../../../issues/12',
      contextFile: 'docs/architecture/daemon.md',
      remoteUrl: 'https://example.com/repo',
      pathType: 'not_found',
    })).toEqual({
      type: 'external',
      url: 'https://example.com/repo/issues/12',
    })
  })

  it('returns a directory action', () => {
    expect(classifyMarkdownNavigateAction({
      relativePath: './images',
      contextFile: 'docs/design-brief.md',
      remoteUrl: null,
      pathType: 'directory',
    })).toEqual({
      type: 'directory',
      directory: 'docs/images',
    })
  })

  it('returns a file action with anchor', () => {
    expect(classifyMarkdownNavigateAction({
      relativePath: './guide.md#install',
      contextFile: 'docs/design-brief.md',
      remoteUrl: null,
      pathType: 'file',
    })).toEqual({
      type: 'file',
      file: 'docs/guide.md',
      anchor: 'install',
    })
  })
})

describe('project position helpers', () => {
  it('captures a deep-copied project position snapshot', () => {
    const gitPosition = { selectedHash: 'abc123' }
    const taskPosition = { column: 'doing' }
    const snapshot = createProjectPosition({
      activeTab: 'git',
      visitedTabs: new Set(['overview', 'git']),
      filesPosition: { selectedFile: 'src/main.rs' },
      gitPosition,
      taskPosition,
    })

    gitPosition.selectedHash = 'mutated'
    taskPosition.column = 'done'

    expect(snapshot).toEqual({
      tab: 'git',
      visitedTabs: new Set(['overview', 'git']),
      file: 'src/main.rs',
      gitPosition: { selectedHash: 'abc123' },
      taskPosition: { column: 'doing' },
    })
  })

  it('builds restored selection state from saved position', () => {
    const state = buildProjectSelectionState({
      project: { id: 'p1', path: '/repo', name: 'repo' },
      detail: { branch: 'main' },
      commits: [{ hash: 'abc' }],
      latest: { id: 'session-1' },
      sessionList: [{ id: 'session-1' }],
      readme: { path: 'README.md' },
      relationships: [{ id: 'rel-1' }],
      savedPosition: {
        tab: 'git',
        visitedTabs: new Set(['overview', 'git']),
        file: 'src/main.rs',
        gitPosition: { selectedHash: 'abc123' },
        taskPosition: { column: 'todo' },
      },
    })

    expect(state.activeTab).toBe('git')
    expect(state.visitedTabs).toEqual(new Set(['overview', 'git']))
    expect(state.gitNavTarget).toEqual({ type: 'commit', hash: 'abc123' })
    expect(state.filesNavTarget).toEqual({ file: 'src/main.rs' })
    expect(state.navEntry).toEqual({ tab: 'git', file: 'src/main.rs' })
    expect(state.selectedProject).toEqual({ id: 'p1', path: '/repo', name: 'repo', branch: 'main' })
  })
})

describe('tab navigation helpers', () => {
  it('marks a switched tab as visited', () => {
    const state = switchTabState(new Set(['overview']), 'mesh')
    expect(state).toEqual({
      activeTab: 'mesh',
      visitedTabs: new Set(['overview', 'mesh']),
    })
  })

  it('restores nav entry state for files', () => {
    expect(applyNavEntryState(new Set(['overview']), { tab: 'files', file: 'src/main.rs', lineNumber: 42 })).toEqual({
      activeTab: 'files',
      visitedTabs: new Set(['overview', 'files']),
      filesNavTarget: { file: 'src/main.rs', lineNumber: 42 },
      gitNavTarget: null,
    })
  })

  it('restores nav entry state for git range filters', () => {
    expect(applyNavEntryState(new Set(['overview']), {
      tab: 'git',
      rangeFilter: { after: '2025-01-01', before: '2025-02-01' },
    })).toEqual({
      activeTab: 'git',
      visitedTabs: new Set(['overview', 'git']),
      filesNavTarget: null,
      gitNavTarget: { type: 'range', after: '2025-01-01', before: '2025-02-01' },
    })
  })
})
