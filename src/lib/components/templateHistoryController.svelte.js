import {
  getTemplateDiff,
  getTemplateHistory,
  getTemplateStorageStatus,
  revertTemplateVersion,
} from '../ipc.js'
import { formatTimestamp, normalizeCommit, normalizeDiff, normalizeStatus } from './templateHistoryUtils.js'

const PAGE_SIZE = 50

export function createTemplateHistoryController({
  getSelectedTemplateId,
  getSelectedTemplateKind,
  getOnReverted,
}) {
  let loadingStatus = $state(false)
  let loadingHistory = $state(false)
  let loadingDiff = $state(false)
  let loadingMore = $state(false)
  let reverting = $state(false)
  let errorMessage = $state('')
  let infoMessage = $state('')
  let storageStatus = $state({
    mode: 'plain_filesystem',
    repoInitialized: false,
    dirty: false,
    pendingActions: [],
    lastCommit: null,
  })
  let commits = $state([])
  let nextCursor = $state(null)
  let selectedCommitId = $state('')
  let diffLoadSequence = 0
  let diff = $state({
    commitId: '',
    files: [],
    stats: { filesChanged: 0, insertions: 0, deletions: 0 },
  })
  let selectedDiffPath = $state('')
  let scope = $state('global')

  const selectedCommit = $derived.by(() =>
    commits.find((commit) => commit.commitId === selectedCommitId) ?? null
  )

  const templatePathCandidates = $derived.by(() => {
    const id = String(getSelectedTemplateId() ?? '').trim()
    const kind = getSelectedTemplateKind()
    if (!id) return []
    if (kind === 'role') return [`roles/${id}.yaml`]
    if (kind === 'preset') return [`presets/${id}.yaml`]
    return [`roles/${id}.yaml`, `presets/${id}.yaml`]
  })

  const visibleCommits = $derived.by(() => {
    if (scope !== 'template') return commits
    return commits.filter((commit) => {
      const changedPaths = commit?.changedPaths ?? []
      return templatePathCandidates.some((candidate) => changedPaths.includes(candidate))
    })
  })

  const selectedDiffFile = $derived.by(() =>
    diff.files.find((file) => file.path === selectedDiffPath) ?? null
  )

  const commitsEmptyMessage = $derived.by(() =>
    scope === 'template'
      ? 'No commits affect the selected template yet.'
      : 'No template commits found.'
  )

  async function loadStatus() {
    loadingStatus = true
    try {
      storageStatus = normalizeStatus(await getTemplateStorageStatus())
    } catch (error) {
      errorMessage = error?.message || 'Failed to load template storage status.'
    } finally {
      loadingStatus = false
    }
  }

  async function loadHistory() {
    loadingHistory = true
    errorMessage = ''
    infoMessage = ''
    try {
      const page = await getTemplateHistory(PAGE_SIZE, null)
      commits = Array.isArray(page?.commits) ? page.commits.map(normalizeCommit) : []
      nextCursor = page?.nextCursor ?? page?.next_cursor ?? null
      selectedCommitId = commits[0]?.commitId ?? ''
    } catch (error) {
      commits = []
      selectedCommitId = ''
      nextCursor = null
      errorMessage = error?.message || 'Failed to load template history.'
    } finally {
      loadingHistory = false
    }
  }

  async function loadMore() {
    if (!nextCursor || loadingMore) return
    loadingMore = true
    try {
      const page = await getTemplateHistory(PAGE_SIZE, nextCursor)
      const additional = Array.isArray(page?.commits) ? page.commits.map(normalizeCommit) : []
      commits = [...commits, ...additional]
      nextCursor = page?.nextCursor ?? page?.next_cursor ?? null
    } catch (error) {
      errorMessage = error?.message || 'Failed to load more history.'
    } finally {
      loadingMore = false
    }
  }

  async function loadDiff(commitId) {
    const sequence = ++diffLoadSequence
    if (!commitId) {
      loadingDiff = false
      diff = {
        commitId: '',
        files: [],
        stats: { filesChanged: 0, insertions: 0, deletions: 0 },
      }
      selectedDiffPath = ''
      return
    }

    loadingDiff = true
    try {
      const result = normalizeDiff(await getTemplateDiff(commitId))
      if (sequence !== diffLoadSequence) return
      diff = result
      selectedDiffPath = result.files[0]?.path ?? ''
    } catch (error) {
      if (sequence !== diffLoadSequence) return
      diff = {
        commitId,
        files: [],
        stats: { filesChanged: 0, insertions: 0, deletions: 0 },
      }
      selectedDiffPath = ''
      errorMessage = error?.message || 'Failed to load template diff.'
    } finally {
      if (sequence === diffLoadSequence) {
        loadingDiff = false
      }
    }
  }

  async function revertSelected() {
    const selectedTemplateId = getSelectedTemplateId()
    if (!selectedTemplateId || !selectedCommitId || reverting) return
    reverting = true
    errorMessage = ''
    infoMessage = ''
    try {
      await revertTemplateVersion(selectedTemplateId, selectedCommitId)
      infoMessage = `Reverted ${selectedTemplateId} to ${selectedCommit?.shortId || selectedCommitId.slice(0, 8)}.`
      await Promise.all([loadStatus(), loadHistory()])
      getOnReverted()({ templateId: selectedTemplateId, commitId: selectedCommitId })
    } catch (error) {
      errorMessage = error?.message || 'Failed to revert template.'
    } finally {
      reverting = false
    }
  }

  function setScope(value) {
    scope = value
  }

  function scopeButtonTone(neutralButton, value) {
    if (scope === value) {
      return 'border-brand-600 bg-brand-600 text-white'
    }
    return neutralButton
  }

  function setSelectedCommit(value) {
    selectedCommitId = value
  }

  function setSelectedDiffPath(value) {
    selectedDiffPath = value
  }

  $effect(() => {
    void Promise.all([loadStatus(), loadHistory()])
  })

  $effect(() => {
    if (scope === 'template' && !getSelectedTemplateId()) {
      scope = 'global'
    }
  })

  $effect(() => {
    const visible = visibleCommits
    if (visible.length === 0) {
      selectedCommitId = ''
      return
    }
    if (!visible.some((commit) => commit.commitId === selectedCommitId)) {
      selectedCommitId = visible[0].commitId
    }
  })

  $effect(() => {
    void loadDiff(selectedCommitId)
  })

  return {
    formatTimestamp,
    get loadingStatus() {
      return loadingStatus
    },
    get loadingHistory() {
      return loadingHistory
    },
    get loadingDiff() {
      return loadingDiff
    },
    get loadingMore() {
      return loadingMore
    },
    get reverting() {
      return reverting
    },
    get errorMessage() {
      return errorMessage
    },
    get infoMessage() {
      return infoMessage
    },
    get storageStatus() {
      return storageStatus
    },
    get selectedCommit() {
      return selectedCommit
    },
    get visibleCommits() {
      return visibleCommits
    },
    get commitsEmptyMessage() {
      return commitsEmptyMessage
    },
    get nextCursor() {
      return nextCursor
    },
    get selectedCommitId() {
      return selectedCommitId
    },
    get diff() {
      return diff
    },
    get selectedDiffPath() {
      return selectedDiffPath
    },
    get selectedDiffFile() {
      return selectedDiffFile
    },
    get scope() {
      return scope
    },
    loadMore,
    revertSelected,
    setScope,
    scopeButtonTone,
    setSelectedCommit,
    setSelectedDiffPath,
  }
}
