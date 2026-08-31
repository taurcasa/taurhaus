function timestamp(value) {
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : null
}

/** The positive-duration intersection of two assigned-to-RESULT windows. */
export function stageWindowOverlap(left, right) {
  const leftStart = timestamp(left?.assignedAt)
  const leftEnd = timestamp(left?.resultAt)
  const rightStart = timestamp(right?.assignedAt)
  const rightEnd = timestamp(right?.resultAt)
  if ([leftStart, leftEnd, rightStart, rightEnd].some((value) => value == null)) return null
  if (leftEnd <= leftStart || rightEnd <= rightStart) return null

  const start = Math.max(leftStart, rightStart)
  const end = Math.min(leftEnd, rightEnd)
  if (end <= start) return null
  return {
    startAt: new Date(start).toISOString(),
    endAt: new Date(end).toISOString(),
    durationMs: end - start,
  }
}

/**
 * The completed Workflow summary the W2 scanner reads for the isolated lead.
 *
 * A managed stage appears as its thin Claude courier in this tree. The real
 * Codex member remains a sibling team session, so both entries deliberately
 * use the production `stage:codex:<title>` label and `Managed stage` phase.
 */
export function completedParallelRunSummary({
  runId,
  workflowName,
  startedAt,
  finishedAt,
  stages,
}) {
  const startTime = timestamp(startedAt)
  const finishTime = timestamp(finishedAt)
  if (startTime == null || finishTime == null || finishTime < startTime) {
    throw new Error('parallel run summary requires an ordered start and finish')
  }

  const workflowProgress = stages.map((stage) => {
    const resultAt = timestamp(stage.resultAt)
    if (resultAt == null) throw new Error(`parallel stage ${stage.key} has no result timestamp`)
    return {
      type: 'workflow_agent',
      agentId: `stage-${stage.key}-${stage.taskId}`,
      label: `stage:codex:${stage.key}`,
      phaseTitle: 'Managed stage',
      model: stage.model ?? 'codex',
      state: 'done',
      lastToolName: 'Bash',
      lastProgressAt: resultAt,
      tokens: 0,
      toolCalls: 0,
      promptPreview: `Courier for managed Codex stage ${stage.key}`,
      resultPreview: { taskId: String(stage.taskId), status: 'completed' },
    }
  })

  return {
    runId,
    status: 'completed',
    result: {
      experiment: 'w4-experiment-5',
      tasks: stages.map((stage) => String(stage.taskId)),
    },
    agentCount: workflowProgress.length,
    durationMs: finishTime - startTime,
    totalTokens: 0,
    totalToolCalls: 0,
    workflowName,
    phases: [{ title: 'Managed stage' }],
    startTime,
    timestamp: new Date(finishTime).toISOString(),
    workflowProgress,
  }
}
