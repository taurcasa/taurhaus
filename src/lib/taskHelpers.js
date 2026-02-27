/** Status badge CSS class by task status. */
export function statusBadgeClass(status) {
  switch (status) {
    case 'in_progress': return 'bg-success-400/15 text-success-400'
    case 'pending': return 'bg-info-400/15 text-info-400'
    case 'completed': return 'bg-zinc-500/15 text-zinc-500'
    default: return 'bg-zinc-500/15 text-zinc-500'
  }
}

/** Status display label. */
export function statusLabel(status) {
  switch (status) {
    case 'in_progress': return 'In Progress'
    case 'pending': return 'Pending'
    case 'completed': return 'Done'
    default: return status
  }
}
