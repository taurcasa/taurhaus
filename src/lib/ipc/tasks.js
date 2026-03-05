import {
  buildMockArchivedSessions,
  buildMockCommitDiff,
  buildMockCommitFiles,
  buildMockCommitsInRange,
  buildMockProjectTasks,
  buildMockTaskDetail,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'

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
  return invokeOrMock('get_commit_diff', { projectId, hash, filePath }, () => buildMockCommitDiff())
}

export function getCommitsInRange(projectId, after, before) {
  return invokeOrMock('get_commits_in_range', { projectId, after, before }, () =>
    buildMockCommitsInRange()
  )
}
