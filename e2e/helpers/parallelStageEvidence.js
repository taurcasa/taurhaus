function timestamp(value) {
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : null
}

/** Capture mesh's delivery timestamp while its attention projection is live. */
export async function captureStageDelivery({
  taskId,
  owner,
  timeout,
  waitUntil,
  refreshTask,
  readAttention,
}) {
  let deliveredAt = null
  const timeoutMsg =
    `mesh attention projection for ${owner}'s task #${taskId} never exposed deliveredAt ` +
    `before the task left the attention set`
  await waitUntil(
    async () => {
      refreshTask()
      const attention = readAttention()
      deliveredAt = attention?.deliveredAt ?? attention?.delivered_at ?? null
      return timestamp(deliveredAt) != null
    },
    { timeout, interval: 2_000, timeoutMsg }
  )
  if (timestamp(deliveredAt) == null) throw new Error(timeoutMsg)
  return deliveredAt
}

/** The stage label prefix and phase emitted by the production workflow. */
export function managedStageVocabulary(workflowSource, harness) {
  const source = String(workflowSource ?? '')
  const harnessName = String(harness ?? '').trim()
  const match = source.match(
    /call\(\{\s*label:\s*'([^']*)'\s*\+\s*harness\s*\+\s*'([^']*)'\s*\+\s*slug,\s*phase:\s*'([^']+)'/
  )
  if (!match || !harnessName) {
    throw new Error('production workflow has no readable managed-stage vocabulary')
  }
  return {
    labelPrefix: `${match[1]}${harnessName}${match[2]}`,
    phaseTitle: match[3],
  }
}

/** The positive-duration intersection of two delivered-to-RESULT windows. */
export function stageWindowOverlap(left, right) {
  const leftStart = timestamp(left?.deliveredAt)
  const leftEnd = timestamp(left?.resultAt)
  const rightStart = timestamp(right?.deliveredAt)
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
 * A production-shaped completed Workflow summary for a W2 scanner read-back.
 *
 * This is synthesized scanner-contract evidence, not a summary emitted by the
 * credential-free lead. Its vocabulary must be captured from the production
 * workflow so emitter drift makes the lane fail instead of certifying itself.
 */
export function completedParallelRunSummary({
  runId,
  workflowName,
  startedAt,
  finishedAt,
  stages,
  vocabulary,
}) {
  const startTime = timestamp(startedAt)
  const finishTime = timestamp(finishedAt)
  if (startTime == null || finishTime == null || finishTime < startTime) {
    throw new Error('parallel run summary requires an ordered start and finish')
  }
  const labelPrefix = String(vocabulary?.labelPrefix ?? '')
  const phaseTitle = String(vocabulary?.phaseTitle ?? '')
  if (!labelPrefix || !phaseTitle) {
    throw new Error('parallel run summary requires production workflow vocabulary')
  }

  const workflowProgress = stages.map((stage) => {
    const resultAt = timestamp(stage.resultAt)
    if (resultAt == null) throw new Error(`parallel stage ${stage.key} has no result timestamp`)
    return {
      type: 'workflow_agent',
      agentId: `stage-${stage.key}-${stage.taskId}`,
      label: `${labelPrefix}${stage.key}`,
      phaseTitle,
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
      evidenceSource: 'synthesized-scanner-contract',
      tasks: stages.map((stage) => String(stage.taskId)),
    },
    agentCount: workflowProgress.length,
    durationMs: finishTime - startTime,
    totalTokens: 0,
    totalToolCalls: 0,
    workflowName,
    phases: [{ title: phaseTitle }],
    startTime,
    timestamp: new Date(finishTime).toISOString(),
    workflowProgress,
  }
}
