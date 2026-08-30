<script>
  import { activityLevel } from '../activitySignal.js'
  import { runTreeDescriptor, workflowSessionId } from '../workflowRuns.js'
  import {
    isWorkflowRunCollapsed,
    toggleWorkflowRun,
    watchWorkflowSession,
    workflowSessionRuns,
  } from '../workflowRunStore.svelte.js'
  import MeshConnection from './MeshConnection.svelte'
  import { computeMeshLayout } from './meshLayout.js'
  import MeshNode from './MeshNode.svelte'
  import MeshNodeRoleCard from './MeshNodeRoleCard.svelte'
  import WorkflowRunTree from './WorkflowRunTree.svelte'

  let {
    lead = null,
    agents = [],
    mode = 'setup',
    initSteps = null,
    dark = false,
    onNodeClick = (id) => {},
    onAddClick = () => {},
    onDetailAnchorChange = (_anchor) => {},
    onDismissDetail = () => {},
    selectedNodeId = null,
  } = $props()

  let containerWidth = $state(0)
  let containerHeight = $state(0)
  let canvasElement = $state(null)
  let emittedAnchorSignature = $state('')
  const connectionGlowFilterId = `mesh-connection-glow-${Math.random().toString(36).slice(2, 9)}`
  const detailWidth = 240
  const detailMinWidth = 176
  const detailEstimatedHeight = 224
  const detailGap = 12
  const detailMargin = 8
  const hoverWidth = 224
  const hoverMinWidth = 192
  const hoverEstimatedHeight = 170
  const hoverDelayMs = 200
  let hoverNodeId = $state(null)
  let hoverAnchor = $state(null)
  let hoverTimer = $state(null)

  function clamp(value, min, max) {
    if (max < min) return min
    return Math.min(Math.max(value, min), max)
  }

  function emitDetailAnchor(anchor) {
    const nextSignature = anchor
      ? [
        Math.round(anchor.left),
        Math.round(anchor.top),
        anchor.placement,
        Math.round(anchor.cardWidth),
      ].join(':')
      : 'none'
    if (nextSignature === emittedAnchorSignature) return
    emittedAnchorSignature = nextSignature
    onDetailAnchorChange(anchor)
  }

  function findSelectedNodeElement(nodeId) {
    if (!canvasElement || nodeId === null || nodeId === undefined) return null
    const nodes = canvasElement.querySelectorAll('[data-node-id]')
    for (const node of nodes) {
      if (String(node.getAttribute('data-node-id') || '') === String(nodeId)) {
        return node
      }
    }
    return null
  }

  function calculateFloatingAnchor(nodeId, options = {}) {
    if (!nodeId || !canvasElement) return null
    const selectedElement = findSelectedNodeElement(nodeId)
    if (!selectedElement) return null

    const {
      width = detailWidth,
      minWidth = detailMinWidth,
      estimatedHeight = detailEstimatedHeight,
      gap = detailGap,
      margin = detailMargin,
    } = options
    const canvasRect = canvasElement.getBoundingClientRect()
    const nodeRect = selectedElement.getBoundingClientRect()
    const canvasW = Math.max(0, Math.round(canvasRect.width || containerWidth || 600))
    const canvasH = Math.max(0, Math.round(canvasRect.height || canvasHeight || 460))
    const availableWidth = Math.max(minWidth, canvasW - margin * 2)
    const cardWidth = Math.min(width, availableWidth)
    const fallbackCenterX = Number(selectedElement.getAttribute('data-center-x') || '0')
    const fallbackCenterY = Number(selectedElement.getAttribute('data-center-y') || '0')
    const fallbackNodeHeight = Number(selectedElement.getAttribute('data-node-height') || '64')
    const hasMeasuredNodeRect = nodeRect.width > 0 && nodeRect.height > 0
    const centerX = hasMeasuredNodeRect
      ? nodeRect.left - canvasRect.left + nodeRect.width / 2
      : fallbackCenterX
    const centerY = hasMeasuredNodeRect
      ? nodeRect.top - canvasRect.top + nodeRect.height / 2
      : fallbackCenterY
    const nodeHeight = hasMeasuredNodeRect ? nodeRect.height : fallbackNodeHeight
    const nodeTop = centerY - nodeHeight / 2
    const nodeBottom = centerY + nodeHeight / 2

    const preferredTop = nodeTop - gap - estimatedHeight
    let placement = 'top'
    let top = preferredTop
    if (preferredTop < margin) {
      placement = 'bottom'
      top = nodeBottom + gap
    }

    const maxLeft = Math.max(margin, canvasW - cardWidth - margin)
    const left = clamp(centerX - cardWidth / 2, margin, maxLeft)
    const maxTop = Math.max(margin, canvasH - estimatedHeight - margin)
    const clampedTop = clamp(top, margin, maxTop)

    return {
      left,
      top: clampedTop,
      placement,
      cardWidth,
    }
  }

  function calculateDetailAnchor() {
    return calculateFloatingAnchor(selectedNodeId, {
      width: detailWidth,
      minWidth: detailMinWidth,
      estimatedHeight: detailEstimatedHeight,
      gap: detailGap,
      margin: detailMargin,
    })
  }

  function calculateHoverAnchor(nodeId) {
    return calculateFloatingAnchor(nodeId, {
      width: hoverWidth,
      minWidth: hoverMinWidth,
      estimatedHeight: hoverEstimatedHeight,
      gap: detailGap,
      margin: detailMargin,
    })
  }

  function clearHoverTimer() {
    if (hoverTimer) {
      clearTimeout(hoverTimer)
      hoverTimer = null
    }
  }

  function hoverSuppressed() {
    if (normalizedMode !== 'runtime') return true
    if (selectedNodeId) return true
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false
    return window.matchMedia('(pointer: coarse)').matches || window.matchMedia('(hover: none)').matches
  }

  function findLayoutNode(nodeId) {
    if (!nodeId) return null
    if (layout.lead && String(layout.lead.id) === String(nodeId)) return layout.lead
    return layout.agents.find((agent) => String(agent.id) === String(nodeId)) ?? null
  }

  function dismissHoverCard() {
    clearHoverTimer()
    hoverNodeId = null
    hoverAnchor = null
  }

  function scheduleHoverCard(nodeId) {
    if (!nodeId || hoverSuppressed()) return
    const node = findLayoutNode(nodeId)
    if (!node) return

    clearHoverTimer()
    hoverTimer = setTimeout(() => {
      if (hoverSuppressed()) return
      const activeNode = findLayoutNode(nodeId)
      if (!activeNode) return
      hoverNodeId = String(nodeId)
      hoverAnchor = calculateHoverAnchor(nodeId)
      hoverTimer = null
    }, hoverDelayMs)
  }

  function refreshHoverAnchor() {
    if (!hoverNodeId) return
    hoverAnchor = calculateHoverAnchor(hoverNodeId)
  }

  function refreshDetailAnchor() {
    emitDetailAnchor(calculateDetailAnchor())
  }

  function isSelected(nodeId) {
    if (selectedNodeId === null || selectedNodeId === undefined) return false
    return String(selectedNodeId) === String(nodeId)
  }

  const normalizedMode = $derived.by(() => {
    const value = String(mode || '').toLowerCase()
    if (value === 'runtime' || value === 'initializing') return value
    return 'setup'
  })

  function parseInitStepState(steps, agentIds) {
    const completedIds = new Set()
    let activeId = null

    const useId = (value) => {
      const id = value === null || value === undefined ? '' : String(value).trim()
      return id.length > 0 ? id : null
    }

    const addCompleted = (value) => {
      const id = useId(value)
      if (id) completedIds.add(id)
    }

    const setActive = (value) => {
      const id = useId(value)
      if (id) activeId = id
    }

    if (Array.isArray(steps)) {
      for (const entry of steps) {
        if (entry && typeof entry === 'object') {
          const status = String(entry.status ?? '').toLowerCase()
          if (status === 'succeeded' || status === 'complete' || status === 'completed') {
            addCompleted(entry.id)
          } else if (status === 'running' || status === 'active' || status === 'initializing') {
            setActive(entry.id)
          }
          continue
        }
        addCompleted(entry)
      }
    } else if (steps && typeof steps === 'object') {
      for (const entry of steps.completedIds ?? steps.completed ?? steps.initialized ?? []) {
        addCompleted(entry)
      }
      setActive(steps.activeId ?? steps.currentId ?? steps.initializingId)

      const activeIndex = Number(steps.activeIndex)
      if (!activeId && Number.isInteger(activeIndex) && activeIndex >= 0) {
        setActive(agentIds[activeIndex])
      }

      const completedCount = Number(steps.completedCount)
      if (Number.isInteger(completedCount) && completedCount > 0) {
        for (const id of agentIds.slice(0, completedCount)) {
          addCompleted(id)
        }
      }
    }

    if (!activeId && completedIds.size < agentIds.length) {
      activeId = agentIds[completedIds.size] ?? null
    }

    return { completedIds, activeId }
  }

  const initState = $derived.by(() => {
    const ids = Array.isArray(agents)
      ? agents.map((agent, index) => String(agent?.id ?? agent?.name ?? `agent-${index}`))
      : []
    return parseInitStepState(initSteps, ids)
  })

  const normalizedLead = $derived.by(() => {
    if (!lead) return null

    let status = activityLevel(lead)
    if (normalizedMode === 'initializing') {
      const started = Boolean(initState.activeId) || initState.completedIds.size > 0
      const allReady = Array.isArray(agents) && agents.length > 0 && initState.completedIds.size >= agents.length
      status = allReady ? 'active' : (started ? 'idle' : 'offline')
    }

    return {
      ...lead,
      id: String(lead.id ?? 'lead'),
      tool: lead.tool ?? lead.cliTool ?? lead.cli_tool,
      model: String(lead.model ?? lead.modelName ?? lead.model_name ?? '').trim(),
      status,
    }
  })

  const normalizedAgents = $derived.by(() => {
    if (!Array.isArray(agents)) return []

    return agents.map((agent, index) => {
      const id = String(agent?.id ?? agent?.name ?? `agent-${index}`)

      if (normalizedMode === 'initializing') {
        const status = initState.completedIds.has(id)
          ? 'active'
          : (initState.activeId === id ? 'idle' : 'offline')
        return {
          ...agent,
          id,
          tool: agent?.tool ?? agent?.cliTool ?? agent?.cli_tool,
          model: String(agent?.model ?? agent?.modelName ?? agent?.model_name ?? '').trim(),
          status,
        }
      }

      return {
        ...agent,
        id,
        tool: agent?.tool ?? agent?.cliTool ?? agent?.cli_tool,
        model: String(agent?.model ?? agent?.modelName ?? agent?.model_name ?? '').trim(),
        status: activityLevel(agent),
      }
    })
  })

  /**
   * The workflow runs one node shows.
   *
   * A caller can hand a node its runs directly (`workflowRuns`) — that is how a
   * fixture draws a tree without a backend. Otherwise the runs come from the
   * shared store, which follows the node's Claude session while this canvas is
   * mounted. A node with neither shows no tree and costs nothing.
   */
  function nodeWorkflow(member) {
    const supplied = Array.isArray(member?.workflowRuns) ? member.workflowRuns : null
    if (supplied) {
      return { sessionId: '', runs: supplied, collapsedRunIds: [] }
    }

    const sessionId = workflowSessionId(member)
    if (!sessionId) return { sessionId: '', runs: [], collapsedRunIds: [] }

    const runs = workflowSessionRuns(sessionId).runs
    return {
      sessionId,
      runs,
      collapsedRunIds: runs
        .map((run) => String(run?.run_id ?? run?.runId ?? ''))
        .filter((runId) => runId && isWorkflowRunCollapsed(sessionId, runId)),
    }
  }

  const workflowByNodeId = $derived.by(() => {
    const byNodeId = new Map()
    for (const member of [normalizedLead, ...normalizedAgents]) {
      if (!member) continue
      byNodeId.set(String(member.id), nodeWorkflow(member))
    }
    return byNodeId
  })

  const watchedSessionIds = $derived.by(() => {
    const ids = []
    for (const workflow of workflowByNodeId.values()) {
      if (workflow.sessionId && !ids.includes(workflow.sessionId)) ids.push(workflow.sessionId)
    }
    return ids
  })

  // One subscription per session on the canvas, released when it leaves. The
  // store owns the single poll timer; nodes never get one of their own.
  //
  // Reconciled against a key rather than torn down on every effect run: the
  // runtime canvas re-renders its members on each team-status refresh, and a
  // fresh array carrying the same sessions would otherwise cost a re-watch —
  // and a `list_workflow_runs` — every time.
  let watchedKey = ''
  let watchStops = []

  function releaseWorkflowWatches() {
    for (const stop of watchStops) stop()
    watchStops = []
    watchedKey = ''
  }

  $effect(() => {
    const key = watchedSessionIds.join('\u0000')
    if (key === watchedKey) return
    releaseWorkflowWatches()
    watchedKey = key
    watchStops = watchedSessionIds.map((sessionId) => watchWorkflowSession(sessionId))
  })

  $effect(() => releaseWorkflowWatches)

  function withRunTree(member) {
    const workflow = workflowByNodeId.get(String(member.id))
    return {
      ...member,
      runTree: runTreeDescriptor(workflow?.runs, workflow?.collapsedRunIds),
    }
  }

  function handleToggleRun(nodeId, runId) {
    const workflow = workflowByNodeId.get(String(nodeId))
    if (!workflow?.sessionId) return
    toggleWorkflowRun(workflow.sessionId, runId)
  }

  const layout = $derived.by(() => {
    const leadData = normalizedLead
    if (!leadData) return { lead: null, agents: [], connections: [], addNode: null }

    const computed = computeMeshLayout({
      width: containerWidth || 600,
      height: Math.max(460, containerHeight || 0),
      mode: normalizedMode,
      lead: withRunTree(leadData),
      agents: normalizedAgents.map(withRunTree),
    })

    return {
      lead: computed.lead
        ? {
          ...computed.lead,
          position: {
            x: computed.lead.x,
            y: computed.lead.y,
          },
        }
        : null,
      agents: computed.agents.map((agent) => ({
        ...agent,
        position: {
          x: agent.x,
          y: agent.y,
        },
      })),
      connections: computed.connections.map((connection, index) => ({
        ...connection,
        isCrossProject: Boolean(
          computed.agents.find((agent) => agent.id === connection.toId)?.isCrossProject
        ),
        status: normalizedMode === 'runtime'
          ? computed.agents.find((agent) => agent.id === connection.toId)?.status ?? 'offline'
          : normalizedMode,
        delay: normalizedMode === 'initializing' ? index * 200 : 0,
        duration: normalizedMode === 'initializing' ? 400 : 0,
      })),
      addNode: computed.addNode,
    }
  })

  const canvasHeight = $derived.by(() => {
    const minHeight = Math.max(460, containerHeight || 0)
    const current = layout
    if (!current.lead) return minHeight

    let maxY = current.lead.position.y + 72
    for (const agent of current.agents) {
      maxY = Math.max(maxY, agent.position.y + 64)
    }

    for (const node of [current.lead, ...current.agents]) {
      if (!node.runTree) continue
      maxY = Math.max(maxY, node.runTree.top + node.runTree.height)
    }

    if (current.addNode) {
      maxY = Math.max(maxY, current.addNode.y + 60)
    }

    return Math.max(minHeight, Math.ceil(maxY + 20))
  })

  $effect(() => {
    const currentNodeId = selectedNodeId
    void layout
    void containerWidth
    void containerHeight
    void canvasHeight

    if (!currentNodeId) {
      emitDetailAnchor(null)
      return
    }
    refreshDetailAnchor()
  })

  $effect(() => {
    if (!canvasElement || (!selectedNodeId && !hoverNodeId)) return

    let frame = 0
    const schedule = () => {
      if (frame) cancelAnimationFrame(frame)
      frame = requestAnimationFrame(() => {
        refreshDetailAnchor()
        refreshHoverAnchor()
      })
    }

    const observer = new ResizeObserver(schedule)
    observer.observe(canvasElement)

    window.addEventListener('resize', schedule)
    window.addEventListener('scroll', schedule, true)
    schedule()

    return () => {
      if (frame) cancelAnimationFrame(frame)
      observer.disconnect()
      window.removeEventListener('resize', schedule)
      window.removeEventListener('scroll', schedule, true)
    }
  })

  $effect(() => {
    if (!selectedNodeId) return

    const onKeyDown = (event) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      onDismissDetail()
    }

    const onPointerDown = (event) => {
      const target = event.target
      if (!(target instanceof Element)) return
      if (target.closest('[data-testid="mesh-node-detail"]')) return
      if (target.closest('[data-node-id]')) return
      onDismissDetail()
    }

    window.addEventListener('keydown', onKeyDown)
    document.addEventListener('pointerdown', onPointerDown)

    return () => {
      window.removeEventListener('keydown', onKeyDown)
      document.removeEventListener('pointerdown', onPointerDown)
    }
  })

  $effect(() => {
    void selectedNodeId
    void normalizedMode
    if (!hoverSuppressed()) return
    dismissHoverCard()
  })
</script>

<div
  class="mesh-canvas"
  class:is-light={!dark}
  bind:this={canvasElement}
  bind:clientWidth={containerWidth}
  bind:clientHeight={containerHeight}
  data-testid="mesh-canvas"
  style={`min-height: ${canvasHeight}px;`}
>
  {#if layout.lead}
    <svg
      class="mesh-canvas-connections"
      width="100%"
      height={canvasHeight}
      viewBox={`0 0 ${containerWidth || 600} ${canvasHeight}`}
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <defs>
        <filter id={connectionGlowFilterId} x="-20%" y="-20%" width="140%" height="140%">
          <feGaussianBlur stdDeviation="1.35" result="blur"></feGaussianBlur>
          <feMerge>
            <feMergeNode in="blur"></feMergeNode>
            <feMergeNode in="SourceGraphic"></feMergeNode>
          </feMerge>
        </filter>
      </defs>

      {#each layout.connections as connection (connection.id)}
        <MeshConnection
          start={connection.start}
          end={connection.end}
          control1={connection.control1}
          control2={connection.control2}
          isCrossProject={connection.isCrossProject}
          status={connection.status}
          delay={connection.delay}
          duration={connection.duration}
          glowFilterId={connectionGlowFilterId}
          {dark}
        />
      {/each}
    </svg>

    <div class="mesh-canvas-nodes">
      <MeshNode
        nodeId={layout.lead.id}
        name={layout.lead.name}
        role="lead"
        roleName={layout.lead.roleName}
        focusArea={layout.lead.focusArea}
        contextSummary={layout.lead.contextSummary}
        behaviorSummary={layout.lead.behaviorSummary}
        tool={layout.lead.tool}
        model={layout.lead.model}
        reasoningEffort={layout.lead.reasoningEffort}
        taskEffort={layout.lead.taskEffort}
        taskEffortWhy={layout.lead.taskEffortWhy}
        accountApplied={layout.lead.accountApplied}
        accountNote={layout.lead.accountNote}
        accountNoteDetail={layout.lead.accountNoteDetail}
        status={layout.lead.status}
        isCrossProject={layout.lead.isCrossProject}
        projectLabel={layout.lead.projectLabel}
        selected={isSelected(layout.lead.id)}
        position={layout.lead.position}
        width={layout.lead.width}
        height={layout.lead.height}
        {dark}
        onClick={() => onNodeClick(layout.lead.id)}
        onHoverStart={() => scheduleHoverCard(layout.lead.id)}
        onHoverEnd={dismissHoverCard}
      />

      {#each layout.agents as agent (agent.id)}
        <MeshNode
          nodeId={agent.id}
          name={agent.name}
          role="agent"
          roleName={agent.roleName}
          focusArea={agent.focusArea}
          contextSummary={agent.contextSummary}
          behaviorSummary={agent.behaviorSummary}
          tool={agent.tool}
          model={agent.model}
          reasoningEffort={agent.reasoningEffort}
          taskEffort={agent.taskEffort}
          taskEffortWhy={agent.taskEffortWhy}
          accountApplied={agent.accountApplied}
          accountNote={agent.accountNote}
          accountNoteDetail={agent.accountNoteDetail}
          status={agent.status}
          isCrossProject={agent.isCrossProject}
          projectLabel={agent.projectLabel}
          selected={isSelected(agent.id)}
          position={agent.position}
          width={agent.width}
          height={agent.height}
          {dark}
          onClick={() => onNodeClick(agent.id)}
          onHoverStart={() => scheduleHoverCard(agent.id)}
          onHoverEnd={dismissHoverCard}
        />
      {/each}

      {#each [layout.lead, ...layout.agents] as node (`run-tree-${node.id}`)}
        {#if node.runTree}
          <WorkflowRunTree
            box={node.runTree}
            runs={workflowByNodeId.get(String(node.id))?.runs ?? []}
            collapsedRunIds={workflowByNodeId.get(String(node.id))?.collapsedRunIds ?? []}
            {dark}
            onToggleRun={(runId) => handleToggleRun(node.id, runId)}
          />
        {/if}
      {/each}

      {#if layout.addNode}
        <button
          type="button"
          class="mesh-add-node"
          data-testid="mesh-add-node"
          style={`left: ${layout.addNode.x - 24}px; top: ${layout.addNode.y - 24}px;`}
          onclick={onAddClick}
          aria-label="Add agent"
        >
          +
        </button>
      {/if}

      {#if hoverNodeId && hoverAnchor && !selectedNodeId}
        {@const hoverNode = findLayoutNode(hoverNodeId)}
        {#if hoverNode}
          <div class="pointer-events-none absolute inset-0 z-20" data-testid="mesh-node-role-card-host">
            <MeshNodeRoleCard
              node={hoverNode}
              {dark}
              anchor={hoverAnchor}
            />
          </div>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .mesh-canvas {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 460px;
    border-radius: 12px;
    border: 1px solid var(--mesh-canvas-border-dark);
    background-color: var(--mesh-canvas-bg-dark);
    background-image:
      radial-gradient(ellipse at 50% 40%, rgba(13, 148, 136, 0.06) 0%, transparent 70%),
      radial-gradient(circle, rgba(255, 255, 255, 0.03) 1px, transparent 1px);
    background-size: auto, 20px 20px;
    box-shadow: var(--mesh-canvas-shadow);
    overflow: hidden;
  }

  .mesh-canvas.is-light {
    background-color: var(--mesh-canvas-bg-light);
    background-image:
      radial-gradient(circle at 1px 1px, rgba(13, 148, 136, 0.08) 1px, transparent 0),
      linear-gradient(180deg, rgba(255, 255, 255, 0.95) 0%, rgba(240, 249, 247, 0.9) 100%);
    background-size: 16px 16px, 100% 100%;
    border-radius: 12px;
    border: 1px solid var(--mesh-canvas-border-light);
    box-shadow: var(--mesh-canvas-shadow-light);
  }

  .mesh-canvas-connections {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: visible;
  }

  .mesh-canvas-nodes {
    position: absolute;
    inset: 0;
  }

  .mesh-add-node {
    position: absolute;
    width: 48px;
    height: 48px;
    border-radius: 14px;
    border: 2px dashed var(--mesh-add-border);
    color: var(--mesh-add-icon);
    font-size: 18px;
    font-weight: 500;
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: rgba(45, 212, 191, 0.02);
    transition: border-color 150ms ease-out, background-color 150ms ease-out, transform 150ms ease-out;
    cursor: pointer;
  }

  .mesh-add-node:hover {
    border-color: var(--mesh-add-border-hover);
    background: rgba(45, 212, 191, 0.05);
    transform: translateY(-1px);
  }
</style>
