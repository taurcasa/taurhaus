<script>
  /**
   * Workflow run history for one project, as a section of its Overview tab.
   *
   * The run APIs are keyed by Claude session, and a project has no single
   * session, so this asks the sessions it can name: the ones running right now,
   * plus the ones the project's tasks came from. A session that cannot be read
   * is dropped from the merge rather than emptying the list — the other
   * sessions' runs are still true.
   *
   * The section hides itself when nothing came back, the way the sibling
   * Sessions and Relationships sections do.
   */
  import {
    getProjectTasks,
    getWorkflowRun,
    listWorkflowRuns,
    workflowLedgerRow,
  } from '../ipc.js'
  import { formatTokens, collectWorkflowSessionIds, runListRow } from '../workflowRuns.js'
  import { themeTokens } from '../themeTokens.js'

  let { projectId = '', sessions = [], dark = false } = $props()

  /** Enough sessions to cover a project's recent history without a fan-out. */
  const MAX_SESSIONS = 8

  let runs = $state([])
  let selected = $state(null)
  let detail = $state(null)
  let ledgerRow = $state(null)
  let copied = $state(false)

  const t = $derived(themeTokens(dark))
  const rows = $derived(runs.map((run) => ({ ...runListRow(run), sessionId: run.sessionId })))
  const detailAgents = $derived(Array.isArray(detail?.agents) ? detail.agents : [])
  const copyLabel = $derived(copied ? 'Copied' : 'Copy ledger row')
  const copyTitle = $derived(
    ledgerRow
      ? 'Copy this run’s plan ledger row'
      : 'This run returned no ledger row'
  )

  $effect(() => {
    const id = String(projectId || '')
    const liveSessions = Array.isArray(sessions) ? sessions : []
    let cancelled = false

    runs = []
    selected = null
    detail = null
    ledgerRow = null
    copied = false

    void (async () => {
      let taskSessions = []
      if (id) {
        try {
          const answer = await getProjectTasks(id)
          taskSessions = Array.isArray(answer?.tasks) ? answer.tasks : []
        } catch {
          taskSessions = []
        }
      }
      if (cancelled) return

      const sessionIds = collectWorkflowSessionIds(liveSessions, taskSessions).slice(0, MAX_SESSIONS)
      const answers = await Promise.allSettled(
        sessionIds.map((sessionId) => listWorkflowRuns(sessionId))
      )
      if (cancelled) return

      const merged = []
      for (const [index, answer] of answers.entries()) {
        if (answer.status !== 'fulfilled' || !Array.isArray(answer.value)) continue
        for (const run of answer.value) {
          merged.push({ ...run, sessionId: sessionIds[index] })
        }
      }
      runs = merged.sort((left, right) => (right?.started_at ?? 0) - (left?.started_at ?? 0))
    })()

    return () => {
      cancelled = true
    }
  })

  async function selectRun(row) {
    if (selected === row.runId) {
      selected = null
      detail = null
      ledgerRow = null
      return
    }

    selected = row.runId
    detail = null
    ledgerRow = null
    copied = false

    const [run, ledger] = await Promise.allSettled([
      getWorkflowRun(row.sessionId, row.runId),
      workflowLedgerRow(row.sessionId, row.runId),
    ])
    if (selected !== row.runId) return

    detail = run.status === 'fulfilled' ? run.value : null
    ledgerRow = ledger.status === 'fulfilled' && typeof ledger.value === 'string'
      ? ledger.value
      : null
  }

  async function copyLedgerRow() {
    if (!ledgerRow) return
    try {
      await navigator.clipboard.writeText(ledgerRow)
      copied = true
    } catch (error) {
      console.warn('[workflow-runs] could not copy the ledger row:', error)
    }
  }
</script>

{#if rows.length > 0}
  <section class="py-5 border-b {t.keyline}" data-testid="overview-workflow-runs">
    <div class="flex items-center justify-between mb-3">
      <span class="text-[11px] {t.textTertiary}">Workflow runs</span>
      <span class="text-[11px] {t.textTertiary}">
        {rows.length} run{rows.length !== 1 ? 's' : ''}
      </span>
    </div>

    <div>
      {#each rows as row (`${row.sessionId}:${row.runId}`)}
        <button
          type="button"
          class="w-full flex items-center gap-3 h-[30px] text-[13px] text-left {t.hoverRow} -mx-2 px-2 rounded transition-colors cursor-pointer {selected === row.runId ? t.sectionBg : ''}"
          data-testid="workflow-run-row"
          aria-expanded={selected === row.runId}
          onclick={() => selectRun(row)}
        >
          <span
            class="workflow-run-status shrink-0"
            data-status={row.status}
            title={row.phasesLabel}
          >{row.statusLabel}</span>
          <span class="{t.textBody} truncate flex-1 min-w-0">{row.name}</span>
          {#if row.phasesLabel}
            <span class="text-[11px] {t.textTertiary} truncate max-w-[168px] hidden sm:inline">
              {row.phasesLabel}
            </span>
          {/if}
          <span class="text-[11px] {t.textTertiary} shrink-0 tabular-nums w-[38px] text-right">
            {row.doneLabel ?? ''}
          </span>
          <span class="text-[11px] {t.textTertiary} shrink-0 tabular-nums w-[48px] text-right">
            {row.tokensLabel ?? ''}
          </span>
          <span class="text-[11px] {t.textTertiary} shrink-0 tabular-nums w-[60px] text-right">
            {row.durationLabel ?? ''}
          </span>
        </button>

        {#if selected === row.runId}
          <div class="ml-1 border-l {t.keyline} pl-3 pr-2 pb-2 pt-1" data-testid="workflow-run-detail">
            {#if detailAgents.length > 0}
              <ul class="space-y-0.5">
                {#each detailAgents as agent (agent.agent_id)}
                  <li
                    class="flex items-center gap-2 text-[12px] {t.textBody}"
                    data-testid="workflow-detail-agent"
                  >
                    <span class="workflow-run-status shrink-0" data-state={agent.state}>
                      {agent.state}
                    </span>
                    <span class="truncate flex-1 min-w-0">
                      {agent.label || agent.prompt_preview}
                    </span>
                    {#if agent.phase}
                      <span class="text-[11px] {t.textTertiary} shrink-0">{agent.phase}</span>
                    {/if}
                    {#if agent.model}
                      <span class="text-[11px] {t.textTertiary} shrink-0 truncate max-w-[110px]">
                        {agent.model}
                      </span>
                    {/if}
                    {#if agent.last_tool}
                      <span class="text-[11px] {t.textTertiary} shrink-0">{agent.last_tool}</span>
                    {/if}
                    <span class="text-[11px] {t.textTertiary} shrink-0 tabular-nums w-[48px] text-right">
                      {formatTokens(agent.tokens) ?? ''}
                    </span>
                    <!-- Keeps the agent tokens column under the run tokens
                         column: the run row carries a duration this one has no
                         equivalent for. -->
                    <span class="shrink-0 w-[60px]" aria-hidden="true"></span>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="text-[12px] {t.textMuted}">No agent detail for this run.</p>
            {/if}

            <button
              type="button"
              class="mt-2.5 text-[11px] {t.textTertiary} hover:underline disabled:no-underline disabled:opacity-40 disabled:cursor-not-allowed"
              data-testid="workflow-copy-ledger"
              disabled={!ledgerRow}
              title={copyTitle}
              onclick={copyLedgerRow}
            >{copyLabel}</button>
          </div>
        {/if}
      {/each}
    </div>
  </section>
{/if}

<style>
  .workflow-run-status {
    display: inline-flex;
    align-items: center;
    height: 16px;
    padding-inline: 6px;
    border-radius: 999px;
    border: 1px solid transparent;
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    line-height: 1;
    white-space: nowrap;
  }

  .workflow-run-status[data-status='live'],
  .workflow-run-status[data-state='running'] {
    border-color: rgba(34, 197, 94, 0.45);
    color: var(--color-success-600);
  }

  .workflow-run-status[data-status='completed'],
  .workflow-run-status[data-state='done'] {
    border-color: rgba(45, 212, 191, 0.42);
    color: var(--color-brand-600);
  }

  .workflow-run-status[data-status='failed'],
  .workflow-run-status[data-state='failed'] {
    border-color: rgba(239, 68, 68, 0.45);
    color: var(--color-danger-500);
  }

  .workflow-run-status[data-status='unknown'],
  .workflow-run-status[data-state='queued'] {
    border-color: rgba(113, 113, 122, 0.4);
    color: var(--color-zinc-500, #71717a);
  }

  :global(.dark) .workflow-run-status[data-status='completed'],
  :global(.dark) .workflow-run-status[data-state='done'] {
    color: var(--color-brand-400);
  }

  :global(.dark) .workflow-run-status[data-status='live'],
  :global(.dark) .workflow-run-status[data-state='running'] {
    color: var(--color-success-400);
  }
</style>
