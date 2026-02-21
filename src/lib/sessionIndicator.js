/** Session indicator semantics for sidebar rows. */

/** Return true when a row has an active or idle Claude session. */
export function hasLiveSession(session) {
  return session?.state === 'active' || session?.state === 'idle'
}

/** Return true when Claude is actively working (not waiting for input). */
export function isActiveSession(session) {
  return session?.state === 'active'
}

/** Row-level tint class when any live session exists. */
export function rowTintClass(session) {
  return hasLiveSession(session) ? 'bg-white/[0.03]' : ''
}

/** Human-readable tooltip for hover information on the session badge. */
export function sessionTooltip(session) {
  if (!hasLiveSession(session)) return 'No Claude session'

  const lines = [
    `Claude session: ${session.state === 'idle' ? 'IDLE (waiting for input)' : 'RUNNING (Claude working)'}`,
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
      badgeClass: 'session-pill-idle rounded-[4px] bg-warning-300/18 text-warning-300 border border-warning-300/65',
      ariaLabel: 'Claude session idle, waiting for input',
      interactive: Boolean(session.tmux_session && session.tmux_window && session.tmux_pane),
    }
  }

  if (session?.state === 'active') {
    return {
      visible: true,
      label: 'RUN',
      badgeClass: 'session-pill-active rounded-full bg-success-300/18 text-success-300 border border-success-300/55',
      ariaLabel: 'Claude session active',
      interactive: Boolean(session.tmux_session && session.tmux_window && session.tmux_pane),
    }
  }

  return {
    visible: false,
    label: '',
    badgeClass: '',
    ariaLabel: 'No Claude session',
    interactive: false,
  }
}
