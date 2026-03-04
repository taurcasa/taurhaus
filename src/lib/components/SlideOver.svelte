<script>
  import { tick } from 'svelte'
  import { themeTokens } from '../themeTokens.js'

  let {
    open = false,
    title = '',
    width = 420,
    onClose = () => {},
    dark = false,
    children,
  } = $props()

  const t = $derived(themeTokens(dark))
  const panelTone = $derived(dark ? 'bg-brand-950 border-l border-zinc-800 text-zinc-100' : 'bg-white border-l border-zinc-200 text-zinc-900')
  const headerTone = $derived(dark ? 'bg-brand-950 border-zinc-800' : 'bg-white border-zinc-200')
  const closeTone = $derived(
    dark
      ? 'text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800/80'
      : 'text-zinc-500 hover:text-zinc-900 hover:bg-zinc-100'
  )

  let panelElement = $state(null)
  let rendered = $state(false)
  let closing = $state(false)
  let prevOpen = false
  let exitTimer = null

  const visible = $derived(open || rendered)

  function getFocusableElements(panel) {
    if (!panel) return []
    return [...panel.querySelectorAll(
      'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
    )]
  }

  function handleBackdropClick(event) {
    if (event.target === event.currentTarget) {
      event.stopPropagation()
      onClose()
    }
  }

  function handleGlobalKeydown(event, panel) {
    if (!visible || closing) return

    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
      return
    }

    if (event.key !== 'Tab') return

    const focusable = getFocusableElements(panel)
    if (focusable.length === 0) {
      event.preventDefault()
      panel?.focus()
      return
    }

    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    const active = document.activeElement

    if (event.shiftKey) {
      if (active === first || !panel.contains(active)) {
        event.preventDefault()
        last.focus()
      }
      return
    }

    if (active === last) {
      event.preventDefault()
      first.focus()
    } else if (!panel.contains(active)) {
      event.preventDefault()
      first.focus()
    }
  }

  $effect(() => {
    if (open) {
      prevOpen = true
      if (exitTimer) {
        clearTimeout(exitTimer)
        exitTimer = null
      }
      rendered = false
      closing = false
      return
    }

    if (!prevOpen || rendered || closing) return

    closing = true
    rendered = true
    exitTimer = setTimeout(() => {
      rendered = false
      closing = false
      prevOpen = false
      exitTimer = null
    }, 150)

    return () => {
      if (exitTimer) {
        clearTimeout(exitTimer)
        exitTimer = null
      }
    }
  })

  $effect(() => {
    if (!visible || closing) return

    const panel = panelElement
    if (!panel) return
    let cancelled = false

    async function focusOnOpen() {
      await tick()
      if (cancelled) return
      const focusable = getFocusableElements(panel)
      const target = focusable[0] || panel
      target?.focus()
    }

    void focusOnOpen()

    function handleKeydown(event) {
      handleGlobalKeydown(event, panel)
    }

    window.addEventListener('keydown', handleKeydown)

    return () => {
      cancelled = true
      window.removeEventListener('keydown', handleKeydown)
    }
  })
</script>

{#if visible}
  <div class="fixed inset-0 z-[45] pointer-events-none" data-testid="slideover-root">
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="fixed inset-0 z-40 bg-black/30 {closing ? 'pointer-events-none slideover-backdrop-exit' : 'pointer-events-auto slideover-backdrop-enter'}"
      onclick={handleBackdropClick}
      data-testid="slideover-backdrop"
    ></div>

    <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
    <aside
      bind:this={panelElement}
      class="fixed top-0 right-0 bottom-0 z-[45] flex flex-col pointer-events-auto {panelTone} {closing ? 'slideover-panel-exit' : 'slideover-panel-enter'}"
      style={`width: ${width}px;`}
      role="dialog"
      aria-modal="true"
      aria-label={title || 'Panel'}
      tabindex="-1"
      data-testid="slideover-panel"
    >
      <header class="sticky top-0 h-12 flex items-center justify-between px-4 border-b shrink-0 {headerTone}">
        <h2 class="text-sm font-semibold {t.textPrimary}" data-testid="slideover-title">{title}</h2>
        <button
          class="h-7 w-7 rounded-md flex items-center justify-center transition-colors {closeTone}"
          onclick={onClose}
          aria-label="Close"
          data-testid="slideover-close"
        >
          ✕
        </button>
      </header>

      <div class="flex-1 overflow-y-auto p-4" data-testid="slideover-body">
        {@render children()}
      </div>
    </aside>
  </div>
{/if}

<style>
  .slideover-panel-enter {
    animation: slideover-enter 200ms ease-out;
  }

  .slideover-panel-exit {
    animation: slideover-exit 150ms ease-in forwards;
  }

  .slideover-backdrop-enter {
    animation: slideover-backdrop-enter 200ms ease-out;
  }

  .slideover-backdrop-exit {
    animation: slideover-backdrop-exit 150ms ease-in forwards;
  }

  @keyframes slideover-enter {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }

  @keyframes slideover-exit {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(100%);
    }
  }

  @keyframes slideover-backdrop-enter {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  @keyframes slideover-backdrop-exit {
    from {
      opacity: 1;
    }
    to {
      opacity: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .slideover-panel-enter,
    .slideover-panel-exit,
    .slideover-backdrop-enter,
    .slideover-backdrop-exit {
      animation: none;
    }
  }
</style>
