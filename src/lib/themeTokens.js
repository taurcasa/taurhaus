/**
 * Shared theme tokens for dark/light mode.
 *
 * These are the color class strings used across 3+ components.
 * Component-specific tokens (diff colors, card states, etc.) stay local.
 *
 * Usage:
 *   const t = $derived(themeTokens(dark))
 *   // then in template: {t.textPrimary}
 */
export function themeTokens(dark) {
  return {
    // Text hierarchy
    textPrimary:   dark ? 'text-zinc-100' : 'text-zinc-900',
    textSecondary: dark ? 'text-zinc-300' : 'text-zinc-600',
    textTertiary:  'text-zinc-500',
    textMuted:     dark ? 'text-zinc-600' : 'text-zinc-500',
    textBody:      dark ? 'text-zinc-300' : 'text-zinc-700',

    // Surfaces
    mainBg:    dark ? 'bg-zinc-950' : 'bg-white',
    cardBg:    dark ? 'bg-zinc-900/60' : 'bg-zinc-50/80',
    sectionBg: dark ? 'bg-zinc-900/30' : 'bg-zinc-50/50',
    listBg:    dark ? 'bg-zinc-900' : 'bg-zinc-50',

    // Borders
    keyline: dark ? 'border-zinc-800' : 'border-zinc-200',

    // Interactive
    hoverRow:    dark ? 'hover:bg-zinc-900' : 'hover:bg-zinc-50',
    listHover:   dark ? 'hover:bg-zinc-800' : 'hover:bg-zinc-100',
    listSelected: dark ? 'bg-brand-900/40 text-brand-300' : 'bg-brand-100/80 text-brand-700',
    fileBg:      dark ? 'hover:bg-zinc-800/50' : 'hover:bg-zinc-100/80',

    // Links & accents
    linkColor: dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700',
    hashColor: dark ? 'text-brand-400' : 'text-brand-600',
    questionMark: dark ? 'text-amber-400' : 'text-amber-600',

    // Form elements
    inputBg: dark ? 'bg-zinc-800 text-zinc-100 placeholder:text-zinc-500' : 'bg-zinc-100 text-zinc-900 placeholder:text-zinc-400',
    checkBg: dark ? 'bg-zinc-800 border-zinc-600' : 'bg-white border-zinc-300',
    labelColor: dark ? 'text-zinc-500' : 'text-zinc-400',
  }
}
