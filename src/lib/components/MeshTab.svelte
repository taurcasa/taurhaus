<script>
  import {
    coordinationAddAgent,
    coordinationDisbandTeam,
    coordinationGetLiveTeamStatus,
    coordinationListTeams,
    coordinationRemoveMember,
    coordinationResumeMember,
    listRoleTemplates,
    upsertRoleTemplate,
  } from '../ipc.js'
  import { normalizeProjectOption } from '../projectOptions.js'
  import { themeTokens } from '../themeTokens.js'
  import ConfirmDialog from './ConfirmDialog.svelte'
  import MeshActionBar from './MeshActionBar.svelte'
  import MeshAvailabilityGate from './MeshAvailabilityGate.svelte'
  import MeshCanvas from './MeshCanvas.svelte'
  import MeshEmptyState from './MeshEmptyState.svelte'
  import MeshInitProgress from './MeshInitProgress.svelte'
  import MeshNodeDetail from './MeshNodeDetail.svelte'
  import MeshRuntimeBar from './MeshRuntimeBar.svelte'
  import SlideOver from './SlideOver.svelte'
  import TeamCustomizerPanel from './TeamCustomizerPanel.svelte'
  import TemplateBrowserPanel from './TemplateBrowserPanel.svelte'

  let {
    dark = false,
    projectPath = '',
    availableProjects = [],
    onAddAgent: onAddAgentProp = () => {},
    onDisband: onDisbandProp = () => {},
    onRemoveAgent: onRemoveAgentProp = () => {},
    onFocusPane: onFocusPaneProp = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const fieldTone = $derived(
    dark
      ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20'
      : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'
  )
  const selectScheme = $derived(dark ? '[color-scheme:dark]' : '[color-scheme:light]')

  const modelOptionsByTool = {
    claude: ['opus', 'sonnet', 'haiku'],
    codex: ['gpt-5.3-codex', 'gpt-5-mini'],
    gemini: ['gemini-3.1-pro', 'gemini-2.5-pro', 'gemini-2.0-flash'],
  }

  const quickPresets = [
    {
      presetId: 'fullstack-dev',
      name: 'Full Stack Dev Team',
      description: 'Lead with implementation and review agents',
      leadCount: 1,
      agentCount: 3,
      tools: ['claude', 'codex', 'gemini'],
      builtIn: true,
    },
    {
      presetId: 'research-dev',
      name: 'Research + Development Team',
      description: 'Lead with research and implementation collaboration',
      leadCount: 1,
      agentCount: 3,
      tools: ['claude', 'gemini', 'codex'],
      builtIn: true,
    },
    {
      presetId: 'review-team',
      name: 'Review Team',
      description: 'Lead with focused implementation and QA reviewers',
      leadCount: 1,
      agentCount: 2,
      tools: ['claude', 'codex'],
      builtIn: true,
    },
  ]

  let mode = $state('gate')
  let teamName = $state('')
  let teamConfig = $state(null)
  let slideOver = $state(null)
  let slideOverContext = $state(null)
  let selectedNodeId = $state(null)
  let initProgress = $state(null)
  let errorMessage = $state('')
  let runtimeMessage = $state('')
  let confirmContext = $state(null)

  let roleTemplates = $state([])
  let loadingRoles = $state(false)
  let captureRoleDialog = $state(null)

  let gateBootstrapping = false
  let discoverySequence = 0
  let runtimeMessageTimer = null
  let errorMessageTimer = null

  const projectOptions = $derived.by(() =>
    (availableProjects ?? [])
      .map((project) =>
        normalizeProjectOption(project, { stringLabel: 'raw', objectFallbackLabel: 'raw' })
      )
      .filter((project) => project.id)
  )

  const selectedNode = $derived.by(() => {
    const config = teamConfig
    if (!config || !selectedNodeId) return null

    if (String(config.lead?.id ?? 'lead') === String(selectedNodeId)) {
      return {
        ...config.lead,
        id: String(config.lead?.id ?? 'lead'),
        role: 'lead',
      }
    }

    const agent = (config.agents ?? []).find((entry) => String(entry.id) === String(selectedNodeId))
    if (!agent) return null

    return {
      ...agent,
      role: 'agent',
    }
  })

  const canInitialize = $derived.by(() => {
    const config = teamConfig
    if (!config?.lead) return false
    return Array.isArray(config.agents)
  })

  const addAgentDraft = $derived(
    slideOver === 'addAgent' && slideOverContext && typeof slideOverContext === 'object'
      ? slideOverContext
      : null
  )

  const canSubmitAddAgent = $derived.by(() => {
    const draft = addAgentDraft
    if (!draft) return false
    if (draft.submitting) return false

    return (
      String(draft.name || '').trim().length > 0 &&
      String(draft.tool || '').trim().length > 0 &&
      String(draft.model || '').trim().length > 0 &&
      String(draft.projectId || '').trim().length > 0
    )
  })

  const captureRoleDraft = $derived(
    captureRoleDialog && typeof captureRoleDialog === 'object' ? captureRoleDialog : null
  )

  const canSaveCapturedRole = $derived.by(() => {
    const draft = captureRoleDraft
    if (!draft) return false
    if (draft.submitting) return false
    return (
      String(draft.name || '').trim().length > 0 &&
      String(draft.roleId || '').trim().length > 0
    )
  })

  function normalizeTool(tool) {
    const value = String(tool || '').trim().toLowerCase()
    if (value === 'claude' || value === 'codex' || value === 'gemini') return value
    return 'claude'
  }

  function normalizeStatus(status) {
    const value = String(status || '').trim().toLowerCase()
    if (value === 'active' || value === 'idle') return value
    return 'offline'
  }

  function inferTeamName(path) {
    const segments = String(path || '')
      .replace(/\\/g, '/')
      .split('/')
      .filter(Boolean)
    const project = segments.at(-1) || 'project'
    return `${project}-team`
      .toLowerCase()
      .replace(/[^a-z0-9-]+/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '')
  }

  function defaultModelForTool(tool) {
    const normalized = normalizeTool(tool)
    return modelOptionsByTool[normalized]?.[0] ?? 'default'
  }

  function coerceTeams(response) {
    if (Array.isArray(response)) return response
    return Array.isArray(response?.teams) ? response.teams : []
  }

  function normalizeTeamName(team) {
    return team?.teamName ?? team?.team_name ?? ''
  }

  function normalizeLeadPath(team) {
    return team?.leadProjectPath ?? team?.lead_project_path ?? null
  }

  function normalizeLinuxPath(path) {
    let value = String(path || '').trim()
    if (!value) return ''
    value = value.replace(/\\/g, '/')
    value = value.replace(/\/+/g, '/')
    while (value.length > 1 && value.endsWith('/')) {
      value = value.slice(0, -1)
    }
    return value
  }

  function wslUncToLinux(path) {
    const normalized = String(path || '').trim().replace(/\//g, '\\')
    const lower = normalized.toLowerCase()

    let prefix = ''
    if (lower.startsWith('\\\\wsl$\\')) {
      prefix = '\\\\wsl$\\'
    } else if (lower.startsWith('\\\\wsl.localhost\\')) {
      prefix = '\\\\wsl.localhost\\'
    } else {
      return null
    }

    const remainder = normalized.slice(prefix.length)
    const firstSeparator = remainder.indexOf('\\')
    if (firstSeparator === -1) return null

    const afterDistro = remainder.slice(firstSeparator)
    if (!afterDistro || afterDistro === '\\') return '/'
    return normalizeLinuxPath(afterDistro)
  }

  function windowsDriveToLinux(path) {
    const match = String(path || '').trim().match(/^([a-zA-Z]):[\\/](.*)$/)
    if (!match) return null
    const [, drive, rest] = match
    return normalizeLinuxPath(`/mnt/${drive.toLowerCase()}/${rest}`)
  }

  function normalizeProjectPath(path) {
    const raw = String(path || '').trim()
    if (!raw) return ''
    return wslUncToLinux(raw) ?? windowsDriveToLinux(raw) ?? normalizeLinuxPath(raw)
  }

  function isSameProjectPath(left, right) {
    const leftNormalized = normalizeProjectPath(left)
    const rightNormalized = normalizeProjectPath(right)
    if (!leftNormalized || !rightNormalized) return false
    return leftNormalized === rightNormalized
  }

  function teamMatchesProject(team, currentProjectPath) {
    return isSameProjectPath(normalizeLeadPath(team), currentProjectPath)
  }

  function createLead(overrides = {}) {
    return {
      id: String(overrides.id ?? 'lead'),
      name: String(overrides.name ?? 'team-lead'),
      tool: normalizeTool(overrides.tool ?? overrides.cliTool),
      model: String(overrides.model ?? defaultModelForTool(overrides.tool ?? overrides.cliTool)),
      status: normalizeStatus(overrides.status),
      projectId: String(overrides.projectId ?? projectPath ?? ''),
      description: overrides.description ?? 'Team lead',
      paneId: overrides.paneId ?? null,
      roleId: overrides.roleId ?? null,
      instructions: overrides.instructions ?? null,
      behavioralContract: overrides.behavioralContract ?? null,
      capabilities: Array.isArray(overrides.capabilities) ? overrides.capabilities : null,
    }
  }

  function createAgent(index, overrides = {}) {
    const normalizedTool = normalizeTool(overrides.tool ?? overrides.cliTool ?? 'codex')
    return {
      id: String(overrides.id ?? `agent-${index + 1}`),
      name: String(overrides.name ?? `agent-${index + 1}`),
      tool: normalizedTool,
      model: String(overrides.model ?? defaultModelForTool(normalizedTool)),
      status: normalizeStatus(overrides.status),
      projectId: String(overrides.projectId ?? projectPath ?? ''),
      description: overrides.description ?? null,
      paneId: overrides.paneId ?? null,
      roleId: overrides.roleId ?? null,
      instructions: overrides.instructions ?? null,
      behavioralContract: overrides.behavioralContract ?? null,
      capabilities: Array.isArray(overrides.capabilities) ? overrides.capabilities : null,
    }
  }

  function buildTeamConfigFromPreset(preset) {
    const tools = Array.isArray(preset?.tools) && preset.tools.length > 0
      ? preset.tools.map((entry) => normalizeTool(entry))
      : ['claude', 'codex', 'gemini']

    const lead = createLead({
      id: 'lead',
      name: 'team-lead',
      tool: tools[0] ?? 'claude',
      status: 'offline',
      projectId: projectPath,
    })

    const agentCount = Math.max(
      1,
      Number(preset?.agentCount ?? Math.max(0, Number(preset?.roleCount ?? 1) - 1) ?? 1)
    )

    const agents = Array.from({ length: agentCount }, (_, index) => {
      const tool = tools[(index + 1) % tools.length] ?? 'codex'
      return createAgent(index, {
        id: `agent-${index + 1}`,
        name: `agent-${index + 1}`,
        tool,
        status: 'offline',
        projectId: projectPath,
      })
    })

    return {
      lead,
      agents,
      presetId: preset?.presetId ?? '',
      presetName: preset?.name ?? '',
      composition: {
        presetId: preset?.presetId ?? '',
        name: preset?.name ?? '',
        leadRoleId: preset?.leadRoleId ?? preset?.lead_role_id ?? '',
        agentSlots: Array.isArray(preset?.agentSlots ?? preset?.agent_slots)
          ? (preset?.agentSlots ?? preset?.agent_slots)
          : [],
      },
    }
  }

  function buildTeamConfigFromRuntimeStatus(status) {
    const members = Array.isArray(status?.members) ? status.members : []
    const normalizedMembers = members.map((member, index) => ({
      ...member,
      name: String(member?.name ?? `member-${index + 1}`),
      role: String(member?.role ?? '').toLowerCase(),
      tool: normalizeTool(member?.cliTool ?? member?.cli_tool),
      model: String(member?.model ?? ''),
      status: normalizeStatus(member?.sessionStatus ?? member?.session_status),
      projectId: String(member?.projectId ?? member?.project_id ?? projectPath ?? ''),
      description: member?.description ?? null,
      paneId: member?.paneId ?? member?.pane_id ?? null,
      roleId: member?.roleId ?? member?.role_id ?? null,
      instructions: member?.instructions ?? null,
      behavioralContract: member?.behavioralContract ?? member?.behavioral_contract ?? null,
      capabilities: Array.isArray(member?.capabilities) ? member.capabilities : null,
    }))

    const leadMember = normalizedMembers.find((member) => member.role === 'lead')
    const fallbackLeadName = status?.leadName ?? status?.lead_name ?? 'team-lead'

    const lead = createLead({
      id: String(leadMember?.name ?? 'lead'),
      name: leadMember?.name ?? fallbackLeadName,
      tool: leadMember?.tool ?? 'claude',
      model: leadMember?.model ?? defaultModelForTool(leadMember?.tool ?? 'claude'),
      status: leadMember?.status ?? 'active',
      projectId: leadMember?.projectId ?? projectPath,
      description: leadMember?.description ?? 'Team lead',
      paneId: leadMember?.paneId ?? null,
      roleId: leadMember?.roleId ?? null,
      instructions: leadMember?.instructions ?? null,
      behavioralContract: leadMember?.behavioralContract ?? null,
      capabilities: leadMember?.capabilities ?? null,
    })

    const agents = normalizedMembers
      .filter((member) => member.role !== 'lead')
      .map((member, index) => createAgent(index, {
        id: member.name,
        name: member.name,
        tool: member.tool,
        model: member.model,
        status: member.status,
        projectId: member.projectId,
        description: member.description,
        paneId: member.paneId,
        roleId: member.roleId,
        instructions: member.instructions,
        behavioralContract: member.behavioralContract,
        capabilities: member.capabilities,
      }))

    return {
      lead,
      agents,
      presetId: '',
      presetName: '',
      composition: null,
    }
  }

  function composeConfigFromPayload(payload) {
    const lead = createLead({
      id: String(payload?.lead?.name ?? 'lead'),
      name: payload?.lead?.name ?? 'team-lead',
      tool: payload?.lead?.cliTool ?? payload?.lead?.cli_tool,
      model: payload?.lead?.model,
      status: 'offline',
      projectId: payload?.lead?.projectId ?? payload?.lead?.project_id ?? projectPath,
      description: payload?.lead?.description ?? 'Team lead',
      roleId: payload?.lead?.roleId ?? payload?.lead?.role_id ?? null,
      instructions: payload?.lead?.instructions ?? null,
      behavioralContract: payload?.lead?.behavioralContract ?? payload?.lead?.behavioral_contract ?? null,
      capabilities: Array.isArray(payload?.lead?.capabilities) ? payload.lead.capabilities : null,
    })

    const rawAgents = Array.isArray(payload?.agents) ? payload.agents : []
    const agents = rawAgents.map((agent, index) =>
      createAgent(index, {
        id: String(agent?.name ?? `agent-${index + 1}`),
        name: agent?.name ?? `agent-${index + 1}`,
        tool: agent?.cliTool ?? agent?.cli_tool,
        model: agent?.model,
        status: 'offline',
        projectId: agent?.projectId ?? agent?.project_id ?? projectPath,
        description: agent?.description ?? null,
        roleId: agent?.roleId ?? agent?.role_id ?? null,
        instructions: agent?.instructions ?? null,
        behavioralContract: agent?.behavioralContract ?? agent?.behavioral_contract ?? null,
        capabilities: Array.isArray(agent?.capabilities) ? agent.capabilities : null,
      })
    )

    return {
      lead,
      agents,
      presetId: '',
      presetName: '',
      composition: {
        presetId: payload?.presetId ?? '',
        name: payload?.presetName ?? '',
        leadRoleId: payload?.leadRoleId ?? payload?.lead_role_id ?? '',
        agentSlots: Array.isArray(payload?.agentSlots ?? payload?.agent_slots)
          ? (payload?.agentSlots ?? payload?.agent_slots)
          : [],
      },
    }
  }

  function buildInitializationRequest(config) {
    const lead = config?.lead
    const agents = Array.isArray(config?.agents) ? config.agents : []

    return {
      teamName: teamName.trim() || inferTeamName(projectPath),
      teamDescription: null,
      leadMode: 'launch_new',
      lead: {
        name: lead?.name ?? 'team-lead',
        cliTool: normalizeTool(lead?.tool),
        model: lead?.model ?? defaultModelForTool(lead?.tool),
        projectId: lead?.projectId || projectPath,
        description: lead?.description ?? 'Team lead',
        roleId: lead?.roleId ?? null,
        instructions: lead?.instructions ?? null,
        behavioralContract: lead?.behavioralContract ?? null,
        capabilities: Array.isArray(lead?.capabilities) ? lead.capabilities : null,
      },
      agents: agents.map((agent, index) => ({
        name: agent?.name || `agent-${index + 1}`,
        cliTool: normalizeTool(agent?.tool),
        model: agent?.model ?? defaultModelForTool(agent?.tool),
        projectId: agent?.projectId || projectPath,
        description: agent?.description ?? null,
        roleId: agent?.roleId ?? null,
        instructions: agent?.instructions ?? null,
        behavioralContract: agent?.behavioralContract ?? null,
        capabilities: Array.isArray(agent?.capabilities) ? agent.capabilities : null,
      })),
    }
  }

  async function refreshRuntimeTeamConfig(nextTeamName, sequence) {
    const report = await coordinationGetLiveTeamStatus(nextTeamName)
    if (sequence !== discoverySequence) return
    teamConfig = buildTeamConfigFromRuntimeStatus(report)
  }

  async function bootstrapFromGate() {
    const sequence = ++discoverySequence
    errorMessage = ''
    runtimeMessage = ''

    try {
      const response = await coordinationListTeams()
      if (sequence !== discoverySequence) return

      const teams = coerceTeams(response)
      const matchingTeam = teams.find((team) => teamMatchesProject(team, projectPath))

      if (matchingTeam) {
        const matchedTeamName = normalizeTeamName(matchingTeam)
        teamName = matchedTeamName
        mode = 'runtime'
        selectedNodeId = null
        initProgress = null
        await refreshRuntimeTeamConfig(matchedTeamName, sequence)
        return
      }

      teamName = inferTeamName(projectPath)
      teamConfig = null
      selectedNodeId = null
      initProgress = null
      mode = 'empty'
    } catch (error) {
      if (sequence !== discoverySequence) return
      errorMessage = error?.message || 'Failed to load Mesh team state.'
      teamName = inferTeamName(projectPath)
      mode = 'empty'
    }
  }

  function ensureGateReady() {
    if (mode !== 'gate' || gateBootstrapping) return ''
    gateBootstrapping = true
    void bootstrapFromGate().finally(() => {
      gateBootstrapping = false
    })
    return ''
  }

  function triggerGateReady() {
    ensureGateReady()
    return {}
  }

  function closeSlideOver() {
    slideOver = null
    slideOverContext = null
  }

  function handlePresetSelect(preset) {
    teamConfig = buildTeamConfigFromPreset(preset)
    teamName = inferTeamName(projectPath)
    selectedNodeId = null
    mode = 'setup'
    closeSlideOver()
    runtimeMessage = ''
  }

  function handlePresetFromBrowser(preset) {
    handlePresetSelect(preset)
    closeSlideOver()
  }

  function handleRoleFromBrowser(role) {
    slideOver = 'customizer'
    slideOverContext = {
      selectedRole: role,
    }
  }

  function handleStartCustom() {
    teamConfig = {
      lead: createLead({ id: 'lead', name: 'team-lead', tool: 'claude', status: 'offline' }),
      agents: [
        createAgent(0, { id: 'agent-1', name: 'agent-1', tool: 'codex', status: 'offline' }),
      ],
      presetId: '',
      presetName: '',
      composition: null,
    }
    teamName = inferTeamName(projectPath)
    selectedNodeId = null
    mode = 'setup'
    closeSlideOver()
    runtimeMessage = ''
  }

  function handleNodeClick(nodeId) {
    selectedNodeId = String(selectedNodeId) === String(nodeId) ? null : String(nodeId)
  }

  function handleInitialize() {
    if (!canInitialize) return
    initProgress = buildInitializationRequest(teamConfig)
    mode = 'initializing'
    selectedNodeId = null
    runtimeMessage = ''
  }

  function handleInitializeBack() {
    mode = 'setup'
  }

  async function handleInitializeSuccess(result) {
    const nextTeamName = (
      result?.teamName ??
      result?.team_name ??
      initProgress?.teamName ??
      initProgress?.team_name ??
      teamName
    ) || inferTeamName(projectPath)
    teamName = nextTeamName
    runtimeMessage = result?.openedExisting ? 'Opened existing team.' : 'Team initialized successfully.'
    mode = 'runtime'
    selectedNodeId = null
    closeSlideOver()

    const sequence = ++discoverySequence
    try {
      await refreshRuntimeTeamConfig(nextTeamName, sequence)
    } catch (error) {
      errorMessage = error?.message || 'Failed to load runtime team status.'
      teamConfig = {
        lead: createLead({ id: 'lead', name: 'team-lead', tool: 'claude', status: 'active' }),
        agents: [],
        presetId: '',
        presetName: '',
        composition: null,
      }
    }
  }

  function handleReset() {
    teamConfig = null
    selectedNodeId = null
    initProgress = null
    mode = 'empty'
    runtimeMessage = ''
    errorMessage = ''
    closeSlideOver()
  }

  function openCustomizer() {
    if (!teamConfig) return
    slideOver = 'customizer'
    slideOverContext = {
      ...slideOverContext,
    }
  }

  function handleTeamSave(payload) {
    teamConfig = composeConfigFromPayload(payload)
    if (!teamName.trim()) {
      teamName = inferTeamName(projectPath)
    }
    selectedNodeId = null
    mode = 'setup'
    closeSlideOver()
  }

  function openAddAgentPanel() {
    const defaultProject = projectOptions[0]?.id || projectPath || ''
    roleTemplates = []
    slideOver = 'addAgent'
    slideOverContext = {
      roleId: '',
      name: '',
      tool: 'codex',
      model: defaultModelForTool('codex'),
      projectId: defaultProject,
      description: '',
      submitting: false,
      error: '',
      isLocked: false,
    }
    void loadRoleTemplates()
  }

  async function loadRoleTemplates() {
    loadingRoles = true
    try {
      roleTemplates = await listRoleTemplates()
    } catch (error) {
      console.error('Failed to load role templates:', error)
    } finally {
      loadingRoles = false
    }
  }

  function handleRoleChange(selectedRoleId) {
    const draft = addAgentDraft
    if (!draft) return

    if (!selectedRoleId) {
      slideOverContext = {
        ...draft,
        roleId: '',
        isLocked: false,
      }
      return
    }

    const role = roleTemplates.find((r) => r.roleId === selectedRoleId)
    if (role) {
      slideOverContext = {
        ...draft,
        roleId: selectedRoleId,
        tool: normalizeTool(role.cliTool),
        model: role.model || defaultModelForTool(role.cliTool),
        description: role.instructions || '',
        isLocked: true,
      }
    }
  }

  function toggleAddAgentLock() {
    const draft = addAgentDraft
    if (!draft) return
    slideOverContext = {
      ...draft,
      isLocked: !draft.isLocked,
    }
  }

  function updateAddAgentField(field, value) {
    const draft = addAgentDraft
    if (!draft) return
    const next = {
      ...draft,
      [field]: value,
    }
    if (field === 'tool') {
      next.model = defaultModelForTool(value)
    }
    slideOverContext = next
  }

  function slugifyRoleId(value) {
    const slug = String(value || '')
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9\s_-]+/g, '')
      .replace(/[\s_]+/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '')
    return slug || 'captured-role'
  }

  function normalizeBehavioralContract(value) {
    const base = {
      communication: [],
      execution: [],
      escalation: [],
    }

    if (value && typeof value === 'object' && !Array.isArray(value)) {
      for (const key of Object.keys(base)) {
        if (Array.isArray(value[key])) {
          base[key] = value[key].map((entry) => String(entry || '').trim()).filter(Boolean)
        }
      }
      return base
    }

    if (Array.isArray(value)) {
      base.communication = value
        .map((entry) => {
          if (typeof entry === 'string') return entry.trim()
          if (!entry || typeof entry !== 'object') return ''
          const rule = String(entry.rule ?? entry.text ?? '').trim()
          const enabled = entry.enabled === undefined ? true : Boolean(entry.enabled)
          return enabled ? rule : ''
        })
        .filter(Boolean)
      return base
    }

    return base
  }

  function contractHasRules(contract) {
    return (
      contract.communication.length > 0 ||
      contract.execution.length > 0 ||
      contract.escalation.length > 0
    )
  }

  function defaultBehavioralContract(roleName) {
    const safeName = String(roleName || 'agent').trim() || 'agent'
    return {
      communication: [`Report concise progress as ${safeName} and escalate blockers quickly.`],
      execution: ['Execute scoped tasks and verify outcomes before handoff.'],
      escalation: ['Escalate ambiguous requirements before taking risky actions.'],
    }
  }

  function normalizeCapabilities(capabilities, tool) {
    if (Array.isArray(capabilities)) {
      const normalized = capabilities
        .map((entry) => String(entry || '').trim().toLowerCase())
        .filter(Boolean)
      if (normalized.length > 0) return [...new Set(normalized)]
    }
    return [`${normalizeTool(tool)}-workflow`]
  }

  function openCaptureRoleDialog() {
    if (!selectedNode || mode !== 'runtime') return

    const roleName = String(selectedNode.name || '').trim() || 'captured-role'
    const normalizedContract = normalizeBehavioralContract(selectedNode.behavioralContract)
    const hasBehavioralContract = contractHasRules(normalizedContract)
    const description = String(selectedNode.description || '').trim()

    captureRoleDialog = {
      roleKind: selectedNode.role === 'lead' ? 'lead' : 'agent',
      name: roleName,
      roleId: slugifyRoleId(roleName),
      manualRoleId: false,
      tool: normalizeTool(selectedNode.tool),
      model: String(selectedNode.model || '').trim() || defaultModelForTool(selectedNode.tool),
      description,
      includeInstructions: description.length > 0,
      includeBehavioralContract: hasBehavioralContract,
      behavioralContract: normalizedContract,
      capabilities: Array.isArray(selectedNode.capabilities) ? selectedNode.capabilities : [],
      submitting: false,
      error: '',
    }
  }

  function closeCaptureRoleDialog() {
    captureRoleDialog = null
  }

  function updateCaptureRoleName(value) {
    const draft = captureRoleDraft
    if (!draft) return

    const name = String(value || '')
    captureRoleDialog = {
      ...draft,
      name,
      roleId: draft.manualRoleId ? draft.roleId : slugifyRoleId(name),
    }
  }

  function updateCaptureRoleId(value) {
    const draft = captureRoleDraft
    if (!draft) return
    captureRoleDialog = {
      ...draft,
      roleId: String(value || ''),
      manualRoleId: true,
    }
  }

  function toggleCaptureRoleFlag(field) {
    const draft = captureRoleDraft
    if (!draft) return
    captureRoleDialog = {
      ...draft,
      [field]: !draft[field],
    }
  }

  function buildCapturedRoleTemplate(draft) {
    const roleKind = draft.roleKind === 'lead' ? 'lead' : 'agent'
    const trimmedName = String(draft.name || '').trim()
    const normalizedRoleId = slugifyRoleId(draft.roleId)
    const includeInstructions = Boolean(draft.includeInstructions)
    const includeBehavioralContract = Boolean(draft.includeBehavioralContract)

    const instructionsFromNode = includeInstructions ? String(draft.description || '').trim() : ''
    const instructions = instructionsFromNode || `Captured runtime role for ${trimmedName}.`

    const currentContract = normalizeBehavioralContract(draft.behavioralContract)
    const behavioralContract = includeBehavioralContract && contractHasRules(currentContract)
      ? currentContract
      : defaultBehavioralContract(trimmedName)

    return {
      schema: {
        kind: 'role_template',
        version: 1,
      },
      roleId: normalizedRoleId,
      name: trimmedName,
      version: '1.0.0',
      kind: roleKind,
      defaults: {
        cliTool: normalizeTool(draft.tool),
        model: String(draft.model || '').trim() || defaultModelForTool(draft.tool),
        defaultNamePattern: roleKind === 'lead' ? 'team-lead' : 'agent-{n}',
      },
      instructions,
      behavioralContract,
      capabilities: normalizeCapabilities(draft.capabilities, draft.tool),
      constraints: roleKind === 'lead'
        ? {
          minInstances: 1,
          maxInstances: 1,
          requiresLeadTool: null,
          allowedProjectBinding: 'lead_project',
        }
        : {
          minInstances: 0,
          maxInstances: 8,
          requiresLeadTool: null,
          allowedProjectBinding: 'any',
        },
    }
  }

  async function submitCaptureRole() {
    const draft = captureRoleDraft
    if (!draft || !canSaveCapturedRole) return

    captureRoleDialog = {
      ...draft,
      submitting: true,
      error: '',
    }

    try {
      const payload = buildCapturedRoleTemplate(draft)
      await upsertRoleTemplate(payload)
      runtimeMessage = 'Role saved to catalog'
      closeCaptureRoleDialog()
      void loadRoleTemplates()
    } catch (error) {
      const latest = captureRoleDraft
      if (!latest) return
      captureRoleDialog = {
        ...latest,
        submitting: false,
        error: error?.message || 'Failed to save role to catalog.',
      }
    }
  }

  async function submitAddAgent() {
    const draft = addAgentDraft
    if (!draft || !canSubmitAddAgent) return

    slideOverContext = {
      ...draft,
      submitting: true,
      error: '',
    }

    try {
      const report = await coordinationAddAgent({
        teamName,
        agent: {
          name: String(draft.name || '').trim(),
          cliTool: normalizeTool(draft.tool),
          model: String(draft.model || '').trim(),
          projectId: String(draft.projectId || '').trim(),
          description: String(draft.description || '').trim() || null,
        },
      })

      onAddAgentProp(report)
      runtimeMessage = `Agent '${report?.memberName ?? String(draft.name || '').trim()}' added.`
      closeSlideOver()

      const sequence = ++discoverySequence
      await refreshRuntimeTeamConfig(teamName, sequence)
    } catch (error) {
      const latest = addAgentDraft
      if (!latest) return
      slideOverContext = {
        ...latest,
        submitting: false,
        error: error?.message || 'Failed to add agent.',
      }
    }
  }

  function requestDisband() {
    if (!teamName) return
    confirmContext = {
      kind: 'disband',
    }
  }

  function requestRemoveSelected() {
    if (!selectedNode || selectedNode.role !== 'agent') return
    confirmContext = {
      kind: 'remove',
      memberName: selectedNode.name,
    }
  }

  async function handleConfirmAction() {
    if (!confirmContext) return
    const action = confirmContext

    confirmContext = null

    if (action.kind === 'disband') {
      try {
        const result = await coordinationDisbandTeam(teamName)
        onDisbandProp(result)
        runtimeMessage = result?.alreadyDisbanded
          ? 'Team was already disbanded.'
          : 'Team disbanded and active sessions were stopped.'
        mode = 'empty'
        selectedNodeId = null
        teamConfig = null
      } catch (error) {
        errorMessage = error?.message || 'Failed to disband team.'
      }
      return
    }

    if (action.kind === 'remove' && action.memberName) {
      try {
        const report = await coordinationRemoveMember(teamName, action.memberName)
        onRemoveAgentProp(report)
        runtimeMessage = `Removed '${action.memberName}'.`
        selectedNodeId = null
        const sequence = ++discoverySequence
        await refreshRuntimeTeamConfig(teamName, sequence)
      } catch (error) {
        errorMessage = error?.message || `Failed to remove member '${action.memberName}'.`
      }
    }
  }

  async function handleResumeSelected(contextMode = 'continue') {
    if (!selectedNode || selectedNode.role !== 'agent') return
    try {
      const report = await coordinationResumeMember(
        teamName,
        selectedNode.name,
        contextMode === 'fresh' ? 'fresh' : 'continue'
      )

      if (!report?.resumed) {
        errorMessage = report?.message || `Failed to resume member '${selectedNode.name}'.`
        return
      }

      runtimeMessage = `Resumed '${selectedNode.name}'.`
      const sequence = ++discoverySequence
      await refreshRuntimeTeamConfig(teamName, sequence)
    } catch (error) {
      errorMessage = error?.message || `Failed to resume member '${selectedNode.name}'.`
    }
  }

  function handleStopSelected() {
    if (!selectedNode) return
    if (selectedNode.role === 'lead') {
      requestDisband()
      return
    }
    requestRemoveSelected()
  }

  function handleFocusSelectedPane() {
    if (!selectedNode?.paneId) return
    onFocusPaneProp(selectedNode.paneId)
  }

  function modelsForTool(tool) {
    return modelOptionsByTool[normalizeTool(tool)] ?? ['default']
  }

  function confirmDialogTitle() {
    if (!confirmContext) return 'Confirm action'
    if (confirmContext.kind === 'disband') return 'Disband team?'
    return 'Remove agent?'
  }

  function confirmDialogMessage() {
    if (!confirmContext) return ''
    if (confirmContext.kind === 'disband') {
      return `Disband team "${teamName}"? This removes mesh state and stops active agent sessions.`
    }
    return `Remove agent "${confirmContext.memberName}" from "${teamName}"?`
  }

  function confirmDialogLabel() {
    if (!confirmContext) return 'Confirm'
    return confirmContext.kind === 'disband' ? 'Disband' : 'Remove'
  }

  $effect(() => {
    const currentProjectPath = projectPath
    void currentProjectPath

    mode = 'gate'
    teamName = ''
    teamConfig = null
    slideOver = null
    slideOverContext = null
    captureRoleDialog = null
    selectedNodeId = null
    initProgress = null
    errorMessage = ''
    runtimeMessage = ''
    confirmContext = null
    gateBootstrapping = false
  })

  $effect(() => {
    if (!selectedNodeId) {
      captureRoleDialog = null
      return
    }
    if (!selectedNode) {
      selectedNodeId = null
      captureRoleDialog = null
    }
  })

  $effect(() => {
    if (runtimeMessageTimer) clearTimeout(runtimeMessageTimer)
    if (!runtimeMessage) return
    runtimeMessageTimer = setTimeout(() => {
      runtimeMessage = ''
    }, 5000)
    return () => {
      if (runtimeMessageTimer) clearTimeout(runtimeMessageTimer)
    }
  })

  $effect(() => {
    if (errorMessageTimer) clearTimeout(errorMessageTimer)
    if (!errorMessage) return
    errorMessageTimer = setTimeout(() => {
      errorMessage = ''
    }, 8000)
    return () => {
      if (errorMessageTimer) clearTimeout(errorMessageTimer)
    }
  })
</script>

<section class="flex-1 min-h-0 overflow-y-auto {t.mainBg}" data-testid="mesh-tab">
  {#snippet modeMessages()}
    {#if errorMessage}
      <div class="relative overflow-hidden border-l-2 border-danger-400 pl-3 pr-2 py-1 text-xs text-danger-600/95 flex items-center justify-between gap-2" data-testid="mesh-error">
        <span class="min-w-0">{errorMessage}</span>
        <button
          class="text-xs opacity-60 hover:opacity-100 ml-2"
          aria-label="Dismiss"
          onclick={() => {
            errorMessage = ''
          }}
          data-testid="mesh-dismiss-error-message"
        >
          ✕
        </button>
        <div class="pointer-events-none absolute bottom-0 left-0 h-0.5 bg-danger-400/50 animate-[shrink_8s_linear_forwards]" style="width: 100%"></div>
      </div>
    {/if}

    {#if runtimeMessage}
      <div class="relative overflow-hidden border-l-2 border-success-400 pl-3 pr-2 py-1 text-xs text-success-600/95 flex items-center justify-between gap-2" data-testid="mesh-runtime-message">
        <span class="min-w-0">{runtimeMessage}</span>
        <button
          class="text-xs opacity-60 hover:opacity-100 ml-2"
          aria-label="Dismiss"
          onclick={() => {
            runtimeMessage = ''
          }}
          data-testid="mesh-dismiss-runtime-message"
        >
          ✕
        </button>
        <div class="pointer-events-none absolute bottom-0 left-0 h-0.5 bg-success-400/50 animate-[shrink_5s_linear_forwards]" style="width: 100%"></div>
      </div>
    {/if}
  {/snippet}

  {#if mode === 'gate'}
    <div class="max-w-2xl mx-auto px-6 pt-4 pb-6 space-y-4">
      {@render modeMessages()}
      <div data-testid="mesh-mode-gate">
        <MeshAvailabilityGate {dark} {projectPath}>
          {#snippet children(_agentWarnings)}
            <p class="text-xs {t.textMuted}" data-testid="mesh-gate-ready" use:triggerGateReady>
              Checking project team state...
            </p>
          {/snippet}
        </MeshAvailabilityGate>
      </div>
    </div>
  {:else if mode === 'empty'}
    <div class="max-w-2xl mx-auto px-6 pt-4 pb-6 space-y-4">
      {@render modeMessages()}
      <div class="animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-empty">
        <MeshEmptyState
          {dark}
          presets={quickPresets}
          onSelectPreset={handlePresetSelect}
          onBrowseTemplates={() => {
            slideOver = 'templates'
            slideOverContext = null
          }}
          onStartCustom={handleStartCustom}
        />
      </div>
    </div>
  {:else if mode === 'setup'}
    <div class="px-4 pt-2 pb-4 space-y-3">
      {@render modeMessages()}
      <div class="space-y-3 animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-setup">
        <div data-testid="mesh-setup-canvas-frame">
          <MeshCanvas
            lead={teamConfig?.lead ?? null}
            agents={teamConfig?.agents ?? []}
            mode="setup"
            {dark}
            onNodeClick={handleNodeClick}
            onAddClick={openCustomizer}
            {selectedNodeId}
          />
        </div>

        {#if selectedNode}
          <div class="relative h-0" data-testid="mesh-node-detail-host">
            <MeshNodeDetail
              name={selectedNode.name}
              role={selectedNode.role}
              tool={selectedNode.tool}
              model={selectedNode.model}
              status={selectedNode.status}
              projectId={selectedNode.projectId}
              description={selectedNode.description || ''}
              mode="setup"
              {dark}
              onEdit={openCustomizer}
              onRemove={() => {
                if (selectedNode.role !== 'agent') return
                teamConfig = {
                  ...teamConfig,
                  agents: (teamConfig?.agents ?? []).filter((entry) => entry.id !== selectedNode.id),
                }
                selectedNodeId = null
              }}
              onClose={() => {
                selectedNodeId = null
              }}
            />
          </div>
        {/if}

        <MeshActionBar
          canInitialize={canInitialize}
          {teamName}
          {dark}
          onInitialize={handleInitialize}
          onOpenCustomizer={openCustomizer}
          onReset={handleReset}
        />
      </div>
    </div>
  {:else if mode === 'initializing'}
    <div class="px-4 pt-2 pb-4 space-y-3">
      {@render modeMessages()}
      <div class="space-y-3 animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-initializing">
        <div data-testid="mesh-initializing-canvas-frame">
          <MeshCanvas
            lead={teamConfig?.lead ?? null}
            agents={teamConfig?.agents ?? []}
            mode="initializing"
            initSteps={null}
            {dark}
            onNodeClick={() => {}}
            onAddClick={() => {}}
          />
        </div>

        <MeshInitProgress
          {dark}
          request={initProgress}
          onsuccess={handleInitializeSuccess}
          onback={handleInitializeBack}
        />
      </div>
    </div>
  {:else}
    <div class="px-4 pt-2 pb-4 space-y-3">
      {@render modeMessages()}
      <div class="space-y-3 animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-runtime">
        <MeshRuntimeBar
          teamName={teamName}
          agents={teamConfig?.agents ?? []}
          {dark}
          onAddAgent={openAddAgentPanel}
          onDisband={requestDisband}
          onOverflow={() => {}}
        />

        <div data-testid="mesh-runtime-canvas-frame">
          <MeshCanvas
            lead={teamConfig?.lead ?? null}
            agents={teamConfig?.agents ?? []}
            mode="runtime"
            {dark}
            onNodeClick={handleNodeClick}
            onAddClick={() => {}}
            {selectedNodeId}
          />
        </div>

        {#if selectedNode}
          <div class="relative h-0" data-testid="mesh-node-detail-host">
            <MeshNodeDetail
              name={selectedNode.name}
              role={selectedNode.role}
              tool={selectedNode.tool}
              model={selectedNode.model}
              status={selectedNode.status}
              projectId={selectedNode.projectId}
              description={selectedNode.description || ''}
              mode="runtime"
              {dark}
              onResume={handleResumeSelected}
              onStop={handleStopSelected}
              onFocusPane={handleFocusSelectedPane}
              onCapture={openCaptureRoleDialog}
              onClose={() => {
                selectedNodeId = null
              }}
            />
          </div>
        {/if}
      </div>
    </div>
  {/if}

  {#if slideOver === 'templates'}
    <TemplateBrowserPanel
      open={true}
      {dark}
      onClose={closeSlideOver}
      onSelectPreset={handlePresetFromBrowser}
      onSelectRole={handleRoleFromBrowser}
    />
  {/if}

  {#if slideOver === 'customizer'}
    <TeamCustomizerPanel
      open={true}
      {dark}
      {projectPath}
      {availableProjects}
      {teamConfig}
      context={slideOverContext}
      onClose={closeSlideOver}
      onSave={handleTeamSave}
      onReset={handleReset}
    />
  {/if}

  <SlideOver
    open={slideOver === 'addAgent'}
    title="Add Agent"
    width={420}
    {dark}
    onClose={closeSlideOver}
  >
    {#snippet children()}
      <section class="space-y-5 animate-in fade-in slide-in-from-bottom-1 duration-200" data-testid="mesh-add-agent-form">
        <p class="text-sm {t.textMuted} px-1">Hot-add one member to <span class="font-medium {t.textSecondary}">{teamName}</span>.</p>

        <div class="space-y-2 p-3 rounded-xl border transition-all {dark ? 'bg-brand-500/[0.03] border-brand-500/20 border-l-2 border-l-brand-500' : 'bg-brand-50/50 border-brand-200 border-l-2 border-l-brand-500'}">
          <label for="mesh-add-agent-role-select-input" class="block text-[10px] font-bold uppercase tracking-wide text-brand-500">Pick from Role (Optional)</label>
          <div class="relative">
            <select
              id="mesh-add-agent-role-select-input"
              class="h-10 w-full rounded-lg border px-3 pr-8 text-sm transition-all outline-none appearance-none {fieldTone} {selectScheme}"
              value={addAgentDraft?.roleId ?? ''}
              onchange={(event) => handleRoleChange(event.currentTarget.value)}
              disabled={addAgentDraft?.submitting || loadingRoles}
              data-testid="mesh-add-agent-role-select"
            >
              {#if loadingRoles}
                <option value="">Loading roles...</option>
              {:else}
                <option value="">Manual configuration</option>
                {#each roleTemplates as role}
                  <option value={role.roleId}>{role.name}</option>
                {/each}
              {/if}
            </select>
            <div class="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-brand-500/60">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
            </div>
          </div>
        </div>

        <div class="space-y-4">
          <div class="space-y-1.5">
            <label for="mesh-add-agent-name-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Agent Name</label>
            <input
              id="mesh-add-agent-name-input-field"
              class="h-10 w-full rounded-lg border px-3 text-base transition-all outline-none {fieldTone}"
              placeholder="e.g. backend-dev"
              value={addAgentDraft?.name ?? ''}
              oninput={(event) => updateAddAgentField('name', event.currentTarget.value)}
              disabled={addAgentDraft?.submitting}
              data-testid="mesh-add-agent-name-input"
            />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-1.5">
              <div class="flex items-center justify-between px-1">
                <label for="mesh-add-agent-tool-select-input" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted}">Tool</label>
                {#if addAgentDraft?.roleId}
                  <button
                    type="button"
                    class="h-5 w-5 flex items-center justify-center rounded-md transition-all {addAgentDraft.isLocked ? 'text-brand-500 bg-brand-500/10' : 'text-zinc-400 hover:bg-black/5 dark:hover:bg-white/5'}"
                    onclick={toggleAddAgentLock}
                    title={addAgentDraft.isLocked ? 'Unlock to edit' : 'Lock fields'}
                    data-testid="mesh-add-agent-unlock-toggle"
                  >
                    {#if addAgentDraft.isLocked}
                      <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                    {:else}
                      <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/></svg>
                    {/if}
                  </button>
                {/if}
              </div>
              <div class="relative">
                <select
                  id="mesh-add-agent-tool-select-input"
                  class="h-10 w-full rounded-lg border px-3 pr-8 text-sm transition-all outline-none appearance-none {fieldTone} {selectScheme} {addAgentDraft?.isLocked ? 'opacity-50 cursor-not-allowed' : ''}"
                  value={addAgentDraft?.tool ?? 'codex'}
                  onchange={(event) => updateAddAgentField('tool', event.currentTarget.value)}
                  disabled={addAgentDraft?.submitting || addAgentDraft?.isLocked}
                  data-testid="mesh-add-agent-tool-select"
                >
                  <option value="claude">Claude</option>
                  <option value="codex">Codex</option>
                  <option value="gemini">Gemini</option>
                </select>
                <div class="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
                  <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
                </div>
              </div>
            </div>

            <div class="space-y-1.5">
              <label for="mesh-add-agent-model-select-input" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Model</label>
              <div class="relative">
                <select
                  id="mesh-add-agent-model-select-input"
                  class="h-10 w-full rounded-lg border px-3 pr-8 text-sm transition-all outline-none appearance-none {fieldTone} {selectScheme} {addAgentDraft?.isLocked ? 'opacity-50 cursor-not-allowed' : ''}"
                  value={addAgentDraft?.model ?? defaultModelForTool(addAgentDraft?.tool ?? 'codex')}
                  onchange={(event) => updateAddAgentField('model', event.currentTarget.value)}
                  disabled={addAgentDraft?.submitting || addAgentDraft?.isLocked}
                  data-testid="mesh-add-agent-model-select"
                >
                  {#each modelsForTool(addAgentDraft?.tool ?? 'codex') as model}
                    <option value={model}>{model}</option>
                  {/each}
                </select>
                <div class="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
                  <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
                </div>
              </div>
            </div>
          </div>

          <div class="space-y-1.5">
            <label for="mesh-add-agent-project-select-input" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Project Binding</label>
            <div class="relative">
              <select
                id="mesh-add-agent-project-select-input"
                class="h-10 w-full rounded-lg border px-3 pr-8 text-sm transition-all outline-none appearance-none {fieldTone} {selectScheme}"
                value={addAgentDraft?.projectId ?? ''}
                onchange={(event) => updateAddAgentField('projectId', event.currentTarget.value)}
                disabled={addAgentDraft?.submitting}
                data-testid="mesh-add-agent-project-select"
              >
                <option value="">Select project</option>
                {#each projectOptions as project}
                  <option value={project.id}>{project.label}</option>
                {/each}
              </select>
              <div class="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
              </div>
            </div>
          </div>

          <div class="space-y-1.5">
            <label for="mesh-add-agent-description-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Description</label>
            <textarea
              id="mesh-add-agent-description-input-field"
              class="w-full rounded-lg border px-3 py-2 text-sm transition-all outline-none resize-none {fieldTone} {addAgentDraft?.isLocked ? 'opacity-50 cursor-not-allowed' : ''}"
              rows="3"
              placeholder="Specific goals for this agent..."
              value={addAgentDraft?.description ?? ''}
              oninput={(event) => updateAddAgentField('description', event.currentTarget.value)}
              disabled={addAgentDraft?.submitting || addAgentDraft?.isLocked}
              data-testid="mesh-add-agent-description-input"
            ></textarea>
          </div>
        </div>

        {#if addAgentDraft?.error}
          <div class="p-2 rounded-lg bg-danger-500/10 border border-danger-500/20 animate-in fade-in zoom-in-95 duration-200">
            <p class="text-[11px] font-medium text-danger-500 text-center" data-testid="mesh-add-agent-error">{addAgentDraft.error}</p>
          </div>
        {/if}

        <div class="flex items-center justify-end gap-3 pt-4 border-t {t.keyline}">
          <button
            class="h-10 px-4 rounded-lg text-xs font-bold transition-all active:scale-95 {dark ? 'text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]' : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100'}"
            type="button"
            onclick={closeSlideOver}
            disabled={addAgentDraft?.submitting}
            data-testid="mesh-add-agent-cancel"
          >
            Cancel
          </button>
          <button
            class="h-10 px-6 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 shadow-lg shadow-brand-500/20 disabled:opacity-50 disabled:pointer-events-none transition-all"
            type="button"
            onclick={submitAddAgent}
            disabled={!canSubmitAddAgent}
            data-testid="mesh-add-agent-submit"
          >
            {addAgentDraft?.submitting ? 'Adding...' : 'Add Agent'}
          </button>
        </div>
      </section>
    {/snippet}
  </SlideOver>

  <SlideOver
    open={Boolean(captureRoleDraft)}
    title="Capture as Role"
    width={360}
    {dark}
    onClose={closeCaptureRoleDialog}
  >
    {#snippet children()}
      <section class="space-y-4 animate-in fade-in slide-in-from-bottom-1 duration-200" data-testid="mesh-capture-role-form">
        <p class="text-sm {t.textMuted} px-1">
          Save the selected runtime member as a reusable catalog role.
        </p>

        <div class="space-y-4">
          <div class="space-y-1.5">
            <label for="mesh-capture-role-name-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">New Role Name</label>
            <input
              id="mesh-capture-role-name-input-field"
              class="h-10 w-full rounded-lg border px-3 text-base transition-all outline-none {fieldTone}"
              value={captureRoleDraft?.name ?? ''}
              oninput={(event) => updateCaptureRoleName(event.currentTarget.value)}
              disabled={captureRoleDraft?.submitting}
              data-testid="mesh-capture-role-name-input"
            />
          </div>

          <div class="space-y-1.5">
            <label for="mesh-capture-role-id-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Role ID</label>
            <input
              id="mesh-capture-role-id-input-field"
              class="h-9 w-full rounded-lg border px-3 text-sm transition-all outline-none {fieldTone}"
              value={captureRoleDraft?.roleId ?? ''}
              oninput={(event) => updateCaptureRoleId(event.currentTarget.value)}
              disabled={captureRoleDraft?.submitting}
              data-testid="mesh-capture-role-id-input"
            />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-1.5">
              <label for="mesh-capture-role-tool-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Tool</label>
              <input
                id="mesh-capture-role-tool-input-field"
                class="h-10 w-full rounded-lg border px-3 text-sm transition-all outline-none bg-black/5 dark:bg-white/5 border-transparent opacity-60"
                value={captureRoleDraft?.tool ?? ''}
                readonly
                disabled
                data-testid="mesh-capture-role-tool-input"
              />
            </div>
            <div class="space-y-1.5">
              <label for="mesh-capture-role-model-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Model</label>
              <input
                id="mesh-capture-role-model-input-field"
                class="h-10 w-full rounded-lg border px-3 text-sm transition-all outline-none bg-black/5 dark:bg-white/5 border-transparent opacity-60"
                value={captureRoleDraft?.model ?? ''}
                readonly
                disabled
                data-testid="mesh-capture-role-model-input"
              />
            </div>
          </div>

          <div class="space-y-1.5">
            <label for="mesh-capture-role-description-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Current Description</label>
            <textarea
              id="mesh-capture-role-description-input-field"
              class="w-full rounded-lg border px-3 py-2 text-sm transition-all outline-none resize-none bg-black/5 dark:bg-white/5 border-transparent opacity-60"
              rows="3"
              value={captureRoleDraft?.description ?? ''}
              readonly
              disabled
              data-testid="mesh-capture-role-description-input"
            ></textarea>
          </div>
        </div>

        <div class="space-y-2 p-3 rounded-xl border {t.keyline} {dark ? 'bg-white/[0.02]' : 'bg-brand-50/30'}">
          <label class="group flex items-center gap-3 cursor-pointer">
            <div class="relative flex items-center justify-center">
              <input
                type="checkbox"
                checked={Boolean(captureRoleDraft?.includeInstructions)}
                onchange={() => toggleCaptureRoleFlag('includeInstructions')}
                disabled={captureRoleDraft?.submitting}
                class="peer appearance-none w-4 h-4 rounded border transition-all cursor-pointer {dark ? 'bg-zinc-900 border-white/[0.1] checked:bg-brand-500 checked:border-brand-500' : 'bg-white border-brand-300 checked:bg-brand-500 checked:border-brand-500'} focus:ring-2 focus:ring-brand-500/20"
                data-testid="mesh-capture-role-include-instructions"
              />
              <svg class="absolute w-2.5 h-2.5 text-white pointer-events-none opacity-0 peer-checked:opacity-100 transition-opacity" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
            </div>
            <span class="text-xs font-medium {t.textSecondary} group-hover:text-brand-500 transition-colors">Include current instructions</span>
          </label>
          
          <label class="group flex items-center gap-3 cursor-pointer">
            <div class="relative flex items-center justify-center">
              <input
                type="checkbox"
                checked={Boolean(captureRoleDraft?.includeBehavioralContract)}
                onchange={() => toggleCaptureRoleFlag('includeBehavioralContract')}
                disabled={captureRoleDraft?.submitting}
                class="peer appearance-none w-4 h-4 rounded border transition-all cursor-pointer {dark ? 'bg-zinc-900 border-white/[0.1] checked:bg-brand-500 checked:border-brand-500' : 'bg-white border-brand-300 checked:bg-brand-500 checked:border-brand-500'} focus:ring-2 focus:ring-brand-500/20"
                data-testid="mesh-capture-role-include-behavioral-contract"
              />
              <svg class="absolute w-2.5 h-2.5 text-white pointer-events-none opacity-0 peer-checked:opacity-100 transition-opacity" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
            </div>
            <span class="text-xs font-medium {t.textSecondary} group-hover:text-brand-500 transition-colors">Include behavioral contract</span>
          </label>
        </div>

        {#if captureRoleDraft?.error}
          <div class="p-2 rounded-lg bg-danger-500/10 border border-danger-500/20 animate-in fade-in zoom-in-95 duration-200">
            <p class="text-[11px] font-medium text-danger-500 text-center" data-testid="mesh-capture-role-error">{captureRoleDraft.error}</p>
          </div>
        {/if}

        <div class="flex items-center justify-end gap-3 pt-4 border-t {t.keyline}">
          <button
            class="h-10 px-4 rounded-lg text-xs font-bold transition-all active:scale-95 {dark ? 'text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]' : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100'}"
            type="button"
            onclick={closeCaptureRoleDialog}
            disabled={captureRoleDraft?.submitting}
            data-testid="mesh-capture-role-cancel"
          >
            Cancel
          </button>
          <button
            class="h-10 px-6 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 shadow-lg shadow-brand-500/20 disabled:opacity-50 disabled:pointer-events-none transition-all"
            type="button"
            onclick={submitCaptureRole}
            disabled={!canSaveCapturedRole}
            data-testid="mesh-capture-role-save"
          >
            {captureRoleDraft?.submitting ? 'Saving...' : 'Save to Catalog'}
          </button>
        </div>
      </section>
    {/snippet}
  </SlideOver>

  {#if confirmContext}
    <ConfirmDialog
      {dark}
      open={true}
      title={confirmDialogTitle()}
      message={confirmDialogMessage()}
      confirmLabel={confirmDialogLabel()}
      variant="danger"
      onconfirm={handleConfirmAction}
      oncancel={() => {
        confirmContext = null
      }}
    />
  {/if}
</section>

<style>
  @keyframes shrink {
    from {
      width: 100%;
    }
    to {
      width: 0%;
    }
  }

  @keyframes meshfade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  /* Custom select arrow removal for Safari/Chrome */
  select {
    -webkit-appearance: none;
    -moz-appearance: none;
    appearance: none;
  }
</style>
