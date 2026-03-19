<script>
  let {
    items = [],
    x = 0,
    y = 0,
    dark = false,
    onClose = () => {},
  } = $props()

  // Color tokens
  const menuBg      = $derived(dark ? 'bg-zinc-900' : 'bg-white')
  const menuBorder  = $derived(dark ? 'border-zinc-700' : 'border-zinc-200')
  const textPrimary = $derived(dark ? 'text-zinc-200' : 'text-zinc-900')
  const textMuted   = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const hoverBg     = $derived(dark ? 'hover:bg-zinc-800' : 'hover:bg-zinc-100')
  const focusBg     = $derived(dark ? 'bg-zinc-800' : 'bg-zinc-100')
  const separatorBg = $derived(dark ? 'bg-zinc-800' : 'bg-zinc-200')

  let menuEl = $state(null)
  let focusIndex = $state(-1)
  let typeaheadBuffer = ''
  let typeaheadTimer = null

  // Only actionable (non-separator) items for keyboard nav
  const actionableItems = $derived(items.filter(i => !i.separator))

  // Viewport-aware positioning
  let adjustedX = $state(0)
  let adjustedY = $state(0)

  $effect(() => {
    // Read x/y props reactively
    const px = x
    const py = y

    if (!menuEl) {
      adjustedX = px
      adjustedY = py
      return
    }

    const rect = menuEl.getBoundingClientRect()
    const vw = window.innerWidth
    const vh = window.innerHeight

    let nx = px
    let ny = py

    if (px + rect.width > vw - 8) {
      nx = vw - rect.width - 8
    }
    if (py + rect.height > vh - 8) {
      ny = vh - rect.height - 8
    }

    adjustedX = Math.max(8, nx)
    adjustedY = Math.max(8, ny)
  })

  // Close on click outside
  $effect(() => {
    function handleClick(e) {
      if (menuEl && !menuEl.contains(e.target)) {
        onClose()
      }
    }
    // Use mousedown so it fires before the click on the item
    window.addEventListener('mousedown', handleClick)
    return () => window.removeEventListener('mousedown', handleClick)
  })

  // Keyboard navigation
  $effect(() => {
    if (!menuEl) return
    queueMicrotask(() => {
      menuEl?.focus()
    })
  })

  $effect(() => {
    function handleKeydown(e) {
      if (e.key === 'Escape') {
        e.preventDefault()
        onClose()
        return
      }

      if (e.key === 'ArrowDown') {
        e.preventDefault()
        // Find next non-disabled item
        let next = focusIndex
        for (let i = 0; i < actionableItems.length; i++) {
          next = (next + 1) % actionableItems.length
          if (!actionableItems[next].disabled) break
        }
        focusIndex = next
        return
      }

      if (e.key === 'ArrowUp') {
        e.preventDefault()
        let prev = focusIndex
        for (let i = 0; i < actionableItems.length; i++) {
          prev = prev <= 0 ? actionableItems.length - 1 : prev - 1
          if (!actionableItems[prev].disabled) break
        }
        focusIndex = prev
        return
      }

      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault()
        const itemIndex = focusIndex >= 0
          ? focusIndex
          : actionableItems.findIndex(item => !item.disabled)
        if (itemIndex >= 0 && itemIndex < actionableItems.length) {
          const item = actionableItems[itemIndex]
          if (!item.disabled && item.action) {
            item.action()
            if (!item.keepOpen) onClose()
          }
        }
      }

      if (e.key.length === 1 && !e.altKey && !e.ctrlKey && !e.metaKey && /\S/.test(e.key)) {
        const query = `${typeaheadBuffer}${e.key.toLowerCase()}`
        const findIndex = (value) => actionableItems.findIndex((item) => (
          !item.disabled && String(item.label || '').trim().toLowerCase().startsWith(value)
        ))

        let nextIndex = findIndex(query)
        typeaheadBuffer = query

        if (nextIndex < 0) {
          nextIndex = findIndex(e.key.toLowerCase())
          typeaheadBuffer = e.key.toLowerCase()
        }

        if (typeaheadTimer) {
          clearTimeout(typeaheadTimer)
        }
        typeaheadTimer = setTimeout(() => {
          typeaheadBuffer = ''
          typeaheadTimer = null
        }, 350)

        if (nextIndex >= 0) {
          e.preventDefault()
          focusIndex = nextIndex
        }
      }
    }

    window.addEventListener('keydown', handleKeydown)
    return () => {
      window.removeEventListener('keydown', handleKeydown)
      if (typeaheadTimer) {
        clearTimeout(typeaheadTimer)
        typeaheadTimer = null
      }
      typeaheadBuffer = ''
    }
  })

  function handleItemClick(item) {
    if (item.disabled) return
    if (item.action) item.action()
    if (!item.keepOpen) onClose()
  }

  function getActionableIndex(item) {
    return actionableItems.indexOf(item)
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={menuEl}
  class="fixed z-[100] min-w-[160px] py-1 rounded-lg border shadow-lg {menuBg} {menuBorder}"
  style="left: {adjustedX}px; top: {adjustedY}px;"
  role="menu"
  tabindex="-1"
  data-testid="context-menu"
>
  {#each items as item, i}
    {#if item.separator}
      <div class="h-px mx-2 my-1 {separatorBg}" role="separator"></div>
    {:else}
      {@const aIdx = getActionableIndex(item)}
      <button
        class="w-full flex items-center gap-2.5 px-3 py-1.5 text-left text-[13px] transition-colors
          {item.disabled ? textMuted + ' cursor-default opacity-50' : item.danger ? 'text-danger-500 ' + hoverBg : textPrimary + ' ' + hoverBg}
          {aIdx === focusIndex && !item.disabled ? focusBg : ''}"
        role="menuitem"
        disabled={item.disabled}
        onmousedown={(e) => { e.stopPropagation(); handleItemClick(item) }}
        onmouseenter={() => { if (!item.disabled) focusIndex = aIdx }}
        data-testid={`menu-item-${item.label?.toLowerCase().replace(/\s+/g, '-')}`}
      >
        {#if item.icon}
          <span class="w-4 h-4 flex items-center justify-center shrink-0">{@html item.icon}</span>
        {/if}
        <span class="flex-1">{item.label}</span>
      </button>
    {/if}
  {/each}
</div>
