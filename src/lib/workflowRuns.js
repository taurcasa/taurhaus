/**
 * Presentation shaping for Claude Code workflow runs.
 *
 * The backend hands over exactly what it read off disk
 * (`docs/architecture/workflow-runs.md`): a run, its declared phase titles and
 * its agents, with `null` wherever it refused to guess — a live agent has no
 * label and no phase, and a transcript too large to total exactly reports
 * `null` tokens rather than a partial count. This module turns that into rows
 * and labels without inventing any of the missing values, so every surface
 * (canvas tree, sidebar, run history) says the same thing about a run.
 *
 * Nothing here reads the clock: a duration comes from the run's own totals or
 * its finished window, never from "now", so the same run renders identically
 * on every re-render.
 */

const LABEL_MAX_LENGTH = 48

const STATUS_LABELS = {
  live: 'Live',
  completed: 'Completed',
  failed: 'Failed',
  unknown: 'Unknown',
}

function text(value) {
  return typeof value === 'string' ? value.trim() : ''
}

function count(value) {
  const parsed = typeof value === 'number' ? value : Number.NaN
  return Number.isFinite(parsed) ? parsed : null
}

function runStatus(run) {
  const raw = text(run?.status).toLowerCase()
  return raw in STATUS_LABELS ? raw : 'unknown'
}

/** The Claude session id a record carries, in either spelling. `''` when it has none. */
export function workflowSessionId(record) {
  return text(record?.session_id) || text(record?.sessionId)
}

/**
 * Session ids across any number of record lists, de-duplicated, first seen
 * first. The run APIs are keyed by session, so this is how a project-wide view
 * decides which sessions to ask about.
 */
export function collectWorkflowSessionIds(...sources) {
  const seen = new Set()
  const ids = []
  for (const source of sources) {
    if (!Array.isArray(source)) continue
    for (const record of source) {
      const id = workflowSessionId(record)
      if (!id || seen.has(id)) continue
      seen.add(id)
      ids.push(id)
    }
  }
  return ids
}

function trimNumber(value) {
  return value.toFixed(1).replace(/\.0$/, '')
}

/**
 * A token total, compactly. `null` when the scanner reported no exact count —
 * a partial number would read as fact.
 */
export function formatTokens(tokens) {
  const value = count(tokens)
  if (value === null || value < 0) return null
  if (value < 1000) return String(Math.round(value))
  if (value < 1_000_000) return `${trimNumber(value / 1000)}k`
  return `${trimNumber(value / 1_000_000)}M`
}

function formatMillis(ms) {
  if (ms < 60_000) return `${Math.round(ms / 1000)}s`
  const totalSeconds = Math.round(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  if (minutes < 60) return seconds > 0 ? `${minutes}m ${seconds}s` : `${minutes}m`
  const hours = Math.floor(minutes / 60)
  return `${hours}h ${minutes % 60}m`
}

/**
 * How long a run took: its own total when the summary carries one, otherwise
 * the finished window. A live run has neither, and gets `null`.
 */
export function formatRunDuration(run) {
  const total = count(run?.totals?.duration_ms)
  if (total !== null && total >= 0) return formatMillis(total)

  const startedAt = count(run?.started_at)
  const finishedAt = count(run?.finished_at)
  if (startedAt === null || finishedAt === null || finishedAt < startedAt) return null
  return formatMillis(finishedAt - startedAt)
}

function agentLabel(agent) {
  const label = text(agent?.label)
  if (label) return label

  const preview = text(agent?.prompt_preview) || text(agent?.promptPreview)
  if (!preview) return text(agent?.agent_id) || text(agent?.agentId) || 'agent'
  if (preview.length <= LABEL_MAX_LENGTH) return preview
  return `${preview.slice(0, LABEL_MAX_LENGTH - 1)}…`
}

function agentRow(agent) {
  return {
    agentId: text(agent?.agent_id) || text(agent?.agentId),
    label: agentLabel(agent),
    model: text(agent?.model),
    state: text(agent?.state).toLowerCase() || 'queued',
    lastTool: text(agent?.last_tool) || text(agent?.lastTool),
    tokensLabel: formatTokens(agent?.tokens),
  }
}

/**
 * Agents grouped into rendered rows: the script's declared phases first, in the
 * order the script declared them, then any phase the run reported that the
 * script did not declare, then the agents that carry no phase at all. A group
 * with a `null` title renders no phase row — the scanner does not infer a
 * phase for a live agent and neither do we.
 */
function groupAgents(run, agents) {
  const declared = Array.isArray(run?.phases) ? run.phases.map(text).filter(Boolean) : []
  const byPhase = new Map()
  const unphased = []

  for (const agent of agents) {
    const phase = text(agent?.phase)
    if (!phase) {
      unphased.push(agentRow(agent))
      continue
    }
    if (!byPhase.has(phase)) byPhase.set(phase, [])
    byPhase.get(phase).push(agentRow(agent))
  }

  const groups = []
  for (const phase of declared) {
    if (!byPhase.has(phase)) continue
    groups.push({ title: phase, agents: byPhase.get(phase) })
    byPhase.delete(phase)
  }
  for (const [phase, rows] of byPhase) {
    groups.push({ title: phase, agents: rows })
  }
  if (unphased.length > 0) {
    groups.push({ title: null, agents: unphased })
  }
  return groups
}

function runSummaryLine(run, status) {
  const name = text(run?.name) || text(run?.run_id)
  const parts = [name]

  const agents = count(run?.totals?.agents)
  const done = count(run?.totals?.done)
  if (agents !== null && done !== null) parts.push(`${done}/${agents} done`)

  const tokens = formatTokens(run?.totals?.tokens)
  if (tokens) parts.push(`${tokens} tokens`)

  const duration = formatRunDuration(run)
  if (duration) parts.push(duration)

  if (status === 'failed' && parts.length === 1) parts.push('failed')
  return parts.join(' · ')
}

/**
 * The run tree a canvas node renders: expanded phase/agent rows while the run
 * is live, one summary line once it has finished. `rowCount` is what the mesh
 * layout sizes the child box from.
 */
export function runTreeModel(run) {
  const runId = text(run?.run_id) || text(run?.runId)
  if (!runId) return null

  const status = runStatus(run)
  const isLive = status === 'live'
  const agents = Array.isArray(run?.agents) ? run.agents : []
  const groups = isLive ? groupAgents(run, agents) : []
  const rowCount = groups.reduce(
    (total, group) => total + group.agents.length + (group.title ? 1 : 0),
    0
  )

  return {
    runId,
    name: text(run?.name) || runId,
    status,
    isLive,
    groups,
    rowCount,
    summary: runSummaryLine(run, status),
  }
}

/** One row of the run-history list, built from a `WorkflowRunSummary`. */
export function runListRow(summary) {
  const status = runStatus(summary)
  const phases = Array.isArray(summary?.phases) ? summary.phases.map(text).filter(Boolean) : []
  const agents = count(summary?.totals?.agents)
  const done = count(summary?.totals?.done)

  return {
    runId: text(summary?.run_id) || text(summary?.runId),
    name: text(summary?.name) || text(summary?.run_id),
    description: text(summary?.description),
    status,
    statusLabel: STATUS_LABELS[status],
    isLive: status === 'live',
    phasesLabel: phases.join(' · '),
    phaseCount: phases.length,
    doneLabel: agents !== null && done !== null ? `${done}/${agents}` : null,
    tokensLabel: formatTokens(summary?.totals?.tokens),
    durationLabel: formatRunDuration(summary),
    startedAt: count(summary?.started_at) ?? 0,
  }
}
