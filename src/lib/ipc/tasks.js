import {
  buildMockArchivedSessions,
  buildMockCommitDiff,
  buildMockCommitFiles,
  buildMockCommitsInRange,
  buildMockProjectTasks,
  buildMockTaskDetail,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'

function normalizeDiffLine(raw) {
  const line = raw && typeof raw === 'object' ? raw : {}
  return {
    ...line,
    old_lineno: line.old_lineno ?? line.oldLineno ?? null,
    new_lineno: line.new_lineno ?? line.newLineno ?? null,
  }
}

function normalizeDiffHunk(raw) {
  const hunk = raw && typeof raw === 'object' ? raw : {}
  return {
    ...hunk,
    old_start: hunk.old_start ?? hunk.oldStart ?? 0,
    old_lines: hunk.old_lines ?? hunk.oldLines ?? 0,
    new_start: hunk.new_start ?? hunk.newStart ?? 0,
    new_lines: hunk.new_lines ?? hunk.newLines ?? 0,
    lines: Array.isArray(hunk.lines) ? hunk.lines.map(normalizeDiffLine) : [],
  }
}

function normalizeCommitDiff(raw) {
  if (!Array.isArray(raw)) return []
  return raw.map(normalizeDiffHunk)
}

function normalizeCommitsInRange(raw) {
  const result = raw && typeof raw === 'object' ? raw : {}
  return {
    ...result,
    commits: Array.isArray(result.commits) ? result.commits : [],
    files: Array.isArray(result.files) ? result.files : [],
    truncated: Boolean(result.truncated),
    total_count: result.total_count ?? result.totalCount ?? null,
  }
}

export function getProjectTasks(projectId) {
  return invokeOrMock('get_project_tasks', { projectId }, () => buildMockProjectTasks())
}

export function getTaskDetail(projectId, taskId, source, sourceKey) {
  return invokeOrMock('get_task_detail', { projectId, taskId, source, sourceKey }, () =>
    buildMockTaskDetail(taskId, source, sourceKey)
  )
}

export function getArchivedSessions(projectId) {
  return invokeOrMock('get_archived_sessions', { projectId }, () => buildMockArchivedSessions())
}

export function getCommitFiles(projectId, hash) {
  return invokeOrMock('get_commit_files', { projectId, hash }, () => buildMockCommitFiles())
}

export function getCommitDiff(projectId, hash, filePath) {
  return invokeOrMock('get_commit_diff', { projectId, hash, filePath }, () => buildMockCommitDiff()).then(normalizeCommitDiff)
}

export function getCommitsInRange(projectId, after, before) {
  return invokeOrMock('get_commits_in_range', { projectId, after, before }, () =>
    buildMockCommitsInRange()
  ).then(normalizeCommitsInRange)
}
