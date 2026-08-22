import { describe, expect, it } from 'vitest'

import { normalizeLiveTeamStatus, normalizeProjectMeshSnapshot } from './coordinationResponses.js'

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
