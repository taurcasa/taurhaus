<script>
  /**
   * The doorway — one 48px sticky header every utility surface opens behind.
   *
   * It repeats the exact icon+name key that opened the surface (key echo),
   * carries a labeled back affordance, an optional meta slot, at most one
   * ghost action, and the Esc hint. Nothing here scrolls away: the doorway
   * stays put while the surface body scrolls beneath it.
   */
  import { themeTokens } from '../themeTokens.js'

  let {
    dark = false,
    title,
    icon = null, // { viewBox, path } — the outline noun from railIcons.js
    meta = null, // snippet — the one optional meta slot
    action = null, // snippet — at most one ghost action
    backTestid = null,
    backAriaLabel = 'Back',
    onBack = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const doorwayTone = $derived(dark ? 'border-zinc-800 bg-zinc-950/90' : 'border-zinc-200 bg-white/90')
  const backTone = $derived(dark ? 'text-zinc-500 hover:text-zinc-300' : 'text-zinc-500 hover:text-zinc-700')
  const kbdTone = $derived(dark ? 'border-zinc-700 bg-zinc-900 text-zinc-500' : 'border-zinc-200 bg-white text-zinc-400')
</script>

<header
  class="sticky top-0 z-10 h-12 shrink-0 flex items-center gap-3 border-b px-6 backdrop-blur {doorwayTone}"
  data-testid="surface-doorway"
>
  <button
    class="-ml-1.5 flex items-center gap-1 rounded-md px-1.5 py-1 text-[12px] transition-colors {backTone} focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand-500"
    onclick={onBack}
    aria-label={backAriaLabel}
    data-testid={backTestid}
  >
    <svg class="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5" /></svg>
    Back
  </button>
  <span class="flex items-center gap-2 {t.textPrimary}">
    {#if icon}
      <svg class="h-4 w-4" fill="none" viewBox={icon.viewBox} stroke-width="1.5" stroke="currentColor" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d={icon.path} /></svg>
    {/if}
    <h1 class="text-[16px] font-semibold tracking-tight">{title}</h1>
  </span>
  <span class="ml-auto flex items-center gap-3">
    {#if meta}{@render meta()}{/if}
    {#if action}{@render action()}{/if}
    <kbd class="rounded-md border px-1.5 py-0.5 font-mono text-[10px] font-semibold {kbdTone}">Esc</kbd>
  </span>
</header>
