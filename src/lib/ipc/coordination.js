import { listen } from '@tauri-apps/api/event'

import {
  buildMockAddAgentReport,
  buildMockInitializeReport,
  buildMockLiveTeamStatus,
  buildMockProjectMeshSnapshot,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'
import { normalizeInitializeTeamPayload } from './coordinationPayloads.js'

export function coordinationCreateTeam(teamName) {
  return invokeOrMock('coordination_create_team', { teamName }, () => undefined)
}

export function coordinationDisbandTeam(teamName) {
  return invokeOrMock('coordination_disband_team', { teamName }, () => ({
    teamName,
    disbanded: true,
    alreadyDisbanded: false,
    message: 'team disbanded',
  }))
}

export function coordinationAddMember(teamName, memberName, backendKind) {
  return invokeOrMock('coordination_add_member', { teamName, memberName, backendKind }, () => undefined)
}

export function coordinationRemoveMember(teamName, memberName) {
  return invokeOrMock('coordination_remove_member', { teamName, memberName }, () => ({
    teamName,
    memberName,
    removed: true,
    message: 'member removed',
    steps: [],
    warnings: [],
  }))
}

export function coordinationListTeams() {
  return invokeOrMock('coordination_list_teams', undefined, () => ({ teams: [], warnings: [] }))
}

export function coordinationGetTeamStatus(teamName) {
  return invokeOrMock('coordination_get_team_status', { teamName }, () => ({ teamName, members: [] }))
}

export function coordinationInitializeTeam(request) {
  const payload = normalizeInitializeTeamPayload(request)
  return invokeOrMock('coordination_initialize_team', { request: payload }, () =>
    buildMockInitializeReport(payload?.teamName ?? '')
  )
}

export function coordinationAddAgent(request) {
  return invokeOrMock('coordination_add_agent', { request }, () =>
    buildMockAddAgentReport(request)
  )
}

export function coordinationResumeMember(teamName, memberName, contextMode = 'continue') {
  return invokeOrMock('coordination_resume_member', { request: { teamName, memberName, contextMode } }, () => ({
    teamName,
    memberName,
    resumed: true,
    succeededSteps: ['validate', 'resolve_pane', 'launch_session', 'update_runtime'],
    failedStep: null,
    retryable: false,
    message: 'member resumed',
    steps: [
      { step: 'validate', status: 'succeeded', message: 'request validated' },
      { step: 'update_runtime', status: 'succeeded', message: 'runtime updated' },
    ],
    warnings: [],
    paneId: '%2',
    reusedPane: false,
  }))
}

export function coordinationResumeTeam(teamName, contextMode = 'continue') {
  return invokeOrMock('coordination_resume_team', { request: { teamName, contextMode } }, () => ({
    teamName,
    resumed: true,
    totalMembers: 2,
    resumedMembers: ['team-lead', 'frontend-dev'],
    failedMembers: [],
    warnings: [],
    startedTeamDaemon: false,
    teamDaemonWarning: null,
  }))
}

export function coordinationReonboard(teamName, memberName) {
  return invokeOrMock(
    'coordination_reonboard',
    { request: { teamName, memberName } },
    () => ({ delivered: true, method: 'tmux_injection' })
  )
}

export function coordinationGetFeatureAvailability() {
  return invokeOrMock('coordination_get_feature_availability', undefined, () => ({
    canInitialize: true,
    meshAvailable: true,
    tmuxAvailable: true,
    blockingErrors: [],
  }))
}

export function coordinationPreflightCheck(request) {
  return invokeOrMock('coordination_preflight_check', { request }, () => ({
    canInitialize: true,
    blockingErrors: [],
    agentWarnings: [],
  }))
}

export function coordinationGetLiveTeamStatus(teamName) {
  return invokeOrMock('coordination_get_live_team_status', { teamName }, () =>
    buildMockLiveTeamStatus(teamName)
  )
}

export function coordinationGetProjectMeshSnapshot(projectPath) {
  return invokeOrMock('coordination_get_project_mesh_snapshot', { projectPath }, () =>
    buildMockProjectMeshSnapshot(projectPath)
  )
}

export function onCoordinationStepProgress(callback) {
  return listen('coordination-step-progress', callback)
}
