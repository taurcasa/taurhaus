<script>
  /**
   * The workflow runs of one mesh node, drawn into the child box the layout
   * engine placed for it (`meshLayout.js`). Every dimension in here comes from
   * `RUN_TREE_METRICS`, because the box was sized as header + rows and the
   * rendered rows have to be worth exactly that.
   *
   * A live run is expanded — phase titles, then its agents. A finished run is
   * one line: name, agents done, tokens, duration. Live rows breathe rather
   * than spin; a viewer who does not want the motion gets none.
   */
  import { runTreeModel } from '../workflowRuns.js'
  import { RUN_TREE_METRICS } from './meshLayout.js'

  let {
    runs = [],
    box = null,
    dark = false,
    collapsedRunIds = [],
    onToggleRun = (_runId) => {},
  } = $props()

  const collapsed = $derived(new Set(collapsedRunIds.map((id) => String(id))))

  const models = $derived(
    (Array.isArray(runs) ? runs : [])
      .map(runTreeModel)
      .filter(Boolean)
      .map((model) => ({
        ...model,
        expanded: model.isLive && !collapsed.has(model.runId),
      }))
  )

  const boxStyle = $derived([
    `left: ${Number(box?.left ?? 0)}px`,
    `top: ${Number(box?.top ?? 0)}px`,
    `width: ${Number(box?.width ?? 0)}px`,
    `height: ${Number(box?.height ?? 0)}px`,
    `--run-tree-padding-x: ${RUN_TREE_METRICS.paddingX}px`,
    `--run-tree-padding-y: ${RUN_TREE_METRICS.paddingY}px`,
    `--run-tree-header-height: ${RUN_TREE_METRICS.headerHeight}px`,
    `--run-tree-row-height: ${RUN_TREE_METRICS.rowHeight}px`,
  ].join('; '))
</script>

{#if box && models.length > 0}
  <div
    class="workflow-run-tree"
    class:is-light={!dark}
    data-testid="workflow-run-tree"
    style={boxStyle}
  >
    {#each models as model (model.runId)}
      <button
        type="button"
        class="workflow-run-header"
        data-testid="workflow-run-header"
        data-status={model.status}
        data-expanded={model.expanded}
        aria-expanded={model.expanded}
        title={model.summary}
        onclick={() => onToggleRun(model.runId)}
      >
        <span class="workflow-run-dot" aria-hidden="true"></span>
        <span class="workflow-run-summary">{model.summary}</span>
      </button>

      {#if model.expanded}
        {#each model.groups as group, groupIndex (group.title ?? `unphased-${groupIndex}`)}
          {#if group.title}
            <div class="workflow-run-phase" data-testid="workflow-run-phase">{group.title}</div>
          {/if}
          {#each group.agents as agentRow (agentRow.agentId)}
            <div
              class="workflow-run-agent"
              data-testid="workflow-run-agent"
              data-state={agentRow.state}
            >
              <span class="workflow-run-dot" aria-hidden="true"></span>
              <span class="workflow-run-label">{agentRow.label}</span>
              <span class="workflow-run-meta">
                {#if agentRow.model}<span class="workflow-run-model">{agentRow.model}</span>{/if}
                {#if agentRow.lastTool}<span>{agentRow.lastTool}</span>{/if}
                {#if agentRow.tokensLabel}<span>{agentRow.tokensLabel}</span>{/if}
              </span>
            </div>
          {/each}
        {/each}
      {/if}
    {/each}
  </div>
{/if}

<style>
  .workflow-run-tree {
    position: absolute;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
    padding: var(--run-tree-padding-y) var(--run-tree-padding-x);
    border: 1px solid var(--mesh-node-border-dark);
    border-radius: 8px;
    background: var(--mesh-node-bg-dark);
    color: var(--mesh-node-text-dark);
    overflow: hidden;
    animation: mesh-node-enter 160ms ease-out;
  }

  .workflow-run-tree.is-light {
    border-color: var(--mesh-node-border-light);
    background: var(--mesh-node-bg-light);
    color: var(--mesh-node-text-light);
  }

  .workflow-run-header {
    display: flex;
    align-items: center;
    gap: 5px;
    height: var(--run-tree-header-height);
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: -0.005em;
    line-height: 1;
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }

  .workflow-run-header:focus-visible {
    outline: 1px solid var(--mesh-node-selected-border-dark);
    outline-offset: 1px;
  }

  .workflow-run-summary,
  .workflow-run-label {
    flex: 1 1 auto;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .workflow-run-phase {
    display: flex;
    align-items: center;
    height: var(--run-tree-row-height);
    padding-left: 2px;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mesh-node-model-dark);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .workflow-run-agent {
    display: flex;
    align-items: center;
    gap: 5px;
    height: var(--run-tree-row-height);
    padding-left: 6px;
    font-size: 10px;
    line-height: 1;
    min-width: 0;
  }

  .workflow-run-meta {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: 0 1 auto;
    min-width: 0;
    font-size: 9.5px;
    color: var(--mesh-node-model-dark);
    white-space: nowrap;
    overflow: hidden;
  }

  .workflow-run-model {
    max-width: 54px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .workflow-run-dot {
    flex: 0 0 auto;
    width: 5px;
    height: 5px;
    border-radius: 9999px;
    background: var(--mesh-node-status-offline);
  }

  .workflow-run-header[data-status='live'] .workflow-run-dot,
  .workflow-run-agent[data-state='running'] .workflow-run-dot {
    background: var(--color-success-500);
    animation: workflow-run-breathe 2.4s ease-in-out infinite;
  }

  .workflow-run-header[data-status='completed'] .workflow-run-dot,
  .workflow-run-agent[data-state='done'] .workflow-run-dot {
    background: var(--color-brand-400);
  }

  .workflow-run-header[data-status='failed'] .workflow-run-dot,
  .workflow-run-agent[data-state='failed'] .workflow-run-dot {
    background: var(--color-danger-500);
  }

  .workflow-run-tree.is-light .workflow-run-phase,
  .workflow-run-tree.is-light .workflow-run-meta {
    color: var(--mesh-node-model-light);
  }

  .workflow-run-tree.is-light .workflow-run-header:focus-visible {
    outline-color: var(--mesh-node-selected-border-light);
  }

  @keyframes workflow-run-breathe {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.45; }
  }

  @media (prefers-reduced-motion: reduce) {
    .workflow-run-tree {
      animation: none;
    }

    .workflow-run-header[data-status='live'] .workflow-run-dot,
    .workflow-run-agent[data-state='running'] .workflow-run-dot {
      animation: none;
    }
  }
</style>
