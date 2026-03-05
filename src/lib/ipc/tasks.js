import {
  buildMockArchivedSessions,
  buildMockCommitDiff,
  buildMockCommitFiles,
  buildMockCommitsInRange,
  buildMockProjectTasks,
  buildMockTaskDetail,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'

export function getProjectTasks(projectPath) {
  return invokeOrMock('get_project_tasks', { projectPath }, () => buildMockProjectTasks())
}

export function getTaskDetail(projectPath, taskId, source, sourceKey) {
  return invokeOrMock('get_task_detail', { projectPath, taskId, source, sourceKey }, () =>
    buildMockTaskDetail(taskId, source, sourceKey)
  )
}

export function getArchivedSessions(projectPath) {
  return invokeOrMock('get_archived_sessions', { projectPath }, () => buildMockArchivedSessions())
}

export function getCommitFiles(projectPath, hash) {
  return invokeOrMock('get_commit_files', { projectPath, hash }, () => buildMockCommitFiles())
}

export function getCommitDiff(projectPath, hash, filePath) {
  return invokeOrMock('get_commit_diff', { projectPath, hash, filePath }, () => buildMockCommitDiff())
}

export function getCommitsInRange(projectPath, after, before) {
  return invokeOrMock('get_commits_in_range', { projectPath, after, before }, () =>
    buildMockCommitsInRange()
  )
}
