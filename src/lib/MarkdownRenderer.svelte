<script>
  import { readProjectAsset, openExternalUrl } from './ipc.js'
  import * as assetCache from './assetCache.js'
  import { resolveRelativePath } from './pathUtils.js'

  let {
    source = '',
    dark = false,
    codeTheme = 'github-light',
    projectId = null,
    filePath = null,
    scrollToAnchor = null,
    onNavigate = null,
  } = $props()

  let html = $state('')
  let loading = $state(true)
  let container = $state(null)
  let mermaidRenderCounter = 0
  let markdownModulePromise = null
  let renderRequestId = 0

  async function getMarkdownModule() {
    if (!markdownModulePromise) {
      markdownModulePromise = import('./markdown.js')
    }
    return markdownModulePromise
  }

  // Re-render when source, theme, or project changes.
  // renderMarkdown already falls back to plain markdown-it if Shiki fails,
  // so the catch here only fires if something truly catastrophic happens.
  $effect(() => {
    const src = source
    const theme = codeTheme
    const requestId = ++renderRequestId
    let cancelled = false
    loading = true
    getMarkdownModule()
      .then(({ renderMarkdown }) => renderMarkdown(src, theme))
      .then(result => {
        if (cancelled || requestId !== renderRequestId) return
        html = result
        loading = false
      }).catch((err) => {
        if (cancelled || requestId !== renderRequestId) return
        console.error(`[markdown] render failed completely: ${err}`)
        html = `<pre style="white-space:pre-wrap;word-break:break-word">${src.replace(/&/g,'&amp;').replace(/</g,'&lt;')}</pre>`
        loading = false
      })

    return () => {
      cancelled = true
    }
  })

  // After HTML is rendered, resolve relative image src via cache or IPC
  $effect(() => {
    if (!container || !projectId || loading) return
    const images = container.querySelectorAll('img')
    for (const img of images) {
      const src = img.getAttribute('src')
      if (!src || /^(https?:|data:|blob:|asset:)/.test(src)) continue

      const resolved = resolveRelativePath(filePath, src)

      // Check cache first — synchronous, no flicker
      const cached = assetCache.get(projectId, resolved)
      if (cached) {
        img.src = cached
        continue
      }

      // Cache miss — load via IPC and cache for next time
      readProjectAsset(projectId, resolved).then(dataUri => {
        if (dataUri) {
          assetCache.set(projectId, resolved, dataUri)
          img.src = dataUri
        }
      }).catch(() => {
        // Image not found — leave alt text visible
      })
    }
  })

  // After HTML is rendered, replace Mermaid code blocks with rendered SVG.
  $effect(() => {
    if (!container || loading) return
    const isDark = dark
    let cancelled = false

    ;(async () => {
      const mermaidBlocks = Array.from(container.querySelectorAll('pre:has(> code.language-mermaid):not([data-mermaid-processed])'))
      if (mermaidBlocks.length === 0) return

      const [{ default: mermaid }, { default: DOMPurify }] = await Promise.all([
        import('mermaid'),
        import('dompurify'),
      ])
      if (cancelled) return

      mermaid.initialize({
        startOnLoad: false,
        theme: isDark ? 'dark' : 'default',
      })

      for (const pre of mermaidBlocks) {
        if (cancelled) return

        const codeElement = pre.querySelector('code.language-mermaid')
        const code = codeElement?.textContent?.trim() ?? ''
        if (!code) {
          pre.setAttribute('data-mermaid-processed', 'true')
          continue
        }

        try {
          const id = globalThis.crypto?.randomUUID
            ? `mermaid-${globalThis.crypto.randomUUID().slice(0, 8)}`
            : `mermaid-${Date.now()}-${mermaidRenderCounter++}`
          const { svg } = await mermaid.render(id, code)
          if (cancelled) return

          const sanitizedSvg = DOMPurify.sanitize(svg, {
            USE_PROFILES: { svg: true, svgFilters: true },
            ADD_TAGS: ['foreignObject'],
          })

          pre.innerHTML = sanitizedSvg
          pre.classList.add('mermaid-diagram')
        } catch (err) {
          console.error('[markdown] mermaid render failed', err)

          const prev = pre.previousElementSibling
          if (!prev || !prev.classList.contains('mermaid-error')) {
            const error = document.createElement('div')
            error.className = 'mermaid-error'
            error.textContent = 'Unable to render Mermaid diagram.'
            pre.parentNode?.insertBefore(error, pre)
          }
        } finally {
          pre.setAttribute('data-mermaid-processed', 'true')
        }
      }
    })().catch((err) => {
      console.error('[markdown] failed to initialize mermaid', err)
    })

    return () => {
      cancelled = true
    }
  })

  // Cross-file anchor navigation: scroll to target heading after render.
  $effect(() => {
    if (!scrollToAnchor || loading || !container) return

    const escaped = globalThis.CSS?.escape
      ? globalThis.CSS.escape(scrollToAnchor)
      : scrollToAnchor.replace(/["\\#.:/\\[\\]]/g, '\\$&')
    const target = container.querySelector(`#${escaped}`)
    if (target && typeof target.scrollIntoView === 'function') {
      target.scrollIntoView({ behavior: 'smooth' })
    }
  })

  // Intercept link clicks inside rendered markdown
  function handleClick(e) {
    const anchor = e.target.closest('a')
    if (!anchor) return

    const href = anchor.getAttribute('href')
    if (!href) return

    // Anchor links — let browser handle scroll
    if (href.startsWith('#')) return

    e.preventDefault()

    // External URL — open in system browser
    if (/^https?:\/\//.test(href) || href.startsWith('mailto:')) {
      openExternalUrl(href).catch((err) => {
        console.error(`[markdown] failed to open URL: ${href}`, err)
      })
      return
    }

    // Relative path — navigate to file in the viewer
    if (onNavigate) {
      onNavigate(href)
    }
  }
</script>

{#if loading && !html}
  <div class="th-prose" data-testid="markdown-loading">
    <div class="space-y-3 animate-pulse">
      <div class="h-4 w-3/4 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'}"></div>
      <div class="h-3 w-full rounded {dark ? 'bg-zinc-800/60' : 'bg-zinc-100'}"></div>
      <div class="h-3 w-5/6 rounded {dark ? 'bg-zinc-800/60' : 'bg-zinc-100'}"></div>
      <div class="h-3 w-2/3 rounded {dark ? 'bg-zinc-800/60' : 'bg-zinc-100'}"></div>
    </div>
  </div>
{:else}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={container}
    class="th-prose {dark ? 'th-prose-dark' : ''}"
    data-testid="markdown-content"
    onclick={handleClick}
  >
    {@html html}
  </div>
{/if}

<style>
  /* ═══════════════════════════════════════════
   * taurhaus Rendered Content Typography
   * Aligned with UI type scale (2b) while
   * using reading-optimized line-height (2d)
   * ═══════════════════════════════════════════ */

  .th-prose {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 400;
    line-height: 1.5;
    color: var(--color-zinc-700, #3f3f46);
    word-wrap: break-word;
    overflow-wrap: break-word;
  }

  .th-prose-dark {
    color: var(--color-zinc-300, #d4d4d8);
  }

  /* ─── Headings ─── */

  .th-prose :global(h1) {
    font-size: 20px;
    font-weight: 600;
    line-height: 1.3;
    letter-spacing: -0.01em;
    color: var(--color-zinc-900, #18181b);
    margin-top: 2rem;
    margin-bottom: 0.75rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid var(--color-zinc-200, #e4e4e7);
  }

  .th-prose-dark :global(h1) {
    color: var(--color-zinc-100, #f4f4f5);
    border-bottom-color: var(--color-zinc-800, #27272a);
  }

  .th-prose :global(h2) {
    font-size: 16px;
    font-weight: 600;
    line-height: 1.4;
    color: var(--color-zinc-900, #18181b);
    margin-top: 1.75rem;
    margin-bottom: 0.5rem;
    padding-bottom: 0.375rem;
    border-bottom: 1px solid var(--color-zinc-200, #e4e4e7);
  }

  .th-prose-dark :global(h2) {
    color: var(--color-zinc-100, #f4f4f5);
    border-bottom-color: var(--color-zinc-800, #27272a);
  }

  .th-prose :global(h3) {
    font-size: 14px;
    font-weight: 600;
    line-height: 1.4;
    color: var(--color-zinc-800, #27272a);
    margin-top: 1.5rem;
    margin-bottom: 0.375rem;
  }

  .th-prose-dark :global(h3) {
    color: var(--color-zinc-200, #e4e4e7);
  }

  .th-prose :global(h4) {
    font-size: 13px;
    font-weight: 600;
    line-height: 1.4;
    color: var(--color-zinc-800, #27272a);
    margin-top: 1.25rem;
    margin-bottom: 0.375rem;
  }

  .th-prose-dark :global(h4) {
    color: var(--color-zinc-200, #e4e4e7);
  }

  /* First heading should not have top margin */
  .th-prose :global(:first-child) {
    margin-top: 0;
  }

  /* ─── Paragraphs ─── */

  .th-prose :global(p) {
    margin-top: 0;
    margin-bottom: 0.875rem;
  }

  /* ─── Links ─── */

  .th-prose :global(a) {
    color: var(--color-brand-600, #0d9488);
    text-decoration: none;
    font-weight: 500;
  }

  .th-prose :global(a:hover) {
    color: var(--color-brand-700, #0f766e);
    text-decoration: underline;
  }

  .th-prose-dark :global(a) {
    color: var(--color-brand-400, #2dd4bf);
  }

  .th-prose-dark :global(a:hover) {
    color: var(--color-brand-200, #99f6e4);
  }

  /* ─── Strong / Emphasis ─── */

  .th-prose :global(strong) {
    font-weight: 600;
    color: var(--color-zinc-900, #18181b);
  }

  .th-prose-dark :global(strong) {
    color: var(--color-zinc-100, #f4f4f5);
  }

  .th-prose :global(em) {
    font-style: italic;
  }

  /* ─── Lists ─── */

  .th-prose :global(ul) {
    list-style-type: disc;
    padding-left: 1.5rem;
    margin-top: 0;
    margin-bottom: 1rem;
  }

  .th-prose :global(ol) {
    list-style-type: decimal;
    padding-left: 1.5rem;
    margin-top: 0;
    margin-bottom: 1rem;
  }

  .th-prose :global(li) {
    margin-bottom: 0.375rem;
  }

  .th-prose :global(li > ul),
  .th-prose :global(li > ol) {
    margin-top: 0.25rem;
    margin-bottom: 0;
  }

  /* ─── Blockquotes ─── */

  .th-prose :global(blockquote) {
    font-size: 14px;
    font-style: italic;
    line-height: 1.5;
    color: var(--color-zinc-600, #52525b);
    border-left: 3px solid var(--color-brand-500, #14b8a6);
    padding: 0.5rem 0 0.5rem 1rem;
    margin: 1rem 0;
  }

  .th-prose-dark :global(blockquote) {
    color: var(--color-zinc-400, #a1a1aa);
    border-left-color: var(--color-brand-400, #2dd4bf);
  }

  .th-prose :global(blockquote p) {
    margin-bottom: 0;
  }

  /* ─── Inline Code ─── */

  .th-prose :global(code) {
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 400;
    background: var(--color-zinc-100, #f4f4f5);
    color: var(--color-zinc-800, #27272a);
    padding: 0.125rem 0.375rem;
    border-radius: 4px;
  }

  .th-prose-dark :global(code) {
    background: var(--color-zinc-800, #27272a);
    color: var(--color-zinc-200, #e4e4e7);
  }

  /* Code inside pre blocks — reset inline styles */
  .th-prose :global(pre code) {
    background: transparent;
    padding: 0;
    border-radius: 0;
    font-size: inherit;
    color: inherit;
  }

  /* ─── Code Blocks (Shiki output) ─── */

  .th-prose :global(pre) {
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.5;
    margin: 1rem 0;
    padding: 1rem;
    border-radius: 8px;
    overflow-x: auto;
    background: var(--color-zinc-50, #fafafa);
    border: 1px solid var(--color-zinc-200, #e4e4e7);
  }

  .th-prose-dark :global(pre) {
    background: var(--color-zinc-900, #18181b);
    border-color: var(--color-zinc-800, #27272a);
  }

  /* Let Shiki's inline style="background-color: ..." show through for
     authentic theme backgrounds. The generic pre rule above provides a
     fallback for non-Shiki pre blocks. */
  .th-prose :global(pre.shiki) {
    background: unset;
  }

  .th-prose :global(pre.mermaid-diagram) {
    background: transparent;
    border: none;
    padding: 0;
    overflow-x: auto;
  }

  .th-prose :global(pre.mermaid-diagram svg) {
    display: block;
    max-width: 100%;
    height: auto;
    margin: 0 auto;
  }

  .th-prose :global(.mermaid-error) {
    font-size: 12px;
    font-weight: 500;
    margin-bottom: 0.375rem;
    color: var(--color-red-600, #dc2626);
  }

  .th-prose-dark :global(.mermaid-error) {
    color: var(--color-red-400, #f87171);
  }

  /* ─── Horizontal Rules ─── */

  .th-prose :global(hr) {
    border: none;
    border-top: 1px solid var(--color-zinc-200, #e4e4e7);
    margin: 1.5rem 0;
  }

  .th-prose-dark :global(hr) {
    border-top-color: var(--color-zinc-800, #27272a);
  }

  /* ─── Tables ─── */

  .th-prose :global(table) {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    margin: 1rem 0;
  }

  .th-prose :global(th) {
    font-weight: 600;
    text-align: left;
    padding: 0.5rem 0.75rem;
    border-bottom: 2px solid var(--color-zinc-200, #e4e4e7);
    color: var(--color-zinc-800, #27272a);
  }

  .th-prose-dark :global(th) {
    border-bottom-color: var(--color-zinc-700, #3f3f46);
    color: var(--color-zinc-200, #e4e4e7);
  }

  .th-prose :global(td) {
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--color-zinc-100, #f4f4f5);
  }

  .th-prose-dark :global(td) {
    border-bottom-color: var(--color-zinc-800, #27272a);
  }

  .th-prose :global(tr:last-child td) {
    border-bottom: none;
  }

  /* ─── Images ─── */

  .th-prose :global(img) {
    max-width: 100%;
    height: auto;
    border-radius: 6px;
    margin: 1rem 0;
  }

  /* ─── Task Lists (GitHub-style checkboxes) ─── */

  .th-prose :global(ul.contains-task-list) {
    list-style-type: none;
    padding-left: 0;
  }

  .th-prose :global(li.task-list-item) {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }

  .th-prose :global(input[type="checkbox"]) {
    margin: 0;
    accent-color: var(--color-brand-500, #14b8a6);
  }
</style>
