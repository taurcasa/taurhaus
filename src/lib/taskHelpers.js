/** Status badge CSS class by task status. */
export function statusBadgeClass(status) {
  switch (status) {
    case 'in_progress': return 'bg-success-400/15 text-success-400'
    case 'pending': return 'bg-info-400/15 text-info-400'
    case 'completed': return 'bg-zinc-500/15 text-zinc-500'
    case 'stale': return 'bg-zinc-500/15 text-zinc-500'
    default: return 'bg-zinc-500/15 text-zinc-500'
  }
}

/** Status display label. */
export function statusLabel(status) {
  switch (status) {
    case 'in_progress': return 'In Progress'
    case 'pending': return 'Pending'
    case 'completed': return 'Done'
    case 'stale': return 'Timed out'
    default: return status
  }
}

function parseTimeMs(iso) {
  if (!iso) return 0
  const ms = new Date(iso).getTime()
  return Number.isFinite(ms) ? ms : 0
}

function recencyTimeMs(task) {
  return Math.max(
    parseTimeMs(task.state_changed_at),
    parseTimeMs(task.updated_at),
    parseTimeMs(task.archived_at),
  )
}

function dependencyCount(task) {
  return Array.isArray(task.blocked_by) ? task.blocked_by.length : 0
}

function taskSortIdentity(task) {
  const source = task?.source || ''
  const sourceKey = task?.source_key || ''
  const sourceTaskId = task?.id || task?.source_task_id || ''
  return `${source}/${sourceKey}/${sourceTaskId}`
}

const EMPTY_GROUPED_TASKS = Object.freeze({
  in_progress: Object.freeze([]),
  pending: Object.freeze([]),
  completed: Object.freeze([]),
})

const groupedTaskCache = new WeakMap()

/**
 * Group and sort tasks by status.
 *
 * Memoized by task-array identity so identical input references return
 * identical grouped object references (helps avoid downstream re-renders).
 */
export function groupTasksByStatus(tasks) {
  if (!Array.isArray(tasks) || tasks.length === 0) return EMPTY_GROUPED_TASKS

  const cached = groupedTaskCache.get(tasks)
  if (cached) return cached

  const grouped = {
    in_progress: tasks
      .filter((task) => task.status === 'in_progress')
      .slice()
      .sort((a, b) => {
        const recencyDelta = recencyTimeMs(b) - recencyTimeMs(a)
        if (recencyDelta !== 0) return recencyDelta
        return taskSortIdentity(a).localeCompare(taskSortIdentity(b))
      }),
    pending: tasks
      .filter((task) => task.status === 'pending')
      .slice()
      .sort((a, b) => {
        const depDelta = dependencyCount(b) - dependencyCount(a)
        if (depDelta !== 0) return depDelta
        const recencyDelta = recencyTimeMs(b) - recencyTimeMs(a)
        if (recencyDelta !== 0) return recencyDelta
        return taskSortIdentity(a).localeCompare(taskSortIdentity(b))
      }),
    completed: tasks
      .filter((task) => task.status === 'completed' || task.status === 'stale')
      .slice()
      .sort((a, b) => {
        const updatedDelta = parseTimeMs(b.updated_at) - parseTimeMs(a.updated_at)
        if (updatedDelta !== 0) return updatedDelta
        return taskSortIdentity(a).localeCompare(taskSortIdentity(b))
      }),
  }

  groupedTaskCache.set(tasks, grouped)
  return grouped
}
