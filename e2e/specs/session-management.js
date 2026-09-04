/**
 * Session Management e2e tests — verify runtime-truth session state,
 * sidebar indicators, hover-card details, history drill-down, and
 * click-through tmux navigation.
 */

import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, clickTestId, selectProjectByName, openContextMenu, dismissContextMenu, switchToTab } from '../helpers/navigation.js'
import { POLL, POLL_WIZARD, WAIT_MEDIUM, WAIT_SHORT } from '../helpers/timing.js'
import { PROJECTS_DIR, TAURHAUS_PROJECT_PATH } from '../helpers/platform.js'
import { assertTmuxIsolation } from '../helpers/laneTmux.js'

const TARGET_PROJECT_NAME = 'taurhaus'
const TARGET_PROJECT_PATH = TAURHAUS_PROJECT_PATH
const APP_DATA_DIR = join(dirname(PROJECTS_DIR), 'app-data')
const APP_LOG_PATH = join(APP_DATA_DIR, 'taurhaus.log.jsonl')
const DB_PATH = join(APP_DATA_DIR, 'taurhaus.db')
const TOKEN = `session-mgmt-${Date.now()}`
const WRAPPER_DIR = mkdtempSync(join(tmpdir(), 'taurhaus-session-mgmt-'))
const WRAPPER_PATH = join(WRAPPER_DIR, 'codex')
const PAYLOAD_PATH = join(WRAPPER_DIR, 'payload.bin')
const WRAPPER_LOG = join(WRAPPER_DIR, 'wrapper.log')

const SEEDED_SESSION_ID = `session-${TOKEN}`
const SEEDED_SOURCE_KEY = `codex-${TOKEN}`
const SEEDED_TASK_ONE = `Verify idle transition ${TOKEN}`
const SEEDED_TASK_TWO = `Confirm history drill-down ${TOKEN}`

let mainApp = false
let daemonConnected = false
let originalSettings = null

function tmux(args) {
  assertTmuxIsolation(process.env)
  return execFileSync('tmux', args, { encoding: 'utf8' }).trim()
}

function sqlite(sql) {
  execFileSync('sqlite3', [DB_PATH, sql], { encoding: 'utf8' })
}

function readWrapperLog() {
  if (!existsSync(WRAPPER_LOG)) return []
  return readFileSync(WRAPPER_LOG, 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
}

function wrapperScript() {
  return `#!/usr/bin/env node
const { appendFileSync, readFileSync } = require('node:fs')

let token = ''
let log = ''
let activeMs = 6000
let payload = ''

for (const arg of process.argv.slice(2)) {
  if (arg.startsWith('--taurhaus-e2e-token=')) token = arg.split('=').slice(1).join('=')
  if (arg.startsWith('--taurhaus-e2e-log=')) log = arg.split('=').slice(1).join('=')
  if (arg.startsWith('--taurhaus-e2e-active-ms=')) activeMs = Number(arg.split('=').slice(1).join('=')) || 6000
  if (arg.startsWith('--taurhaus-e2e-payload=')) payload = arg.split('=').slice(1).join('=')
}

if (log) {
  appendFileSync(log, 'event=start token=' + token + ' pid=' + process.pid + ' cwd=' + process.cwd() + '\\n')
}

const startedAt = Date.now()
const idleLoop = setInterval(() => {}, 1000)
const activeLoop = setInterval(() => {
  if (payload) {
    readFileSync(payload)
    readFileSync(payload)
    readFileSync(payload)
  }
  if (Date.now() - startedAt >= activeMs) {
    clearInterval(activeLoop)
    if (log) appendFileSync(log, 'event=idle token=' + token + ' pid=' + process.pid + '\\n')
  }
}, 150)

const exitCleanly = () => {
  clearInterval(activeLoop)
  clearInterval(idleLoop)
  process.exit(0)
}

process.on('SIGINT', exitCleanly)
process.on('SIGTERM', exitCleanly)
`
}

function prepareCodexWrapper() {
  writeFileSync(PAYLOAD_PATH, 'x'.repeat(64 * 1024), 'utf8')
  writeFileSync(WRAPPER_PATH, wrapperScript(), 'utf8')
  chmodSync(WRAPPER_PATH, 0o755)
}

function canonicalizeToolCommands(commands = {}) {
  return {
    continue_cmd: commands.continue_cmd ?? commands.continueCmd ?? '',
    fresh: commands.fresh ?? '',
    resume: commands.resume ?? '',
  }
}

function canonicalizeSettings(settings = {}) {
  const thresholds = settings.thresholds || {}
  const daemon = settings.daemon || {}
  const terminal = settings.terminal || {}
  const cliCommands = terminal.cli_commands ?? terminal.cliCommands ?? {}
  const codeTheme = settings.code_theme ?? settings.codeTheme ?? {}

  return {
    scan_directories: settings.scan_directories ?? settings.scanDirectories ?? [],
    thresholds: {
      active_days: thresholds.active_days ?? thresholds.activeDays ?? 7,
      recent_days: thresholds.recent_days ?? thresholds.recentDays ?? 30,
      stale_days: thresholds.stale_days ?? thresholds.staleDays ?? 90,
    },
    ignore_patterns: settings.ignore_patterns ?? settings.ignorePatterns ?? [],
    daemon: {
      port: daemon.port ?? 17233,
      path: daemon.path ?? '~/.local/bin/taurhaus-daemon',
      auto_start: daemon.auto_start ?? daemon.autoStart ?? true,
    },
    code_theme: {
      light: codeTheme.light ?? 'github-light',
      dark: codeTheme.dark ?? 'github-dark-dimmed',
    },
    terminal: {
      emulator: terminal.emulator ?? 'manual',
      custom_command: terminal.custom_command ?? terminal.customCommand ?? '',
      tmux_layout: terminal.tmux_layout ?? terminal.tmuxLayout ?? 'new_window',
      cli_commands: {
        claude: canonicalizeToolCommands(cliCommands.claude),
        codex: canonicalizeToolCommands(cliCommands.codex),
        agy: canonicalizeToolCommands(cliCommands.agy),
        grok: canonicalizeToolCommands(cliCommands.grok),
      },
    },
    dark_mode: settings.dark_mode ?? settings.darkMode ?? false,
    project_dialog_last_path:
      settings.project_dialog_last_path ?? settings.projectDialogLastPath ?? '',
  }
}

async function invokeTauri(command, args = undefined) {
  return await browser.executeAsync((payload, done) => {
    const tauri = window.__TAURI_INTERNALS__
    if (!tauri || typeof tauri.invoke !== 'function') {
      done({ ok: false, error: 'Tauri internals unavailable' })
      return
    }

    tauri
      .invoke(payload.command, payload.args)
      .then((result) => done({ ok: true, result }))
      .catch((error) => done({ ok: false, error: error?.message ?? String(error) }))
  }, { command, args })
}

async function getSettings() {
  const result = await invokeTauri('get_settings')
  if (!result.ok) throw new Error(result.error || 'Failed to load settings')
  return result.result
}

async function updateSettings(settings) {
  const result = await invokeTauri('update_settings', { settings })
  if (!result.ok) throw new Error(result.error || 'Failed to update settings')
  return result.result
}

async function listCliSessions() {
  const result = await invokeTauri('list_cli_sessions')
  if (!result.ok) throw new Error(result.error || 'Failed to list CLI sessions')
  return Array.isArray(result.result) ? result.result : []
}

async function getDaemonStatus() {
  const result = await invokeTauri('get_daemon_status')
  if (!result.ok) return null
  return result.result?.status ?? null
}

async function getTargetProjectItem() {
  const items = await $$('[data-testid="project-item"]')
  for (const item of items) {
    const projectId = await item.getAttribute('data-project-id')
    const text = String(await browser.execute((el) => el.textContent || '', item)).trim().toLowerCase()
    if (text.includes(TARGET_PROJECT_NAME) || projectId === TARGET_PROJECT_NAME) {
      return item
    }
  }
  throw new Error(`Could not find sidebar project item for ${TARGET_PROJECT_NAME}`)
}

async function openTargetProjectMenu() {
  await dismissContextMenu().catch(() => {})
  const item = await getTargetProjectItem()
  await item.scrollIntoView().catch(() => {})
  await openContextMenu(item)
  await browser.waitUntil(
    async () => (await $('[data-testid="context-menu"]')).isExisting(),
    { timeout: 5_000, interval: POLL, timeoutMsg: 'Context menu did not open' }
  )
}

async function waitForMenuItem(label) {
  await browser.waitUntil(
    async () => {
      return await browser.execute((expectedLabel) => {
        return Array.from(document.querySelectorAll('[data-testid^="menu-item-"]')).some(
          (button) => button.textContent?.trim() === expectedLabel
        )
      }, label)
    },
    { timeout: 15_000, interval: POLL, timeoutMsg: `Menu item "${label}" did not appear` }
  )
}

async function clickMenuItem(label) {
  await waitForMenuItem(label)
  await browser.execute((expectedLabel) => {
    const button = Array.from(document.querySelectorAll('[data-testid^="menu-item-"]')).find(
      (candidate) => candidate.textContent?.trim() === expectedLabel
    )
    if (!button) throw new Error(`Context menu item "${expectedLabel}" not found`)
    button.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
  }, label)
}

async function waitForTokenSessionState(expectedState) {
  await browser.waitUntil(
    async () => {
      const sessions = await listCliSessions()
      return sessions.some((session) =>
        session.cli_tool === 'codex'
        && session.project_path === TARGET_PROJECT_PATH
        && String(session.args || '').includes(TOKEN)
        && session.state === expectedState
      )
    },
    {
      timeout: 30_000,
      interval: POLL_WIZARD,
      timeoutMsg: `Timed out waiting for token session state ${expectedState}`
    }
  )

  const sessions = await listCliSessions()
  return sessions.find((session) =>
    session.cli_tool === 'codex'
    && session.project_path === TARGET_PROJECT_PATH
    && String(session.args || '').includes(TOKEN)
    && session.state === expectedState
  )
}

async function cleanupTokenSessions() {
  const sessions = await listCliSessions().catch(() => [])
  const tokenSessions = sessions.filter((session) => String(session.args || '').includes(TOKEN))

  for (const session of tokenSessions) {
    if (!session.tmux_pane) continue
    try {
      tmux(['kill-pane', '-t', session.tmux_pane])
    } catch {
      // best-effort cleanup
    }
  }
}

function readAppLogEntries() {
  if (!existsSync(APP_LOG_PATH)) return []

  return readFileSync(APP_LOG_PATH, 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      try {
        return [JSON.parse(line)]
      } catch {
        return []
      }
    })
}

function navigationLogSnapshot(tmuxPane) {
  const entries = readAppLogEntries()
  return {
    navigateEvents: entries.filter(
      (entry) =>
        entry.event === 'command_center.navigate'
        && entry.tmux_pane === tmuxPane
        && entry.open_terminal === false
    ).length,
    navigateSuccess: entries.filter(
      (entry) =>
        entry.event === 'command_center.navigate.daemon_success'
        && entry.tmux_pane === tmuxPane
        && entry.open_terminal === false
    ).length,
  }
}

async function waitForSessionNavigation(tmuxPane, baseline) {
  await browser.waitUntil(
    async () => {
      const snapshot = navigationLogSnapshot(tmuxPane)
      return (
        snapshot.navigateEvents > baseline.navigateEvents
        && snapshot.navigateSuccess > baseline.navigateSuccess
      )
    },
    {
      timeout: 10_000,
      interval: POLL,
      timeoutMsg: 'Session-indicator navigation did not emit a successful navigation lifecycle'
    }
  )
}

async function readSidebarIndicator() {
  const item = await getTargetProjectItem()
  return await browser.execute((el) => {
    const indicator = Array.from(el.querySelectorAll('[aria-label]')).find(
      (candidate) => String(candidate.getAttribute('aria-label') || '').startsWith('Codex:')
    )
    if (!indicator) return null
    return {
      ariaLabel: indicator.getAttribute('aria-label') || '',
      className: indicator.getAttribute('class') || '',
    }
  }, item)
}

async function clickSidebarIndicator() {
  const item = await getTargetProjectItem()
  const clicked = await browser.execute((el) => {
    const indicator = Array.from(el.querySelectorAll('[role="button"][aria-label]')).find(
      (candidate) => String(candidate.getAttribute('aria-label') || '').startsWith('Codex:')
    )
    if (!indicator) return false
    indicator.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
    indicator.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }))
    indicator.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    return true
  }, item)
  if (!clicked) throw new Error('Sidebar session indicator was not clickable')
}

async function hoverTargetProject() {
  // Regression: 430e09ee left the pointer on the row after opening its context
  // menu, so moving to the same row did not reliably dispatch a fresh mouseenter.
  const item = await getTargetProjectItem()
  await browser.execute((el) => {
    el.dispatchEvent(new MouseEvent('mouseleave'))
  }, item)
  await browser.execute((el) => {
    el.dispatchEvent(new MouseEvent('mouseenter'))
  }, item)
  await browser.waitUntil(
    async () => {
      const motion = await $('[data-testid="hovercard-motion"]')
      return (await motion.isExisting()) && (await motion.getText()).includes('Codex is working now')
    },
    { timeout: 5_000, interval: POLL, timeoutMsg: 'Active Codex hover card did not appear' }
  )
}

async function hoverCardTexts() {
  const motion = await $('[data-testid="hovercard-motion"]')
  const verdict = await $('[data-testid="hovercard-verdict"]')
  return {
    motion: await motion.getText(),
    verdict: await verdict.getText(),
  }
}

async function expandFirstHistorySession() {
  await browser.waitUntil(
    async () => (await $$('[data-testid="session-header"]')).length > 0,
    { timeout: 15_000, interval: POLL, timeoutMsg: 'Session history did not render any entries' }
  )

  const header = await $('[data-testid="session-header"]')
  await header.waitForDisplayed({ timeout: 10_000 })

  const snapshot = async () => {
    return await browser.execute(() => {
      const button = document.querySelector('[data-testid="session-header"]')
      const detail = document.querySelector('[data-testid="session-detail"]')
      return {
        expanded: button?.getAttribute('aria-expanded') ?? null,
        detailExists: Boolean(detail),
        headerText: button?.textContent?.trim() ?? null,
        activeElementTag: document.activeElement?.tagName ?? null,
      }
    })
  }

  const isExpanded = async () => {
    const state = await snapshot()
    return state.expanded === 'true'
  }

  await header.scrollIntoView()
  await header.moveTo()
  await header.click()

  if (!(await isExpanded())) {
    await browser.execute((el) => el.focus(), header)
    await browser.keys('Enter')
    await browser.pause(150)
  }

  if (!(await isExpanded())) {
    await browser.keys(' ')
    await browser.pause(150)
  }

  if (!(await isExpanded())) {
    await browser.execute((el) => {
      el.scrollIntoView({ block: 'center' })
      el.focus()
      el.click()
    }, header)
  }

  await browser.waitUntil(
    async () => await isExpanded(),
    {
      timeout: 10_000,
      interval: POLL,
      timeoutMsg: `Session history row did not expand: ${JSON.stringify(await snapshot())}`,
    }
  )

  await browser.waitUntil(
    async () => (await $('[data-testid="session-detail"]')).isExisting(),
    { ...WAIT_SHORT, timeoutMsg: 'Session history detail did not expand' }
  )
}

async function waitForForegroundIndicator() {
  await browser.waitUntil(
    async () => {
      const item = await getTargetProjectItem()
      return await browser.execute(
        (el) => Boolean(el.querySelector('[data-testid="sidebar-foreground-indicator"]')),
        item
      )
    },
    { timeout: 10_000, interval: POLL, timeoutMsg: 'Foreground indicator did not move to target project' }
  )

  // Ownership, not presence: the hub resolves the focused pane to exactly one
  // project, so no other row may keep an indicator from the previous focus.
  const owners = await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="project-item"]'))
      .filter((row) => row.querySelector('[data-testid="sidebar-foreground-indicator"]'))
      .map((row) => row.textContent?.trim().toLowerCase() ?? '')
  )
  expect(owners).toHaveLength(1)
  expect(owners[0]).toContain(TARGET_PROJECT_NAME)
}

function seedArchivedSessionHistory() {
  const now = new Date()
  const firstSeenAt = new Date(now.getTime() - 12 * 60 * 1000).toISOString()
  const stateChangedAt = new Date(now.getTime() - 8 * 60 * 1000).toISOString()
  const updatedAt = new Date(now.getTime() - 3 * 60 * 1000).toISOString()
  const archivedAt = new Date(now.getTime() - 2 * 60 * 1000).toISOString()
  const esc = (value) => String(value).replaceAll("'", "''")

  const sql = `
PRAGMA busy_timeout = 5000;
DELETE FROM archived_task_session_summaries WHERE project_path = '${esc(TARGET_PROJECT_PATH)}' AND session_key = '${esc(SEEDED_SESSION_ID)}';
DELETE FROM tasks WHERE project_path = '${esc(TARGET_PROJECT_PATH)}' AND source_key = '${esc(SEEDED_SOURCE_KEY)}';
INSERT INTO tasks (
  project_path, source, source_key, source_task_id, subject, description, active_form, status,
  blocks, blocked_by, owner, session_id, first_seen_at, state_changed_at, updated_at, archived_at,
  last_status, archived_reason
) VALUES
(
  '${esc(TARGET_PROJECT_PATH)}', 'codex', '${esc(SEEDED_SOURCE_KEY)}', '1',
  '${esc(SEEDED_TASK_ONE)}', 'Seeded archived task for session history e2e coverage.', NULL, 'completed',
  '[]', '[]', NULL, '${esc(SEEDED_SESSION_ID)}',
  '${esc(firstSeenAt)}', '${esc(stateChangedAt)}', '${esc(updatedAt)}', '${esc(archivedAt)}',
  'completed', 'completed_and_removed'
),
(
  '${esc(TARGET_PROJECT_PATH)}', 'codex', '${esc(SEEDED_SOURCE_KEY)}', '2',
  '${esc(SEEDED_TASK_TWO)}', 'Ensures history detail view shows archived tasks.', NULL, 'completed',
  '[]', '[]', NULL, '${esc(SEEDED_SESSION_ID)}',
  '${esc(firstSeenAt)}', '${esc(stateChangedAt)}', '${esc(updatedAt)}', '${esc(archivedAt)}',
  'completed', 'completed_and_removed'
);
`

  sqlite(sql)
}

describe('Session Management', () => {
  before(async () => {
    prepareCodexWrapper()

    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (!mainApp) return

    await waitForProjectsLoaded()
    daemonConnected = (await getDaemonStatus()) === 'connected'
    if (!daemonConnected) return

    originalSettings = await getSettings()
    const updatedSettings = canonicalizeSettings(structuredClone(originalSettings))
    updatedSettings.terminal = updatedSettings.terminal || {}
    updatedSettings.terminal.tmux_layout = 'new_window'
    updatedSettings.terminal.cli_commands = updatedSettings.terminal.cli_commands || {}
    updatedSettings.terminal.cli_commands.codex = {
      continue_cmd: updatedSettings.terminal.cli_commands.codex?.continue_cmd || '',
      fresh: `${WRAPPER_PATH} --taurhaus-e2e-token=${TOKEN} --taurhaus-e2e-log=${WRAPPER_LOG} --taurhaus-e2e-active-ms=6000 --taurhaus-e2e-payload=${PAYLOAD_PATH}`,
      resume: updatedSettings.terminal.cli_commands.codex?.resume || '',
    }
    await updateSettings(updatedSettings)

    seedArchivedSessionHistory()
    await selectProjectByName(TARGET_PROJECT_NAME)
  })

  after(async () => {
    await dismissContextMenu().catch(() => {})
    await cleanupTokenSessions().catch(() => {})

    if (originalSettings) {
      await updateSettings(canonicalizeSettings(originalSettings)).catch(() => {})
    }

    rmSync(WRAPPER_DIR, { recursive: true, force: true })
  })

  it('tracks a real session from active to idle, shows truthful UI state, supports history drill-down, and navigates to the tmux pane', async function () {
    if (!mainApp || !daemonConnected) return this.skip()
    this.timeout(90_000)

    await openTargetProjectMenu()
    await clickMenuItem('New Codex Session')
    await browser.waitUntil(
      async () => readWrapperLog().some((line) => line.includes(`event=start token=${TOKEN}`)),
      { timeout: 15_000, interval: POLL, timeoutMsg: 'Wrapped Codex command never started in the launched tmux pane' }
    )

    const activeSession = await waitForTokenSessionState('active')
    expect(activeSession).toBeTruthy()
    expect(activeSession.tmux_session).toBe('taurhaus')
    expect(activeSession.tmux_pane).toBeTruthy()

    await browser.waitUntil(
      async () => {
        const indicator = await readSidebarIndicator()
        return indicator?.ariaLabel?.includes('Codex: running') && indicator?.className?.includes('session-pill-active')
      },
      { timeout: 20_000, interval: POLL, timeoutMsg: 'Sidebar indicator never reflected active Codex state' }
    )

    await hoverTargetProject()
    let hover = await hoverCardTexts()
    expect(hover.motion).toContain('Codex is working now')
    expect(hover.verdict).toContain('Active work in progress')

    await selectProjectByName('ledger')
    const navigationBaseline = navigationLogSnapshot(activeSession.tmux_pane)
    await clickSidebarIndicator()
    await waitForSessionNavigation(activeSession.tmux_pane, navigationBaseline)
    await waitForForegroundIndicator()

    const idleSession = await waitForTokenSessionState('idle')
    expect(idleSession?.tmux_pane).toBe(activeSession.tmux_pane)

    await browser.waitUntil(
      async () => {
        const indicator = await readSidebarIndicator()
        return indicator?.ariaLabel?.includes('Codex: idle') && indicator?.className?.includes('session-pill-idle')
      },
      { timeout: 25_000, interval: POLL, timeoutMsg: 'Sidebar indicator never reflected idle Codex state' }
    )

    await selectProjectByName(TARGET_PROJECT_NAME)
    await switchToTab('tasks')
    await clickTestId('sub-tab-history')
    await $('[data-testid="history-tab-content"]').waitForExist({ timeout: 5_000 })

    await browser.waitUntil(
      async () => {
        const headerCount = (await $$('[data-testid="session-header"]')).length
        if (headerCount > 0) return true
        return await $('[data-testid="history-empty"]').isExisting()
      },
      { timeout: 30_000, interval: POLL, timeoutMsg: 'History content never appeared' }
    )

    await expandFirstHistorySession()

    await browser.waitUntil(
      async () => (await $$('[data-testid="history-task"]')).length >= 2,
      { timeout: 10_000, interval: POLL, timeoutMsg: 'History tasks did not render inside the expanded session' }
    )

    const taskTexts = await browser.execute(() => {
      return Array.from(document.querySelectorAll('[data-testid="history-task"]')).map(
        (task) => task.textContent?.trim() ?? ''
      )
    })
    expect(taskTexts.join('\n')).toContain(SEEDED_TASK_ONE)
    expect(taskTexts.join('\n')).toContain(SEEDED_TASK_TWO)
  })
})
