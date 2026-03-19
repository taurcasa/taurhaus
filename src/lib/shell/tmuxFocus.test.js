import { describe, expect, it } from 'vitest'
import {
  focusPayloadField,
  hasAttachedTmuxFocus,
  resolveProjectIdFromSession,
  resolveProjectIdFromTmuxFocusPayload,
} from './tmuxFocus.js'

describe('tmuxFocus', () => {
  it('trims focus payload fields and accepts snake or camel case keys', () => {
    expect(focusPayloadField({ tmuxSession: '  taurhaus  ' }, 'session', 'tmuxSession')).toBe('taurhaus')
    expect(focusPayloadField({ session: '  4 ' }, 'session', 'tmuxSession')).toBe('4')
    expect(focusPayloadField({ session: '   ' }, 'session', 'tmuxSession')).toBeNull()
  })

  it('resolves a project directly from session project id', () => {
    expect(resolveProjectIdFromSession({ project_id: 'project-a' }, [])).toBe('project-a')
  })

  it('resolves a project from a normalized session path match', () => {
    const projects = [
      { id: 'project-a', path: '\\\\wsl$\\Ubuntu\\home\\user\\proj-a' },
      { id: 'project-b', path: '/home/user/proj-b' },
    ]

    expect(resolveProjectIdFromSession({
      projectPath: '\\\\wsl.localhost\\Ubuntu\\home\\user\\proj-a',
    }, projects)).toBe('project-a')
  })

  it('reports attached focus only when session and window are both present', () => {
    expect(hasAttachedTmuxFocus({ session: 'taurhaus', window: '2' })).toBe(true)
    expect(hasAttachedTmuxFocus({ session: 'taurhaus' })).toBe(false)
    expect(hasAttachedTmuxFocus(null)).toBe(false)
  })

  it('resolves a tmux focus payload directly from project id', () => {
    expect(resolveProjectIdFromTmuxFocusPayload({
      projectId: 'project-a',
    })).toBe('project-a')
  })

  it('matches tmux focus through live sessions using window index', () => {
    const projects = [{ id: 'project-a', path: '/home/user/proj-a' }]
    const liveSessions = [{
      tmux_session: 'taurhaus',
      tmux_window: '2',
      project_path: '/home/user/proj-a',
    }]

    expect(resolveProjectIdFromTmuxFocusPayload({
      session: 'taurhaus',
      window: '2',
    }, { projects, liveSessions })).toBe('project-a')
  })

  it('matches tmux focus through live sessions using window name', () => {
    const projects = [{ id: 'project-b', path: '/home/user/proj-b' }]
    const liveSessions = [{
      tmuxSession: 'taurhaus',
      tmuxWindow: '7',
      tmuxWindowName: 'proj-b',
      projectPath: '/home/user/proj-b',
    }]

    expect(resolveProjectIdFromTmuxFocusPayload({
      tmuxSession: 'taurhaus',
      tmuxWindow: 'proj-b',
    }, { projects, liveSessions })).toBe('project-b')
  })

  it('returns null when focus payload cannot be mapped to a project', () => {
    expect(resolveProjectIdFromTmuxFocusPayload({
      session: 'taurhaus',
      window: '2',
    }, {
      projects: [{ id: 'project-a', path: '/home/user/proj-a' }],
      liveSessions: [],
    })).toBeNull()
  })
})
