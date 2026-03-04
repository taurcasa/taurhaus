<script>
  import {
    coordinationAddAgent,
    coordinationDisbandTeam,
    coordinationGetLiveTeamStatus,
    coordinationListTeams,
    coordinationRemoveMember,
    coordinationResumeMember,
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
  const actionSecondary = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800/80'
      : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'
  )
  const fieldTone = $derived(
    dark
      ? 'border-zinc-700/80 bg-zinc-900 text-zinc-100 placeholder:text-zinc-600 focus:border-brand-500'
      : 'border-zinc-300 bg-white text-zinc-900 placeholder:text-zinc-400 focus:border-brand-500'
  )
  const selectScheme = $derived(dark ? '[color-scheme:dark]' : '[color-scheme:light]')
  const chevronSvg = $derived(
    dark
      ? `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 10 10'%3E%3Cpath d='M3 4l2 2 2-2' fill='none' stroke='%2371717a' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")`
      : `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 10 10'%3E%3Cpath d='M3 4l2 2 2-2' fill='none' stroke='%2352525b' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")`
  )

  const modelOptionsByTool = {
    claude: ['opus', 'sonnet', 'haiku'],
    codex: ['gpt-5.3-codex', 'gpt-5-mini'],
    gemini: ['gemini-2.5-pro', 'gemini-2.0-flash'],
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
    slideOver = 'addAgent'
    slideOverContext = {
      name: '',
      tool: 'codex',
      model: defaultModelForTool('codex'),
      projectId: defaultProject,
      description: '',
      submitting: false,
      error: '',
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
    selectedNodeId = null
    initProgress = null
    errorMessage = ''
    runtimeMessage = ''
    confirmContext = null
    gateBootstrapping = false
  })

  $effect(() => {
    if (!selectedNodeId) return
    if (!selectedNode) {
      selectedNodeId = null
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
  <div class="max-w-3xl px-7 pt-4 pb-6 space-y-4">
    {#if errorMessage}
      <div class="relative overflow-hidden border-l-2 border-danger-400 pl-3 pr-2 py-1 text-xs text-danger-600/95 flex items-center justify-between gap-2" data-testid="mesh-error">
        <span class="min-w-0">{errorMessage}</span>
        <button
          class="text-xs opacity-60 hover:opacity-100 ml-2"
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

    {#if mode === 'gate'}
      <div data-testid="mesh-mode-gate">
        <MeshAvailabilityGate {dark} {projectPath}>
          {#snippet children(_agentWarnings)}
            <p class="text-xs {t.textMuted}" data-testid="mesh-gate-ready" use:triggerGateReady>
              Checking project team state...
            </p>
          {/snippet}
        </MeshAvailabilityGate>
      </div>
    {:else if mode === 'empty'}
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
    {:else if mode === 'setup'}
      <div class="space-y-3 animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-setup">
        <div class="rounded-lg border {t.keyline} p-3 min-h-[320px]" data-testid="mesh-setup-canvas-frame">
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
    {:else if mode === 'initializing'}
      <div class="space-y-3 animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-initializing">
        <div class="rounded-lg border {t.keyline} p-3 min-h-[320px]" data-testid="mesh-initializing-canvas-frame">
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
    {:else}
      <div class="space-y-3 animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-runtime">
        <div class="rounded-lg border {t.keyline} p-3 min-h-[320px]" data-testid="mesh-runtime-canvas-frame">
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

        <MeshRuntimeBar
          teamName={teamName}
          agents={teamConfig?.agents ?? []}
          {dark}
          onAddAgent={openAddAgentPanel}
          onDisband={requestDisband}
          onOverflow={() => {}}
        />

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
              onClose={() => {
                selectedNodeId = null
              }}
            />
          </div>
        {/if}
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
        <section class="space-y-3" data-testid="mesh-add-agent-form">
          <p class="text-xs {t.textMuted}">Hot-add one member to <span class="font-medium {t.textSecondary}">{teamName}</span>.</p>

          <input
            class="w-full rounded-md border px-2 py-1.5 text-sm transition-colors focus:outline-none {fieldTone}"
            placeholder="Agent name"
            value={addAgentDraft?.name ?? ''}
            oninput={(event) => updateAddAgentField('name', event.currentTarget.value)}
            data-testid="mesh-add-agent-name-input"
          />

          <select
            class="h-8 w-full rounded-md border px-2 pr-6 text-xs transition-colors focus:outline-none {fieldTone} {selectScheme}"
            style:background-image={chevronSvg}
            style:background-repeat="no-repeat"
            style:background-position="right 6px center"
            value={addAgentDraft?.tool ?? 'codex'}
            onchange={(event) => updateAddAgentField('tool', event.currentTarget.value)}
            data-testid="mesh-add-agent-tool-select"
          >
            <option value="claude">Claude</option>
            <option value="codex">Codex</option>
            <option value="gemini">Gemini</option>
          </select>

          <select
            class="h-8 w-full rounded-md border px-2 pr-6 text-xs transition-colors focus:outline-none {fieldTone} {selectScheme}"
            style:background-image={chevronSvg}
            style:background-repeat="no-repeat"
            style:background-position="right 6px center"
            value={addAgentDraft?.model ?? defaultModelForTool(addAgentDraft?.tool ?? 'codex')}
            onchange={(event) => updateAddAgentField('model', event.currentTarget.value)}
            data-testid="mesh-add-agent-model-select"
          >
            {#each modelsForTool(addAgentDraft?.tool ?? 'codex') as model}
              <option value={model}>{model}</option>
            {/each}
          </select>

          <select
            class="h-8 w-full rounded-md border px-2 pr-6 text-xs transition-colors focus:outline-none {fieldTone} {selectScheme}"
            style:background-image={chevronSvg}
            style:background-repeat="no-repeat"
            style:background-position="right 6px center"
            value={addAgentDraft?.projectId ?? ''}
            onchange={(event) => updateAddAgentField('projectId', event.currentTarget.value)}
            data-testid="mesh-add-agent-project-select"
          >
            <option value="">Select project</option>
            {#each projectOptions as project}
              <option value={project.id}>{project.label}</option>
            {/each}
          </select>

          <input
            class="w-full rounded-md border px-2 py-1.5 text-sm transition-colors focus:outline-none {fieldTone}"
            placeholder="Description (optional)"
            value={addAgentDraft?.description ?? ''}
            oninput={(event) => updateAddAgentField('description', event.currentTarget.value)}
            data-testid="mesh-add-agent-description-input"
          />

          {#if addAgentDraft?.error}
            <p class="text-xs text-danger-500" data-testid="mesh-add-agent-error">{addAgentDraft.error}</p>
          {/if}

          <div class="flex items-center justify-end gap-2">
            <button
              class="rounded-md border px-2 py-1 text-xs {actionSecondary}"
              type="button"
              onclick={closeSlideOver}
              disabled={addAgentDraft?.submitting}
              data-testid="mesh-add-agent-cancel"
            >
              Cancel
            </button>
            <button
              class="rounded-md bg-brand-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50"
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
  </div>
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
</style>
