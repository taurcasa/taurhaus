/** Session indicator semantics for sidebar rows and HoverCard display. */

import { getToolIcon, getToolName } from './toolLogos.js'

const STACKING_THRESHOLD = 4
const GROUPING_THRESHOLD = 4
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

/** Return true when a row has an active or idle session. */
export function hasLiveSession(session) {
  return session?.state === 'active' || session?.state === 'idle'
}

/** Return true when the tool is actively working (not waiting for input). */
export function isActiveSession(session) {
  return session?.state === 'active'
}

/** Row-level tint class when any live session exists. */
export function rowTintClass(session) {
  return hasLiveSession(session) ? 'bg-white/[0.03]' : ''
}

/** Row-level tint class when any session in the array is live. */
export function rowTintForSessions(sessions) {
  if (!sessions || sessions.length === 0) return ''
  return sessions.some(s => hasLiveSession(s)) ? 'bg-white/[0.03]' : ''
}

function groupMetadata(session) {
  if (session?.group_kind !== 'mesh_team' || !session?.group_id) return null
  return {
    groupId: session.group_id,
    groupLabel: session.group_label || session.group_id,
  }
}

function compareGroupedMembers(left, right) {
  const leftRank = left?.state === 'active' ? 2 : left?.state === 'idle' ? 1 : 0
  const rightRank = right?.state === 'active' ? 2 : right?.state === 'idle' ? 1 : 0
  if (rightRank !== leftRank) return rightRank - leftRank
  return toolName(left).localeCompare(toolName(right))
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
  const isActive = members.some(member => member.state === 'active')
  const count = members.length
  const activityLabel = isActive ? 'active' : 'idle'
  const layout = count >= STACKING_THRESHOLD ? 'stack' : 'rail'
  const tools = uniqueTools(members, 'sidebarSmall').slice(0, 3)

  return {
    kind: 'team',
    layout,
    groupId: group.groupId,
    groupLabel: group.groupLabel,
    count,
    members,
    tools,
    memberTools: members.map(member => ({
      tool: member?.cli_tool || 'claude',
      fullName: getToolName(member?.cli_tool || 'claude'),
      icon: getToolIcon(member?.cli_tool || 'claude', 'sidebarSmall'),
      iconVariant: 'sidebarSmall',
    })),
    isActive,
    interactive: false,
    tone: isActive ? 'active' : 'idle',
    colorClass: isActive ? 'text-success-300' : 'text-warning-300',
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
      const leftActive = left.members.some(member => member.state === 'active') ? 1 : 0
      const rightActive = right.members.some(member => member.state === 'active') ? 1 : 0
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
  if (session?.state === 'idle') {
    return {
      visible: true,
      label: 'IDLE',
      toolLabel: toolName(session),
      badgeClass: 'session-pill-idle rounded-[4px] bg-warning-300/18 text-warning-300 border border-warning-300/65',
      ariaLabel: `${toolName(session)} session idle, waiting for input`,
      interactive: Boolean(session.tmux_session && session.tmux_window && session.tmux_pane),
    }
  }

  if (session?.state === 'active') {
    return {
      visible: true,
      label: 'RUN',
      toolLabel: toolName(session),
      badgeClass: 'session-pill-active rounded-full bg-success-300/18 text-success-300 border border-success-300/55',
      ariaLabel: `${toolName(session)} session active`,
      interactive: Boolean(session.tmux_session && session.tmux_window && session.tmux_pane),
    }
  }

  return {
    visible: false,
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
  if (liveSessions.length < GROUPING_THRESHOLD) {
    return liveSessions.map(singleSessionIndicator)
  }

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
  const isActive = session.state === 'active'
  const isUnattributed = !isActive && session.project_unattributed_active === true
  const interactive = Boolean(session.tmux_session && session.tmux_window && session.tmux_pane)
  const statusLabel = isActive
    ? 'running'
    : (isUnattributed ? 'project active (unattributed)' : 'idle')

  return {
    kind: 'session',
    session,
    tool,
    label: name[0],
    fullName: name,
    icon,
    iconVariant: 'default',
    isActive,
    isUnattributed,
    interactive,
    colorClass: isActive
      ? 'text-success-300'
      : (isUnattributed ? 'text-info-300' : 'text-warning-300'),
    ariaLabel: `${name}: ${statusLabel}`,
  }
}
