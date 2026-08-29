<script>
  /**
   * Workflow run history for one project, as a section of its Overview tab.
   *
   * The run APIs are keyed by Claude session, and a project has no single
   * session, so this asks the sessions it can name, newest first: the ones
   * running right now, the ones the project's open tasks came from, and its
   * archived sessions — a session's tasks archive when it ends, which is
   * precisely when its runs become history. A session that cannot be read is
   * dropped from the merge rather than emptying the list — the other sessions'
   * runs are still true.
   *
   * One search is bounded — a long-lived project has hundreds of sessions and
   * almost none of them ran a workflow — so what it did not reach is reachable
   * by asking: *Search older sessions* carries the same search on from where it
   * stopped, until every session the project can name has been asked.
   *
   * The section hides itself when nothing came back and there is nothing left
   * to ask, the way the sibling Sessions and Relationships sections do.
   */
  import {
    getArchivedSessions,
    getProjectTasks,
    getWorkflowRun,
    listWorkflowRuns,
    workflowLedgerRow,
  } from '../ipc.js'
  import {
    collectWorkflowSessionIds,
    formatTokens,
    orderWorkflowSessionIds,
    runListRow,
    workflowSessionId,
  } from '../workflowRuns.js'
  import { themeTokens } from '../themeTokens.js'

  let { projectId = '', sessions = [], dark = false } = $props()

  /**
   * How many sessions one round asks about. A long-lived project has hundreds
   * of archived sessions and almost none of them ran a workflow, so the list is
   * cut — but only after every candidate has been ordered newest first, and the
   * header says when it was cut.
   */
  const SESSION_PAGE = 24
  /**
   * How far one search keeps looking before it hands the rest to the reader.
   *
   * A page that comes back empty is not an answer, it is the absence of one: a
   * project whose only workflow ran thirty sessions ago would otherwise show no
   * history. So an empty page asks the next one, and a page with runs in it
   * stops — the rest is older than what is already on screen. Both stops are
   * where *Search older sessions* picks the same search back up, so the budget
   * bounds what one load costs, never what the reader can reach.
   */
  const MAX_SESSIONS = SESSION_PAGE * 4

  let runs = $state([])
  let askedSessions = $state(0)
  let candidateSessions = $state(0)
  let sessionsTruncated = $state(false)
  let loadingMore = $state(false)
  let selected = $state(null)
  let detail = $state(null)
  let ledgerRow = $state(null)
  let copied = $state(false)

  // The search's own bookkeeping, deliberately outside `$state`: the load
  // effect writes how far it got, and must never re-run because it read it
  // back. The rendered copies above are written from these.
  let candidateIds = []
  let askedCount = 0

  const t = $derived(themeTokens(dark))
  // A session snapshot hands this panel a fresh array on every daemon update.
  // Only the set of session ids is a reason to ask the backend again, so the
  // load effect depends on this string and never on the array itself.
  const liveSessionKey = $derived(
    collectWorkflowSessionIds(Array.isArray(sessions) ? sessions : []).join('\u0000')
  )
  // A run starts and ends inside a session that is already listed, so the set
  // of sessions cannot see it happen. The live-run count in the session
  // activity hint can: it is the one field that moves when a run begins or
  // finishes, and the daemon's change signature carries it, so this key moves
  // exactly once per transition and not once per agent write.
  const liveRunKey = $derived(
    (Array.isArray(sessions) ? sessions : [])
      .map((session) => {
        const activity = session?.workflow_activity ?? session?.workflowActivity
        const liveRuns = Number(activity?.live_runs ?? activity?.liveRuns)
        if (!Number.isFinite(liveRuns) || liveRuns <= 0) return ''
        return `${workflowSessionId(session)}:${liveRuns}`
      })
      .filter(Boolean)
      .join('\u0000')
  )
  const rows = $derived(runs.map((run) => ({ ...runListRow(run), sessionId: run.sessionId })))
  const moreSessions = $derived(candidateSessions > askedSessions)
  const countLabel = $derived(
    rows.length > 0 ? `${rows.length} run${rows.length !== 1 ? 's' : ''}` : 'No runs'
  )
  const searchedLabel = $derived(sessionsTruncated ? ` · newest ${askedSessions} sessions` : '')
  const loadMoreLabel = $derived(loadingMore ? 'Searching…' : 'Search older sessions')
  const detailAgents = $derived(Array.isArray(detail?.agents) ? detail.agents : [])
  const copyLabel = $derived(copied ? 'Copied' : 'Copy ledger row')
  const copyTitle = $derived(
    ledgerRow
      ? 'Copy this run’s plan ledger row'
      : 'This run returned no ledger row'
  )

  // The load is keyed, not identity-triggered: a session snapshot hands this
  // panel a fresh array on every daemon update, and re-querying every session
  // of every project on each of those would be a poll nobody asked for. A token
  // rather than an effect teardown decides which answer is still wanted, so a
  // re-render during a load cannot cancel it.
  let loadedKey = ''
  let loadedRunKey = ''
  let loadToken = 0

  /**
   * Ask candidate sessions for their runs, one page at a time, newest first.
   *
   * `floor` is how far the search must go whatever it finds — a reload re-asks
   * everything the reader had already opened up, so their older runs do not
   * vanish when a run starts. `limit` is where it stops and leaves the rest to
   * the control. `null` means a newer load won the session and this answer is
   * no longer wanted.
   */
  async function askSessions(candidates, { start, floor, limit, token }) {
    const found = []
    let asked = start
    while (asked < candidates.length && asked < limit && (asked < floor || found.length === 0)) {
      const page = candidates.slice(asked, asked + SESSION_PAGE)
      const answers = await Promise.allSettled(page.map((sessionId) => listWorkflowRuns(sessionId)))
      if (token !== loadToken) return null
      asked += page.length

      for (const [index, answer] of answers.entries()) {
        if (answer.status !== 'fulfilled' || !Array.isArray(answer.value)) continue
        for (const run of answer.value) {
          found.push({ ...run, sessionId: page[index] })
        }
      }
    }
    return { asked, found }
  }

  function newestRunFirst(left, right) {
    return (right?.started_at ?? 0) - (left?.started_at ?? 0)
  }

  $effect(() => {
    const id = String(projectId || '')
    const key = `${id}\u0000${liveSessionKey}`
    const runKey = liveRunKey
    if (key === loadedKey && runKey === loadedRunKey) return
    // A different project or a different set of sessions is a different list.
    // A run starting or finishing inside the same sessions is the same list
    // moving, so the rows and the open run stay while it reloads.
    const restart = key !== loadedKey
    loadedKey = key
    loadedRunKey = runKey

    const token = (loadToken += 1)
    const liveSessionIds = liveSessionKey ? liveSessionKey.split('\u0000') : []

    if (restart) {
      runs = []
      askedCount = 0
      selected = null
      detail = null
      ledgerRow = null
      copied = false
    }

    void (async () => {
      let taskSessions = []
      let archivedSessions = []
      if (id) {
        const [tasks, archived] = await Promise.allSettled([
          getProjectTasks(id),
          getArchivedSessions(id),
        ])
        taskSessions =
          tasks.status === 'fulfilled' && Array.isArray(tasks.value?.tasks) ? tasks.value.tasks : []
        archivedSessions =
          archived.status === 'fulfilled' && Array.isArray(archived.value?.sessions)
            ? archived.value.sessions
            : []
      }
      if (token !== loadToken) return

      candidateIds = orderWorkflowSessionIds(
        liveSessionIds.map((sessionId) => ({ session_id: sessionId })),
        taskSessions,
        archivedSessions
      )

      const floor = restart ? 0 : askedCount
      const result = await askSessions(candidateIds, {
        start: 0,
        floor,
        limit: Math.max(MAX_SESSIONS, floor),
        token,
      })
      if (!result) return

      askedCount = result.asked
      askedSessions = result.asked
      candidateSessions = candidateIds.length
      sessionsTruncated = candidateIds.length > result.asked
      runs = result.found.sort(newestRunFirst)
    })()
  })

  /**
   * Carry the same search on from where it stopped. Everything already on
   * screen stays; one more budget of sessions is asked, and the control
   * disappears when there is no session left to ask.
   */
  async function loadMore() {
    if (loadingMore || askedCount >= candidateIds.length) return
    const token = loadToken
    loadingMore = true
    try {
      const result = await askSessions(candidateIds, {
        start: askedCount,
        floor: 0,
        limit: askedCount + MAX_SESSIONS,
        token,
      })
      if (!result || token !== loadToken) return

      askedCount = result.asked
      askedSessions = result.asked
      candidateSessions = candidateIds.length
      sessionsTruncated = candidateIds.length > result.asked
      runs = [...runs, ...result.found].sort(newestRunFirst)
    } finally {
      loadingMore = false
    }
  }

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

{#if rows.length > 0 || moreSessions}
  <section
    class="py-5 border-b {t.keyline}"
    class:is-dark={dark}
    data-testid="overview-workflow-runs"
  >
    <div class="flex items-center justify-between mb-3">
      <span class="text-[11px] {t.textTertiary}">Workflow runs</span>
      <span class="text-[11px] {t.textTertiary}">{countLabel}{searchedLabel}</span>
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

    {#if moreSessions}
      <button
        type="button"
        class="mt-2 text-[11px] {t.textTertiary} hover:underline disabled:no-underline disabled:opacity-40 disabled:cursor-not-allowed"
        data-testid="workflow-load-more"
        disabled={loadingMore}
        title="Ask the next older sessions this project can name"
        onclick={loadMore}
      >{loadMoreLabel}</button>
    {/if}
  </section>
{/if}

<style>
  /* Fixed widths so every run name starts on one left edge and the agent rows
     under it start on another, whatever the status word is. */
  .workflow-run-status {
    display: inline-flex;
    align-items: center;
    justify-content: center;
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

  .workflow-run-status[data-status] {
    min-width: 70px;
  }

  .workflow-run-status[data-state] {
    min-width: 52px;
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

  /* Themed from the panel's own `dark` prop, like every other component here,
     so a surface rendered dark inside a light document still comes out right. */
  .is-dark .workflow-run-status[data-status='completed'],
  .is-dark .workflow-run-status[data-state='done'] {
    color: var(--color-brand-400);
  }

  .is-dark .workflow-run-status[data-status='live'],
  .is-dark .workflow-run-status[data-state='running'] {
    color: var(--color-success-400);
  }
</style>
