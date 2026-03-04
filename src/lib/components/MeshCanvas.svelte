<script>
  import MeshConnection from './MeshConnection.svelte'
  import MeshNode from './MeshNode.svelte'

  let {
    lead = null,
    agents = [],
    mode = 'setup',
    dark = false,
    onNodeClick = (id) => {},
    onAddClick = () => {},
    selectedNodeId = null,
  } = $props()

  let containerWidth = $state(0)

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

  const normalizedLead = $derived.by(() => {
    if (!lead) return null
    return {
      ...lead,
      id: String(lead.id ?? 'lead'),
      status: normalizeStatus(lead.status),
    }
  })

  const normalizedAgents = $derived.by(() => {
    if (!Array.isArray(agents)) return []

    return agents.map((agent, index) => ({
      ...agent,
      id: String(agent?.id ?? agent?.name ?? `agent-${index}`),
      status: normalizeStatus(agent?.status),
    }))
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
    const leadData = normalizedLead
    if (!leadData) return { lead: null, agents: [], connections: [], addNode: null }

    const leadPos = { x: cw / 2, y: 48 }
    const members = normalizedAgents
    const count = members.length
    const gap = Math.max(12, 24 - (count - 2) * 4)
    const nodeW = Math.max(90, 110 - Math.max(0, count - 3) * 10)

    let positionedAgents = []

    if (count >= 7) {
      const row1Count = Math.ceil(count / 2)
      const row2Count = count - row1Count
      positionedAgents = [
        ...buildRow(members, 0, row1Count, leadPos.y + 100, cw, nodeW, gap),
        ...buildRow(members, row1Count, row2Count, leadPos.y + 180, cw, nodeW, gap),
      ]
    } else if (count > 0) {
      const totalW = count * nodeW + (count - 1) * gap
      const startX = (cw - totalW) / 2

      positionedAgents = members.map((agent, i) => ({
        ...agent,
        position: {
          x: startX + i * (nodeW + gap) + nodeW / 2,
          y: leadPos.y + 120,
        },
        width: nodeW,
      }))
    }

    const connections = positionedAgents.map(agent => ({
      id: agent.id,
      from: leadPos,
      to: agent.position,
      status: normalizedMode === 'runtime' ? agent.status : normalizedMode,
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
          y: leadPos.y + 120,
        })
      : null

    return {
      lead: {
        ...leadData,
        position: leadPos,
        width: 130,
      },
      agents: positionedAgents,
      connections,
      addNode,
    }
  })

  const canvasHeight = $derived.by(() => {
    const current = layout
    if (!current.lead) return 280

    let maxY = current.lead.position.y + 60
    for (const agent of current.agents) {
      maxY = Math.max(maxY, agent.position.y + 70)
    }

    if (current.addNode) {
      maxY = Math.max(maxY, current.addNode.y + 60)
    }

    return Math.max(280, Math.ceil(maxY))
  })
</script>

<div
  class="mesh-canvas"
  bind:clientWidth={containerWidth}
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
      {#each layout.connections as connection (connection.id)}
        <MeshConnection
          from={connection.from}
          to={connection.to}
          status={connection.status}
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
    min-height: 280px;
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
