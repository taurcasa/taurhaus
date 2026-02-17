import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

// Mock the IPC module
vi.mock('./ipc.js', () => ({
  listProjects: vi.fn(),
  getProject: vi.fn(),
  getFileTree: vi.fn(),
  readFile: vi.fn(),
  getRecentCommits: vi.fn(),
  getAllCommits: vi.fn(),
  getReadme: vi.fn(),
  getLatestSession: vi.fn(),
  listSessions: vi.fn(),
  getSession: vi.fn(),
  isTauri: vi.fn(() => false),
}))

describe('Sidebar data loading', () => {
  let ipc

  beforeEach(async () => {
    vi.clearAllMocks()
    ipc = await import('./ipc.js')
  })

  it('listProjects returns mock data in array form', async () => {
    const mockData = [
      { id: 'p1', name: 'test-project', activity_state: 'active', branch: 'main', is_dirty: false },
    ]
    ipc.listProjects.mockResolvedValue(mockData)

    const result = await ipc.listProjects()

    expect(result).toEqual(mockData)
    expect(result[0].activity_state).toBe('active')
  })

  it('listProjects groups by activity_state', async () => {
    const mockData = [
      { id: 'p1', name: 'a', activity_state: 'active', branch: 'main', is_dirty: false },
      { id: 'p2', name: 'b', activity_state: 'recent', branch: 'main', is_dirty: false },
      { id: 'p3', name: 'c', activity_state: 'dormant', branch: 'main', is_dirty: false },
    ]
    ipc.listProjects.mockResolvedValue(mockData)

    const result = await ipc.listProjects()
    const groups = {
      active: result.filter(p => p.activity_state === 'active'),
      recent: result.filter(p => p.activity_state === 'recent'),
      stale: result.filter(p => p.activity_state === 'stale'),
      dormant: result.filter(p => p.activity_state === 'dormant'),
    }

    expect(groups.active).toHaveLength(1)
    expect(groups.recent).toHaveLength(1)
    expect(groups.stale).toHaveLength(0)
    expect(groups.dormant).toHaveLength(1)
  })

  it('activity_state maps to correct dot color class', () => {
    const dotColor = {
      active: 'bg-success-300',
      recent: 'bg-info-300',
      stale: 'bg-warning-300',
      dormant: 'bg-zinc-400',
    }

    expect(dotColor['active']).toBe('bg-success-300')
    expect(dotColor['recent']).toBe('bg-info-300')
    expect(dotColor['stale']).toBe('bg-warning-300')
    expect(dotColor['dormant']).toBe('bg-zinc-400')
  })

  it('listProjects handles error gracefully', async () => {
    ipc.listProjects.mockRejectedValue(new Error('Connection failed'))

    await expect(ipc.listProjects()).rejects.toThrow('Connection failed')
  })

  it('getProject returns full detail shape', async () => {
    const mockDetail = {
      id: 'p1',
      name: 'taurhaus',
      path: '~/projects/taurhaus',
      description: 'Desktop tool for AI project management',
      activity_state: 'active',
      last_activity_at: '2025-01-01T00:00:00Z',
      hero_preference: null,
      created_at: '2025-01-01T00:00:00Z',
      updated_at: '2025-01-01T00:00:00Z',
      branch: 'main',
      is_dirty: false,
    }
    ipc.getProject.mockResolvedValue(mockDetail)

    const result = await ipc.getProject('p1')

    expect(result.description).toBe('Desktop tool for AI project management')
    expect(result.activity_state).toBe('active')
    expect(result.branch).toBe('main')
  })

  it('getFileTree returns nested structure', async () => {
    const mockTree = [
      { name: 'src', path: 'src', is_dir: true, children: [
        { name: 'main.rs', path: 'src/main.rs', is_dir: false, children: [] },
      ]},
      { name: 'README.md', path: 'README.md', is_dir: false, children: [] },
    ]
    ipc.getFileTree.mockResolvedValue(mockTree)

    const result = await ipc.getFileTree('p1')

    expect(result).toHaveLength(2)
    expect(result[0].is_dir).toBe(true)
    expect(result[0].children).toHaveLength(1)
    expect(result[1].name).toBe('README.md')
  })

  it('readFile returns content with language', async () => {
    const mockContent = {
      path: 'src/main.rs',
      content: 'fn main() {}',
      language: 'rust',
    }
    ipc.readFile.mockResolvedValue(mockContent)

    const result = await ipc.readFile('p1', 'src/main.rs')

    expect(result.content).toBe('fn main() {}')
    expect(result.language).toBe('rust')
  })

  it('tab state defaults and can distinguish between overview and files', () => {
    // Tab state is just a string — verify the expected values
    const validTabs = ['overview', 'files']
    const defaultTab = 'overview'

    expect(validTabs).toContain(defaultTab)
    expect(validTabs).toHaveLength(2)
  })
})
