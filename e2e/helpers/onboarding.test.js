import { describe, expect, it, vi } from 'vitest'
import { seedOnboarding, assertOnboardedProjects, needsWizard } from './onboarding.js'

describe('post-onboarding fixture', () => {
  it('reserves a virgin root only for the dedicated wizard spec', () => {
    expect(needsWizard(['/repo/e2e/specs/first-run-wizard.js'])).toBe(true)
    expect(needsWizard(['/repo/e2e/specs/settings-persistence.js'])).toBe(false)
    expect(() => needsWizard(['first-run-wizard.js', 'role-detail-screenshots.js'])).toThrow(/dedicated/)
  })

  it('scans and registers the two generated repositories through wizard commands', async () => {
    const projects = ['ledger', 'taurhaus'].map(name => ({ name, path: `/fixture/${name}`, hasGit: true }))
    const invoke = vi.fn(async (command) => {
      if (command === 'scan_directory' || command === 'list_projects') return projects
      if (command === 'register_projects_batch') return projects.map(project => ({ success: true, project }))
      if (command === 'is_first_run') return false
      throw new Error(`Unexpected command: ${command}`)
    })
    await seedOnboarding(invoke, '/fixture')
    expect(invoke.mock.calls).toEqual([
      ['scan_directory', { path: '/fixture' }],
      ['register_projects_batch', { paths: ['/fixture/ledger', '/fixture/taurhaus'] }],
      ['list_projects'], ['is_first_run'],
    ])
  })

  it('refuses partial registration instead of letting the suite walk the wizard', async () => {
    const invoke = vi.fn(async command => command === 'scan_directory'
      ? ['ledger', 'taurhaus'].map(name => ({ name, path: `/fixture/${name}`, hasGit: true }))
      : [{ success: true }, { success: false, error: 'registration failed' }])
    await expect(seedOnboarding(invoke, '/fixture')).rejects.toThrow(/registration/)
  })

  it('validates the same persisted two-project handoff for seeded and wizard paths', () => {
    expect(() => assertOnboardedProjects([{ name: 'ledger' }, { name: 'taurhaus' }], false)).not.toThrow()
    expect(() => assertOnboardedProjects([{ name: 'taurhaus' }], false)).toThrow(/ledger/)
    expect(() => assertOnboardedProjects([{ name: 'ledger' }, { name: 'taurhaus' }], true)).toThrow(/first run/)
  })
})
