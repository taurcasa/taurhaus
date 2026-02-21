import { describe, it, expect } from 'vitest'
import { hasLiveSession, isActiveSession, rowTintClass, rowTintForSessions, sessionBadge, sessionTooltip, sidebarHoverInfo, toolIndicators } from './sessionIndicator.js'

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

  it('builds detailed tooltip with available fields', () => {
    const text = sessionTooltip({
      state: 'idle',
      cli_tool: 'claude',
      session_id: 'abc-123',
      tmux_session: 'main',
      tmux_window: '1',
      tmux_pane: '%3',
      pid: 9012,
    })

    expect(text).toContain('Claude session')
    expect(text).toContain('IDLE (waiting for input)')
    expect(text).toContain('Session ID: abc-123')
    expect(text).toContain('tmux: main:1 %3')
    expect(text).toContain('PID: 9012')
  })

  it('tooltip shows correct tool name for non-Claude tools', () => {
    const codexText = sessionTooltip({ state: 'active', cli_tool: 'codex' })
    expect(codexText).toContain('Codex session')
    expect(codexText).toContain('RUNNING (Codex working)')

    const geminiText = sessionTooltip({ state: 'idle', cli_tool: 'gemini' })
    expect(geminiText).toContain('Gemini session')
  })

  it('builds row hover info with project, git, and session summary', () => {
    const text = sidebarHoverInfo(
      { name: 'taurhaus', activity_state: 'recent', branch: 'main', is_dirty: true },
      { state: 'idle', cli_tool: 'claude', session_id: 's-1' },
    )

    expect(text).toContain('Project: taurhaus')
    expect(text).toContain('Git: RECENT')
    expect(text).toContain('Branch: main')
    expect(text).toContain('Working tree: dirty')
    expect(text).toContain('IDLE (waiting for input)')
    expect(text).toContain('Session ID: s-1')
  })

  // --- Multi-session support ---

  it('rowTintForSessions returns tint when any session is live', () => {
    expect(rowTintForSessions([{ state: 'active' }])).toBe('bg-white/[0.03]')
    expect(rowTintForSessions([{ state: 'idle' }])).toBe('bg-white/[0.03]')
    expect(rowTintForSessions([])).toBe('')
    expect(rowTintForSessions(null)).toBe('')
  })

  it('toolIndicators returns one indicator per live session', () => {
    const sessions = [
      { state: 'active', cli_tool: 'claude', tmux_session: '0', tmux_window: '1', tmux_pane: '%3' },
      { state: 'idle', cli_tool: 'codex', tmux_session: '0', tmux_window: '1', tmux_pane: '%4' },
    ]
    const indicators = toolIndicators(sessions)
    expect(indicators).toHaveLength(2)
    expect(indicators[0].label).toBe('C')
    expect(indicators[0].fullName).toBe('Claude')
    expect(indicators[0].isActive).toBe(true)
    expect(indicators[0].interactive).toBe(true)
    expect(indicators[1].label).toBe('X')
    expect(indicators[1].fullName).toBe('Codex')
    expect(indicators[1].isActive).toBe(false)
    expect(indicators[1].interactive).toBe(true)
  })

  it('toolIndicators returns empty array for no sessions', () => {
    expect(toolIndicators([])).toEqual([])
    expect(toolIndicators(null)).toEqual([])
  })

  it('toolIndicators for all three tools', () => {
    const sessions = [
      { state: 'active', cli_tool: 'claude' },
      { state: 'active', cli_tool: 'codex' },
      { state: 'idle', cli_tool: 'gemini' },
    ]
    const indicators = toolIndicators(sessions)
    expect(indicators).toHaveLength(3)
    expect(indicators.map(i => i.label)).toEqual(['C', 'X', 'G'])
    expect(indicators.map(i => i.fullName)).toEqual(['Claude', 'Codex', 'Gemini'])
  })

  it('toolIndicators carries session reference', () => {
    const session = { state: 'active', cli_tool: 'claude', pid: 12345 }
    const indicators = toolIndicators([session])
    expect(indicators[0].session).toBe(session)
  })
})
