import { describe, expect, it } from 'vitest'
import { commands } from 'vitest/browser'

import Settings from '../../../lib/Settings.svelte'
import { captureVisual, renderVisual } from '../renderVisual.js'

function cliCommandDefaults() {
  return {
    claude: {
      continue_cmd: 'claude --dangerously-skip-permissions --continue',
      fresh: 'claude --dangerously-skip-permissions',
      resume: 'claude --dangerously-skip-permissions --resume',
    },
    codex: {
      continue_cmd: 'codex --yolo',
      fresh: 'codex --yolo',
      resume: 'codex resume --last --yolo',
    },
    agy: {
      continue_cmd: 'agy --dangerously-skip-permissions --continue',
      fresh: 'agy --dangerously-skip-permissions',
      resume: 'agy --dangerously-skip-permissions --conversation {session_id}',
    },
  }
}

describe('Settings visual regression', () => {
  it('captures the Linux terminal contract state', async () => {
    const screenshotPath = 'settings/linux-terminal-contract.png'

    await renderVisual(Settings, {
      theme: 'dark',
      viewport: { width: 1280, height: 1800 },
      ipc: {
        getSettings: {
          scan_directories: ['~/projects', '~/clients/mesh'],
          thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
          ignore_patterns: ['node_modules', '.git', 'target', 'dist'],
          code_theme: { light: 'github-light', dark: 'github-dark-dimmed' },
          daemon: { port: 17233, path: '~/.local/bin/taurhaus-daemon', auto_start: true },
          terminal: {
            emulator: 'manual',
            custom_command: '',
            tmux_layout: 'per_project',
            cli_commands: cliCommandDefaults(),
          },
          terminal_contract: {
            platform: 'linux',
            default_emulator: 'manual',
            supported_emulators: ['manual'],
            cli_command_defaults: cliCommandDefaults(),
          },
          dark_mode: true,
          project_dialog_last_path: '/workspace',
        },
        getIndexStatus: { doc_count: 1284, is_empty: false },
      },
    })

    const file = await captureVisual(screenshotPath, {
      clip: { x: 0, y: 0, width: 1280, height: 1700 },
    })
    const artifact = await commands.readVisualArtifact(file)

    expect(artifact.path.endsWith(screenshotPath)).toBe(true)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
  })
})
