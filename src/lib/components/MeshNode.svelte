<script>
  import { getToolIcon } from '../toolLogos.js'

  let {
    name = '',
    role = 'agent',
    tool = 'claude',
    model = '',
    status = 'offline',
    selected = false,
    position = { x: 0, y: 0 },
    width = 180,
    dark = false,
    onClick = () => {},
  } = $props()

  const normalizedRole = $derived(role === 'lead' ? 'lead' : 'agent')
  const isLead = $derived(normalizedRole === 'lead')
  const nodeHeight = $derived(isLead ? 72 : 64)

  const safeName = $derived(String(name || '').trim() || 'unnamed')
  const safeModel = $derived(String(model || '').trim())
  const hasModel = $derived(safeModel.length > 0)

  const safeTool = $derived.by(() => {
    const value = String(tool || '').trim().toLowerCase()
    if (value === 'claude' || value === 'codex' || value === 'gemini') return value
    return 'claude'
  })

  const icon = $derived.by(() => getToolIcon(safeTool))

  const safeStatus = $derived.by(() => {
    const value = String(status || '').trim().toLowerCase()
    if (value === 'active' || value === 'idle') return value
    return 'offline'
  })

  const statusColor = $derived.by(() => {
    if (safeStatus === 'active') return '#22C55E'
    if (safeStatus === 'idle') return '#F59E0B'
    return '#6B7280'
  })

  const centerX = $derived(Number(position?.x ?? 0))
  const centerY = $derived(Number(position?.y ?? 0))

  const surfaceStyle = $derived.by(() => {
    const left = Number(position?.x ?? 0) - Number(width) / 2
    const top = Number(position?.y ?? 0) - Number(nodeHeight) / 2

    return [
      `left: ${left}px`,
      `top: ${top}px`,
      `width: ${Number(width)}px`,
      `height: ${Number(nodeHeight)}px`,
    ].join('; ')
  })
</script>

<button
  type="button"
  class="mesh-node"
  class:is-lead={isLead}
  class:is-selected={selected}
  class:is-light={!dark}
  data-testid={`mesh-node-${normalizedRole}`}
  data-center-x={centerX}
  data-center-y={centerY}
  data-node-width={width}
  data-node-height={nodeHeight}
  onclick={onClick}
  style={surfaceStyle}
>
  <span class="mesh-node-content">
    <span class="mesh-node-title-row">
      <span class="mesh-node-title-left">
        <svg
          class="mesh-node-tool"
          width="12"
          height="12"
          viewBox={icon.viewBox}
          fill="none"
          aria-hidden="true"
          data-testid={`mesh-node-icon-${normalizedRole}`}
        >
          <path d={icon.path} fill="currentColor"></path>
        </svg>
        <span class="mesh-node-name" data-testid={`mesh-node-name-${normalizedRole}`}>
          {isLead ? '★ ' : ''}{safeName}
        </span>
      </span>
      <span class="mesh-node-status" style={`background-color: ${statusColor};`}></span>
    </span>

    {#if hasModel}
      <span class="mesh-node-model" data-testid={`mesh-node-model-${normalizedRole}`}>{safeModel}</span>
    {/if}
  </span>
</button>

<style>
  .mesh-node {
    position: absolute;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3), 0 1px 3px rgba(0, 0, 0, 0.2);
    color: #e7f5f2;
    transition: border-color 150ms ease-out, transform 150ms ease-out, box-shadow 150ms ease-out;
    cursor: pointer;
    padding: 12px 16px;
    display: flex;
    align-items: flex-start;
    animation: mesh-node-enter 160ms ease-out;
  }

  .mesh-node:hover {
    transform: translateY(-1px);
    border-color: rgba(255, 255, 255, 0.14);
    box-shadow: 0 8px 18px rgba(0, 0, 0, 0.32), 0 2px 5px rgba(0, 0, 0, 0.22);
  }

  .mesh-node.is-lead {
    border-width: 1.5px;
    border-color: rgba(13, 148, 136, 0.42);
  }

  .mesh-node.is-lead:hover {
    border-color: rgba(13, 148, 136, 0.56);
  }

  .mesh-node.is-selected {
    border-width: 1.5px;
    border-color: rgba(13, 148, 136, 0.5);
    box-shadow: 0 0 0 2px rgba(13, 148, 136, 0.2), 0 8px 18px rgba(0, 0, 0, 0.3);
  }

  .mesh-node.is-lead.is-selected {
    border-color: rgba(13, 148, 136, 0.6);
  }

  .mesh-node.is-light {
    border: 1px solid rgba(13, 148, 136, 0.15);
    background: rgba(255, 255, 255, 0.9);
    box-shadow: var(--mesh-node-shadow-light);
    color: #134e4a;
  }

  .mesh-node.is-light:hover {
    border-color: rgba(13, 148, 136, 0.28);
    box-shadow: var(--mesh-node-shadow-light-hover);
  }

  .mesh-node.is-light.is-lead {
    border-color: rgba(13, 148, 136, 0.3);
  }

  .mesh-node.is-light.is-lead:hover {
    border-color: rgba(13, 148, 136, 0.42);
  }

  .mesh-node.is-light.is-selected {
    border-color: rgba(13, 148, 136, 0.5);
    box-shadow: 0 0 0 2px rgba(13, 148, 136, 0.2), var(--mesh-node-shadow-light-hover);
  }

  .mesh-node.is-light.is-lead.is-selected {
    border-color: rgba(13, 148, 136, 0.58);
  }

  .mesh-node-status {
    width: 6px;
    height: 6px;
    border-radius: 9999px;
    box-shadow: 0 0 6px rgba(0, 0, 0, 0.35);
    flex: 0 0 auto;
    margin-top: 5px;
    pointer-events: none;
  }

  .mesh-node-content {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .mesh-node-title-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
  }

  .mesh-node-title-left {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }

  .mesh-node-tool {
    color: rgba(156, 214, 206, 0.9);
    flex: 0 0 auto;
  }

  .mesh-node-name {
    font-size: 14px;
    font-weight: 600;
    line-height: 1.2;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mesh-node-model {
    font-size: 11px;
    line-height: 1.25;
    color: #9cb3b1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mesh-node.is-light .mesh-node-model {
    color: #0d7c73;
  }

  .mesh-node.is-light .mesh-node-tool {
    color: #0f766e;
  }

  .mesh-node.is-light .mesh-node-status {
    box-shadow: 0 0 6px rgba(13, 148, 136, 0.28);
  }
</style>
