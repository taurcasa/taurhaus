/** Session indicator semantics for sidebar rows. */

/** Display names for each CLI tool. */
const TOOL_NAMES = {
  claude: 'Claude',
  codex: 'Codex',
  gemini: 'Gemini',
}

/** Get the display name for a session's CLI tool, defaulting to "Claude". */
function toolName(session) {
  return TOOL_NAMES[session?.cli_tool] || 'Claude'
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

/** Human-readable tooltip for hover information on the session badge. */
export function sessionTooltip(session) {
  if (!hasLiveSession(session)) return 'No active session'

  const name = toolName(session)
  const lines = [
    `${name} session: ${session.state === 'idle' ? 'IDLE (waiting for input)' : `RUNNING (${name} working)`}`,
  ]

  if (session.session_id) lines.push(`Session ID: ${session.session_id}`)
  if (session.tmux_session && session.tmux_window && session.tmux_pane) {
    lines.push(`tmux: ${session.tmux_session}:${session.tmux_window} ${session.tmux_pane}`)
  }
  if (session.pid) lines.push(`PID: ${session.pid}`)

  return lines.join('\n')
}

/** Full sidebar row hover text (project + git + session). */
export function sidebarHoverInfo(project, session) {
  const lines = [
    project?.name ? `Project: ${project.name}` : 'Project',
    `Git: ${String(project?.activity_state ?? 'unknown').toUpperCase()}`,
    `Branch: ${project?.branch || '(none)'}`,
  ]

  if (project?.is_dirty) lines.push('Working tree: dirty')

  lines.push(sessionTooltip(session))
  return lines.join('\n')
}

/**
 * Visual config for the sidebar session badge.
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
