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
    width = 110,
    dark = false,
    onClick = () => {},
  } = $props()

  const normalizedRole = $derived(role === 'lead' ? 'lead' : 'agent')
  const isLead = $derived(normalizedRole === 'lead')
  const nodeHeight = $derived(isLead ? 58 : 52)

  const safeName = $derived(String(name || '').trim() || 'unnamed')
  const safeModel = $derived(String(model || '').trim() || 'model')

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
  <span class="mesh-node-status" style={`background-color: ${statusColor};`}></span>

  <span class="mesh-node-content">
    <span class="mesh-node-title-row">
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
    <span class="mesh-node-model" data-testid={`mesh-node-model-${normalizedRole}`}>{safeModel}</span>
  </span>
</button>

<style>
  .mesh-node {
    position: absolute;
    border: 1px solid var(--mesh-node-border);
    border-radius: 28px;
    background: linear-gradient(
      180deg,
      var(--mesh-node-gradient-from) 0%,
      var(--mesh-node-gradient-to) 100%
    );
    box-shadow: var(--mesh-node-shadow);
    color: #e7f5f2;
    transition: all 150ms ease-out;
    cursor: pointer;
    padding: 8px 12px;
    display: flex;
    align-items: center;
    animation: mesh-node-enter 160ms ease-out;
  }

  .mesh-node:hover {
    transform: translateY(-1px);
    border-color: var(--mesh-node-border-hover);
    background: linear-gradient(
      180deg,
      var(--mesh-node-gradient-hover-from) 0%,
      var(--mesh-node-gradient-hover-to) 100%
    );
    box-shadow: var(--mesh-node-shadow-hover);
  }

  .mesh-node.is-lead {
    border-width: 1.5px;
    border-color: var(--mesh-lead-border);
    box-shadow: var(--mesh-node-shadow), var(--mesh-lead-glow);
  }

  .mesh-node.is-lead:hover {
    box-shadow: var(--mesh-node-shadow-hover), var(--mesh-lead-glow);
  }

  .mesh-node.is-selected {
    border-width: 1.5px;
    border-color: var(--mesh-selected-border);
    box-shadow: var(--mesh-node-shadow-hover), var(--mesh-selected-glow);
  }

  .mesh-node.is-lead.is-selected {
    box-shadow: var(--mesh-node-shadow-hover), var(--mesh-lead-glow), var(--mesh-selected-glow);
  }

  .mesh-node.is-light {
    background: linear-gradient(180deg, #f0fdfa 0%, #e6f7f4 100%);
    border-color: #b2d8d0;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
    color: #134e4a;
  }

  .mesh-node.is-light:hover {
    border-color: #8ec5ba;
    background: linear-gradient(180deg, #effbf9 0%, #ddf3ef 100%);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12);
  }

  .mesh-node.is-light.is-lead {
    border-color: rgba(13, 148, 136, 0.5);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08), 0 0 8px rgba(13, 148, 136, 0.15);
  }

  .mesh-node.is-light.is-lead:hover {
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12), 0 0 8px rgba(13, 148, 136, 0.15);
  }

  .mesh-node.is-light.is-selected {
    border-color: rgba(13, 148, 136, 0.65);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12), 0 0 0 2px rgba(13, 148, 136, 0.15);
  }

  .mesh-node.is-light.is-lead.is-selected {
    box-shadow:
      0 4px 14px rgba(0, 0, 0, 0.12),
      0 0 8px rgba(13, 148, 136, 0.15),
      0 0 0 2px rgba(13, 148, 136, 0.15);
  }

  .mesh-node-status {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 6px;
    height: 6px;
    border-radius: 9999px;
    box-shadow: 0 0 6px rgba(0, 0, 0, 0.35);
    pointer-events: none;
  }

  .mesh-node-content {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .mesh-node-title-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .mesh-node-tool {
    color: rgba(136, 168, 166, 0.95);
    flex: 0 0 auto;
  }

  .mesh-node-name {
    font-size: 13px;
    font-weight: 600;
    line-height: 1.2;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mesh-node-model {
    font-size: 11px;
    line-height: 1.2;
    color: #8aa8a6;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mesh-node.is-light .mesh-node-model {
    color: #0f766e;
  }

  .mesh-node.is-light .mesh-node-tool {
    color: #0f766e;
  }

  .mesh-node.is-light .mesh-node-status {
    box-shadow: 0 0 4px rgba(13, 148, 136, 0.2);
  }
</style>
