import { basename } from 'node:path'

export function needsWizard(specs) {
  const wizard = specs.some(spec => basename(spec) === 'first-run-wizard.js')
  if (wizard && specs.length !== 1) throw new Error('Wizard requires a dedicated virgin session')
  return wizard
}

// Shared by the real wizard test and the generated setup path: neither may
// claim onboarding succeeded with a partial registration or an unpersisted root.
export function assertOnboardedProjects(projects, firstRun) {
  const names = projects.map(project => project.name).sort()
  if (JSON.stringify(names) !== JSON.stringify(['ledger', 'taurhaus'])) {
    throw new Error(`Expected registered ledger and taurhaus projects; got ${names.join(', ')}`)
  }
  if (firstRun !== false) throw new Error('Registered root still reports first run')
}

export async function seedOnboarding(invoke, projectsDir) {
  const discovered = await invoke('scan_directory', { path: projectsDir })
  const selected = discovered.filter(project => project.hasGit)
  assertOnboardedProjects(selected, false)
  const results = await invoke('register_projects_batch', { paths: selected.map(project => project.path) })
  if (results.length !== 2 || results.some(result => !result.success)) {
    throw new Error('Fixture project registration failed')
  }
  assertOnboardedProjects(await invoke('list_projects'), await invoke('is_first_run'))
}

export async function invokeApp(command, args) {
  const result = await browser.executeAsync((command, args, done) => {
    window.__TAURI_INTERNALS__.invoke(command, args)
      .then(value => done({ value }))
      .catch(error => done({ error: String(error) }))
  }, command, args)
  if (result.error) throw new Error(`${command}: ${result.error}`)
  return result.value
}
