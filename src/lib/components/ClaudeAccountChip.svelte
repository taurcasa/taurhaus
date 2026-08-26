<script>
  /**
   * Which Claude subscription this project runs on, and a menu to change it.
   * Hidden entirely when the host has a single account — the common case.
   */
  let {
    accounts = [],
    selectedAccountId = null,
    /** The account configured as the global default, if the user chose one. */
    defaultAccountId = null,
    dark = false,
    onSelect = () => {},
  } = $props()

  let open = $state(false)

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
  const menuTone = $derived(
    dark
      ? 'border-white/[0.08] bg-zinc-950/95 text-zinc-200 shadow-2xl shadow-black/50'
      : 'border-brand-200/60 bg-white text-zinc-800 shadow-xl shadow-brand-900/10'
  )
  const itemTone = $derived(dark ? 'hover:bg-zinc-900' : 'hover:bg-brand-50')
  const metaTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const focusRing = $derived(
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/30'
  )

  function labelFor(account) {
    return String(account?.display_name ?? '').trim() || account?.email || ''
  }

  const title = $derived(
    selected
      ? `${selected.email}${inherited ? ' (default account)' : ''}`
      : 'No Claude account detected'
  )

  function pick(accountId) {
    open = false
    onSelect(accountId)
  }
</script>

{#if visible}
  <div class="relative inline-block">
    <button
      type="button"
      class="flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] transition-colors {chipTone} {focusRing}"
      {title}
      aria-haspopup="menu"
      aria-expanded={open}
      onclick={() => (open = !open)}
      data-testid="claude-account-chip"
    >
      <span class="max-w-[10rem] truncate">{labelFor(selected)}</span>
      {#if inherited}
        <span class="{metaTone} text-[10px]">default</span>
      {/if}
    </button>

    {#if open}
      <div
        class="absolute right-0 z-30 mt-1 w-56 rounded-lg border p-1 {menuTone}"
        role="menu"
        data-testid="claude-account-menu"
      >
        {#each accounts as account (account.id)}
          <button
            type="button"
            role="menuitem"
            class="flex w-full flex-col items-start rounded-md px-2 py-1.5 text-left disabled:cursor-not-allowed disabled:opacity-50 {itemTone} {focusRing}"
            disabled={!account.logged_in}
            onclick={() => pick(account.id)}
            data-testid="claude-account-menu-item-{account.id}"
          >
            <span class="text-[12px]">{labelFor(account)}</span>
            <span class="text-[10px] {metaTone}">
              {account.email}{account.logged_in ? '' : ' · not logged in'}
            </span>
          </button>
        {/each}
        {#if selectedAccountId}
          <button
            type="button"
            role="menuitem"
            class="mt-1 w-full rounded-md px-2 py-1.5 text-left text-[11px] {metaTone} {itemTone} {focusRing}"
            onclick={() => pick(null)}
            data-testid="claude-account-menu-clear"
          >
            Use the default account
          </button>
        {/if}
      </div>
    {/if}
  </div>
{/if}
