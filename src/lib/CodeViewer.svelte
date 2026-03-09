<script>
  import { themeTokens } from './themeTokens.js'

  let { code = '', language = '', dark = false, codeTheme = 'github-light', scrollToLine = null } = $props()

  let highlightedHtml = $state('')
  let ready = $state(false)
  let markdownModulePromise = null
  let highlightRequestId = 0

  async function getMarkdownModule() {
    if (!markdownModulePromise) {
      markdownModulePromise = import('./markdown.js')
    }
    return markdownModulePromise
  }

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens
  const lineNumColor = $derived(dark ? 'text-zinc-700' : 'text-zinc-300')

  // Highlight when code, language, or theme changes
  $effect(() => {
    const src = code
    const lang = language
    const theme = codeTheme
    const requestId = ++highlightRequestId
    let cancelled = false
    if (!src) { highlightedHtml = ''; ready = true; return }

    ready = false
    getMarkdownModule()
      .then(({ highlightCode }) => highlightCode(src, lang || 'text', theme))
      .then(html => {
        if (cancelled || requestId !== highlightRequestId) return
        highlightedHtml = html
        ready = true
      }).catch((err) => {
        if (cancelled || requestId !== highlightRequestId) return
        // Shiki failed (e.g., WASM blocked by CSP) — show plain text fallback
        highlightedHtml = ''
        ready = true
        console.error(`[code] Shiki failed for "${lang}": ${err}`)
      })

    return () => {
      cancelled = true
    }
  })

  // Split code into lines for fallback display
  const lines = $derived(code ? code.split('\n') : [])

  let containerEl = $state(null)

  // Scroll to target line after highlighting completes
  $effect(() => {
    if (!ready || !scrollToLine || !containerEl) return
    // Wait a tick for DOM to update
    requestAnimationFrame(() => {
      const lineEls = containerEl.querySelectorAll('.line')
      const targetIdx = scrollToLine - 1
      if (targetIdx >= 0 && targetIdx < lineEls.length) {
        const el = lineEls[targetIdx]
        el.scrollIntoView({ block: 'center', behavior: 'smooth' })
        el.classList.add('line-highlight')
        setTimeout(() => el.classList.remove('line-highlight'), 2000)
      }
    })
  })
</script>

<div class="code-viewer {dark ? 'code-viewer-dark' : ''}" data-testid="code-viewer" bind:this={containerEl}>
  {#if ready && highlightedHtml}
    <div class="code-highlighted">
      {@html highlightedHtml}
    </div>
  {:else}
    <!-- Fallback: plain text with line numbers while Shiki loads -->
    <pre class="p-6 text-[13px] font-mono leading-[1.6] {t.textBody} whitespace-pre-wrap break-words"><code>{#each lines as line, i}<span class="inline-block w-[3em] text-right mr-4 select-none {lineNumColor}">{i + 1}</span>{line}
{/each}</code></pre>
  {/if}
</div>

<style>
  .code-viewer {
    min-height: 100%;
  }

  .code-highlighted {
    padding: 1.5rem;
  }

  /* Shiki pre/code reset */
  .code-highlighted :global(pre.shiki) {
    margin: 0 !important;
    padding: 0 !important;
    background: transparent !important;
    border: none !important;
    font-family: var(--font-mono) !important;
    font-size: 13px !important;
    line-height: 1.6 !important;
    overflow-x: auto;
  }

  .code-highlighted :global(pre.shiki code) {
    font-family: inherit !important;
    font-size: inherit !important;
    line-height: inherit !important;
    padding: 0 !important;
    background: transparent !important;
    counter-reset: line;
  }

  /* Line numbers via CSS counter on Shiki's .line spans */
  .code-highlighted :global(pre.shiki code .line) {
    display: inline-block;
    width: 100%;
  }

  .code-highlighted :global(pre.shiki code .line::before) {
    counter-increment: line;
    content: counter(line);
    display: inline-block;
    width: 3em;
    text-align: right;
    margin-right: 1rem;
    padding-right: 0.75rem;
    user-select: none;
    color: var(--color-zinc-300, #d4d4d8);
    border-right: 1px solid var(--color-zinc-100, #f4f4f5);
  }

  .code-viewer-dark :global(pre.shiki code .line::before) {
    color: var(--color-zinc-700, #3f3f46);
    border-right-color: var(--color-zinc-800, #27272a);
  }

  /* Line highlight flash for scroll-to-line */
  .code-viewer :global(.line-highlight) {
    background: var(--color-warning-500, #eab308) / 0.15;
    background: color-mix(in srgb, var(--color-warning-500, #eab308) 15%, transparent);
    transition: background 0.5s ease-out;
  }
</style>
