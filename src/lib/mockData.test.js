import { describe, it, expect } from 'vitest'
import {
  MOCK_PROJECTS, MOCK_COMMITS, MOCK_DIFF_HUNKS, MOCK_FILE_TREE,
  MOCK_SESSION, MOCK_SESSIONS, MOCK_DETAIL,
  MOCK_SEARCH_RESULTS, MOCK_RELATIONSHIPS, MOCK_SETTINGS,
  MOCK_CLAUDE_SESSIONS,
} from './mockData.js'

describe('mockData', () => {
  it('exports MOCK_PROJECTS as a non-empty array with expected shape', () => {
    expect(Array.isArray(MOCK_PROJECTS)).toBe(true)
    expect(MOCK_PROJECTS.length).toBeGreaterThan(0)
    expect(MOCK_PROJECTS[0]).toHaveProperty('id')
    expect(MOCK_PROJECTS[0]).toHaveProperty('name')
    expect(MOCK_PROJECTS[0]).toHaveProperty('activityState')
  })

  it('exports MOCK_COMMITS as a non-empty array with expected shape', () => {
    expect(Array.isArray(MOCK_COMMITS)).toBe(true)
    expect(MOCK_COMMITS[0]).toHaveProperty('hash')
    expect(MOCK_COMMITS[0]).toHaveProperty('message')
  })

  it('exports MOCK_DIFF_HUNKS as a non-empty array', () => {
    expect(Array.isArray(MOCK_DIFF_HUNKS)).toBe(true)
    expect(MOCK_DIFF_HUNKS[0]).toHaveProperty('lines')
  })

  it('exports MOCK_FILE_TREE as a non-empty array', () => {
    expect(Array.isArray(MOCK_FILE_TREE)).toBe(true)
    expect(MOCK_FILE_TREE[0]).toHaveProperty('name')
    expect(MOCK_FILE_TREE[0]).toHaveProperty('is_dir')
  })

  it('exports MOCK_SESSION with expected shape', () => {
    expect(MOCK_SESSION).toHaveProperty('id')
    expect(MOCK_SESSION).toHaveProperty('summary')
    expect(MOCK_SESSION).toHaveProperty('next_steps')
  })

  it('exports MOCK_SESSIONS as a non-empty array', () => {
    expect(Array.isArray(MOCK_SESSIONS)).toBe(true)
    expect(MOCK_SESSIONS[0]).toHaveProperty('summary')
  })

  it('exports MOCK_DETAIL with expected shape', () => {
    expect(MOCK_DETAIL).toHaveProperty('id')
    expect(MOCK_DETAIL).toHaveProperty('name')
    expect(MOCK_DETAIL).toHaveProperty('description')
    expect(MOCK_DETAIL).toHaveProperty('activityState')
  })

  it('exports MOCK_SEARCH_RESULTS as a non-empty array', () => {
    expect(Array.isArray(MOCK_SEARCH_RESULTS)).toBe(true)
    expect(MOCK_SEARCH_RESULTS[0]).toHaveProperty('entity_type')
    expect(MOCK_SEARCH_RESULTS[0]).toHaveProperty('snippet')
  })

  it('exports MOCK_RELATIONSHIPS as a non-empty array', () => {
    expect(Array.isArray(MOCK_RELATIONSHIPS)).toBe(true)
    expect(MOCK_RELATIONSHIPS[0]).toHaveProperty('relationship_type')
  })

  it('exports MOCK_SETTINGS with expected shape', () => {
    expect(MOCK_SETTINGS).toHaveProperty('scan_directories')
    expect(MOCK_SETTINGS).toHaveProperty('thresholds')
    expect(MOCK_SETTINGS).toHaveProperty('code_theme')
    expect(MOCK_SETTINGS).toHaveProperty('project_dialog_last_path')
  })

  it('exports MOCK_CLAUDE_SESSIONS as a non-empty array with expected shape', () => {
    expect(Array.isArray(MOCK_CLAUDE_SESSIONS)).toBe(true)
    expect(MOCK_CLAUDE_SESSIONS[0]).toHaveProperty('pid')
    expect(MOCK_CLAUDE_SESSIONS[0]).toHaveProperty('cli_tool')
    expect(MOCK_CLAUDE_SESSIONS[0]).toHaveProperty('state')
  })

  it('covers all three activity states in MOCK_PROJECTS', () => {
    const states = new Set(MOCK_PROJECTS.map(p => p.activityState))
    expect(states).toContain('active')
    expect(states).toContain('recent')
    expect(states).toContain('stale')
    expect(states).toContain('dormant')
  })

  it('covers all three CLI tools in MOCK_CLAUDE_SESSIONS', () => {
    const tools = new Set(MOCK_CLAUDE_SESSIONS.map(s => s.cli_tool))
    expect(tools).toContain('claude')
    expect(tools).toContain('codex')
    expect(tools).toContain('agy')
  })
})
