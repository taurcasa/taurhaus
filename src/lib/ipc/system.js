import {
  MOCK_DETAIL,
  MOCK_PROJECTS,
  MOCK_SEARCH_RESULTS,
  MOCK_SETTINGS,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'

function normalizeDaemonStatus(raw) {
  const status = raw && typeof raw === 'object' ? raw : {}
  return {
    status: status.status ?? 'disconnected',
    version: status.version ?? null,
    protocol_version: status.protocol_version ?? status.protocolVersion ?? 0,
    expected_protocol_version:
      status.expected_protocol_version ?? status.expectedProtocolVersion ?? 0,
    uptime_secs: status.uptime_secs ?? status.uptimeSecs ?? null,
    port: status.port ?? 17233,
    wsl_distro: status.wsl_distro ?? status.wslDistro ?? null,
  }
}

function normalizeDaemonInstallStatus(raw) {
  const status = raw && typeof raw === 'object' ? raw : {}
  return {
    installed: Boolean(status.installed),
    version: status.version ?? null,
    bundled_version: status.bundled_version ?? status.bundledVersion ?? '',
    needs_update: status.needs_update ?? status.needsUpdate ?? false,
    wsl_available: status.wsl_available ?? status.wslAvailable ?? true,
    error: status.error ?? null,
  }
}

function normalizeMeshInstallStatus(raw) {
  const status = raw && typeof raw === 'object' ? raw : {}
  return {
    installed: Boolean(status.installed),
    version: status.version ?? null,
    bundled_version: status.bundled_version ?? status.bundledVersion ?? '',
    needs_update: status.needs_update ?? status.needsUpdate ?? false,
    environment_available:
      status.environment_available ?? status.environmentAvailable ?? true,
    error: status.error ?? null,
  }
}

export function search(query, limit = 20) {
  return invokeOrMock('search', { query, limit }, () => {
    if (!query || !query.trim()) {
      return []
    }

    const needle = query.toLowerCase()
    return MOCK_SEARCH_RESULTS.filter((result) =>
      result.title.toLowerCase().includes(needle) || result.snippet.toLowerCase().includes(needle)
    )
  })
}

export function getIndexStatus() {
  return invokeOrMock('get_index_status', undefined, () => ({
    doc_count: 42,
    is_empty: false,
  }))
}

export function rebuildIndex() {
  return invokeOrMock('rebuild_index', undefined, () => 42)
}

export function getSettings() {
  return invokeOrMock('get_settings', undefined, () => MOCK_SETTINGS)
}

export function updateSettings(settings) {
  return invokeOrMock('update_settings', { settings }, () => ({
    ...MOCK_SETTINGS,
    ...settings,
  }))
}

export function openExternalUrl(url) {
  return invokeOrMock('plugin:opener|open_url', { url }, () => {
    window.open(url, '_blank')
  })
}

export function isFirstRun() {
  return invokeOrMock('is_first_run', undefined, () => MOCK_PROJECTS.length === 0)
}

export function registerProjectsBatch(paths) {
  return invokeOrMock('register_projects_batch', { paths }, () =>
    paths.map((path, index) => ({
      path,
      success: true,
      project: {
        ...MOCK_DETAIL,
        id: `mock-batch-${index}`,
        path,
        name: path.split('/').pop(),
      },
      error: null,
    }))
  )
}

export function getDaemonStatus() {
  return invokeOrMock(
    'get_daemon_status',
    undefined,
    () => ({
      status: 'connected',
      version: null,
      protocol_version: 0,
      expected_protocol_version: 0,
      uptime_secs: null,
      port: 17233,
      wsl_distro: null,
    })
  ).then(normalizeDaemonStatus)
}

export function getPlatform() {
  return invokeOrMock('get_platform', undefined, () => 'linux')
}

export function startDaemon() {
  return invokeOrMock('start_daemon', undefined, () => 'Daemon started')
}

export function checkDaemonInstallStatus() {
  return invokeOrMock(
    'check_daemon_install_status',
    undefined,
    () => ({
      installed: true,
      version: '0.3.1',
      bundled_version: '0.3.1',
      needs_update: false,
      wsl_available: true,
      error: null,
    })
  ).then(normalizeDaemonInstallStatus)
}

export function installDaemon() {
  return invokeOrMock('install_daemon', undefined, () => ({
    success: true,
    message: 'Daemon installed successfully: taurhaus-daemon 0.3.1',
  }))
}

export function checkMeshInstallStatus() {
  return invokeOrMock(
    'check_mesh_install_status',
    undefined,
    () => ({
      installed: true,
      version: '0.1.0',
      bundled_version: '0.1.0',
      needs_update: false,
      environment_available: true,
      error: null,
    })
  ).then(normalizeMeshInstallStatus)
}

export function installMesh() {
  return invokeOrMock('install_mesh', undefined, () => ({
    success: true,
    message: 'Mesh installed successfully: mesh 0.1.0',
  }))
}
