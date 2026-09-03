<script>
  /**
   * Which subscription this project runs on, and the shared picker to change
   * it. Hidden entirely when the host has a single account — the common case.
   */
  import AccountPicker from './AccountPicker.svelte'
  import UsageMeter from './UsageMeter.svelte'
  import { accountOriginHint } from '../accountPresentation.js'
  import { toolDescriptor } from '../toolRegistry.js'

  let {
    tool,
    accounts = [],
    /** Bindable so a fixture or a test can mount the menu already open. */
    open = $bindable(false),
    selectedAccountId = null,
    /** The account configured as the global default, if the user chose one. */
    defaultAccountId = null,
    /** Detection could not run: these are the accounts last known, not current. */
    degraded = false,
    origin = null,
    projectName = '',
    dark = false,
    onSelect = () => {},
    /**
     * Ask for current usage. The menu is where two subscriptions get compared,
     * and the numbers behind them move while the project stays mounted.
     */
    onRequestUsage = () => {},
    /** The picker footer's two actions, the whole hub affordance offered here. */
    onAddAccount = () => {},
    onManageAccounts = () => {},
  } = $props()

  const toolLabel = $derived(toolDescriptor(tool)?.label ?? tool)
  /** The popup the chip owns, named so the button can point at it. */
  const popoverId = $derived(`account-popover-${tool}`)

  /** How often an open menu asks again. Percentages move in tens of seconds. */
  const USAGE_POLL_MS = 30 * 1000

  /** Breathing room between the menu and the window edge, as `ContextMenu`. */
  const VIEWPORT_MARGIN = 8
  /** The `w-[22rem]` popover skin, for the first paint before it is measured. */
  const ASSUMED_WIDTH = 352

  let chipEl = $state(null)
  let menuEl = $state(null)
  let menuLeft = $state(0)
  let menuTop = $state(0)
  /** Bumped by anything that can have moved the chip under an open menu. */
  let anchorMoved = $state(0)

  /**
   * The menu belongs to the viewport, not to the header it hangs off.
   *
   * Positioned `absolute`, it was laid out against whichever ancestor happened
   * to be positioned and clipped by the Overview panel's `overflow-hidden`. So
   * it is measured and clamped the same way `ContextMenu` is: below the chip
   * when there is room, above it when there is not, never past an edge.
   */
  $effect(() => {
    void anchorMoved
    if (!open || !chipEl) return

    const anchor = chipEl.getBoundingClientRect()
    const menu = menuEl?.getBoundingClientRect()
    const width = menu?.width || ASSUMED_WIDTH
    const height = menu?.height ?? 0
    const vw = window.innerWidth
    const vh = window.innerHeight

    // Right-aligned to the chip, the way the chip's own menu has always read.
    let left = anchor.right - width
    if (left + width > vw - VIEWPORT_MARGIN) left = vw - width - VIEWPORT_MARGIN
    menuLeft = Math.max(VIEWPORT_MARGIN, left)

    let top = anchor.bottom + 4
    if (height && top + height > vh - VIEWPORT_MARGIN) {
      // Flip above the chip when the space below cannot hold the menu.
      const above = anchor.top - height - 4
      top = above >= VIEWPORT_MARGIN ? above : Math.max(VIEWPORT_MARGIN, vh - height - VIEWPORT_MARGIN)
    }
    menuTop = Math.max(VIEWPORT_MARGIN, top)
  })

  // A popup anchored to the viewport has to follow the thing it is anchored
  // to: the Overview body scrolls under the header, and the window resizes.
  $effect(() => {
    if (!open) return
    const reposition = () => { anchorMoved += 1 }
    window.addEventListener('resize', reposition)
    window.addEventListener('scroll', reposition, true)
    return () => {
      window.removeEventListener('resize', reposition)
      window.removeEventListener('scroll', reposition, true)
    }
  })

  // Both ends of the anchoring can change size after the menu opened: opening
  // asks for fresh usage, and the meters that answer make the menu taller and
  // the chip wider. Nothing else would ask for the clamp again, so the popup
  // would keep the coordinates its empty size earned.
  $effect(() => {
    if (!open || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(() => { anchorMoved += 1 })
    if (chipEl) observer.observe(chipEl)
    if (menuEl) observer.observe(menuEl)
    return () => observer.disconnect()
  })

  // Clicking anywhere else closes it — the menu is no longer inside the chip's
  // own subtree, so a stray click can no longer be caught by hover alone.
  $effect(() => {
    if (!open) return
    function handlePointerDown(event) {
      if (chipEl?.contains(event.target) || menuEl?.contains(event.target)) return
      open = false
    }
    function handleKeydown(event) {
      if (event.key !== 'Escape') return
      open = false
      chipEl?.focus()
    }
    window.addEventListener('mousedown', handlePointerDown)
    window.addEventListener('keydown', handleKeydown)
    return () => {
      window.removeEventListener('mousedown', handlePointerDown)
      window.removeEventListener('keydown', handleKeydown)
    }
  })

  // Only `open` is read synchronously, so the poll restarts when the menu
  // opens and closes — not every time the parent hands down a new callback.
  $effect(() => {
    if (!open) return
    const timer = setInterval(() => onRequestUsage(), USAGE_POLL_MS)
    return () => clearInterval(timer)
  })

  function toggle() {
    open = !open
    // The first ask belongs to the click, not to the effect: an effect that
    // re-runs would otherwise spend an IPC every time.
    if (open) onRequestUsage()
  }

  const visible = $derived(accounts.length >= 2)
  // What a project inherits: the configured global default while it can run,
  // otherwise the account in the default config dir.
  const inheritedAccount = $derived(
    accounts.find((account) => account.id === defaultAccountId && account.logged_in) ??
      accounts.find((account) => account.is_default) ??
      null
  )
  const selected = $derived(
    accounts.find((account) => account.id === selectedAccountId) ?? inheritedAccount
  )
  const inherited = $derived(!selectedAccountId)

  const chipTone = $derived(
    dark
      ? 'border-white/[0.08] bg-zinc-900/70 text-zinc-300 hover:border-brand-500/50 hover:text-zinc-100'
      : 'border-brand-200/70 bg-white text-zinc-600 hover:border-brand-400 hover:text-zinc-900'
  )
  const dividerTone = $derived(dark ? 'border-white/[0.08]' : 'border-brand-200/70')
  const metaTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const focusRing = $derived(
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/30'
  )

  function labelFor(account) {
    return String(account?.display_name ?? '').trim() || account?.label || account?.id || ''
  }

  const originHint = $derived(accountOriginHint(origin))

  const staleNote = 'Accounts unavailable (daemon offline) — using last known'
  const title = $derived(
    degraded
      ? staleNote
      : selected
        ? `${selected.label ?? selected.email ?? labelFor(selected)}${originHint ? ` (${originHint})` : ''}`
        : `No ${toolLabel} account detected`
  )

  function pick(accountId) {
    open = false
    onSelect(accountId)
  }
</script>

{#if visible}
  <div class="inline-block">
    <button
      bind:this={chipEl}
      type="button"
      class="flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] transition-colors {chipTone} {focusRing}"
      {title}
      aria-haspopup="dialog"
      aria-controls={open ? popoverId : undefined}
      aria-expanded={open}
      onclick={toggle}
      data-testid="account-chip"
    >
      <span class="max-w-[10rem] truncate">{labelFor(selected)}</span>
      {#if originHint || inherited}
        <span class="{metaTone} text-[10px]">{originHint || 'default'}</span>
      {/if}
      {#if selected?.usage}
        <!-- `empty:hidden`: the meter renders nothing once every window it had
             has reset, and a divider with nothing after it reads as a bug. -->
        <span class="border-l pl-1.5 empty:hidden {dividerTone}">
          <UsageMeter {tool} usage={selected.usage} {dark} compact />
        </span>
      {/if}
    </button>

    {#if open}
      <!-- The popover skin of the shared picker, clamped to the viewport the
           way `ContextMenu` is. Management — unpinning, defaults, sign-in —
           belongs to the accounts home the footer points at. -->
      <div
        bind:this={menuEl}
        id={popoverId}
        class="fixed z-[100] max-h-[calc(100vh-2rem)] overflow-y-auto"
        style="left: {menuLeft}px; top: {menuTop}px;"
        data-testid="account-menu"
      >
        <AccountPicker
          {tool}
          {accounts}
          {projectName}
          {defaultAccountId}
          {degraded}
          {dark}
          preselectedAccountId={selected?.id ?? null}
          skin="popover"
          showRemember={false}
          onConfirm={(accountId) => pick(accountId)}
          onCancel={() => { open = false }}
          onAddAccount={(toolId) => { open = false; onAddAccount(toolId) }}
          onManageAccounts={(toolId) => { open = false; onManageAccounts(toolId) }}
        />
      </div>
    {/if}
  </div>
{/if}
