/**
 * Platform-aware constants for E2E tests.
 *
 * Paths and keyboard modifiers differ across Linux and macOS.
 * All platform-specific values live here — specs import from this module
 * instead of hardcoding OS-specific strings.
 */

import { homedir } from 'node:os'
import { resolve } from 'node:path'

const isMac = process.platform === 'darwin'

/**
 * Keyboard modifier for shortcuts (Cmd on macOS, Ctrl elsewhere).
 * Usage: `await browser.keys([MOD_KEY, 'k'])`
 */
export const MOD_KEY = isMac ? 'Meta' : 'Control'

/**
 * Directory to scan for projects in the first-run wizard.
 * Must exist and contain git repos on the test machine.
 */
export const PROJECTS_DIR = isMac
  ? resolve(homedir(), 'projects')
  : '/home/mstie/projects'

/**
 * A path that definitely does not exist — for "invalid path" validation tests.
 */
export const NONEXISTENT_PATH = '/nonexistent/path/xyz123'

/**
 * A real directory that is NOT a git repo — for "not a git repository" tests.
 */
export const NON_GIT_DIR = '/tmp'

/**
 * The taurhaus project's own path — guaranteed to be registered after wizard.
 * Used for "already registered" duplicate-path validation tests.
 */
export const TAURHAUS_PROJECT_PATH = isMac
  ? resolve(homedir(), 'projects', 'taurhaus')
  : '/home/mstie/projects/taurhaus'
