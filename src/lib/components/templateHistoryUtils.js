export function normalizeStatus(value) {
  return {
    mode: value?.mode ?? 'plain_filesystem',
    repoInitialized: Boolean(value?.repoInitialized ?? value?.repo_initialized),
    dirty: Boolean(value?.dirty),
    pendingActions: Array.isArray(value?.pendingActions ?? value?.pending_actions)
      ? value?.pendingActions ?? value?.pending_actions
      : [],
    lastCommit: value?.lastCommit ?? value?.last_commit ?? null,
  }
}

export function normalizeCommit(value) {
  return {
    commitId: value?.commitId ?? value?.commit_id ?? '',
    shortId: value?.shortId ?? value?.short_id ?? '',
    message: value?.message ?? '',
    author: value?.author ?? 'unknown',
    timestamp: Number(value?.timestamp ?? 0),
    changedPaths: Array.isArray(value?.changedPaths ?? value?.changed_paths)
      ? value?.changedPaths ?? value?.changed_paths
      : [],
  }
}

export function normalizeDiff(value) {
  return {
    commitId: value?.commitId ?? value?.commit_id ?? '',
    files: Array.isArray(value?.files) ? value.files : [],
    stats: {
      filesChanged: value?.stats?.filesChanged ?? value?.stats?.files_changed ?? 0,
      insertions: value?.stats?.insertions ?? 0,
      deletions: value?.stats?.deletions ?? 0,
    },
  }
}

export function formatTimestamp(unixSeconds) {
  if (!unixSeconds) return 'Unknown time'
  try {
    return new Date(unixSeconds * 1000).toLocaleString()
  } catch {
    return String(unixSeconds)
  }
}
