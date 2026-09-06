import { readFileSync } from 'node:fs'
import { waitForAppReady } from '../helpers.js'
import { assertOnboardedProjects, invokeApp } from '../helpers/onboarding.js'
import { clickTestId, waitForProjectsLoaded } from '../helpers/navigation.js'
import { openSettings, closeSettings, setSettingValue, getSettingValue } from '../helpers/settings.js'
import { MOD_KEY } from '../helpers/platform.js'
import { E2E_RUN_TOKEN_ENV, findRunTokenProcessRecords, killOwnedProcessRecord } from '../helpers/laneCleanup.js'

async function selectFixtureProject(name) {
  const project = (await invokeApp('list_projects')).find(project => project.name === name)
  const item = await $(`[data-testid="project-item"][data-project-id="${project.id}"]`)
  await item.waitForDisplayed()
  await item.click()
  await browser.waitUntil(async () => (await $('h1').getText()) === name)
}

describe('Critical native smoke', () => {
  before(async () => {
    await waitForAppReady()
    await waitForProjectsLoaded()
    await $('[data-testid="project-item"]').waitForDisplayed({ timeout: 10_000 })
    assertOnboardedProjects(await invokeApp('list_projects'), await invokeApp('is_first_run'))
  })

  it('reads a registered project through startup and the native file boundary', async () => {
    await selectFixtureProject('taurhaus')
    await $('[data-testid="overview-readme"]').waitForDisplayed()
    expect(await $('[data-testid="overview-readme"]').getText()).toContain('Sample repository used by taurhaus E2E tests.')
  })

  // Regression: 5c6b2681 normalized the backend-owned terminal contract and
  // round-tripped it in update_settings, breaking every frontend save.
  it('saves settings through the frontend payload and persists across reload', async () => {
    await openSettings()
    const original = await getSettingValue('threshold-active')
    const changed = String(Number(original) + 1)
    await setSettingValue('threshold-active', changed)
    await browser.waitUntil(async () => (await invokeApp('get_settings')).thresholds.activeDays === Number(changed), {
      timeout: 5_000, timeoutMsg: 'Frontend settings save did not persist through Tauri',
    })
    await browser.refresh()
    await $('[data-testid="tab-overview"]').waitForExist({ timeout: 30_000 })
    await openSettings()
    expect(await getSettingValue('threshold-active')).toBe(changed)
    await setSettingValue('threshold-active', original)
    expect(await closeSettings()).toBe(true)
  })

  // Regression: 55d47709 cancelled its own exit timer, leaving SlideOver's
  // modal layer mounted and blocking keyboard interaction underneath.
  it('closes SlideOver and then navigates using the keyboard', async () => {
    await clickTestId('tab-mesh')
    await $('[data-testid="mesh-template-open-browser"]').waitForDisplayed({ timeout: 20_000 })
    await clickTestId('mesh-template-open-browser')
    await $('[data-testid="slideover-root"]').waitForDisplayed()
    await clickTestId('slideover-close')
    await $('[data-testid="slideover-root"]').waitForExist({ reverse: true })
    await browser.keys([MOD_KEY, 'k'])
    await $('[data-testid="search-overlay"]').waitForDisplayed()
    await browser.keys('Escape')
    await $('[data-testid="search-overlay"]').waitForExist({ reverse: true })
    await clickTestId('tab-overview')
  })

  it('reconnects its own daemon and performs useful project work afterward', async () => {
    await browser.waitUntil(async () => (await invokeApp('get_daemon_status')).status === 'connected', {
      timeout: 20_000, timeoutMsg: 'Required worker daemon is unavailable',
    })
    const records = findRunTokenProcessRecords(process.env[E2E_RUN_TOKEN_ENV])
    const daemon = records.find(record => {
      try {
        return readFileSync(`/proc/${record.pid}/cmdline`, 'utf8').split('\0')[0] === process.env.TAURHAUS_DAEMON_BINARY
      } catch { return false }
    })
    if (!daemon || !killOwnedProcessRecord(daemon, { signal: 'SIGTERM' })) {
      throw new Error('Required run-owned daemon identity unavailable; refusing to stop any other process')
    }
    // The status snapshot is refreshed by a 30s health poll. Confirm process
    // exit directly, then exercise the supported manual reconnect command.
    await browser.waitUntil(() => {
      try {
        return /\) Z /.test(readFileSync(`/proc/${daemon.pid}/stat`, 'utf8'))
      } catch { return true }
    }, { timeout: 5_000, timeoutMsg: 'Run-owned daemon did not stop' })
    await invokeApp('start_daemon')
    await browser.waitUntil(async () => (await invokeApp('get_daemon_status')).status === 'connected', {
      timeout: 20_000, timeoutMsg: 'App did not reconnect to its worker daemon',
    })
    const projects = await invokeApp('list_projects')
    const project = projects.find(project => project.name === 'taurhaus')
    expect((await invokeApp('read_file', { projectId: project.id, relativePath: 'README.md' })).content).toContain('taurhaus fixture')
    await selectFixtureProject('ledger')
    await $('[data-testid="overview-readme"]').waitForDisplayed()
    expect(await $('h1').getText()).toBe('ledger')
  })
})
