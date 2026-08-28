/**
 * Command-center real-action e2e tests — verify sidebar context-menu session
 * actions create, stop, restart, resume, and navigate real tmux-backed sessions.
 */

import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { execFileSync } from 'node:child_process'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, openContextMenu, dismissContextMenu } from '../helpers/navigation.js'
import { PROJECTS_DIR } from '../helpers/platform.js'
import { POLL, POLL_WIZARD } from '../helpers/timing.js'

const TARGET_PROJECT_NAME = 'ledger'
const TARGET_PROJECT_PATH = join(PROJECTS_DIR, TARGET_PROJECT_NAME)
const APP_LOG_PATH = join(dirname(PROJECTS_DIR), 'app-data', 'taurhaus.log.jsonl')
const TOKEN = `cmd-center-${Date.now()}`
const WRAPPER_DIR = mkdtempSync(join(tmpdir(), 'taurhaus-cmd-center-'))
const CLAUDE_WRAPPER_PATH = join(WRAPPER_DIR, 'claude')
const CODEX_WRAPPER_PATH = join(WRAPPER_DIR, 'codex')
const WRAPPER_LOG = join(WRAPPER_DIR, 'codex-launches.log')

let mainApp = false
let originalSettings = null

function tmux(args) {
  return execFileSync('tmux', args, { encoding: 'utf8' }).trim()
}

function wrapperScript() {
  return `#!/usr/bin/env node
const { appendFileSync } = require('node:fs')

let mode = 'unknown'
let log = ''
let token = ''
let tool = 'unknown'

for (const arg of process.argv.slice(2)) {
  if (arg.startsWith('--taurhaus-e2e-mode=')) mode = arg.split('=').slice(1).join('=')
  if (arg.startsWith('--taurhaus-e2e-log=')) log = arg.split('=').slice(1).join('=')
  if (arg.startsWith('--taurhaus-e2e-token=')) token = arg.split('=').slice(1).join('=')
  if (arg.startsWith('--taurhaus-e2e-tool=')) tool = arg.split('=').slice(1).join('=')
}

if (log) {
  appendFileSync(
    log,
    'pid=' + process.pid + ' tool=' + tool + ' mode=' + mode + ' token=' + token + ' cwd=' + process.cwd() + ' args=' + process.argv.slice(2).join(' ') + '\\n',
  )
}

console.log('taurhaus-e2e tool=' + tool + ' mode=' + mode + ' token=' + token)

const exitCleanly = () => process.exit(0)
process.on('SIGINT', exitCleanly)
process.on('SIGTERM', exitCleanly)
setInterval(() => {}, 1000)
`
}

function prepareCliWrappers() {
  for (const wrapperPath of [CLAUDE_WRAPPER_PATH, CODEX_WRAPPER_PATH]) {
    writeFileSync(wrapperPath, wrapperScript(), 'utf8')
    chmodSync(wrapperPath, 0o755)
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

async function listCliSessions() {
  const result = await invokeTauri('list_cli_sessions')
  if (!result.ok) throw new Error(result.error || 'Failed to list CLI sessions')
  return Array.isArray(result.result) ? result.result : []
}

function readWrapperLog() {
  if (!existsSync(WRAPPER_LOG)) return []
  return readFileSync(WRAPPER_LOG, 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
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
        && entry.open_terminal === true
        && entry.tmux_pane === tmuxPane
    ).length,
    navigateCompletions: entries.filter(
      (entry) =>
        entry.event === 'ipc.command.completed'
        && entry.command === 'navigate_to_session'
        && entry.status === 'ok'
    ).length,
  }
}

async function getTargetProjectItem() {
  const items = await $$('[data-testid="project-item"]')
  for (const item of items) {
    const text = String(await browser.execute((el) => el.textContent || '', item)).trim().toLowerCase()
    if (text.includes(TARGET_PROJECT_NAME)) return item
  }
  throw new Error(`Could not find sidebar project item for ${TARGET_PROJECT_NAME}`)
}

async function openProjectMenu() {
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

async function hasMenuItem(label) {
  return await browser.execute((expectedLabel) => {
    return Array.from(document.querySelectorAll('[data-testid^="menu-item-"]')).some(
      (button) => button.textContent?.trim() === expectedLabel
    )
  }, label)
}

async function chooseContinueAction() {
  const actions = [
    { label: 'Continue Codex', tool: 'codex' },
    { label: 'Continue Claude', tool: 'claude' },
  ]

  await browser.waitUntil(
    async () => {
      for (const action of actions) {
        if (await hasMenuItem(action.label)) return true
      }
      return false
    },
    {
      timeout: 15_000,
      interval: POLL,
      timeoutMsg: 'No supported Continue action appeared in the command center menu'
    }
  )

  for (const action of actions) {
    if (await hasMenuItem(action.label)) return action
  }

  throw new Error('No supported Continue action is available')
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

async function waitForTokenSessions(expectedTool, expectedMode, expectedCount = 1) {
  await browser.waitUntil(
    async () => {
      const sessions = await listCliSessions()
      return sessions.filter(
        (session) =>
          session.cli_tool === expectedTool &&
          session.project_path === TARGET_PROJECT_PATH &&
          String(session.args || '').includes(TOKEN) &&
          String(session.args || '').includes(`--taurhaus-e2e-mode=${expectedMode}`)
      ).length >= expectedCount
    },
    {
      timeout: 20_000,
      interval: POLL_WIZARD,
      timeoutMsg: `Timed out waiting for ${expectedMode} session for ${TARGET_PROJECT_NAME}`
    }
  )

  const sessions = await listCliSessions()
  return sessions.filter(
    (session) =>
      session.cli_tool === expectedTool &&
      session.project_path === TARGET_PROJECT_PATH &&
      String(session.args || '').includes(TOKEN) &&
      String(session.args || '').includes(`--taurhaus-e2e-mode=${expectedMode}`)
  )
}

async function waitForNoTokenSessions() {
  await browser.waitUntil(
    async () => {
      const sessions = await listCliSessions()
      return sessions.every((session) => !String(session.args || '').includes(TOKEN))
    },
    {
      timeout: 20_000,
      interval: POLL_WIZARD,
      timeoutMsg: 'Token-tagged sessions did not disappear after stop'
    }
  )
}

async function waitForWrapperLog(mode, tool, minimumLines = 1) {
  await browser.waitUntil(
    async () => {
      const lines = readWrapperLog()
      return lines.filter(
        (line) =>
          line.includes(`tool=${tool}`)
          && line.includes(`mode=${mode}`)
          && line.includes(`token=${TOKEN}`)
      ).length >= minimumLines
    },
    { timeout: 15_000, interval: POLL, timeoutMsg: `Wrapper log never recorded ${mode}` }
  )
}

async function waitForTerminalNavigation(tmuxPane, baseline) {
  await browser.waitUntil(
    async () => {
      const snapshot = navigationLogSnapshot(tmuxPane)
      return (
        snapshot.navigateEvents > baseline.navigateEvents
        && snapshot.navigateCompletions > baseline.navigateCompletions
      )
    },
    {
      timeout: 10_000,
      interval: POLL,
      timeoutMsg: 'Open in Terminal did not emit a successful navigation lifecycle'
    }
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
      // best-effort cleanup only
    }
  }
}

describe('Command center real actions', () => {
  before(async () => {
    prepareCliWrappers()

    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (!mainApp) return

    await waitForProjectsLoaded()
    originalSettings = await getSettings()

    const updatedSettings = canonicalizeSettings(structuredClone(originalSettings))
    updatedSettings.terminal = updatedSettings.terminal || {}
    updatedSettings.terminal.cli_commands = updatedSettings.terminal.cli_commands || {}
    updatedSettings.terminal.cli_commands.claude = {
      continue_cmd: updatedSettings.terminal.cli_commands.claude?.continue_cmd || '',
      fresh: updatedSettings.terminal.cli_commands.claude?.fresh || '',
      resume: updatedSettings.terminal.cli_commands.claude?.resume || '',
    }
    updatedSettings.terminal.cli_commands.codex = {
      continue_cmd: updatedSettings.terminal.cli_commands.codex?.continue_cmd || '',
      fresh: updatedSettings.terminal.cli_commands.codex?.fresh || '',
      resume: updatedSettings.terminal.cli_commands.codex?.resume || '',
    }
    updatedSettings.terminal.tmux_layout = 'new_window'
    updatedSettings.terminal.cli_commands.claude.continue_cmd =
      `${CLAUDE_WRAPPER_PATH} --taurhaus-e2e-tool=claude --taurhaus-e2e-token=${TOKEN} --taurhaus-e2e-log=${WRAPPER_LOG} --taurhaus-e2e-mode=continue`
    updatedSettings.terminal.cli_commands.claude.fresh =
      `${CLAUDE_WRAPPER_PATH} --taurhaus-e2e-tool=claude --taurhaus-e2e-token=${TOKEN} --taurhaus-e2e-log=${WRAPPER_LOG} --taurhaus-e2e-mode=fresh`
    updatedSettings.terminal.cli_commands.claude.resume =
      `${CLAUDE_WRAPPER_PATH} --resume --taurhaus-e2e-tool=claude --taurhaus-e2e-token=${TOKEN} --taurhaus-e2e-log=${WRAPPER_LOG} --taurhaus-e2e-mode=resume`
    updatedSettings.terminal.cli_commands.codex.continue_cmd =
      `${CODEX_WRAPPER_PATH} --taurhaus-e2e-tool=codex --taurhaus-e2e-token=${TOKEN} --taurhaus-e2e-log=${WRAPPER_LOG} --taurhaus-e2e-mode=continue`
    updatedSettings.terminal.cli_commands.codex.fresh =
      `${CODEX_WRAPPER_PATH} --taurhaus-e2e-tool=codex --taurhaus-e2e-token=${TOKEN} --taurhaus-e2e-log=${WRAPPER_LOG} --taurhaus-e2e-mode=fresh`
    updatedSettings.terminal.cli_commands.codex.resume =
      `${CODEX_WRAPPER_PATH} resume --last --taurhaus-e2e-tool=codex --taurhaus-e2e-token=${TOKEN} --taurhaus-e2e-log=${WRAPPER_LOG} --taurhaus-e2e-mode=resume`

    await updateSettings(updatedSettings)
    await cleanupTokenSessions()
  })

  after(async () => {
    await dismissContextMenu().catch(() => {})
    await cleanupTokenSessions()

    if (originalSettings) {
      await updateSettings(canonicalizeSettings(originalSettings)).catch(() => {})
    }

    rmSync(WRAPPER_DIR, { recursive: true, force: true })
  })

  it('launches, continues, resumes, stops, restarts, and navigates real tmux sessions', async function () {
    if (!mainApp) return this.skip()
    this.timeout(60_000)

    await openProjectMenu()
    await clickMenuItem('New Codex Session')

    const freshSessions = await waitForTokenSessions('codex', 'fresh')
    const freshSession = freshSessions[0]
    expect(freshSession.cli_tool).toBe('codex')
    expect(freshSession.project_path).toBe(TARGET_PROJECT_PATH)
    expect(String(freshSession.args)).toContain('--taurhaus-e2e-mode=fresh')
    expect(freshSession.tmux_session).toBe('taurhaus')
    expect(freshSession.tmux_pane).toBeTruthy()
    await waitForWrapperLog('fresh', 'codex')

    await openProjectMenu()
    await waitForMenuItem('Open in Terminal')
    const navigationBaseline = navigationLogSnapshot(freshSession.tmux_pane)
    await clickMenuItem('Open in Terminal')
    await waitForTerminalNavigation(freshSession.tmux_pane, navigationBaseline)

    await openProjectMenu()
    await waitForMenuItem('Restart Codex')
    await clickMenuItem('Restart Codex')

    await browser.waitUntil(
      async () => {
        const sessions = await listCliSessions()
        return !sessions.some((session) => session.tmux_pane === freshSession.tmux_pane)
      },
      {
        timeout: 20_000,
        interval: POLL_WIZARD,
        timeoutMsg: 'Restart did not remove the original fresh pane'
      }
    )

    const restartedSessions = await waitForTokenSessions('codex', 'fresh')
    const restartedSession = restartedSessions.find((session) => session.tmux_pane !== freshSession.tmux_pane)
    expect(restartedSession).toBeTruthy()
    expect(String(restartedSession.args)).toContain('--taurhaus-e2e-mode=fresh')
    await waitForWrapperLog('fresh', 'codex', 2)

    await openProjectMenu()
    await waitForMenuItem('Stop Codex')
    await clickMenuItem('Stop Codex')
    await waitForMenuItem('Confirm stop Codex?')
    await clickMenuItem('Confirm stop Codex?')
    await waitForNoTokenSessions()

    await openProjectMenu()
    const continueAction = await chooseContinueAction()
    await clickMenuItem(continueAction.label)

    const continuedSessions = await waitForTokenSessions(continueAction.tool, 'continue')
    const continuedSession = continuedSessions[0]
    expect(continuedSession.cli_tool).toBe(continueAction.tool)
    expect(String(continuedSession.args)).toContain('--taurhaus-e2e-mode=continue')
    await waitForWrapperLog('continue', continueAction.tool)

    await openProjectMenu()
    const continueStopLabel = continueAction.tool === 'codex' ? 'Stop Codex' : 'Stop Claude'
    const continueConfirmLabel =
      continueAction.tool === 'codex' ? 'Confirm stop Codex?' : 'Confirm stop Claude?'
    await waitForMenuItem(continueStopLabel)
    await clickMenuItem(continueStopLabel)
    await waitForMenuItem(continueConfirmLabel)
    await clickMenuItem(continueConfirmLabel)
    await waitForNoTokenSessions()

    await openProjectMenu()
    await clickMenuItem('Resume Codex')

    const resumedSessions = await waitForTokenSessions('codex', 'resume')
    const resumedSession = resumedSessions[0]
    expect(String(resumedSession.args)).toContain('--taurhaus-e2e-mode=resume')
    expect(resumedSession.tmux_pane).toBeTruthy()
    await waitForWrapperLog('resume', 'codex')
  })
})
