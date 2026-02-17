<script>
  import { highlightCode } from './markdown.js'

  let { code = '', language = '', dark = false } = $props()

  let highlightedHtml = $state('')
  let ready = $state(false)

  const textBody = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const lineNumColor = $derived(dark ? 'text-zinc-700' : 'text-zinc-300')

  // Highlight when code, language, or dark changes
  $effect(() => {
    const src = code
    const lang = language
    const isDark = dark
    if (!src) { highlightedHtml = ''; ready = true; return }

    highlightCode(src, lang || 'text', isDark).then(html => {
      highlightedHtml = html
      ready = true
    })
  })

  // Split code into lines for fallback display
  const lines = $derived(code ? code.split('\n') : [])
</script>

<div class="code-viewer {dark ? 'code-viewer-dark' : ''}" data-testid="code-viewer">
  {#if ready && highlightedHtml}
    <div class="code-highlighted">
      {@html highlightedHtml}
    </div>
  {:else}
    <!-- Fallback: plain text with line numbers while Shiki loads -->
    <pre class="p-6 text-[13px] font-mono leading-[1.6] {textBody} whitespace-pre-wrap break-words"><code>{#each lines as line, i}<span class="inline-block w-[3em] text-right mr-4 select-none {lineNumColor}">{i + 1}</span>{line}
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
</style>
