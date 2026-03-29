import { BEHAVIORAL_CONTRACT_MODES, normalizeBehavioralContract } from './normalize.js'

function normalizeStringList(value) {
  if (!Array.isArray(value)) return []
  return value.map((entry) => String(entry ?? '').trim()).filter(Boolean)
}

function assignIfDefined(target, key, value) {
  if (value !== undefined) {
    target[key] = value
  }
}

function normalizeFailedMember(entry) {
  if (!entry || typeof entry !== 'object') return null

  const memberName = String(entry.memberName ?? entry.member_name ?? '').trim()
  if (!memberName) return null

  return {
    memberName,
    message: String(entry.message ?? '').trim() || 'Failed',
  }
}

function normalizeTeamRuntimeState(value) {
  const normalized = String(value ?? '').trim().toLowerCase()
  if (normalized === 'active') return 'active'
  if (normalized === 'degraded') return 'degraded'
  if (normalized === 'coldresume' || normalized === 'cold_resume') return 'coldResume'
  return 'none'
}

function normalizeStepStatus(value) {
  const normalized = String(value ?? '').trim().toLowerCase()
  if (normalized === 'running') return 'running'
  if (normalized === 'succeeded') return 'succeeded'
  if (normalized === 'failed') return 'failed'
  return 'pending'
}

function normalizeCoordinationMember(value) {
  if (!value || typeof value !== 'object') return null

  const normalized = {
    name: String(value.name ?? '').trim(),
    role: String(value.role ?? '').trim().toLowerCase() || 'member',
    cliTool: value.cliTool ?? value.cli_tool ?? 'codex',
    projectId: String(value.projectId ?? value.project_id ?? '').trim(),
    description: value.description ?? null,
    roleId: value.roleId ?? value.role_id ?? null,
    roleName: value.roleName ?? value.role_name ?? null,
    focusArea: value.focusArea ?? value.focus_area ?? null,
    contextSummary: value.contextSummary ?? value.context_summary ?? null,
    behaviorSummary: value.behaviorSummary ?? value.behavior_summary ?? null,
    sessionStatus: value.sessionStatus ?? value.session_status ?? 'offline',
    paneId: value.paneId ?? value.pane_id ?? null,
    isCrossProject: Boolean(value.isCrossProject ?? value.is_cross_project),
    projectLabel: String(value.projectLabel ?? value.project_label ?? '').trim(),
  }

  const model = String(value.model ?? '').trim()
  if (model) normalized.model = model

  const instructions = value.instructions ?? null
  if (instructions) normalized.instructions = instructions

  const behavioralContract = normalizeBehavioralContract(
    value.behavioralContract ?? value.behavioral_contract,
    { mode: BEHAVIORAL_CONTRACT_MODES.OPTIONAL_OBJECT }
  )
  if (behavioralContract) normalized.behavioralContract = behavioralContract

  if (Array.isArray(value.capabilities)) {
    normalized.capabilities = value.capabilities
  }

  return normalized
}

export function normalizeLiveTeamStatus(value) {
  if (!value || typeof value !== 'object') return null

  const normalized = {
    teamName: value.teamName ?? value.team_name ?? '',
    leadName: value.leadName ?? value.lead_name ?? 'team-lead',
    members: Array.isArray(value.members)
      ? value.members.map((member) => normalizeCoordinationMember(member)).filter(Boolean)
      : [],
  }

  assignIfDefined(normalized, 'description', value.description ?? undefined)
  return normalized
}

export function normalizeProjectMeshSnapshot(value) {
  if (!value || typeof value !== 'object') return null

  const normalizedStatus = normalizeLiveTeamStatus(
    value.teamStatus ?? value.team_status
      ? {
          teamName: value.teamName ?? value.team_name ?? '',
          leadName: value.teamStatus?.leadName ?? value.teamStatus?.lead_name ?? value.team_status?.leadName ?? value.team_status?.lead_name ?? 'team-lead',
          members: value.teamStatus?.members ?? value.team_status?.members ?? [],
        }
      : null
  )

  return {
    meshAvailable: value.meshAvailable ?? value.mesh_available ?? true,
    tmuxAvailable: value.tmuxAvailable ?? value.tmux_available ?? true,
    teamName: value.teamName ?? value.team_name ?? null,
    teamRuntimeState: normalizeTeamRuntimeState(
      value.teamRuntimeState ?? value.team_runtime_state ?? 'none'
    ),
    teamStatus: normalizedStatus
      ? {
          leadName: normalizedStatus.leadName,
          members: normalizedStatus.members,
        }
      : null,
    warnings: normalizeStringList(value.warnings),
  }
}

export function normalizeResumeTeamReport(value) {
  if (!value || typeof value !== 'object') return null

  return {
    teamName: value.teamName ?? value.team_name ?? '',
    resumed: Boolean(value.resumed),
    totalMembers: Number(value.totalMembers ?? value.total_members ?? 0),
    resumedMembers: normalizeStringList(value.resumedMembers ?? value.resumed_members),
    failedMembers: Array.isArray(value.failedMembers ?? value.failed_members)
      ? (value.failedMembers ?? value.failed_members).map((entry) => normalizeFailedMember(entry)).filter(Boolean)
      : [],
    warnings: normalizeStringList(value.warnings),
    startedTeamDaemon: Boolean(value.startedTeamDaemon ?? value.started_team_daemon),
    teamDaemonWarning: value.teamDaemonWarning ?? value.team_daemon_warning ?? null,
  }
}

export function normalizeResumeTeamProgressEvent(value) {
  if (!value || typeof value !== 'object') return null

  const teamName = String(value.teamName ?? value.team_name ?? '').trim()
  const memberName = String(value.memberName ?? value.member_name ?? '').trim()
  const stage = String(value.stage ?? '').trim()
  if (!teamName || !memberName || !stage) return null

  return {
    operation: String(value.operation ?? 'resume_team').trim() || 'resume_team',
    teamName,
    memberName,
    memberIndex: Number(value.memberIndex ?? value.member_index ?? 0),
    memberCount: Number(value.memberCount ?? value.member_count ?? 0),
    stage,
    status: normalizeStepStatus(value.status),
    message: value.message == null ? null : String(value.message),
  }
}

export function normalizeInitializeTeamResult(value) {
  if (!value || typeof value !== 'object') return value

  const normalized = {
    ...value,
    teamName: value.teamName ?? value.team_name ?? '',
  }

  if (value.openedExisting !== undefined || value.opened_existing !== undefined) {
    normalized.openedExisting = Boolean(value.openedExisting ?? value.opened_existing)
  }

  return normalized
}

export function normalizeMemberActionReport(value) {
  if (!value || typeof value !== 'object') return value

  return {
    ...value,
    teamName: value.teamName ?? value.team_name ?? '',
    memberName: value.memberName ?? value.member_name ?? '',
    paneId: value.paneId ?? value.pane_id ?? null,
    reusedPane: Boolean(value.reusedPane ?? value.reused_pane),
    warnings: normalizeStringList(value.warnings),
  }
}
