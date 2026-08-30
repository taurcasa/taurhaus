import { describe, expect, it } from 'vitest'

import {
  normalizeDeliveryResult,
  normalizeLiveTeamStatus,
  normalizeProjectMeshSnapshot,
} from './coordinationResponses.js'

describe('coordinationResponses delivery outcomes', () => {
  it('normalizes the additive delivery facts without dropping future fields', () => {
    const result = normalizeDeliveryResult({
      delivered: true,
      method: 'inbox_file',
      durable: true,
      wake: {
        status: 'failed',
        reason: 'daemon spawn failed',
        futureWakeFact: 'preserved',
      },
      post_write_warnings: ['runtime state was not persisted'],
      futureDeliveryFact: { preserved: true },
    })

    expect(result).toEqual({
      delivered: true,
      method: 'inbox_file',
      durable: true,
      wake: {
        status: 'failed',
        reason: 'daemon spawn failed',
        futureWakeFact: 'preserved',
      },
      postWriteWarnings: ['runtime state was not persisted'],
      futureDeliveryFact: { preserved: true },
    })
  })
})

describe('coordinationResponses agent snapshots', () => {
  // Regression: FastAgentSnapshot had no model field at all, so the runtime view
  // could not show which model an agent was actually running (plan Finding 3).
  it('carries model and reasoning effort in both spellings', () => {
    const status = normalizeLiveTeamStatus({
      teamName: 'taurhaus-team',
      leadName: 'team-lead',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          model: 'opus',
          reasoningEffort: 'high',
          projectId: '/projects/taurhaus',
        },
        {
          name: 'dev-1',
          role: 'member',
          cli_tool: 'codex',
          model: 'gpt-5.6-terra',
          reasoning_effort: 'xhigh',
          project_id: '/projects/taurhaus',
        },
      ],
    })

    expect(status.members.map((member) => [member.model, member.reasoningEffort])).toEqual([
      ['opus', 'high'],
      ['gpt-5.6-terra', 'xhigh'],
    ])
  })

  it('omits the effort when the backend does not report one', () => {
    const status = normalizeLiveTeamStatus({
      teamName: 'taurhaus-team',
      members: [{ name: 'dev-1', role: 'member', cliTool: 'codex', model: 'gpt-5.6-terra' }],
    })

    expect(status.members[0]).not.toHaveProperty('reasoningEffort')
  })

  it('keeps model and effort on the project mesh snapshot members', () => {
    const snapshot = normalizeProjectMeshSnapshot({
      team_name: 'taurhaus-team',
      team_status: {
        lead_name: 'team-lead',
        members: [
          {
            name: 'dev-1',
            role: 'member',
            cli_tool: 'codex',
            model: 'gpt-5.6-terra',
            reasoning_effort: 'high',
          },
        ],
      },
    })

    expect(snapshot.teamStatus.members[0]).toEqual(
      expect.objectContaining({ model: 'gpt-5.6-terra', reasoningEffort: 'high' })
    )
  })
})

describe('coordinationResponses session identity', () => {
  // Regression: 9e15e4e gave mesh nodes a run tree keyed on the member's Claude
  // session, but the live-team normalizer dropped `session_id`, so a real
  // runtime node never had one and never loaded a run.
  it('carries the runtime session id in both spellings', () => {
    const status = normalizeLiveTeamStatus({
      teamName: 'taurhaus-team',
      leadName: 'team-lead',
      members: [
        { name: 'team-lead', role: 'lead', cliTool: 'claude', sessionId: 'sess-lead' },
        { name: 'dev-1', role: 'member', cli_tool: 'codex', session_id: 'sess-dev' },
      ],
    })

    expect(status.members.map((member) => member.sessionId)).toEqual(['sess-lead', 'sess-dev'])
  })

  // Regression: d442cf6 carried the session but not the workflow hint, so a
  // member running a headless workflow stayed Idle on the canvas.
  it('carries the workflow activity hint in both spellings', () => {
    const status = normalizeLiveTeamStatus({
      teamName: 'taurhaus-team',
      leadName: 'team-lead',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          workflowActivity: { live_runs: 1, last_write_at: 1_800_000_000_000 },
        },
        {
          name: 'dev-1',
          role: 'member',
          cli_tool: 'claude',
          workflow_activity: { live_runs: 2, last_write_at: 1_800_000_000_500 },
        },
        { name: 'dev-2', role: 'member', cli_tool: 'codex' },
      ],
    })

    expect(status.members.map((member) => member.workflowActivity)).toEqual([
      { live_runs: 1, last_write_at: 1_800_000_000_000 },
      { live_runs: 2, last_write_at: 1_800_000_000_500 },
      null,
    ])
  })

  it('reports no session for a member that is not attached', () => {
    const status = normalizeLiveTeamStatus({
      teamName: 'taurhaus-team',
      members: [{ name: 'dev-1', role: 'member', cliTool: 'codex' }],
    })

    expect(status.members[0].sessionId).toBeNull()
  })

  it('keeps the session id on the project mesh snapshot members', () => {
    const snapshot = normalizeProjectMeshSnapshot({
      team_name: 'taurhaus-team',
      team_status: {
        lead_name: 'team-lead',
        members: [{ name: 'dev-1', role: 'member', cli_tool: 'codex', session_id: 'sess-dev' }],
      },
    })

    expect(snapshot.teamStatus.members[0].sessionId).toBe('sess-dev')
  })
})

describe('coordinationResponses task effort', () => {
  // Regression: f0facb6 added assignment effort to LiveAgentStatus, but the
  // fixed-list member normalizer dropped both fields before the canvas saw it.
  it('carries assignment effort and its reason in both wire spellings', () => {
    const status = normalizeLiveTeamStatus({
      teamName: 'taurhaus-team',
      members: [
        {
          name: 'dev-1',
          role: 'member',
          taskEffort: 'high',
          taskEffortWhy: 'the migration is irreversible',
        },
        {
          name: 'dev-2',
          role: 'member',
          task_effort: 'medium',
          task_effort_why: 'routine lane work',
        },
      ],
    })

    expect(status.members.map(({ taskEffort, taskEffortWhy }) => [taskEffort, taskEffortWhy]))
      .toEqual([
        ['high', 'the migration is irreversible'],
        ['medium', 'routine lane work'],
      ])
  })
})
