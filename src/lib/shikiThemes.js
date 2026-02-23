/**
 * Shiki theme metadata for the Settings UI.
 *
 * Uses shiki's bundledThemesInfo to list all available themes,
 * split into light and dark categories and sorted by display name.
 */
import { bundledThemesInfo } from 'shiki'

export const DEFAULT_LIGHT_THEME = 'github-light'
export const DEFAULT_DARK_THEME = 'github-dark-dimmed'

/** All bundled light themes, sorted by displayName. */
export const lightThemes = bundledThemesInfo
  .filter(t => t.type === 'light')
  .sort((a, b) => a.displayName.localeCompare(b.displayName))

/** All bundled dark themes, sorted by displayName. */
export const darkThemes = bundledThemesInfo
  .filter(t => t.type === 'dark')
  .sort((a, b) => a.displayName.localeCompare(b.displayName))
