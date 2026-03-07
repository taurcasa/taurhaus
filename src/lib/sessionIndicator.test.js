import { describe, it, expect } from 'vitest'
import { groupedSessionIndicators, hasLiveSession, isActiveSession, rowTintClass, rowTintForSessions, sessionBadge, toolIndicators, uniqueTools } from './sessionIndicator.js'
import { getToolIcon, getGroupedIcon, TOOL_GROUPED_ICONS, TOOL_SIDEBAR_SMALL_ICONS } from './toolLogos.js'

function session(overrides = {}) {
  return {
    state: 'active',
    cli_tool: 'claude',
    pid: 1,
    group_kind: 'standalone',
    group_id: null,
    group_label: null,
    member_name: null,
    ...overrides,
  }
}

describe('getGroupedIcon', () => {
  it('returns grouped icon for claude+codex', () => {
    const icon = getGroupedIcon(['claude', 'codex'])
    expect(icon).toBeTruthy()
    expect(icon.viewBox).toBe('0 0 36 16')
    expect(icon.paths).toHaveLength(2)
    expect(icon.paths[0].d).toBe(TOOL_SIDEBAR_SMALL_ICONS.claude.path)
    expect(icon.paths[1].d).toBe(TOOL_SIDEBAR_SMALL_ICONS.codex.path)
    expect(icon.paths[1].transform).toBe('translate(20 0)')
  })

  it('returns grouped icon for all three tools', () => {
    const icon = getGroupedIcon(['claude', 'codex', 'gemini'])
    expect(icon).toBeTruthy()
    expect(icon.viewBox).toBe('0 0 56 16')
    expect(icon.paths).toHaveLength(3)
    expect(icon.paths[2].transform).toBe('translate(40 0)')
  })

  it('sorts tools to canonical order regardless of input order', () => {
    const a = getGroupedIcon(['gemini', 'claude'])
    const b = getGroupedIcon(['claude', 'gemini'])
    expect(a).toEqual(b)
    expect(a).toBe(TOOL_GROUPED_ICONS['claude+gemini'])
  })

  it('returns null for single tool', () => {
    expect(getGroupedIcon(['claude'])).toBeNull()
  })

  it('returns null for empty or invalid input', () => {
    expect(getGroupedIcon([])).toBeNull()
    expect(getGroupedIcon(null)).toBeNull()
    expect(getGroupedIcon(undefined)).toBeNull()
  })

  it('returns null for unknown tool combinations', () => {
    expect(getGroupedIcon(['claude', 'unknown'])).toBeNull()
  })

  it('covers all four defined combinations', () => {
    expect(Object.keys(TOOL_GROUPED_ICONS)).toEqual([
      'claude+codex',
      'claude+gemini',
      'codex+gemini',
      'claude+codex+gemini',
    ])
  })
})

describe('sessionIndicator', () => {
  it('detects live sessions', () => {
    expect(hasLiveSession({ state: 'active' })).toBe(true)
    expect(hasLiveSession({ state: 'idle' })).toBe(true)
    expect(hasLiveSession(null)).toBe(false)
  })

  it('detects active sessions only', () => {
    expect(isActiveSession({ state: 'active' })).toBe(true)
    expect(isActiveSession({ state: 'idle' })).toBe(false)
    expect(isActiveSession(undefined)).toBe(false)
  })

  it('applies row tint only when session exists', () => {
    expect(rowTintClass({ state: 'active' })).toBe('bg-white/[0.03]')
    expect(rowTintClass({ state: 'idle' })).toBe('bg-white/[0.03]')
    expect(rowTintClass(null)).toBe('')
  })

  it('returns idle badge as explicit and interactive when tmux fields exist', () => {
    const badge = sessionBadge({
      state: 'idle',
      cli_tool: 'claude',
      tmux_session: 'main',
      tmux_window: '2',
      tmux_pane: '%7',
    })

    expect(badge.visible).toBe(true)
    expect(badge.label).toBe('IDLE')
    expect(badge.toolLabel).toBe('Claude')
    expect(badge.badgeClass).toContain('session-pill-idle')
    expect(badge.interactive).toBe(true)
  })

  it('returns active badge as run state', () => {
    const badge = sessionBadge({ state: 'active', cli_tool: 'claude' })

    expect(badge.visible).toBe(true)
    expect(badge.label).toBe('RUN')
    expect(badge.toolLabel).toBe('Claude')
    expect(badge.badgeClass).toContain('session-pill-active')
  })

  it('returns hidden badge with no session', () => {
    const badge = sessionBadge(null)

    expect(badge.visible).toBe(false)
    expect(badge.label).toBe('')
    expect(badge.toolLabel).toBe('')
    expect(badge.interactive).toBe(false)
  })

  it('badge shows correct tool name for each CLI tool', () => {
    const claudeBadge = sessionBadge({ state: 'active', cli_tool: 'claude' })
    expect(claudeBadge.toolLabel).toBe('Claude')
    expect(claudeBadge.ariaLabel).toContain('Claude')

    const codexBadge = sessionBadge({ state: 'idle', cli_tool: 'codex' })
    expect(codexBadge.toolLabel).toBe('Codex')
    expect(codexBadge.ariaLabel).toContain('Codex')

    const geminiBadge = sessionBadge({ state: 'active', cli_tool: 'gemini' })
    expect(geminiBadge.toolLabel).toBe('Gemini')
    expect(geminiBadge.ariaLabel).toContain('Gemini')
  })

  it('defaults to Claude when cli_tool is missing', () => {
    const badge = sessionBadge({ state: 'active' })
    expect(badge.toolLabel).toBe('Claude')
    expect(badge.ariaLabel).toContain('Claude')
  })

  it('rowTintForSessions returns tint when any session is live', () => {
    expect(rowTintForSessions([{ state: 'active' }])).toBe('bg-white/[0.03]')
    expect(rowTintForSessions([{ state: 'idle' }])).toBe('bg-white/[0.03]')
    expect(rowTintForSessions([])).toBe('')
    expect(rowTintForSessions(null)).toBe('')
  })

  it('uniqueTools deduplicates tools in stable sidebar order', () => {
    const tools = uniqueTools([
      session({ cli_tool: 'gemini' }),
      session({ cli_tool: 'codex', pid: 2 }),
      session({ cli_tool: 'claude', pid: 3 }),
      session({ cli_tool: 'codex', pid: 4 }),
    ])

    expect(tools.map(tool => tool.tool)).toEqual(['claude', 'codex', 'gemini'])
    expect(tools.map(tool => tool.fullName)).toEqual(['Claude', 'Codex', 'Gemini'])
  })

  it('toolIndicators returns one indicator per live session with icon data', () => {
    const sessions = [
      session({ state: 'active', cli_tool: 'claude', tmux_session: '0', tmux_window: '1', tmux_pane: '%3' }),
      session({ state: 'idle', cli_tool: 'codex', pid: 2, tmux_session: '0', tmux_window: '1', tmux_pane: '%4' }),
    ]
    const indicators = toolIndicators(sessions)
    expect(indicators).toHaveLength(2)

    // Claude indicator (active)
    expect(indicators[0].kind).toBe('session')
    expect(indicators[0].fullName).toBe('Claude')
    expect(indicators[0].isActive).toBe(true)
    expect(indicators[0].interactive).toBe(true)
    expect(indicators[0].colorClass).toBe('text-success-300')
    expect(indicators[0].icon).toBeTruthy()
    expect(indicators[0].icon.viewBox).toBe('0 0 16 16')
    expect(indicators[0].icon.path).toBeTruthy()

    // Codex indicator (idle)
    expect(indicators[1].fullName).toBe('Codex')
    expect(indicators[1].isActive).toBe(false)
    expect(indicators[1].interactive).toBe(true)
    expect(indicators[1].colorClass).toBe('text-warning-300')
    expect(indicators[1].icon).toBeTruthy()
    expect(indicators[1].icon.viewBox).toBe('0 0 16 16')
  })

  it('toolIndicators returns empty array for no sessions', () => {
    expect(toolIndicators([])).toEqual([])
    expect(toolIndicators(null)).toEqual([])
  })

  it('toolIndicators for all three tools have distinct icons', () => {
    const sessions = [
      session({ state: 'active', cli_tool: 'claude' }),
      session({ state: 'active', cli_tool: 'codex', pid: 2 }),
      session({ state: 'idle', cli_tool: 'gemini', pid: 3 }),
    ]
    const indicators = toolIndicators(sessions)
    expect(indicators).toHaveLength(3)
    expect(indicators.map(i => i.fullName)).toEqual(['Claude', 'Codex', 'Gemini'])

    // Each tool has a distinct SVG path
    const paths = indicators.map(i => i.icon.path)
    expect(new Set(paths).size).toBe(3)

    // Gemini uses 65x65 viewBox (different from the 16x16 bootstrap icons)
    expect(indicators[2].icon.viewBox).toBe('0 0 65 65')
  })

  it('toolIndicators carries session reference', () => {
    const active = session({ state: 'active', cli_tool: 'claude', pid: 12345 })
    const indicators = toolIndicators([active])
    expect(indicators[0].session).toBe(active)
  })

  it('toolIndicators marks unattributed project activity distinctly', () => {
    const indicators = toolIndicators([
      session({ state: 'idle', cli_tool: 'codex', project_unattributed_active: true }),
    ])
    expect(indicators[0].isActive).toBe(false)
    expect(indicators[0].isUnattributed).toBe(true)
    expect(indicators[0].colorClass).toBe('text-info-300')
    expect(indicators[0].ariaLabel).toContain('unattributed')
  })

  it('groupedSessionIndicators collapses matching team sessions into one active token', () => {
    const indicators = groupedSessionIndicators([
      session({ group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'lead' }),
      session({ state: 'idle', cli_tool: 'codex', pid: 2, group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer2' }),
      session({ state: 'idle', cli_tool: 'gemini', pid: 3, group_kind: 'standalone' }),
    ])

    expect(indicators).toHaveLength(1)
    expect(indicators[0]).toMatchObject({
      kind: 'team',
      groupId: 'team-a',
      groupLabel: 'team-a',
      count: 2,
      layout: 'rail',
      isActive: true,
      tone: 'active',
    })
    expect(indicators[0].members.map(member => member.member_name)).toEqual(['lead', 'developer2'])
    expect(indicators[0].memberTools.map(tool => tool.tool)).toEqual(['claude', 'codex'])
    expect(indicators[0].memberTools.map(tool => tool.iconVariant)).toEqual(['sidebarSmall', 'sidebarSmall'])
    expect(indicators[0].memberTools[0].icon).toEqual(getToolIcon('claude', 'sidebarSmall'))
  })

  it('toolIndicators shows a connector rail for a 2-member team even when only two sessions are present', () => {
    const indicators = toolIndicators([
      session({ group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'lead' }),
      session({ pid: 2, cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer2' }),
    ])

    expect(indicators).toHaveLength(1)
    expect(indicators[0]).toMatchObject({
      kind: 'team',
      groupId: 'team-a',
      layout: 'rail',
      count: 2,
      tone: 'active',
    })
  })

  it('toolIndicators shows a connector rail for a 3-member team below the stack threshold', () => {
    const indicators = toolIndicators([
      session({ group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'lead' }),
      session({ pid: 2, cli_tool: 'codex', state: 'idle', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer2' }),
      session({ pid: 3, cli_tool: 'gemini', state: 'idle', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer3' }),
    ])

    expect(indicators).toHaveLength(1)
    expect(indicators[0]).toMatchObject({
      kind: 'team',
      groupId: 'team-a',
      layout: 'rail',
      count: 3,
      tone: 'active',
    })
    expect(indicators[0].memberTools.map(tool => tool.tool)).toEqual(['claude', 'codex', 'gemini'])
  })

  it('toolIndicators leaves one-member team sessions as standalone logos', () => {
    const indicators = toolIndicators([
      session({ group_kind: 'mesh_team', group_id: 'solo-team', group_label: 'solo-team', member_name: 'developer2' }),
      session({ pid: 2, cli_tool: 'codex', state: 'idle', group_kind: 'standalone' }),
    ])

    expect(indicators).toHaveLength(2)
    expect(indicators.every(indicator => indicator.kind === 'session')).toBe(true)
    expect(indicators.map(indicator => indicator.fullName)).toEqual(['Claude', 'Codex'])
  })

  it('toolIndicators uses a rail group once the row reaches 4+ total live sessions', () => {
    const indicators = toolIndicators([
      session({ group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'lead' }),
      session({ pid: 2, cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer2', state: 'idle' }),
      session({ pid: 3, cli_tool: 'gemini', state: 'idle', group_kind: 'standalone' }),
      session({ pid: 4, cli_tool: 'claude', state: 'active', group_kind: 'standalone' }),
    ])

    expect(indicators).toHaveLength(3)
    expect(indicators[0]).toMatchObject({
      kind: 'team',
      groupId: 'team-a',
      layout: 'rail',
      count: 2,
      isActive: true,
    })
    expect(indicators.slice(1).map(indicator => indicator.fullName)).toEqual(['Gemini', 'Claude'])
    expect(indicators.slice(1).every(indicator => indicator.iconVariant === 'default')).toBe(true)
  })

  it('toolIndicators keeps standalone sessions individual alongside a small team rail', () => {
    const indicators = toolIndicators([
      session({ group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'lead' }),
      session({ pid: 2, cli_tool: 'codex', state: 'idle', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer2' }),
      session({ pid: 3, cli_tool: 'gemini', state: 'idle', group_kind: 'standalone' }),
    ])

    expect(indicators).toHaveLength(2)
    expect(indicators[0]).toMatchObject({
      kind: 'team',
      groupId: 'team-a',
      layout: 'rail',
      count: 2,
    })
    expect(indicators[1]).toMatchObject({
      kind: 'session',
      fullName: 'Gemini',
      iconVariant: 'default',
    })
  })

  it('toolIndicators uses a stacked unique-tool group at 4+ team members and keeps standalone sessions visible', () => {
    const indicators = toolIndicators([
      session({ group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'lead' }),
      session({ pid: 2, cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer2', state: 'idle' }),
      session({ pid: 3, cli_tool: 'gemini', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer3', state: 'idle' }),
      session({ pid: 4, cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer4', state: 'active' }),
      session({ pid: 5, cli_tool: 'gemini', state: 'idle' }),
    ])

    expect(indicators).toHaveLength(2)
    expect(indicators[0]).toMatchObject({
      kind: 'team',
      groupId: 'team-a',
      layout: 'stack',
      count: 4,
      isActive: true,
    })
    expect(indicators[0].tools.map(tool => tool.tool)).toEqual(['claude', 'codex', 'gemini'])
    expect(indicators[0].tools.map(tool => tool.iconVariant)).toEqual(['sidebarSmall', 'sidebarSmall', 'sidebarSmall'])
    expect(indicators[0].tools[1].icon).toEqual(getToolIcon('codex', 'sidebarSmall'))
    expect(indicators[1]).toMatchObject({
      kind: 'session',
      fullName: 'Gemini',
    })
  })

  it('toolIndicators derives idle aggregate state when all grouped members are idle', () => {
    const indicators = toolIndicators([
      session({ state: 'idle', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'lead' }),
      session({ state: 'idle', pid: 2, cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer2' }),
      session({ state: 'idle', pid: 3, cli_tool: 'gemini', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer3' }),
      session({ state: 'idle', pid: 4, cli_tool: 'claude', group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer4' }),
    ])

    expect(indicators[0].kind).toBe('team')
    expect(indicators[0].isActive).toBe(false)
    expect(indicators[0].layout).toBe('stack')
    expect(indicators[0].ariaLabel).toContain('idle')
  })

  it('groupedSessionIndicators attaches groupedIcon for multi-tool teams', () => {
    const indicators = groupedSessionIndicators([
      session({ group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', cli_tool: 'claude', member_name: 'lead' }),
      session({ state: 'idle', cli_tool: 'codex', pid: 2, group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer2' }),
    ])

    expect(indicators).toHaveLength(1)
    expect(indicators[0].groupedIcon).toBeTruthy()
    expect(indicators[0].groupedIcon.viewBox).toBe('0 0 36 16')
    expect(indicators[0].groupedIcon.paths).toHaveLength(2)
    expect(indicators[0].groupedIcon.paths[0].d).toBe(TOOL_SIDEBAR_SMALL_ICONS.claude.path)
    expect(indicators[0].groupedIcon.paths[1].d).toBe(TOOL_SIDEBAR_SMALL_ICONS.codex.path)
  })

  it('groupedSessionIndicators attaches 3-tool groupedIcon for all tool types', () => {
    const indicators = groupedSessionIndicators([
      session({ group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', cli_tool: 'claude', member_name: 'lead' }),
      session({ state: 'idle', cli_tool: 'codex', pid: 2, group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer2' }),
      session({ state: 'idle', cli_tool: 'gemini', pid: 3, group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer3' }),
    ])

    expect(indicators).toHaveLength(1)
    expect(indicators[0].groupedIcon).toBeTruthy()
    expect(indicators[0].groupedIcon.viewBox).toBe('0 0 56 16')
    expect(indicators[0].groupedIcon.paths).toHaveLength(3)
  })

  it('groupedSessionIndicators returns null groupedIcon for single-tool teams', () => {
    const indicators = groupedSessionIndicators([
      session({ group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', cli_tool: 'codex', member_name: 'developer1' }),
      session({ state: 'idle', cli_tool: 'codex', pid: 2, group_kind: 'mesh_team', group_id: 'team-a', group_label: 'team-a', member_name: 'developer2' }),
    ])

    expect(indicators).toHaveLength(1)
    expect(indicators[0].groupedIcon).toBeNull()
  })

  it('toolIndicators stacks single-tool teams without duplicating the logo legend', () => {
    const indicators = toolIndicators([
      session({ state: 'idle', cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'codex-team', group_label: 'codex-team', member_name: 'developer1' }),
      session({ state: 'idle', pid: 2, cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'codex-team', group_label: 'codex-team', member_name: 'developer2' }),
      session({ state: 'idle', pid: 3, cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'codex-team', group_label: 'codex-team', member_name: 'developer3' }),
      session({ state: 'idle', pid: 4, cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'codex-team', group_label: 'codex-team', member_name: 'developer4' }),
      session({ state: 'idle', pid: 5, cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'codex-team', group_label: 'codex-team', member_name: 'developer5' }),
      session({ state: 'idle', pid: 6, cli_tool: 'codex', group_kind: 'mesh_team', group_id: 'codex-team', group_label: 'codex-team', member_name: 'developer6' }),
    ])

    expect(indicators).toHaveLength(1)
    expect(indicators[0]).toMatchObject({
      kind: 'team',
      layout: 'stack',
      count: 6,
      tone: 'idle',
    })
    expect(indicators[0].tools.map(tool => tool.tool)).toEqual(['codex'])
  })
})
