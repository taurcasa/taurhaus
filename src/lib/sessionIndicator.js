/** Session indicator semantics for sidebar rows and HoverCard display. */

import { activityLevel, activitySignal, isActiveLevel, isLiveLevel } from './activitySignal.js'
import { getToolIcon, getToolName } from './toolLogos.js'

const STACKING_THRESHOLD = 4
const TEAM_GROUP_MIN_MEMBERS = 2
const TOOL_ORDER = ['claude', 'codex', 'gemini']

/** Get the display name for a session's CLI tool, defaulting to "Claude". */
function toolName(session) {
  return getToolName(session?.cli_tool)
}

/** Get the SVG icon data for a session's CLI tool. */
export function toolIcon(session, variant = 'default') {
  return getToolIcon(session?.cli_tool || 'claude', variant)
}

/** Return true when a row has a session we still consider present. */
export function hasLiveSession(session) {
  return isLiveLevel(activityLevel(session))
}

export function isStalePresence(session) {
  return session?._presenceStale === true || session?._presenceStatus === 'stale'
}

/** Return true when the tool is actively working (not waiting for input). */
export function isActiveSession(session) {
  return isActiveLevel(activityLevel(session))
}

/** Row-level tint class when any live session exists. */
export function rowTintClass(session) {
  if (!hasLiveSession(session)) return ''
  return isStalePresence(session) ? 'bg-white/[0.015]' : 'bg-white/[0.03]'
}

/** Row-level tint class when any session in the array is live. */
export function rowTintForSessions(sessions) {
  if (!sessions || sessions.length === 0) return ''
  if (!sessions.some(s => hasLiveSession(s))) return ''
  return sessions.some(isStalePresence) ? 'bg-white/[0.015]' : 'bg-white/[0.03]'
}

function groupMetadata(session) {
  if (session?.group_kind !== 'mesh_team' || !session?.group_id) return null
  return {
    groupId: session.group_id,
    groupLabel: session.group_label || session.group_id,
  }
}

/** Rank used to order grouped members: working first, then idle, then gone. */
function memberRank(member) {
  const level = activityLevel(member)
  if (isActiveLevel(level)) return 2
  if (level === 'offline') return 0
  return 1
}

function compareGroupedMembers(left, right) {
  const rankDelta = memberRank(right) - memberRank(left)
  if (rankDelta !== 0) return rankDelta
  return toolName(left).localeCompare(toolName(right))
}

const LEVEL_COLOR_CLASS = {
  working: 'text-success-300',
  active: 'text-success-300',
  idle: 'text-warning-300',
  uncertain: 'text-info-300',
  offline: 'text-zinc-400',
}

const LEVEL_TONE_CLASS = {
  working: 'session-pill-active',
  active: 'session-pill-active',
  idle: 'session-pill-idle',
  uncertain: 'session-pill-stale',
  offline: 'session-pill-idle',
}

/**
 * The signal a retained record last reported, used for wording only: a daemon
 * gap makes the reading uncertain, it does not mean the session went idle.
 */
function reportedSignal(session, signal) {
  if (signal.source !== 'stale' && signal.source !== 'degraded') return signal
  return activitySignal({ ...session, _presenceStale: false, _presenceStatus: 'live', degraded: false })
}

function sessionColorClass(session) {
  return LEVEL_COLOR_CLASS[activityLevel(session)]
}

function sessionToneClass(session) {
  return LEVEL_TONE_CLASS[activityLevel(session)]
}

function teamToolVisual(session, variant = 'default') {
  const tool = session?.cli_tool || 'claude'
  return {
    tool,
    fullName: getToolName(tool),
    icon: getToolIcon(tool, variant),
    iconVariant: variant,
    isActive: isActiveSession(session),
    colorClass: sessionColorClass(session),
    toneClass: sessionToneClass(session),
  }
}

function stackedTeamTools(members, variant = 'default') {
  const liveMembers = Array.isArray(members) ? members : []
  const orderedTools = uniqueTools(liveMembers, variant)
  return orderedTools.map(toolEntry => {
    const matchingMembers = liveMembers.filter(member => (member?.cli_tool || 'claude') === toolEntry.tool)
    const representative = matchingMembers.find(isActiveSession) || matchingMembers[0]
    return {
      ...toolEntry,
      isActive: isActiveSession(representative),
      colorClass: sessionColorClass(representative),
      toneClass: sessionToneClass(representative),
    }
  })
}

export function uniqueTools(sessions, variant = 'default') {
  if (!Array.isArray(sessions) || sessions.length === 0) return []

  const seen = new Set()
  const ordered = []
  for (const tool of TOOL_ORDER) {
    if (sessions.some(session => (session?.cli_tool || 'claude') === tool)) {
      seen.add(tool)
      ordered.push(tool)
    }
  }

  for (const session of sessions) {
    const tool = session?.cli_tool || 'claude'
    if (seen.has(tool)) continue
    seen.add(tool)
    ordered.push(tool)
  }

  return ordered.map(tool => ({
    tool,
    fullName: getToolName(tool),
    icon: getToolIcon(tool, variant),
    iconVariant: variant,
  }))
}

function buildTeamIndicator(group) {
  const members = [...group.members].sort(compareGroupedMembers)
  const isActive = members.some(isActiveSession)
  const isStale = members.some(isStalePresence)
  const count = members.length
  const activityLabel = isStale ? 'retained stale' : (isActive ? 'active' : 'idle')
  const layout = count >= STACKING_THRESHOLD ? 'stack' : 'rail'
  const tools = stackedTeamTools(members, 'default').slice(0, 3)

  return {
    kind: 'team',
    layout,
    groupId: group.groupId,
    groupLabel: group.groupLabel,
    count,
    members,
    tools,
    memberTools: members.map(member => teamToolVisual(member, 'default')),
    isActive,
    interactive: false,
    tone: isStale ? 'stale' : (isActive ? 'active' : 'idle'),
    colorClass: isStale ? 'text-info-300' : (isActive ? 'text-success-300' : 'text-warning-300'),
    ariaLabel: `${group.groupLabel}: ${count} team sessions ${activityLabel}`,
  }
}

export function groupedSessionIndicators(sessions) {
  const liveSessions = Array.isArray(sessions) ? sessions.filter(s => hasLiveSession(s)) : []
  if (liveSessions.length === 0) return []

  const grouped = new Map()
  for (const session of liveSessions) {
    const metadata = groupMetadata(session)
    if (!metadata) continue
    const existing = grouped.get(metadata.groupId)
    if (existing) {
      existing.members.push(session)
      continue
    }
    grouped.set(metadata.groupId, {
      groupId: metadata.groupId,
      groupLabel: metadata.groupLabel,
      members: [session],
    })
  }

  return [...grouped.values()]
    .filter(group => group.members.length >= TEAM_GROUP_MIN_MEMBERS)
    .sort((left, right) => {
      const leftActive = left.members.some(isActiveSession) ? 1 : 0
      const rightActive = right.members.some(isActiveSession) ? 1 : 0
      if (rightActive !== leftActive) return rightActive - leftActive
      if (right.members.length !== left.members.length) return right.members.length - left.members.length
      return left.groupLabel.localeCompare(right.groupLabel)
    })
    .map(buildTeamIndicator)
}

/**
 * Visual config for the sidebar session badge (single session).
 * - idle is intentionally more explicit because it requires user action.
 */
export function sessionBadge(session) {
  const signal = activitySignal(session)
  const interactive = Boolean(session?.tmux_session && session?.tmux_window && session?.tmux_pane)

  if (signal.level === 'uncertain') {
    const retained = signal.source === 'stale' || signal.source === 'degraded'
    return {
      visible: true,
      signal,
      label: retained ? 'STALE' : 'IDLE',
      toolLabel: toolName(session),
      badgeClass: 'session-pill-stale rounded-[4px] bg-info-300/14 text-info-300 border border-info-300/45',
      ariaLabel: retained
        ? `${toolName(session)} session presence retained stale during daemon gap`
        : `${toolName(session)} session active but unattributed`,
      interactive,
    }
  }

  if (signal.level === 'idle') {
    return {
      visible: true,
      signal,
      label: 'IDLE',
      toolLabel: toolName(session),
      badgeClass: 'session-pill-idle rounded-[4px] bg-warning-300/18 text-warning-300 border border-warning-300/65',
      ariaLabel: `${toolName(session)} session idle, waiting for input`,
      interactive,
    }
  }

  if (isActiveLevel(signal.level)) {
    return {
      visible: true,
      signal,
      label: 'RUN',
      toolLabel: toolName(session),
      badgeClass: 'session-pill-active rounded-full bg-success-300/18 text-success-300 border border-success-300/55',
      ariaLabel: `${toolName(session)} session active`,
      interactive,
    }
  }

  return {
    visible: false,
    signal,
    label: '',
    toolLabel: '',
    badgeClass: '',
    ariaLabel: 'No active session',
    interactive: false,
  }
}

/**
 * Compact tool indicator for multi-session sidebar display.
 * Returns an array of tool indicators, one per live session.
 * Each indicator includes SVG icon data (monochrome, uses currentColor),
 * state-based styling, and session reference for click-to-jump.
 */
export function toolIndicators(sessions) {
  if (!sessions || sessions.length === 0) return []

  const liveSessions = sessions.filter(s => hasLiveSession(s))
  const teamIndicators = groupedSessionIndicators(liveSessions)
  if (teamIndicators.length === 0) {
    return liveSessions.map(singleSessionIndicator)
  }

  const groupedSessionKeys = new Set(
    teamIndicators.flatMap(indicator => indicator.members.map(sessionKey))
  )
  const standaloneIndicators = liveSessions
    .filter(session => !groupedSessionKeys.has(sessionKey(session)))
    .map(singleSessionIndicator)

  return [...teamIndicators, ...standaloneIndicators]
}

function sessionKey(session) {
  const paneKey = session?.tmux_pane || ''
  const sessionId = session?.session_id || ''
  return `${session?.pid || 0}:${session?.cli_tool || 'claude'}:${paneKey}:${sessionId}:${session?.member_name || ''}`
}

function singleSessionIndicator(session) {
  const tool = session.cli_tool || 'claude'
  const name = getToolName(tool)
  const icon = getToolIcon(tool)
  const signal = activitySignal(session)
  const reported = reportedSignal(session, signal)
  const isActive = isActiveLevel(reported.level)
  const isUnattributed = reported.source === 'project'
  const interactive = Boolean(session.tmux_session && session.tmux_window && session.tmux_pane)
  const isStale = isStalePresence(session)
  const statusLabel = isActive
    ? 'running'
    : (isUnattributed ? 'project active (unattributed)' : 'idle')

  return {
    kind: 'session',
    session,
    signal,
    tool,
    label: name[0],
    fullName: name,
    icon,
    iconVariant: 'default',
    isActive,
    isUnattributed,
    interactive,
    colorClass: sessionColorClass(session),
    toneClass: sessionToneClass(session),
    ariaLabel: `${name}: ${isStale ? 'retained stale ' : ''}${statusLabel}`,
  }
}
