import {
  MOCK_COMMITS,
  MOCK_DETAIL,
  MOCK_FILE_TREE,
  MOCK_PROJECTS,
  MOCK_RELATIONSHIPS,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'

function normalizeProjectRecord(project) {
  if (!project || typeof project !== 'object') return project

  const normalized = { ...project }
  if (normalized.activity_state === undefined && normalized.activityState !== undefined) {
    normalized.activity_state = normalized.activityState
  }
  if (normalized.last_activity_at === undefined && normalized.lastActivityAt !== undefined) {
    normalized.last_activity_at = normalized.lastActivityAt
  }
  if (normalized.hero_preference === undefined && normalized.heroPreference !== undefined) {
    normalized.hero_preference = normalized.heroPreference
  }
  if (normalized.is_dirty === undefined && normalized.isDirty !== undefined) {
    normalized.is_dirty = normalized.isDirty
  }
  return normalized
}

function normalizeProjectList(projects) {
  if (!Array.isArray(projects)) return []
  return projects.map((project) => normalizeProjectRecord(project))
}

export function listProjects() {
  return invokeOrMock('list_projects', undefined, () => MOCK_PROJECTS).then((projects) =>
    normalizeProjectList(projects)
  )
}

export function getProject(projectId) {
  return invokeOrMock('get_project', { projectId }, () => ({ ...MOCK_DETAIL, id: projectId })).then(
    (project) => normalizeProjectRecord(project)
  )
}

export function registerProject(path, name) {
  return invokeOrMock('register_project', { path, name }, () => ({
    ...MOCK_DETAIL,
    path,
    name: name || path.split('/').pop(),
  })).then((project) => normalizeProjectRecord(project))
}

export function createProject(name, parentDir) {
  return invokeOrMock('create_project', { name, parentDir }, () => ({
    ...MOCK_DETAIL,
    name,
    path: `${parentDir.replace(/[\\/]+$/, '')}/${name}`,
  })).then((project) => normalizeProjectRecord(project))
}

export function updateProject(projectId, fields) {
  return invokeOrMock('update_project', { projectId, fields }, () => ({
    ...MOCK_DETAIL,
    id: projectId,
    ...fields,
  })).then((project) => normalizeProjectRecord(project))
}

export function removeProject(projectId) {
  return invokeOrMock('remove_project', { projectId }, () => undefined)
}

export function scanDirectory(path) {
  return invokeOrMock('scan_directory', { path }, () => [])
}

export function listDirectory(path) {
  return invokeOrMock('list_directory', { path }, () => [
    { name: 'project-a', path: `${path}/project-a`, isExpandable: true },
    { name: 'project-b', path: `${path}/project-b`, isExpandable: false },
  ])
}

export function getSystemRoots() {
  return invokeOrMock('get_system_roots', undefined, () => [{ name: '/', path: '/', isExpandable: true }])
}

export function validateProjectPath(path) {
  return invokeOrMock('validate_project_path', { path }, () => ({
    exists: true,
    isGitRepo: true,
    isRegistered: false,
  }))
}

export function getRecentCommits(projectId, limit = 10) {
  return invokeOrMock('get_recent_commits', { projectId, limit }, () => MOCK_COMMITS)
}

export function getAllCommits(projectId, limit = 50, offset = 0) {
  return invokeOrMock('get_all_commits', { projectId, limit, offset }, () => MOCK_COMMITS)
}

export function getGitStatus(projectId) {
  return invokeOrMock('get_git_status', { projectId }, () => ({
    branch: 'main',
    is_dirty: false,
    ahead: 0,
    behind: 0,
  }))
}

export function getRemoteUrl(projectId) {
  return invokeOrMock('get_remote_url', { projectId }, () => null)
}

export function getFileTree(projectId) {
  return invokeOrMock('get_file_tree', { projectId }, () => MOCK_FILE_TREE)
}

export function readFile(projectId, relativePath) {
  return invokeOrMock('read_file', { projectId, relativePath }, () => ({
    path: relativePath,
    content: '// Mock file content',
    language: 'javascript',
  }))
}

export function readProjectAsset(projectId, relativePath) {
  return invokeOrMock('read_project_asset', { projectId, relativePath }, () => null)
}

export function checkPathType(projectId, relativePath) {
  return invokeOrMock('check_path_type', { projectId, relativePath }, () => 'not_found')
}

export function getReadme(projectId) {
  return invokeOrMock('get_readme', { projectId }, () => {
    if (!projectId || projectId === 'missing-project') {
      return null
    }

    return {
      path: 'README.md',
      content: '# Mock Project\n\nThis is a mock README.',
      language: 'markdown',
    }
  })
}

export function getRelationships(projectId) {
  return invokeOrMock('get_relationships', { projectId }, () => MOCK_RELATIONSHIPS)
}

export function dismissRelationship(relationshipId) {
  return invokeOrMock('dismiss_relationship', { relationshipId }, () => undefined)
}

export function createRelationship(sourceId, targetId, relationshipType) {
  return invokeOrMock('create_relationship', { sourceId, targetId, relationshipType }, () => ({
    id: 'rel-new',
    source_project_id: sourceId,
    target_project_id: targetId,
    relationship_type: relationshipType,
    detection_source: 'manual',
    dismissed: false,
    first_detected_at: new Date().toISOString(),
    last_seen_at: new Date().toISOString(),
  }))
}

export function removeRelationship(relationshipId) {
  return invokeOrMock('remove_relationship', { relationshipId }, () => undefined)
}
