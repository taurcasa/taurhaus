import {
  MOCK_CLAUDE_SESSIONS,
  MOCK_SESSION,
  MOCK_SESSIONS,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'

function normalizeSessionSummary(raw) {
  const session = raw && typeof raw === 'object' ? raw : {}
  return {
    ...session,
    project_id: session.project_id ?? session.projectId ?? '',
  }
}

function normalizeSessionDetail(raw) {
  if (!raw || typeof raw !== 'object') return raw
  return {
    ...raw,
    project_id: raw.project_id ?? raw.projectId ?? '',
    next_steps: Array.isArray(raw.next_steps)
      ? raw.next_steps
      : Array.isArray(raw.nextSteps)
        ? raw.nextSteps
        : [],
    open_questions: Array.isArray(raw.open_questions)
      ? raw.open_questions
      : Array.isArray(raw.openQuestions)
        ? raw.openQuestions
        : [],
    file_path: raw.file_path ?? raw.filePath ?? '',
    created_at: raw.created_at ?? raw.createdAt ?? null,
  }
}

export function getLatestSession(projectId) {
  return invokeOrMock('get_latest_session', { projectId }, () => {
    if (!projectId || projectId === 'missing-project') {
      return null
    }
    return MOCK_SESSION
  }).then(normalizeSessionDetail)
}

export function listSessions(projectId, limit = 20, offset = 0) {
  return invokeOrMock('list_sessions', { projectId, limit, offset }, () => MOCK_SESSIONS).then(
    (sessions) => (Array.isArray(sessions) ? sessions.map(normalizeSessionSummary) : [])
  )
}

export function listClaudeSessions() {
  return invokeOrMock('list_cli_sessions', undefined, () => MOCK_CLAUDE_SESSIONS)
}

/**
 * The same sessions, plus how the backend obtained them:
 * `fresh` | `degraded` | `cached` | `unavailable`.
 *
 * The store polls this rather than the bare list because it measures session
 * time against the interval between two observations — a replayed or cached
 * list is not one.
 */
export function listCliSessionSnapshot() {
  return invokeOrMock('list_cli_session_snapshot', undefined, () => ({
    sessions: MOCK_CLAUDE_SESSIONS,
    freshness: 'fresh',
  }))
}

/**
 * Which Claude subscription a launch would run on, before it runs.
 *
 * `needsChoice` is the only question the chooser exists for. The frontend
 * cannot answer it alone: a resume runs in the config dir that owns the
 * project's history, and only the backend can see which one that is.
 */
export function resolveLaunchAccount(projectId, tool, mode, sessionId = null) {
  return invokeOrMock('resolve_launch_account', { projectId, tool, mode, sessionId }, () => ({
    accountId: null,
    email: null,
    source: 'default_config_dir',
    needsChoice: false,
  }))
}

export function launchCliSession(projectId, mode, cliTool, accountId = null) {
  return invokeOrMock(
    'launch_cli_session',
    {
      projectId,
      mode,
      cliTool: cliTool ?? null,
      accountId: accountId ?? null,
    },
    () => ({
      tmux_session: 'taurhaus',
      tmux_window: 'project',
      tmux_pane: '%99',
    })
  )
}

export function stopClaudeSession(tmuxPane, cliTool) {
  return invokeOrMock('stop_cli_session', { tmuxPane, cliTool: cliTool ?? null }, () => undefined)
}

export function navigateToSession(tmuxSession, tmuxWindow, tmuxPane, openTerminal = false) {
  return invokeOrMock('navigate_to_session', { tmuxSession, tmuxWindow, tmuxPane, openTerminal }, () => undefined)
}

export function getForegroundProject() {
  return invokeOrMock('get_foreground_project', undefined, () => null)
}

export function recordSessionActivity(projectId, cliTool, startedAt, endedAt, activeDurationMs, totalDurationMs) {
  return invokeOrMock('record_session_activity', { projectId, cliTool, startedAt, endedAt, activeDurationMs, totalDurationMs }, () => undefined)
}
