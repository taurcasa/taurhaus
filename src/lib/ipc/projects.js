import {
  MOCK_COMMITS,
  MOCK_DETAIL,
  MOCK_FILE_TREE,
  MOCK_PROJECTS,
  MOCK_RELATIONSHIPS,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'

function normalizeFileTreeNode(node) {
  if (!node || typeof node !== 'object') return null
  const children = Array.isArray(node.children)
    ? node.children.map(normalizeFileTreeNode).filter(Boolean)
    : []
  return {
    ...node,
    is_dir: node.is_dir ?? node.isDir ?? false,
    children,
  }
}

function normalizeFileTree(raw) {
  if (!Array.isArray(raw)) return []
  return raw.map(normalizeFileTreeNode).filter(Boolean)
}

function normalizeRelationship(raw) {
  const rel = raw && typeof raw === 'object' ? raw : {}
  return {
    ...rel,
    source_project_id: rel.source_project_id ?? rel.sourceProjectId ?? null,
    target_project_id: rel.target_project_id ?? rel.targetProjectId ?? null,
    relationship_type: rel.relationship_type ?? rel.relationshipType ?? '',
    detection_source: rel.detection_source ?? rel.detectionSource ?? '',
    first_detected_at: rel.first_detected_at ?? rel.firstDetectedAt ?? null,
    last_seen_at: rel.last_seen_at ?? rel.lastSeenAt ?? null,
  }
}

export function listProjects() {
  return invokeOrMock('list_projects', undefined, () => MOCK_PROJECTS)
}

export function getProject(projectId) {
  return invokeOrMock('get_project', { projectId }, () => ({ ...MOCK_DETAIL, id: projectId }))
}

export function createProject(name, parentDir) {
  return invokeOrMock('create_project', { name, parentDir }, () => ({
    ...MOCK_DETAIL,
    name,
    path: `${parentDir.replace(/[\\/]+$/, '')}/${name}`,
  }))
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

export function getRemoteUrl(projectId) {
  return invokeOrMock('get_remote_url', { projectId }, () => null)
}

export function getFileTree(projectId) {
  return invokeOrMock('get_file_tree', { projectId }, () => MOCK_FILE_TREE).then(normalizeFileTree)
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
  return invokeOrMock('get_relationships', { projectId }, () => MOCK_RELATIONSHIPS).then((rels) =>
    Array.isArray(rels) ? rels.map(normalizeRelationship) : []
  )
}

export function dismissRelationship(relationshipId) {
  return invokeOrMock('dismiss_relationship', { relationshipId }, () => undefined)
}

/** Pin a project to one Claude subscription. `null` restores the default. */
export function setProjectClaudeAccount(projectId, accountId) {
  return invokeOrMock(
    'set_project_claude_account',
    { projectId, accountId: accountId ?? null },
    () => undefined
  )
}
