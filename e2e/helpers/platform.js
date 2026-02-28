/**
 * Platform-aware constants for cross-platform E2E tests.
 *
 * Paths and keyboard modifiers differ across Linux, macOS, and Windows.
 * All platform-specific values live here — specs import from this module
 * instead of hardcoding OS-specific strings.
 */

import { homedir } from 'node:os'
import { resolve } from 'node:path'

const isMac = process.platform === 'darwin'
export const isWindows = process.platform === 'win32'

/**
 * Keyboard modifier for shortcuts (Cmd on macOS, Ctrl elsewhere).
 * Usage: `await browser.keys([MOD_KEY, 'k'])`
 */
export const MOD_KEY = isMac ? 'Meta' : 'Control'

/**
 * Directory to scan for projects in the first-run wizard.
 * Must exist and contain git repos on the test machine.
 *
 * On Windows, projects live in WSL — use the UNC path so the app registers
 * them as WSL paths. This lets `is_wsl_path()` route operations through the
 * daemon provider (or local UNC access as fallback).
 */
export const PROJECTS_DIR = isWindows
  ? '\\\\wsl$\\Ubuntu\\home\\mstie\\projects'
  : isMac
    ? resolve(homedir(), 'projects')
    : '/home/mstie/projects'

/**
 * A path that definitely does not exist — for "invalid path" validation tests.
 */
export const NONEXISTENT_PATH = isWindows
  ? 'C:\\nonexistent\\path\\xyz123'
  : '/nonexistent/path/xyz123'

/**
 * A real directory that is NOT a git repo — for "not a git repository" tests.
 */
export const NON_GIT_DIR = isWindows
  ? resolve(homedir())
  : '/tmp'

/**
 * The taurhaus project's own path — guaranteed to be registered after wizard.
 * Used for "already registered" duplicate-path validation tests.
 *
 * On Windows, projects are registered with WSL UNC paths (the wizard scans
 * via \\wsl$\...). The DB stores the UNC form, so duplicate-path checks
 * must use the same format.
 */
export const TAURHAUS_PROJECT_PATH = isWindows
  ? '\\\\wsl$\\Ubuntu\\home\\mstie\\projects\\taurhaus'
  : isMac
    ? resolve(homedir(), 'projects', 'taurhaus')
    : '/home/mstie/projects/taurhaus'
