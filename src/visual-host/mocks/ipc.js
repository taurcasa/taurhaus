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

export function launchCliSession(...args) {
  return asyncResult('launchCliSession', args)
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

export function listAccounts(...args) {
  return asyncResult('listAccounts', args)
}

export function resolveLaunchBases(...args) {
  return asyncResult('resolveLaunchBases', args)
}

export function setProjectAccount(...args) {
  return asyncResult('setProjectAccount', args)
}

export function listAccountRelationships(...args) {
  return asyncResult('listAccountRelationships', args)
}

export function setGlobalDefaultAccount(...args) {
  return asyncResult('setGlobalDefaultAccount', args)
}

export function prepareAccountDirectory(...args) {
  return asyncResult('prepareAccountDirectory', args)
}

export function launchAccountLogin(...args) {
  return asyncResult('launchAccountLogin', args)
}

export function revealDirectory(...args) {
  return asyncResult('revealDirectory', args)
}

export function getProjectTasks(...args) {
  return asyncResult('getProjectTasks', args)
}

export function getArchivedSessions(...args) {
  return asyncResult('getArchivedSessions', args)
}

export function listWorkflowRuns(...args) {
  return asyncResult('listWorkflowRuns', args)
}

export function getWorkflowRun(...args) {
  return asyncResult('getWorkflowRun', args)
}

export function workflowLedgerRow(...args) {
  return asyncResult('workflowLedgerRow', args)
}
