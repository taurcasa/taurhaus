import { describe, expect, it } from 'vitest'

import {
  describeDaemonSetupError,
  describeMeshAvailabilityIssue,
  describeMeshInitFailure,
  describeProjectLoadBanner,
  describeScanDirectoryError,
  describeSessionActionError,
} from './errorCopy.js'

describe('describeSessionActionError', () => {
  it('maps navigation failures to terminal guidance', () => {
    expect(describeSessionActionError('navigate', {}, new Error('pane not found'))).toBe(
      'Could not open that terminal. The session may have already closed.'
    )
  })

  it('maps launch and stop failures to tool-specific copy', () => {
    expect(describeSessionActionError('launch', { tool: 'codex' }, new Error('boom'))).toBe(
      'Could not start Codex. Please try again.'
    )
    expect(describeSessionActionError('stop', { tool: 'claude' }, new Error('boom'))).toBe(
      'Could not stop Claude. Please try again.'
    )
    // Regression: 9a66d1c exposed Gemini-specific failure copy for Google's harness.
    expect(describeSessionActionError('restart', { tool: 'agy' }, new Error('boom'))).toBe(
      'Could not restart Antigravity. Please try again.'
    )
  })
})

describe('describeScanDirectoryError', () => {
  it('rewrites permission and missing-folder failures', () => {
    expect(describeScanDirectoryError(new Error('Permission denied'))).toBe(
      'Taurhaus cannot scan that folder yet. Check that you can open it, then try again.'
    )
    expect(describeScanDirectoryError(new Error('No such file or directory'))).toBe(
      'That folder could not be found. Choose another folder and try again.'
    )
  })
})

describe('describeDaemonSetupError', () => {
  it('rewrites WSL and distro guidance for Windows setup', () => {
    expect(describeDaemonSetupError(new Error('WSL is not installed'), { isWindows: true })).toBe(
      'WSL 2 is not ready yet. Install WSL 2, restart Windows if it asks, then try again.'
    )
    expect(describeDaemonSetupError(new Error('No WSL distro configured'), { isWindows: true })).toBe(
      'WSL is installed, but it does not have a Linux distribution yet. Install Ubuntu or another distro, open it once, then try again.'
    )
  })
})

describe('describeMeshAvailabilityIssue', () => {
  it('rewrites common setup blockers', () => {
    expect(describeMeshAvailabilityIssue('Mesh CLI not found. Install it to enable multi-agent collaboration.')).toBe(
      'Install Mesh to set up a team in this project.'
    )
    expect(describeMeshAvailabilityIssue('tmux is required for multi-agent sessions.')).toBe(
      'Install tmux to launch and manage team sessions.'
    )
  })
})

describe('describeMeshInitFailure', () => {
  it('rewrites common initialization failures', () => {
    expect(describeMeshInitFailure(new Error('team already exists'), { failedStep: 'create_team' })).toBe(
      'A team with this name already exists. Open it or replace it to continue.'
    )
    expect(describeMeshInitFailure(new Error('boom'), { failedStep: 'launch_sessions' })).toBe(
      'Taurhaus could not confirm that every session started correctly. Try again.'
    )
  })
})

describe('describeProjectLoadBanner', () => {
  it('summarizes one or more failed sections in plain language', () => {
    expect(describeProjectLoadBanner([{ section: 'README' }])).toBe('README could not be loaded.')
    expect(describeProjectLoadBanner([{ section: 'Recent commits' }, { section: 'README' }])).toBe(
      'Some project details could not be loaded: Recent commits, README.'
    )
  })
})
