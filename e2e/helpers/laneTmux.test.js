import { describe, expect, it } from 'vitest'
import { join } from 'node:path'

import { isolatedTmuxTmpdir, parseProcEnviron, tmuxIsolationProblem } from './laneTmux.js'

const root = '/tmp/taurhaus-e2e-1234-abcd'

describe('isolatedTmuxTmpdir', () => {
  it('is a directory inside the session temp root', () => {
    expect(isolatedTmuxTmpdir(root)).toBe(join(root, 'tmux'))
  })
})

describe('tmuxIsolationProblem', () => {
  // Regression: 5e1d0ae ran the paid lane against the operator's own tmux
  // server, where `set-environment` on the shared `taurhaus` session hands
  // temporary roots to any pane the operator opens next. The lane has to know
  // it is on a server of its own before it creates anything there.
  it('accepts an environment pointed at the socket directory the lane owns', () => {
    expect(tmuxIsolationProblem({ TMUX_TMPDIR: isolatedTmuxTmpdir(root) }, root)).toBe('')
  })

  it('rejects an unset TMUX_TMPDIR', () => {
    expect(tmuxIsolationProblem({}, root)).toMatch(/TMUX_TMPDIR/)
  })

  it('rejects a socket directory outside the session temp root', () => {
    expect(tmuxIsolationProblem({ TMUX_TMPDIR: '/tmp/tmux-1000' }, root)).toMatch(/TMUX_TMPDIR/)
  })

  it('rejects an inherited TMUX, which outranks TMUX_TMPDIR for every client', () => {
    const problem = tmuxIsolationProblem(
      { TMUX_TMPDIR: isolatedTmuxTmpdir(root), TMUX: '/tmp/tmux-1000/default,407334,0' },
      root
    )
    expect(problem).toMatch(/TMUX/)
  })

  it('rejects an empty session temp root rather than calling it isolated', () => {
    expect(tmuxIsolationProblem({ TMUX_TMPDIR: '/tmp/whatever/tmux' }, '')).toMatch(/temp root/)
  })
})

describe('parseProcEnviron', () => {
  it('reads the NUL-separated pairs Linux writes', () => {
    const parsed = parseProcEnviron('TMUX_TMPDIR=/tmp/a\0PATH=/usr/bin\0')
    expect(parsed.TMUX_TMPDIR).toBe('/tmp/a')
    expect(parsed.PATH).toBe('/usr/bin')
  })

  it('keeps a value that contains an equals sign', () => {
    expect(parseProcEnviron('TMUX=/tmp/tmux-1000/default,407334,0\0A=b=c\0').A).toBe('b=c')
  })

  it('is empty for empty input', () => {
    expect(parseProcEnviron('')).toEqual({})
  })
})
