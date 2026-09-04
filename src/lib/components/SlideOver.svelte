<script>
  import { tick } from 'svelte'
  import { focusFirstInteractiveElement, handleModalKeydown, registerModalLayer } from '../a11y.js'
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
  const panelTone = $derived(
    dark
      ? 'bg-brand-950 border-l border-white/10 text-zinc-100'
      : 'bg-white border-l border-brand-200 text-zinc-900'
  )
  const headerTone = $derived(dark ? 'bg-brand-950 border-zinc-800' : 'bg-white border-zinc-200')
  const closeTone = $derived(
    dark
      ? 'text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800/80'
      : 'text-zinc-500 hover:text-zinc-900 hover:bg-zinc-100'
  )

  let panelElement = $state(null)
  let rootElement = $state(null)
  let rendered = $state(false)
  let closing = $state(false)
  let prevOpen = false
  let exitTimer = null
  let restoreFocusElement = null

  const visible = $derived(open || rendered)

  function captureRestoreFocusElement() {
    if (
      document.activeElement instanceof HTMLElement
      && !rootElement?.contains(document.activeElement)
    ) {
      restoreFocusElement = document.activeElement
    }
  }

  function restoreFocusToTrigger() {
    if (restoreFocusElement?.isConnected) {
      restoreFocusElement.focus()
    }
    restoreFocusElement = null
  }

  function handleBackdropClick(event) {
    if (event.target === event.currentTarget) {
      event.stopPropagation()
      onClose()
    }
  }

  $effect(() => {
    if (open) {
      captureRestoreFocusElement()
      prevOpen = true
      if (exitTimer) {
        clearTimeout(exitTimer)
        exitTimer = null
      }
      rendered = false
      closing = false
      return
    }

    // Keep this effect dependent on `open` alone; reading rendered/closing here cancels the exit timer.
    if (!prevOpen) return

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
    const root = rootElement
    if (!panel || !root) return
    let cancelled = false

    const unregisterModal = registerModalLayer(root)

    async function focusOnOpen() {
      await tick()
      if (cancelled) return
      focusFirstInteractiveElement(panel)
    }

    void focusOnOpen()

    function handleKeydown(event) {
      if (!visible || closing) return
      handleModalKeydown(event, panel, onClose)
    }

    window.addEventListener('keydown', handleKeydown)

    return () => {
      cancelled = true
      unregisterModal()
      window.removeEventListener('keydown', handleKeydown)
      restoreFocusToTrigger()
    }
  })
</script>

{#if visible}
  <div bind:this={rootElement} class="fixed inset-0 z-[45] pointer-events-none" data-testid="slideover-root">
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
      class="fixed top-0 right-0 bottom-0 z-[45] flex flex-col pointer-events-auto shadow-2xl {panelTone} {closing ? 'slideover-panel-exit' : 'slideover-panel-enter'}"
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
