/**
 * Shared SVG icon data and display names for CLI tools.
 *
 * Single source of truth — imported by sessionIndicator.js,
 * TaskBoard.svelte, TaskDetailPanel.svelte, SessionHistory.svelte.
 *
 * Monochrome paths using currentColor so state colors apply.
 *
 * Claude: Anthropic starburst (Bootstrap Icons, 16x16)
 * Codex:  OpenAI blossom/knot (Bootstrap Icons, 16x16)
 * Gemini: Google four-pointed sparkle (scaled from official, 65x65)
 */

export const TOOL_ICONS = {
  claude: {
    viewBox: '0 0 16 16',
    path: 'M3.127 10.604l3.135-1.76.053-.153-.053-.085H6.11l-.525-.032-1.791-.048-1.554-.065-1.505-.08-.38-.081L0 7.832l.036-.234.32-.214.455.04 1.009.069 1.513.105 1.097.064 1.626.17h.259l.036-.105-.089-.065-.068-.064-1.566-1.062-1.695-1.121-.887-.646-.48-.327-.243-.306-.104-.67.435-.48.585.04.15.04.593.456 1.267.981 1.654 1.218.242.202.097-.068.012-.049-.109-.181-.9-1.626-.96-1.655-.428-.686-.113-.411a2 2 0 01-.068-.484l.496-.674L4.446 0l.662.089.279.242.411.94.666 1.48 1.033 2.014.302.597.162.553.06.17h.105v-.097l.085-1.134.157-1.392.154-1.792.052-.504.25-.605.497-.327.387.186.319.456-.045.294-.19 1.23-.37 1.93-.243 1.29h.142l.161-.16.654-.868 1.097-1.372.484-.545.565-.601.363-.287h.686l.505.751-.226.775-.707.895-.585.759-.839 1.13-.524.904.048.072.125-.012 1.897-.403 1.024-.186 1.223-.21.553.258.06.263-.218.536-1.307.323-1.533.307-2.284.54-.028.02.032.04 1.029.098.44.024h1.077l2.005.15.525.346.315.424-.053.323-.807.411-3.631-.863-.872-.218h-.12v.073l.726.71 1.331 1.202 1.667 1.55.084.383-.214.302-.226-.032-1.464-1.101-.565-.497-1.28-1.077h-.084v.113l.295.432 1.557 2.34.08.718-.112.234-.404.141-.444-.08-.911-1.28-.94-1.44-.759-1.291-.093.053-.448 4.821-.21.246-.484.186-.403-.307-.214-.496.214-.98.258-1.28.21-1.016.19-1.263.112-.42-.008-.028-.092.012-.953 1.307-1.448 1.957-1.146 1.227-.274.109-.477-.247.045-.44.266-.39 1.586-2.018.956-1.25.617-.723-.004-.105h-.036l-4.212 2.736-.75.096-.324-.302.04-.496.154-.162 1.267-.871z',
  },
  codex: {
    viewBox: '0 0 16 16',
    path: 'M14.949 6.547a3.94 3.94 0 00-.348-3.273 4.11 4.11 0 00-4.4-1.934A4.1 4.1 0 008.423.2 4.15 4.15 0 006.305.086a4.1 4.1 0 00-1.891.948 4.04 4.04 0 00-1.158 1.753 4.1 4.1 0 00-1.563.679A4 4 0 00.554 4.72a3.99 3.99 0 00.502 4.731 3.94 3.94 0 00.346 3.274 4.11 4.11 0 004.402 1.933c.382.425.852.764 1.377.995.526.231 1.095.35 1.67.346 1.78.002 3.358-1.132 3.901-2.804a4.1 4.1 0 001.563-.68 4 4 0 001.14-1.253 3.99 3.99 0 00-.506-4.716m-6.097 8.406a3.05 3.05 0 01-1.945-.694l.096-.054 3.23-1.838a.53.53 0 00.265-.455v-4.49l1.366.778q.02.011.025.035v3.722c-.003 1.653-1.361 2.992-3.037 2.996m-6.53-2.75a2.95 2.95 0 01-.36-2.01l.095.057L5.29 12.09a.53.53 0 00.527 0l3.949-2.246v1.555a.05.05 0 01-.022.041L6.473 13.3c-1.454.826-3.311.335-4.15-1.098m-.85-6.94A3.02 3.02 0 013.07 3.949v3.785a.51.51 0 00.262.451l3.93 2.237-1.366.779a.05.05 0 01-.048 0L2.585 9.342a2.98 2.98 0 01-1.113-4.094zm11.216 2.571L8.747 5.576l1.362-.776a.05.05 0 01.048 0l3.265 1.86a3 3 0 011.173 1.207 2.96 2.96 0 01-.27 3.2 3.05 3.05 0 01-1.36.997V8.279a.52.52 0 00-.276-.445m1.36-2.015l-.097-.057-3.226-1.855a.53.53 0 00-.53 0L6.249 6.153V4.598a.04.04 0 01.019-.04L9.533 2.7a3.07 3.07 0 013.257.139c.474.325.843.778 1.066 1.303.223.526.289 1.103.191 1.664zM5.503 8.575L4.139 7.8a.05.05 0 01-.026-.037V4.049c0-.57.166-1.127.476-1.607s.752-.864 1.275-1.105a3.08 3.08 0 013.234.41l-.096.054-3.23 1.838a.53.53 0 00-.265.455zm.742-1.577l1.758-1 1.762 1v2l-1.755 1-1.762-1z',
  },
  gemini: {
    viewBox: '0 0 65 65',
    path: 'M32.447 0c.68 0 1.273.465 1.439 1.125a38.904 38.904 0 001.999 5.905c2.152 5 5.105 9.376 8.854 13.125 3.751 3.75 8.126 6.703 13.125 8.855a38.98 38.98 0 005.906 1.999c.66.166 1.124.758 1.124 1.438 0 .68-.464 1.273-1.125 1.439a38.902 38.902 0 00-5.905 1.999c-5 2.152-9.375 5.105-13.125 8.854-3.749 3.751-6.702 8.126-8.854 13.125a38.973 38.973 0 00-2 5.906 1.485 1.485 0 01-1.438 1.124c-.68 0-1.272-.464-1.438-1.125a38.913 38.913 0 00-2-5.905c-2.151-5-5.103-9.375-8.854-13.125-3.75-3.749-8.125-6.702-13.125-8.854a38.973 38.973 0 00-5.905-2A1.485 1.485 0 010 32.448c0-.68.465-1.272 1.125-1.438a38.903 38.903 0 005.905-2c5-2.151 9.376-5.104 13.125-8.854 3.75-3.749 6.703-8.125 8.855-13.125a38.972 38.972 0 001.999-5.905A1.485 1.485 0 0132.447 0z',
  },
}

/** Simplified small variants tuned for dense 12-13px sidebar rendering. */
export const TOOL_SIDEBAR_SMALL_ICONS = {
  claude: {
    viewBox: '0 0 16 16',
    path: 'M8 0.9l1.38 3.36 3.58-.9-.9 3.56L15.1 8l-3.04 1.08.9 3.56-3.58-.9L8 15.1l-1.38-3.36-3.58.9.9-3.56L.9 8l3.04-1.08-.9-3.56 3.58.9z',
  },
  codex: {
    viewBox: '0 0 16 16',
    path: 'M7.95 1.4c.98 0 1.77.79 1.77 1.77v1.01l.87-.5c.84-.49 1.92-.2 2.41.64.49.84.2 1.92-.64 2.41l-.87.5.87.5c.84.49 1.13 1.57.64 2.41-.49.84-1.57 1.13-2.41.64l-.87-.5v1.01c0 .98-.79 1.77-1.77 1.77s-1.77-.79-1.77-1.77v-1.01l-.87.5c-.84.49-1.92.2-2.41-.64-.49-.84-.2-1.92.64-2.41l.87-.5-.87-.5c-.84-.49-1.13-1.57-.64-2.41.49-.84 1.57-1.13 2.41-.64l.87.5V3.17c0-.98.79-1.77 1.77-1.77zM8 5.68A2.32 2.32 0 108 10.32 2.32 2.32 0 108 5.68z',
  },
  gemini: {
    viewBox: '0 0 16 16',
    path: 'M8 0.7c.3 0 .56.2.63.49.49 1.95 1.2 3.48 2.13 4.58.93.93 2.46 1.64 4.58 2.13.29.07.49.33.49.63 0 .3-.2.56-.49.63-1.95.49-3.48 1.2-4.58 2.13-.93.93-1.64 2.46-2.13 4.58-.07.29-.33.49-.63.49-.3 0-.56-.2-.63-.49-.49-1.95-1.2-3.48-2.13-4.58-.93-.93-2.46-1.64-4.58-2.13A.65.65 0 010 8c0-.3.2-.56.49-.63 1.95-.49 3.48-1.2 4.58-2.13.93-.93 1.64-2.46 2.13-4.58A.65.65 0 018 .7z',
  },
}

/** Display names for each CLI tool. */
export const TOOL_NAMES = {
  claude: 'Claude',
  codex: 'Codex',
  gemini: 'Gemini',
}

/**
 * Pre-composed grouped icon variants for team session indicators.
 * Each entry contains a wider viewBox with multiple path segments,
 * rendered as a single cohesive SVG instead of CSS-composed individuals.
 * Uses sidebarSmall paths arranged in a shared coordinate space.
 *
 * Layout: 16×16 per icon, 4-unit gap between icons.
 *   2-icon: viewBox 0 0 36 16
 *   3-icon: viewBox 0 0 56 16
 */
export const TOOL_GROUPED_ICONS = {
  'claude+codex': {
    viewBox: '0 0 36 16',
    paths: [
      { d: TOOL_SIDEBAR_SMALL_ICONS.claude.path },
      { d: TOOL_SIDEBAR_SMALL_ICONS.codex.path, transform: 'translate(20 0)' },
    ],
  },
  'claude+gemini': {
    viewBox: '0 0 36 16',
    paths: [
      { d: TOOL_SIDEBAR_SMALL_ICONS.claude.path },
      { d: TOOL_SIDEBAR_SMALL_ICONS.gemini.path, transform: 'translate(20 0)' },
    ],
  },
  'codex+gemini': {
    viewBox: '0 0 36 16',
    paths: [
      { d: TOOL_SIDEBAR_SMALL_ICONS.codex.path },
      { d: TOOL_SIDEBAR_SMALL_ICONS.gemini.path, transform: 'translate(20 0)' },
    ],
  },
  'claude+codex+gemini': {
    viewBox: '0 0 56 16',
    paths: [
      { d: TOOL_SIDEBAR_SMALL_ICONS.claude.path },
      { d: TOOL_SIDEBAR_SMALL_ICONS.codex.path, transform: 'translate(20 0)' },
      { d: TOOL_SIDEBAR_SMALL_ICONS.gemini.path, transform: 'translate(40 0)' },
    ],
  },
}

/**
 * Look up a pre-composed grouped icon for a set of tool keys.
 * Tools are sorted to a canonical order so lookup is order-independent.
 * Returns null if no grouped icon exists for the combination.
 */
export function getGroupedIcon(tools) {
  if (!Array.isArray(tools) || tools.length < 2) return null
  const CANONICAL_ORDER = ['claude', 'codex', 'gemini']
  const sorted = [...tools].sort((a, b) => {
    const ai = CANONICAL_ORDER.indexOf(a)
    const bi = CANONICAL_ORDER.indexOf(b)
    return (ai === -1 ? 99 : ai) - (bi === -1 ? 99 : bi)
  })
  const key = sorted.join('+')
  return TOOL_GROUPED_ICONS[key] || null
}

/** Get icon data for a tool key, with claude fallback. */
export function getToolIcon(tool, variant = 'default') {
  if (variant === 'sidebarSmall') {
    return TOOL_SIDEBAR_SMALL_ICONS[tool] || TOOL_SIDEBAR_SMALL_ICONS.claude
  }
  return TOOL_ICONS[tool] || TOOL_ICONS.claude
}

/** Get display name for a tool key, with claude fallback. */
export function getToolName(tool) {
  return TOOL_NAMES[tool] || 'Claude'
}
