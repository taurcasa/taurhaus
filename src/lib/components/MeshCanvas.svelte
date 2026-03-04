<script>
  import MeshConnection from './MeshConnection.svelte'
  import MeshNode from './MeshNode.svelte'

  let {
    lead = null,
    agents = [],
    mode = 'setup',
    initSteps = null,
    dark = false,
    onNodeClick = (id) => {},
    onAddClick = () => {},
    selectedNodeId = null,
  } = $props()

  let containerWidth = $state(0)
  let containerHeight = $state(0)
  const connectionGlowFilterId = `mesh-connection-glow-${Math.random().toString(36).slice(2, 9)}`

  function normalizeStatus(status) {
    const value = String(status || '').toLowerCase()
    if (value === 'active' || value === 'idle') return value
    return 'offline'
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

    let status = normalizeStatus(lead.status)
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
        status: normalizeStatus(agent?.status),
      }
    })
  })

  function buildRow(items, startIndex, rowCount, y, cw, nodeW, gap) {
    if (rowCount <= 0) return []
    const rowItems = items.slice(startIndex, startIndex + rowCount)
    const totalW = rowCount * nodeW + (rowCount - 1) * gap
    const startX = (cw - totalW) / 2

    return rowItems.map((agent, i) => ({
      ...agent,
      position: {
        x: startX + i * (nodeW + gap) + nodeW / 2,
        y,
      },
      width: nodeW,
    }))
  }

  const layout = $derived.by(() => {
    const cw = containerWidth || 600
    const ch = Math.max(420, containerHeight || 0)
    const leadData = normalizedLead
    if (!leadData) return { lead: null, agents: [], connections: [], addNode: null }

    const members = normalizedAgents
    const count = members.length
    const gap = 24
    const nodeW = count <= 3 ? 140 : (count <= 5 ? 130 : 118)
    const leadPos = { x: cw / 2, y: Math.round(ch * 0.3) }
    const primaryAgentY = Math.round(ch * 0.65)

    let positionedAgents = []

    if (count >= 7) {
      const row1Count = Math.ceil(count / 2)
      const row2Count = count - row1Count
      const rowOffset = 44
      positionedAgents = [
        ...buildRow(members, 0, row1Count, primaryAgentY - rowOffset, cw, nodeW, gap),
        ...buildRow(members, row1Count, row2Count, primaryAgentY + rowOffset, cw, nodeW, gap),
      ]
    } else if (count > 0) {
      const totalW = count * nodeW + (count - 1) * gap
      const startX = (cw - totalW) / 2

      positionedAgents = members.map((agent, i) => ({
        ...agent,
        position: {
          x: startX + i * (nodeW + gap) + nodeW / 2,
          y: primaryAgentY,
        },
        width: nodeW,
      }))
    }

    const connections = positionedAgents.map((agent, index) => ({
      id: agent.id,
      from: leadPos,
      to: agent.position,
      status: normalizedMode === 'runtime' ? agent.status : normalizedMode,
      delay: normalizedMode === 'initializing' ? index * 200 : 0,
      duration: normalizedMode === 'initializing' ? 400 : 0,
    }))

    const lastAgent = positionedAgents[positionedAgents.length - 1] ?? null
    const addNode = normalizedMode === 'setup'
      ? (lastAgent
        ? {
          x: lastAgent.position.x + nodeW / 2 + gap + 24,
          y: lastAgent.position.y,
        }
        : {
          x: leadPos.x,
          y: primaryAgentY,
        })
      : null

    return {
      lead: {
        ...leadData,
        position: leadPos,
        width: Math.max(140, nodeW),
      },
      agents: positionedAgents,
      connections,
      addNode,
    }
  })

  const canvasHeight = $derived.by(() => {
    const minHeight = Math.max(420, containerHeight || 0)
    const current = layout
    if (!current.lead) return minHeight

    let maxY = current.lead.position.y + 60
    for (const agent of current.agents) {
      maxY = Math.max(maxY, agent.position.y + 70)
    }

    if (current.addNode) {
      maxY = Math.max(maxY, current.addNode.y + 60)
    }

    return Math.max(minHeight, Math.ceil(maxY + 20))
  })
</script>

<div
  class="mesh-canvas"
  class:is-light={!dark}
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
          from={connection.from}
          to={connection.to}
          status={connection.status}
          delay={connection.delay}
          duration={connection.duration}
          glowFilterId={connectionGlowFilterId}
          nodeHeight={58}
          {dark}
        />
      {/each}
    </svg>

    <div class="mesh-canvas-nodes">
      <MeshNode
        name={layout.lead.name}
        role="lead"
        tool={layout.lead.tool}
        model={layout.lead.model}
        status={layout.lead.status}
        selected={isSelected(layout.lead.id)}
        position={layout.lead.position}
        width={layout.lead.width}
        {dark}
        onClick={() => onNodeClick(layout.lead.id)}
      />

      {#each layout.agents as agent (agent.id)}
        <MeshNode
          name={agent.name}
          role="agent"
          tool={agent.tool}
          model={agent.model}
          status={agent.status}
          selected={isSelected(agent.id)}
          position={agent.position}
          width={agent.width}
          {dark}
          onClick={() => onNodeClick(agent.id)}
        />
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
    </div>
  {/if}
</div>

<style>
  .mesh-canvas {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 420px;
  }

  .mesh-canvas.is-light {
    background-color: #f8fcfb;
    background-image:
      radial-gradient(circle at 1px 1px, rgba(13, 148, 136, 0.08) 1px, transparent 0),
      linear-gradient(180deg, rgba(255, 255, 255, 0.95) 0%, rgba(240, 249, 247, 0.9) 100%);
    background-size: 16px 16px, 100% 100%;
    border-radius: 10px;
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
