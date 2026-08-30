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

  const normalized = {
    ...entry,
    memberName,
    message: String(entry.message ?? '').trim() || 'Failed',
  }
  delete normalized.member_name
  return normalized
}

function normalizeTeamRuntimeState(value) {
  const normalized = String(value ?? '').trim().toLowerCase()
  if (normalized === 'active') return 'active'
  if (normalized === 'degraded') return 'degraded'
  if (normalized === 'coldresume' || normalized === 'cold_resume') return 'coldResume'
  return 'none'
}

function normalizeRuntimeSnapshotFreshness(value) {
  const normalized = String(value ?? '').trim().toLowerCase()
  if (normalized === 'fresh') return 'fresh'
  if (normalized === 'cached') return 'cached'
  if (normalized === 'attachmentsonly' || normalized === 'attachments_only') {
    return 'attachments_only'
  }
  return null
}

const LEGACY_MEMBER_ACTIVATION_STAGE_ALIASES = {
  validate: 'prepare_member',
  load_member: 'prepare_member',
  validate_configuration: 'prepare_member',
  resolve_pane: 'acquire_pane',
  create_pane: 'acquire_pane',
  create_panes: 'acquire_pane',
  launch_sessions: 'capture_session_identity',
  start_daemon: 'start_member_daemon',
  start_daemons: 'start_member_daemon',
  send_onboarding: 'deliver_onboarding',
  update_runtime: 'commit_runtime',
  update_roster: 'commit_runtime',
  commit_member_runtime: 'commit_runtime',
}

const LEGACY_STEP_CANONICAL_STAGE_MAP = {
  initialize_team: {
    validate_configuration: ['prepare_member'],
    create_team: [],
    add_lead: [],
    create_panes: ['acquire_pane', 'launch_session'],
    launch_sessions: ['capture_session_identity'],
    join_mesh: ['join_mesh'],
    start_daemons: ['start_member_daemon'],
    send_onboarding: ['deliver_onboarding'],
  },
  add_agent: {
    validate: ['prepare_member'],
    create_pane: ['acquire_pane', 'launch_session'],
    launch_session: ['capture_session_identity'],
    join_mesh: ['join_mesh'],
    start_daemon: ['start_member_daemon'],
    send_onboarding: ['deliver_onboarding'],
    update_roster: ['commit_runtime'],
  },
  resume_member: {
    validate: ['prepare_member'],
    load_member: ['prepare_member'],
    resolve_pane: ['acquire_pane'],
    launch_session: ['launch_session', 'capture_session_identity'],
    join_mesh: ['join_mesh'],
    start_daemon: ['start_member_daemon'],
    send_onboarding: ['deliver_onboarding'],
    update_runtime: ['commit_runtime'],
  },
}

function normalizeStepStatus(value) {
  const normalized = String(value ?? '').trim().toLowerCase()
  if (normalized === 'running') return 'running'
  if (normalized === 'succeeded') return 'succeeded'
  if (normalized === 'failed') return 'failed'
  return 'pending'
}

function normalizeMemberActivationStage(value) {
  const normalized = String(value ?? '').trim().toLowerCase()
  if (!normalized) return null
  return LEGACY_MEMBER_ACTIVATION_STAGE_ALIASES[normalized] ?? normalized
}

function normalizeCanonicalStages(value) {
  if (!Array.isArray(value)) return []
  return value
    .map((entry) => normalizeMemberActivationStage(entry))
    .filter(Boolean)
}

function deriveCanonicalStages(operation, legacyStep) {
  const operationKey = String(operation ?? '').trim().toLowerCase()
  const legacyKey = String(legacyStep ?? '').trim()
  if (!legacyKey) return []
  return [...(LEGACY_STEP_CANONICAL_STAGE_MAP[operationKey]?.[legacyKey] ?? [])]
}

export function normalizeStepProgressEvent(value) {
  if (!value || typeof value !== 'object') return null

  const progress = value.progress
  const step = String(progress?.step ?? '').trim()
  if (!step) return null

  const operation = String(value.operation ?? '').trim()
  const canonicalStages = normalizeCanonicalStages(
    value.canonicalStages ?? value.canonical_stages ?? progress?.canonicalStages ?? progress?.canonical_stages
  )
  const normalizedCanonicalStages =
    canonicalStages.length > 0 ? canonicalStages : deriveCanonicalStages(operation, step)

  const normalized = {
    ...value,
    teamName: String(value.teamName ?? value.team_name ?? '').trim(),
    operation,
    progress: {
      ...(progress && typeof progress === 'object' ? progress : {}),
      step,
      status: normalizeStepStatus(progress?.status),
      message: progress?.message == null ? null : String(progress.message),
      canonicalStages: normalizedCanonicalStages,
    },
  }
  delete normalized.team_name
  delete normalized.canonical_stages
  delete normalized.progress.canonical_stages
  return normalized
}

function normalizeCoordinationMember(value) {
  if (!value || typeof value !== 'object') return null

  const normalized = {
    ...value,
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
    // The Claude session behind the member, when the backend attached one. The
    // mesh canvas keys a node's workflow runs on it.
    sessionId: value.sessionId ?? value.session_id ?? null,
    // Live workflow writes under that session. A headless workflow parent
    // reports idle for the whole run, so this is the only evidence the node's
    // activity derivation has that it is working.
    workflowActivity: value.workflowActivity ?? value.workflow_activity ?? null,
    isCrossProject: Boolean(value.isCrossProject ?? value.is_cross_project),
    projectLabel: String(value.projectLabel ?? value.project_label ?? '').trim(),
  }

  assignIfDefined(normalized, 'taskEffort', value.taskEffort ?? value.task_effort)
  assignIfDefined(normalized, 'taskEffortWhy', value.taskEffortWhy ?? value.task_effort_why)

  const model = String(value.model ?? '').trim()
  if (model) normalized.model = model

  const reasoningEffort = String(value.reasoningEffort ?? value.reasoning_effort ?? '').trim()
  if (reasoningEffort) normalized.reasoningEffort = reasoningEffort

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

  for (const alias of [
    'cli_tool',
    'project_id',
    'role_id',
    'role_name',
    'focus_area',
    'context_summary',
    'behavior_summary',
    'session_status',
    'pane_id',
    'session_id',
    'workflow_activity',
    'task_effort',
    'task_effort_why',
    'is_cross_project',
    'project_label',
    'reasoning_effort',
    'behavioral_contract',
  ]) {
    delete normalized[alias]
  }

  return normalized
}

export function normalizeLiveTeamStatus(value) {
  if (!value || typeof value !== 'object') return null

  const normalized = {
    ...value,
    teamName: value.teamName ?? value.team_name ?? '',
    leadName: value.leadName ?? value.lead_name ?? 'team-lead',
    members: Array.isArray(value.members)
      ? value.members.map((member) => normalizeCoordinationMember(member)).filter(Boolean)
      : [],
  }

  assignIfDefined(
    normalized,
    'runtimeSnapshotFreshness',
    normalizeRuntimeSnapshotFreshness(
      value.runtimeSnapshotFreshness ?? value.runtime_snapshot_freshness
    ) ?? undefined
  )
  assignIfDefined(normalized, 'description', value.description ?? undefined)
  delete normalized.team_name
  delete normalized.lead_name
  delete normalized.runtime_snapshot_freshness
  return normalized
}

export function normalizeProjectMeshSnapshot(value) {
  if (!value || typeof value !== 'object') return null

  const rawTeamStatus = value.teamStatus ?? value.team_status
  const normalizedStatus = normalizeLiveTeamStatus(
    rawTeamStatus
      ? {
          ...rawTeamStatus,
          teamName: value.teamName ?? value.team_name ?? '',
          leadName:
            value.teamStatus?.leadName ??
            value.teamStatus?.lead_name ??
            value.team_status?.leadName ??
            value.team_status?.lead_name ??
            'team-lead',
          runtimeSnapshotFreshness:
            value.teamStatus?.runtimeSnapshotFreshness ??
            value.teamStatus?.runtime_snapshot_freshness ??
            value.team_status?.runtimeSnapshotFreshness ??
            value.team_status?.runtime_snapshot_freshness,
          members: value.teamStatus?.members ?? value.team_status?.members ?? [],
        }
      : null
  )

  const normalized = {
    ...value,
    meshAvailable: value.meshAvailable ?? value.mesh_available ?? true,
    tmuxAvailable: value.tmuxAvailable ?? value.tmux_available ?? true,
    teamName: value.teamName ?? value.team_name ?? null,
    teamRuntimeState: normalizeTeamRuntimeState(
      value.teamRuntimeState ?? value.team_runtime_state ?? 'none'
    ),
    teamStatus: normalizedStatus
      ? {
          ...(rawTeamStatus && typeof rawTeamStatus === 'object' ? rawTeamStatus : {}),
          leadName: normalizedStatus.leadName,
          runtimeSnapshotFreshness: normalizedStatus.runtimeSnapshotFreshness ?? null,
          members: normalizedStatus.members,
        }
      : null,
    warnings: normalizeStringList(value.warnings),
  }
  delete normalized.mesh_available
  delete normalized.tmux_available
  delete normalized.team_name
  delete normalized.team_runtime_state
  delete normalized.team_status
  if (normalized.teamStatus) {
    delete normalized.teamStatus.lead_name
    delete normalized.teamStatus.runtime_snapshot_freshness
  }
  return normalized
}

export function normalizeResumeTeamReport(value) {
  if (!value || typeof value !== 'object') return null

  const normalized = {
    ...value,
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
  delete normalized.team_name
  delete normalized.total_members
  delete normalized.resumed_members
  delete normalized.failed_members
  delete normalized.started_team_daemon
  delete normalized.team_daemon_warning
  return normalized
}

export function normalizeResumeTeamProgressEvent(value) {
  if (!value || typeof value !== 'object') return null

  const teamName = String(value.teamName ?? value.team_name ?? '').trim()
  const memberName = String(value.memberName ?? value.member_name ?? '').trim()
  const stage = normalizeMemberActivationStage(value.stage)
  if (!teamName || !memberName || !stage) return null

  const normalized = {
    ...value,
    operation: String(value.operation ?? 'resume_team').trim() || 'resume_team',
    teamName,
    memberName,
    memberIndex: Number(value.memberIndex ?? value.member_index ?? 0),
    memberCount: Number(value.memberCount ?? value.member_count ?? 0),
    stage,
    status: normalizeStepStatus(value.status),
    message: value.message == null ? null : String(value.message),
  }
  delete normalized.team_name
  delete normalized.member_name
  delete normalized.member_index
  delete normalized.member_count
  return normalized
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

  delete normalized.team_name
  delete normalized.opened_existing

  return normalized
}

export function normalizeMemberActionReport(value) {
  if (!value || typeof value !== 'object') return value

  const normalized = {
    ...value,
    teamName: value.teamName ?? value.team_name ?? '',
    memberName: value.memberName ?? value.member_name ?? '',
    paneId: value.paneId ?? value.pane_id ?? null,
    reusedPane: Boolean(value.reusedPane ?? value.reused_pane),
    warnings: normalizeStringList(value.warnings),
  }
  delete normalized.team_name
  delete normalized.member_name
  delete normalized.pane_id
  delete normalized.reused_pane
  return normalized
}
