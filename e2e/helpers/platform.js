/**
 * Platform-aware constants for E2E tests.
 *
 * Paths and keyboard modifiers differ across Linux and macOS.
 * All platform-specific values live here — specs import from this module
 * instead of hardcoding OS-specific strings.
 */

import { homedir } from 'node:os'
import { dirname, join, resolve } from 'node:path'

const isMac = process.platform === 'darwin'

/**
 * Keyboard modifier for shortcuts (Cmd on macOS, Ctrl elsewhere).
 * Usage: `await browser.keys([MOD_KEY, 'k'])`
 */
export const MOD_KEY = isMac ? 'Meta' : 'Control'

const defaultTaurhausProjectPath = resolve(homedir(), 'projects', 'taurhaus')

/**
 * The taurhaus project's own path — guaranteed to be a git repo on dev machines.
 * Used as a stable onboarding target and duplicate-path validation input.
 */
export const TAURHAUS_PROJECT_PATH = process.env.E2E_TAURHAUS_PROJECT_PATH || defaultTaurhausProjectPath

/**
 * Claude root used by the running E2E app session.
 * Packaged E2E runs override this to an isolated temp directory.
 */
export const TAURHAUS_CLAUDE_DIR = process.env.TAURHAUS_CLAUDE_DIR
  || join(dirname(TAURHAUS_PROJECT_PATH), 'claude')

/**
 * Directory to scan for projects in the first-run wizard.
 * Defaults to taurhaus itself for deterministic/fast onboarding.
 * Set E2E_PROJECTS_DIR to override for multi-repo scenarios.
 */
export const PROJECTS_DIR = process.env.E2E_PROJECTS_DIR || TAURHAUS_PROJECT_PATH

/**
 * A path that definitely does not exist — for "invalid path" validation tests.
 */
export const NONEXISTENT_PATH = '/nonexistent/path/xyz123'

/**
 * A real directory that is NOT a git repo — for "not a git repository" tests.
 */
export const NON_GIT_DIR = '/tmp'
