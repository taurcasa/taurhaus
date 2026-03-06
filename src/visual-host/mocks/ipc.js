import { resolveVisualHostMock } from '../mockState.js'

function asyncResult(name, args = []) {
  return Promise.resolve(resolveVisualHostMock(name, args))
}

export function getLatestSession(...args) {
  return asyncResult('getLatestSession', args)
}

export function getRecentCommits(...args) {
  return asyncResult('getRecentCommits', args)
}

export function getRelationships(...args) {
  return asyncResult('getRelationships', args)
}

export function navigateToSession(...args) {
  return asyncResult('navigateToSession', args)
}

export function launchClaudeSession(...args) {
  return asyncResult('launchClaudeSession', args)
}

export function stopClaudeSession(...args) {
  return asyncResult('stopClaudeSession', args)
}

export function removeProject(...args) {
  return asyncResult('removeProject', args)
}

export function openExternalUrl(...args) {
  return asyncResult('openExternalUrl', args)
}
