/** Session indicator semantics for sidebar rows and HoverCard display. */

import { TOOL_ICONS, TOOL_NAMES, getToolIcon, getToolName } from './toolLogos.js'

/** Get the display name for a session's CLI tool, defaulting to "Claude". */
function toolName(session) {
  return getToolName(session?.cli_tool)
}

/** Get the SVG icon data for a session's CLI tool. */
export function toolIcon(session) {
  return getToolIcon(session?.cli_tool || 'claude')
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

  return sessions
    .filter(s => hasLiveSession(s))
    .map(session => {
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
        session,
        tool,
        label: name[0],
        fullName: name,
        icon,
        isActive,
        isUnattributed,
        interactive,
        colorClass: isActive
          ? 'text-success-300'
          : (isUnattributed ? 'text-info-300' : 'text-warning-300'),
        ariaLabel: `${name}: ${statusLabel}`,
      }
    })
}
