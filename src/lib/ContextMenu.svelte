<script>
  /**
   * The app's one context menu, with a single level of submenus.
   *
   * An item that carries `children` becomes a parent row: hovering it after a
   * short intent delay or pressing ArrowRight opens a flyout beside it. Depth
   * stops at one — a menu that needs two is a panel.
   *
   * A parent that also has an `action` keeps it: clicking the row does what the
   * row has always done, and the flyout is the shortcut past it. Without an
   * action the row is nothing but its children, so a click opens them.
   *
   * Item shape: `{label, action, icon, disabled, separator, keepOpen, danger,
   * children}`. A child may be a `separator` too, and adds `meta`
   * (right-aligned muted text), `check` (a
   * leading tick, with the column reserved on every child so labels line up)
   * and `key` — a unique identity for the row, because a child's label is
   * whatever the caller's data says and two accounts can carry the same one.
   */
  let {
    items = [],
    x = 0,
    y = 0,
    dark = false,
    onClose = () => {},
    /** Label of a parent whose flyout opens on mount. Fixtures and tests only. */
    openChildOf = null,
  } = $props()

  // Color tokens
  const menuBg      = $derived(dark ? 'bg-zinc-900' : 'bg-white')
  const menuBorder  = $derived(dark ? 'border-zinc-700' : 'border-zinc-200')
  const textPrimary = $derived(dark ? 'text-zinc-200' : 'text-zinc-900')
  const textMuted   = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const hoverBg     = $derived(dark ? 'hover:bg-zinc-800' : 'hover:bg-zinc-100')
  const focusBg     = $derived(dark ? 'bg-zinc-800' : 'bg-zinc-100')
  const separatorBg = $derived(dark ? 'bg-zinc-800' : 'bg-zinc-200')
  const metaTone    = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const checkTone   = $derived(dark ? 'text-brand-400' : 'text-brand-600')

  /** Long enough that crossing a parent row on the way down does not open it. */
  const HOVER_INTENT_MS = 150
  /** The corridor: how long the pointer may be between row and flyout. */
  const LEAVE_GRACE_MS = 200
  const VIEWPORT_MARGIN = 8

  let menuEl = $state(null)
  let focusIndex = $state(-1)
  let typeaheadBuffer = ''
  let typeaheadTimer = null

  /** Index into `actionableItems` of the parent whose flyout is open. */
  let openIndex = $state(-1)
  let childFocusIndex = $state(-1)
  let submenuEl = $state(null)
  let parentEls = $state({})
  let submenuLeft = $state(0)
  let submenuTop = $state(0)
  /** The room a flyout has, or 0 before anything measured it. */
  let submenuMaxHeight = $state(
    typeof window === 'undefined' ? 0 : Math.max(0, window.innerHeight - VIEWPORT_MARGIN * 2)
  )
  let childEls = $state({})
  let hoverTimer = null
  let leaveTimer = null
  /**
   * Bumped by anything that can have moved a menu or the room around it: a
   * window resize, or the root menu growing rows after it opened.
   */
  let layoutTick = $state(0)

  // Only actionable (non-separator) items for keyboard nav
  const actionableItems = $derived(items.filter(i => !i.separator))
  const openItem = $derived(openIndex >= 0 ? actionableItems[openIndex] ?? null : null)
  const openChildren = $derived(openItem?.children ?? [])

  // Depth stops at one, and a caller that nests deeper gets told rather than
  // silently losing the level.
  $effect(() => {
    if (!import.meta.env?.DEV) return
    if (items.some((item) => item?.children?.some((child) => child?.children?.length))) {
      console.warn('[context-menu] nested submenus are not supported; depth stops at one')
    }
  })

  // Viewport-aware positioning
  let adjustedX = $state(0)
  let adjustedY = $state(0)

  $effect(() => {
    // Read x/y props reactively
    const px = x
    const py = y
    // ...and re-measure whenever the menu or the viewport changed size under it.
    void layoutTick

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

    if (px + rect.width > vw - VIEWPORT_MARGIN) {
      nx = vw - rect.width - VIEWPORT_MARGIN
    }
    if (py + rect.height > vh - VIEWPORT_MARGIN) {
      ny = vh - rect.height - VIEWPORT_MARGIN
    }

    adjustedX = Math.max(VIEWPORT_MARGIN, nx)
    adjustedY = Math.max(VIEWPORT_MARGIN, ny)
  })

  /**
   * The flyout sits beside its parent row, and flips to the row's left edge
   * when the viewport cannot hold it on the right. Vertically it is clamped the
   * same way the root menu is, and capped at the viewport: more rows than the
   * screen is tall cannot be reached by moving the flyout, only by scrolling
   * inside it.
   */
  $effect(() => {
    void layoutTick
    // A root menu that re-clamped took its rows with it, and the flyout is
    // anchored to a row.
    void adjustedX
    void adjustedY
    // The rows the flyout shows can arrive after it opened, and each one
    // changes the height this clamps.
    void openChildren
    if (openIndex < 0) return

    // The cap does not depend on the anchor: a flyout too tall for the screen
    // is too tall wherever its row is.
    submenuMaxHeight = Math.max(0, window.innerHeight - VIEWPORT_MARGIN * 2)

    const anchor = parentEls[openIndex]
    if (!anchor) return

    const row = anchor.getBoundingClientRect()
    const flyout = submenuEl?.getBoundingClientRect()
    const width = flyout?.width || 0
    const height = flyout?.height || 0
    const vw = window.innerWidth
    const vh = window.innerHeight

    let left = row.right - 2
    if (width && left + width > vw - VIEWPORT_MARGIN) {
      const flipped = row.left - width + 2
      left = flipped >= VIEWPORT_MARGIN ? flipped : Math.max(VIEWPORT_MARGIN, vw - width - VIEWPORT_MARGIN)
    }
    submenuLeft = Math.max(VIEWPORT_MARGIN, left)

    let top = row.top - 4
    const shown = height ? Math.min(height, submenuMaxHeight) : 0
    if (shown && top + shown > vh - VIEWPORT_MARGIN) {
      top = vh - shown - VIEWPORT_MARGIN
    }
    submenuTop = Math.max(VIEWPORT_MARGIN, top)
  })

  // A row the keyboard moved to may be past the flyout's own bottom edge.
  $effect(() => {
    if (childFocusIndex < 0) return
    childEls[childFocusIndex]?.scrollIntoView?.({ block: 'nearest' })
  })

  function clearHoverTimer() {
    if (hoverTimer) {
      clearTimeout(hoverTimer)
      hoverTimer = null
    }
  }

  function clearLeaveTimer() {
    if (leaveTimer) {
      clearTimeout(leaveTimer)
      leaveTimer = null
    }
  }

  function openSubmenu(index) {
    clearHoverTimer()
    clearLeaveTimer()
    openIndex = index
    childFocusIndex = -1
  }

  function closeSubmenu() {
    clearHoverTimer()
    clearLeaveTimer()
    openIndex = -1
    childFocusIndex = -1
  }

  // A flyout named by the caller opens as soon as its parent row is measurable.
  $effect(() => {
    if (!openChildOf) return
    const index = actionableItems.findIndex((item) => item.label === openChildOf)
    if (index >= 0 && openIndex !== index) openSubmenu(index)
  })

  // Close on click outside
  $effect(() => {
    function handleClick(e) {
      if (submenuEl?.contains(e.target)) return
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
    return () => {
      clearHoverTimer()
      clearLeaveTimer()
    }
  })

  // A window that resizes under an open menu leaves it hanging off the edge,
  // and an open flyout beside nothing.
  $effect(() => {
    const reposition = () => { layoutTick += 1 }
    window.addEventListener('resize', reposition)
    return () => window.removeEventListener('resize', reposition)
  })

  // Rows can arrive after a menu opened: the sidebar starts account detection
  // when it opens the menu and grows the rows when the answer lands. Both
  // levels grow — the accounts are the flyout's rows — and either element is
  // only resized, so nothing else would ask for the clamp again.
  $effect(() => {
    if (typeof ResizeObserver === 'undefined') return
    const watched = [menuEl, submenuEl].filter(Boolean)
    if (watched.length === 0) return
    const observer = new ResizeObserver(() => { layoutTick += 1 })
    for (const element of watched) observer.observe(element)
    return () => observer.disconnect()
  })

  function activate(item) {
    if (item.disabled) return
    if (item.action) item.action()
    if (!item.keepOpen) onClose()
  }

  function moveFocus(list, current, step) {
    if (list.length === 0) return current
    let next = current
    for (let i = 0; i < list.length; i++) {
      next = step > 0
        ? (next + 1) % list.length
        : (next <= 0 ? list.length - 1 : next - 1)
      if (!list[next].disabled) break
    }
    return next
  }

  function runTypeahead(event, list, current, setIndex) {
    const query = `${typeaheadBuffer}${event.key.toLowerCase()}`
    const findIndex = (value) => list.findIndex((item) => (
      !item.disabled && String(item.label || '').trim().toLowerCase().startsWith(value)
    ))

    let nextIndex = findIndex(query)
    typeaheadBuffer = query

    if (nextIndex < 0) {
      nextIndex = findIndex(event.key.toLowerCase())
      typeaheadBuffer = event.key.toLowerCase()
    }

    if (typeaheadTimer) clearTimeout(typeaheadTimer)
    typeaheadTimer = setTimeout(() => {
      typeaheadBuffer = ''
      typeaheadTimer = null
    }, 350)

    if (nextIndex >= 0) {
      event.preventDefault()
      setIndex(nextIndex)
    }
    void current
  }

  $effect(() => {
    function handleKeydown(e) {
      // An open flyout owns the keyboard: it is the level the user is on.
      if (openIndex >= 0) {
        if (e.key === 'Escape' || e.key === 'ArrowLeft') {
          e.preventDefault()
          closeSubmenu()
          return
        }
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          e.preventDefault()
          childFocusIndex = moveFocus(openChildren, childFocusIndex, e.key === 'ArrowDown' ? 1 : -1)
          return
        }
        // ArrowRight opened this flyout, and depth stops here. The key repeat
        // of the press that opened it must not also pick a row: on a restart
        // parent that would stop a live session on an account nobody chose.
        if (e.key === 'ArrowRight') {
          e.preventDefault()
          return
        }
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          const index = childFocusIndex >= 0
            ? childFocusIndex
            : openChildren.findIndex((child) => !child.disabled)
          const child = openChildren[index]
          if (child) activate(child)
          return
        }
        if (e.key.length === 1 && !e.altKey && !e.ctrlKey && !e.metaKey && /\S/.test(e.key)) {
          runTypeahead(e, openChildren, childFocusIndex, (index) => { childFocusIndex = index })
        }
        return
      }

      if (e.key === 'Escape') {
        e.preventDefault()
        onClose()
        return
      }

      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault()
        focusIndex = moveFocus(actionableItems, focusIndex, e.key === 'ArrowDown' ? 1 : -1)
        return
      }

      if (e.key === 'ArrowRight') {
        const item = actionableItems[focusIndex]
        if (item?.children?.length && !item.disabled) {
          e.preventDefault()
          openSubmenu(focusIndex)
        }
        return
      }

      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault()
        const itemIndex = focusIndex >= 0
          ? focusIndex
          : actionableItems.findIndex(item => !item.disabled)
        if (itemIndex >= 0 && itemIndex < actionableItems.length) {
          const item = actionableItems[itemIndex]
          if (item.disabled) return
          if (item.children?.length && !item.action) {
            openSubmenu(itemIndex)
            return
          }
          activate(item)
        }
      }

      if (e.key.length === 1 && !e.altKey && !e.ctrlKey && !e.metaKey && /\S/.test(e.key)) {
        runTypeahead(e, actionableItems, focusIndex, (index) => { focusIndex = index })
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

  function handleItemClick(item, index) {
    if (item.disabled) return
    // A parent that carries an action runs it: the account submenu must not
    // turn "New Claude Session" into a two-step. Only a row that is nothing but
    // its children opens them on click.
    if (item.children?.length && !item.action) {
      if (openIndex === index) closeSubmenu()
      else openSubmenu(index)
      return
    }
    activate(item)
  }

  function handleParentEnter(item, index) {
    if (item.disabled) return
    focusIndex = index
    clearLeaveTimer()
    if (!item.children?.length) {
      // Hovering an ordinary row closes a flyout left open next to it.
      clearHoverTimer()
      if (openIndex >= 0) hoverTimer = setTimeout(closeSubmenu, HOVER_INTENT_MS)
      return
    }
    if (openIndex === index) return
    clearHoverTimer()
    hoverTimer = setTimeout(() => openSubmenu(index), HOVER_INTENT_MS)
  }

  function handleParentLeave() {
    clearHoverTimer()
    if (openIndex < 0) return
    clearLeaveTimer()
    // The pointer may be crossing the gap into the flyout; give it the corridor.
    leaveTimer = setTimeout(closeSubmenu, LEAVE_GRACE_MS)
  }

  function getActionableIndex(item) {
    return actionableItems.indexOf(item)
  }

  function testId(prefix, label) {
    return `${prefix}-${String(label ?? '').toLowerCase().replace(/\s+/g, '-')}`
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
      {@const hasChildren = Boolean(item.children?.length)}
      <button
        bind:this={parentEls[aIdx]}
        class="w-full flex items-center gap-2.5 px-3 py-1.5 text-left text-[13px] transition-colors
          {item.disabled ? textMuted + ' cursor-default opacity-50' : item.danger ? 'text-danger-500 ' + hoverBg : textPrimary + ' ' + hoverBg}
          {aIdx === focusIndex && !item.disabled ? focusBg : ''}"
        role="menuitem"
        disabled={item.disabled}
        aria-haspopup={hasChildren ? 'menu' : undefined}
        aria-expanded={hasChildren ? openIndex === aIdx : undefined}
        onmousedown={(e) => { e.stopPropagation(); handleItemClick(item, aIdx) }}
        onmouseenter={() => handleParentEnter(item, aIdx)}
        onmouseleave={() => { if (hasChildren) handleParentLeave() }}
        data-testid={testId('menu-item', item.label)}
      >
        {#if item.icon}
          <span class="w-4 h-4 flex items-center justify-center shrink-0">{@html item.icon}</span>
        {/if}
        <span class="flex-1">{item.label}</span>
        {#if hasChildren}
          <svg class="w-3 h-3 shrink-0 {textMuted}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" aria-hidden="true">
            <path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5"/>
          </svg>
        {/if}
      </button>
    {/if}
  {/each}
</div>

{#if openIndex >= 0 && openChildren.length}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={submenuEl}
    class="fixed z-[101] min-w-[200px] max-w-[18rem] py-1 rounded-lg border shadow-lg {menuBg} {menuBorder}"
    style="left: {submenuLeft}px; top: {submenuTop}px;{submenuMaxHeight ? ` max-height: ${submenuMaxHeight}px; overflow-y: auto;` : ''}"
    role="menu"
    tabindex="-1"
    data-testid="context-submenu"
    onmouseenter={clearLeaveTimer}
    onmouseleave={handleParentLeave}
  >
    {#each openChildren as child, childIndex (child.key ?? `${childIndex}:${child.label}`)}
    {#if child.separator}
      <div class="h-px mx-2 my-1 {separatorBg}" role="separator"></div>
    {:else}
      <button
        bind:this={childEls[childIndex]}
        class="w-full flex items-center gap-2 px-2.5 py-1.5 text-left text-[13px] transition-colors
          {child.disabled ? textMuted + ' cursor-default opacity-50' : textPrimary + ' ' + hoverBg}
          {childIndex === childFocusIndex && !child.disabled ? focusBg : ''}"
        role="menuitemradio"
        aria-checked={Boolean(child.check)}
        disabled={child.disabled}
        onmousedown={(e) => { e.stopPropagation(); activate(child) }}
        onmouseenter={() => { if (!child.disabled) childFocusIndex = childIndex }}
        data-testid={testId('submenu-item', child.label)}
      >
        <!-- The tick column is reserved on every row so the labels line up. -->
        <span
          class="w-3 shrink-0 text-[11px] leading-none {checkTone}"
          data-checked={child.check ? 'true' : 'false'}
          data-testid={testId('submenu-check', child.label)}
          aria-hidden="true"
        >{child.check ? '✓' : ''}</span>
        <span class="flex-1 truncate">{child.label}</span>
        {#if child.meta}
          <span class="shrink-0 max-w-[9rem] truncate text-[11px] tabular-nums {metaTone}">
            {child.meta}
          </span>
        {/if}
      </button>
    {/if}
    {/each}
  </div>
{/if}
