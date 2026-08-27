import { formatUserFacingError } from './format.js'
import { toolLabel as registeredToolLabel } from './toolRegistry.js'

export const WSL_INSTALL_URL = 'https://learn.microsoft.com/windows/wsl/install'

function normalizedDetail(error) {
  return formatUserFacingError(error, '').trim().toLowerCase()
}

function includesAny(text, patterns) {
  return patterns.some((pattern) => text.includes(pattern))
}

function titleCaseTool(tool) {
  const value = String(tool || 'session').trim().toLowerCase()
  const registered = registeredToolLabel(value, '')
  if (registered) return registered
  if (!value) return 'Session'
  return value[0].toUpperCase() + value.slice(1)
}

export function describeSessionActionError(action, { tool = 'session' } = {}, error) {
  const detail = normalizedDetail(error)
  const toolLabel = titleCaseTool(tool)

  if (action === 'navigate') {
    if (includesAny(detail, ['pane', 'window', 'session not found', 'no such'])) {
      return 'Could not open that terminal. The session may have already closed.'
    }
    return 'Could not open that terminal. Please try again.'
  }

  if (action === 'stop') {
    return `Could not stop ${toolLabel}. Please try again.`
  }

  if (action === 'restart') {
    return `Could not restart ${toolLabel}. Please try again.`
  }

  return `Could not start ${toolLabel}. Please try again.`
}

export function describeScanDirectoryError(error) {
  const detail = normalizedDetail(error)

  if (includesAny(detail, ['permission denied', 'access is denied', 'operation not permitted'])) {
    return 'Taurhaus cannot scan that folder yet. Check that you can open it, then try again.'
  }

  if (includesAny(detail, ['no such file', 'not found', 'does not exist'])) {
    return 'That folder could not be found. Choose another folder and try again.'
  }

  return 'Could not scan that folder. Try again, choose another folder, or enter a path manually.'
}

export function describeDaemonSetupError(error, { isWindows = false, action = 'check' } = {}) {
  const detail = normalizedDetail(error)

  if (includesAny(detail, ['wsl is not installed', 'wsl is not available'])) {
    return isWindows
      ? 'WSL 2 is not ready yet. Install WSL 2, restart Windows if it asks, then try again.'
      : 'The helper service is not available on this machine yet. Try again in a moment.'
  }

  if (detail.includes('no wsl distro configured')) {
    return 'WSL is installed, but it does not have a Linux distribution yet. Install Ubuntu or another distro, open it once, then try again.'
  }

  if (includesAny(detail, ['permission denied', 'access is denied', 'operation not permitted'])) {
    return 'Taurhaus could not update the helper service because access was denied. Check permissions and try again.'
  }

  if (detail.includes('dev-mode placeholder')) {
    return 'This is a development build without a bundled daemon. Build the daemon with: just install-daemon'
  }

  if (action === 'install') {
    return 'Could not install the helper service. Try again, or restart the app.'
  }

  if (action === 'restart') {
    return 'Could not restart the helper service. Try again in a moment.'
  }

  return 'Could not check the helper service right now. You can skip this step and come back later.'
}

export function describeMeshAvailabilityIssue(error) {
  const detail = normalizedDetail(error)

  if (detail.includes('mesh cli not found')) {
    return 'Install Mesh to set up a team in this project.'
  }

  if (detail.includes('tmux is required')) {
    return 'Install tmux to launch and manage team sessions.'
  }

  if (includesAny(detail, ['protocol version', 'version mismatch', 'bundled mesh'])) {
    return 'The installed Mesh version does not match taurhaus. Install the bundled Mesh version to continue.'
  }

  return formatUserFacingError(error, 'Mesh setup is blocked right now. Check the issue above, then try again.')
}

export function describeMeshInitFailure(error, { failedStep = '' } = {}) {
  const detail = normalizedDetail(error)
  const step = String(failedStep || '').trim().toLowerCase()

  if (detail.includes('already exists')) {
    return 'A team with this name already exists. Open it or replace it to continue.'
  }

  if (detail.includes('mesh cli not found')) {
    return 'Mesh is not installed yet. Install it, then try again.'
  }

  if (detail.includes('tmux')) {
    return 'tmux is not available yet. Install it, then try again.'
  }

  if (includesAny(detail, ['permission denied', 'access is denied', 'operation not permitted'])) {
    return 'Taurhaus could not create the team files it needs. Check permissions and try again.'
  }

  if (step === 'launch_sessions') {
    return 'Taurhaus could not confirm that every session started correctly. Try again.'
  }

  if (step === 'join_mesh') {
    return 'Taurhaus could not connect every agent to Mesh. Try again after Mesh is ready.'
  }

  if (step === 'start_daemons') {
    return 'Taurhaus could not start the background watchers for this team. Try again.'
  }

  return 'Taurhaus could not finish setting up the team. Try again.'
}

export function describeProjectLoadBanner(issues) {
  const labels = Array.isArray(issues)
    ? issues
      .map((issue) => String(issue?.section || '').trim())
      .filter(Boolean)
    : []

  if (labels.length === 0) return ''
  if (labels.length === 1) return `${labels[0]} could not be loaded.`
  return `Some project details could not be loaded: ${labels.join(', ')}.`
}
