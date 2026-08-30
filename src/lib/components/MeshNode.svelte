<script>
  import { activityLevel } from '../activitySignal.js'
  import { getToolIcon } from '../toolLogos.js'
  import { normalizeTool } from '../toolRegistry.js'

  let {
    nodeId = '',
    name = '',
    role = 'agent',
    roleName = '',
    focusArea = '',
    contextSummary = '',
    behaviorSummary = '',
    tool = 'claude',
    model = '',
    reasoningEffort = '',
    taskEffort = '',
    taskEffortWhy = '',
    accountApplied = null,
    accountNote = '',
    accountNoteDetail = '',
    status = 'offline',
    isCrossProject = false,
    projectLabel = '',
    selected = false,
    position = { x: 0, y: 0 },
    width = 180,
    height = null,
    dark = false,
    onClick = () => {},
    onHoverStart = () => {},
    onHoverEnd = () => {},
  } = $props()

  const normalizedRole = $derived(role === 'lead' ? 'lead' : 'agent')
  const isLead = $derived(normalizedRole === 'lead')
  const safeAccountNoteDetail = $derived(String(accountNoteDetail || '').trim())
  const showOpaqueAccountNote = $derived(
    accountApplied === false
      && accountNote === 'opaque_base_command'
      && safeAccountNoteDetail.length > 0
  )
  const accountNoteSentence = $derived(
    `Account selection not guaranteed: ${safeAccountNoteDetail} wraps the CLI.`
  )
  const requestedHeight = $derived(Number(height))
  const nodeHeight = $derived(
    Number.isFinite(requestedHeight) && requestedHeight > 0
      ? requestedHeight
      : (showOpaqueAccountNote ? (isLead ? 90 : 82) : (isLead ? 72 : 64))
  )

  const safeName = $derived(String(name || '').trim() || 'unnamed')
  const safeModel = $derived(String(model || '').trim())
  const hasModel = $derived(safeModel.length > 0)
  const safeProjectLabel = $derived(String(projectLabel || '').trim())
  const showProjectChip = $derived(Boolean(isCrossProject) && safeProjectLabel.length > 0)

  // The effort this session was launched at, shown next to the one the lead
  // attached to the current assignment so the two can be read against each
  // other: a task asking for more than the session runs at is the interesting
  // case.
  const safeLaunchEffort = $derived(String(reasoningEffort || '').trim())
  const showLaunchEffort = $derived(safeLaunchEffort.length > 0)
  const launchEffortTitle = $derived(`Launch effort: ${safeLaunchEffort}`)

  const safeTaskEffort = $derived(String(taskEffort || '').trim())
  const safeTaskEffortWhy = $derived(String(taskEffortWhy || '').trim())
  const showTaskEffort = $derived(safeTaskEffort.length > 0)
  const taskEffortTitle = $derived(
    safeTaskEffortWhy
      ? `Task effort: ${safeTaskEffort} — ${safeTaskEffortWhy}`
      : `Task effort: ${safeTaskEffort}`
  )

  const safeTool = $derived.by(() => {
    return normalizeTool(tool)
  })

  const icon = $derived.by(() => getToolIcon(safeTool))

  const safeStatus = $derived(activityLevel({ status }))

  const STATUS_COLORS = {
    working: 'var(--color-success-500)',
    active: 'var(--color-success-500)',
    idle: 'var(--color-warning-500)',
    uncertain: 'var(--color-info-500)',
    offline: 'var(--mesh-node-status-offline)',
  }

  const statusColor = $derived(STATUS_COLORS[safeStatus])

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
  class:is-offline={safeStatus === 'offline'}
  class:is-selected={selected}
  class:is-light={!dark}
  data-testid={`mesh-node-${normalizedRole}`}
  data-node-id={String(nodeId || '')}
  data-center-x={centerX}
  data-center-y={centerY}
  data-node-width={width}
  data-node-height={nodeHeight}
  onclick={onClick}
  onmouseenter={onHoverStart}
  onmouseleave={onHoverEnd}
  onfocus={onHoverStart}
  onblur={onHoverEnd}
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

    {#if hasModel || showLaunchEffort || showTaskEffort || showProjectChip}
      <span
        class="mesh-node-meta-row"
        class:chip-only={!hasModel && (showLaunchEffort || showTaskEffort || showProjectChip)}
        data-testid={`mesh-node-meta-row-${normalizedRole}`}
      >
        {#if hasModel}
          <span class="mesh-node-model" data-testid={`mesh-node-model-${normalizedRole}`}>{safeModel}</span>
        {/if}

        {#if showLaunchEffort}
          <span
            class="mesh-node-launch-effort"
            data-testid={`mesh-node-launch-effort-${normalizedRole}`}
            title={launchEffortTitle}
          >
            {safeLaunchEffort}
          </span>
        {/if}

        {#if showTaskEffort}
          <span
            class="mesh-node-task-effort"
            data-testid={`mesh-node-task-effort-${normalizedRole}`}
            title={taskEffortTitle}
          >
            {safeTaskEffort}
          </span>
        {/if}

        {#if showProjectChip}
          <span
            class="mesh-node-project-chip"
            data-testid={`mesh-node-project-chip-${normalizedRole}`}
            title={`Works in ${safeProjectLabel}`}
          >
            [{safeProjectLabel}]
          </span>
        {/if}
      </span>
    {/if}

    {#if showOpaqueAccountNote}
      <span
        class="mesh-node-account-note"
        data-testid={`mesh-node-account-note-${normalizedRole}`}
        title={accountNoteSentence}
      >{accountNoteSentence}</span>
    {/if}
  </span>
</button>

<style>
  .mesh-node {
    position: absolute;
    border: 1px solid var(--mesh-node-border-dark);
    border-radius: 12px;
    background: var(--mesh-node-bg-dark);
    box-shadow: var(--mesh-node-shadow);
    color: var(--mesh-node-text-dark);
    transition: border-color 150ms ease-out, transform 150ms ease-out, box-shadow 150ms ease-out;
    cursor: pointer;
    padding: 12px 16px;
    display: flex;
    align-items: flex-start;
    animation: mesh-node-enter 160ms ease-out;
  }

  .mesh-node:hover {
    transform: translateY(-1px);
    border-color: var(--mesh-node-border-dark-hover);
    box-shadow: var(--mesh-node-shadow-hover);
  }

  .mesh-node:focus-visible {
    outline: none;
    border-color: var(--mesh-node-selected-border-dark);
    box-shadow: var(--mesh-node-selected-ring), var(--mesh-node-shadow-hover);
  }

  .mesh-node.is-lead {
    border-width: 1.5px;
    border-color: var(--mesh-node-lead-border-dark);
  }

  .mesh-node.is-lead:hover {
    border-color: var(--mesh-node-lead-border-dark-hover);
  }

  .mesh-node.is-lead:focus-visible {
    border-color: var(--mesh-node-lead-selected-border-dark);
  }

  .mesh-node.is-selected {
    border-width: 1.5px;
    border-color: var(--mesh-node-selected-border-dark);
    box-shadow: var(--mesh-node-selected-ring), var(--mesh-node-shadow-hover);
  }

  .mesh-node.is-offline {
    opacity: 0.82;
    filter: saturate(0.88);
  }

  .mesh-node.is-lead.is-selected {
    border-color: var(--mesh-node-lead-selected-border-dark);
  }

  .mesh-node.is-light {
    border: 1px solid var(--mesh-node-border-light);
    background: var(--mesh-node-bg-light);
    box-shadow: var(--mesh-node-shadow-light);
    color: var(--mesh-node-text-light);
  }

  .mesh-node.is-light:hover {
    border-color: var(--mesh-node-border-light-hover);
    box-shadow: var(--mesh-node-shadow-light-hover);
  }

  .mesh-node.is-light:focus-visible {
    border-color: var(--mesh-node-selected-border-light);
    box-shadow: var(--mesh-node-selected-ring), var(--mesh-node-shadow-light-hover);
  }

  .mesh-node.is-light.is-lead {
    border-color: var(--mesh-node-lead-border-light);
  }

  .mesh-node.is-light.is-lead:hover {
    border-color: var(--mesh-node-lead-border-light-hover);
  }

  .mesh-node.is-light.is-lead:focus-visible {
    border-color: var(--mesh-node-lead-selected-border-light);
  }

  .mesh-node.is-light.is-selected {
    border-color: var(--mesh-node-selected-border-light);
    box-shadow: var(--mesh-node-selected-ring), var(--mesh-node-shadow-light-hover);
  }

  .mesh-node.is-light.is-offline {
    opacity: 0.84;
  }

  .mesh-node.is-light.is-lead.is-selected {
    border-color: var(--mesh-node-lead-selected-border-light);
  }

  .mesh-node-status {
    width: 6px;
    height: 6px;
    border-radius: 9999px;
    box-shadow: var(--mesh-node-status-shadow-dark);
    flex: 0 0 auto;
    margin-top: 5px;
    pointer-events: none;
  }

  .mesh-node-content {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
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
    color: var(--mesh-node-tool-dark);
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
    color: var(--mesh-node-model-dark);
    min-width: 0;
    flex: 1 1 auto;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mesh-node-account-note {
    color: var(--color-warning-500);
    font-size: 10px;
    line-height: 1.2;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mesh-node-meta-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
  }

  .mesh-node-meta-row.chip-only {
    justify-content: flex-end;
  }

  /* The launch effort is context for the assignment effort beside it, so it
     stays plain text where the assignment level wears the chip. */
  .mesh-node-launch-effort {
    flex: 0 0 auto;
    line-height: 1.4;
    letter-spacing: 0.02em;
    opacity: 0.6;
  }

  .mesh-node-task-effort {
    flex: 0 0 auto;
    border: 1px solid currentColor;
    border-radius: 999px;
    padding: 0 5px;
    line-height: 1.4;
    letter-spacing: 0.02em;
    opacity: 0.85;
  }

  .mesh-node-project-chip {
    display: inline-flex;
    align-items: center;
    flex: 0 0 auto;
    max-width: 88px;
    margin-left: auto;
    border: 1px solid color-mix(in srgb, var(--mesh-node-border-dark) 58%, var(--color-brand-400) 42%);
    border-radius: 9999px;
    padding: 2px 7px;
    font-size: 9.5px;
    line-height: 1.15;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--mesh-node-text-dark) 68%, var(--color-brand-300) 32%);
    background: color-mix(in srgb, var(--mesh-node-bg-dark) 76%, var(--color-brand-500) 24%);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mesh-node.is-light .mesh-node-model {
    color: var(--mesh-node-model-light);
  }

  .mesh-node.is-light .mesh-node-project-chip {
    border-color: color-mix(in srgb, var(--mesh-node-border-light) 46%, var(--color-brand-300) 54%);
    color: color-mix(in srgb, var(--mesh-node-text-light) 72%, var(--color-brand-700) 28%);
    background: color-mix(in srgb, var(--mesh-node-bg-light) 82%, var(--color-brand-100) 18%);
  }

  .mesh-node.is-light .mesh-node-tool {
    color: var(--mesh-node-tool-light);
  }

  .mesh-node.is-light .mesh-node-status {
    box-shadow: var(--mesh-node-status-shadow-light);
  }
</style>
