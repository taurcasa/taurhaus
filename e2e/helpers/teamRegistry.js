/**
 * Ownership registry for teams this e2e run created, shared across the specs
 * of a sealed group (one Node process per worker, so module state persists
 * across spec files). Cleanup guards check ownership here instead of
 * pattern-matching another spec's naming convention — a disband is only ever
 * issued for a team some spec in this run registered.
 */
const createdTeamNames = new Set()

export function registerCreatedTeam(teamName) {
  createdTeamNames.add(teamName)
}

export function forgetCreatedTeam(teamName) {
  createdTeamNames.delete(teamName)
}

export function isOwnedTeam(teamName) {
  return createdTeamNames.has(teamName)
}

export function ownedTeams() {
  return [...createdTeamNames]
}

export function clearOwnedTeams() {
  createdTeamNames.clear()
}
