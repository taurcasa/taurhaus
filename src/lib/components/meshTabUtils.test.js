import { describe, expect, it } from 'vitest'

import {
  buildTeamConfigFromRuntimeStatus,
  deriveCrossProjectMeta,
} from './meshTabUtils.js'

describe('meshTabUtils cross-project metadata', () => {
  it('derives cross-project metadata from explicit camelCase fields', () => {
    expect(
      deriveCrossProjectMeta(
        {
          projectId: '/home/user/projects/mesh',
          isCrossProject: true,
          projectLabel: 'mesh',
        },
        '/home/user/projects/taurhaus'
      )
    ).toEqual({
      isCrossProject: true,
      projectLabel: 'mesh',
    })
  })

  it('derives cross-project metadata from snake_case backend fields', () => {
    expect(
      deriveCrossProjectMeta(
        {
          project_id: '/home/user/projects/mesh',
          is_cross_project: true,
          project_label: 'mesh',
        },
        '/home/user/projects/taurhaus'
      )
    ).toEqual({
      isCrossProject: true,
      projectLabel: 'mesh',
    })
  })

  it('falls back to normalized project-path comparison when explicit fields are missing', () => {
    expect(
      deriveCrossProjectMeta(
        {
          projectId: 'C:\\Users\\me\\code\\mesh',
        },
        '/mnt/c/Users/me/code/taurhaus'
      )
    ).toEqual({
      isCrossProject: true,
      projectLabel: 'mesh',
    })
  })

  it('keeps local members local when normalized project paths match', () => {
    expect(
      deriveCrossProjectMeta(
        {
          projectId: '\\\\wsl.localhost\\Ubuntu\\home\\user\\projects\\taurhaus',
        },
        '/home/user/projects/taurhaus'
      )
    ).toEqual({
      isCrossProject: false,
      projectLabel: '',
    })
  })

  it('treats case-variant Windows paths as the same project', () => {
    expect(
      deriveCrossProjectMeta(
        {
          projectId: 'c:\\users\\me\\code\\taurhaus',
        },
        'C:\\Users\\Me\\Code\\Taurhaus'
      )
    ).toEqual({
      isCrossProject: false,
      projectLabel: '',
    })
  })

  it('normalizes runtime status members with cross-project fields for the mesh canvas', () => {
    const config = buildTeamConfigFromRuntimeStatus({
      leadName: 'team-lead',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: 'claude',
          model: 'opus',
          projectId: '/home/user/projects/taurhaus',
          sessionStatus: 'active',
          isCrossProject: false,
          projectLabel: '',
        },
        {
          name: 'mesh-expert',
          role: 'member',
          cliTool: 'gemini',
          model: '2.5-pro',
          projectId: '/home/user/projects/mesh',
          sessionStatus: 'active',
          isCrossProject: true,
          projectLabel: 'mesh',
        },
      ],
    })

    expect(config.lead.isCrossProject).toBe(false)
    expect(config.agents).toEqual([
      expect.objectContaining({
        id: 'mesh-expert',
        isCrossProject: true,
        projectLabel: 'mesh',
      }),
    ])
  })
})
