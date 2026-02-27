/**
 * WebdriverIO configuration for Tauri e2e tests.
 *
 * Uses tauri-driver as the WebDriver bridge, which delegates to the
 * platform's native WebDriver (WebKitWebDriver on Linux, msedgedriver on Windows).
 *
 * Prerequisites:
 *   cargo install tauri-driver --locked
 *   WebKitWebDriver available (Linux) or msedgedriver on PATH (Windows)
 *
 * Usage:
 *   just test-e2e          (builds + runs)
 *   npx wdio run e2e/wdio.conf.js  (runs against existing debug build)
 */

import { spawn } from 'node:child_process'
import { resolve } from 'node:path'

const projectRoot = resolve(import.meta.dirname, '..')

// Resolve the debug binary path based on platform
const isWindows = process.platform === 'win32'
const binaryName = isWindows ? 'taurhaus.exe' : 'taurhaus'
const binaryPath = resolve(projectRoot, 'src-tauri', 'target', 'debug', binaryName)

let tauriDriver

export const config = {
  // ── Runner ──────────────────────────────────────────────────────────────
  runner: 'local',
  hostname: '127.0.0.1',
  port: 4444,
  maxInstances: 1,

  // ── Specs ───────────────────────────────────────────────────────────────
  specs: [resolve(import.meta.dirname, 'specs', '**', '*.js')],

  // ── Capabilities ────────────────────────────────────────────────────────
  capabilities: [
    {
      'tauri:options': {
        application: binaryPath,
      },
    },
  ],

  // ── Framework ───────────────────────────────────────────────────────────
  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 60_000,
  },

  // ── Reporter ────────────────────────────────────────────────────────────
  reporters: ['spec'],

  // ── Hooks ───────────────────────────────────────────────────────────────

  /**
   * Build the Tauri debug binary before running tests.
   * Skip with E2E_SKIP_BUILD=1 if you already have a fresh build.
   */
  async onPrepare() {
    if (process.env.E2E_SKIP_BUILD === '1') {
      console.log('[e2e] Skipping build (E2E_SKIP_BUILD=1)')
      return
    }

    console.log('[e2e] Building Tauri debug binary...')
    return new Promise((resolve, reject) => {
      const build = spawn('npx', ['tauri', 'build', '--debug', '--no-bundle'], {
        cwd: projectRoot,
        stdio: 'inherit',
      })
      build.on('close', (code) => {
        if (code === 0) {
          console.log('[e2e] Build complete')
          resolve()
        } else {
          reject(new Error(`Build failed with exit code ${code}`))
        }
      })
    })
  },

  /**
   * Start tauri-driver before each test session.
   */
  async beforeSession() {
    return new Promise((resolve) => {
      tauriDriver = spawn('tauri-driver', [], {
        stdio: [null, process.stdout, process.stderr],
      })

      // Give tauri-driver time to start its WebDriver server
      setTimeout(resolve, 500)
    })
  },

  /**
   * Kill tauri-driver after each test session.
   */
  async afterSession() {
    if (tauriDriver) {
      tauriDriver.kill()
      tauriDriver = null
    }
  },
}
