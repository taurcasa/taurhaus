<script>
  /**
   * One decision, once: which Claude subscription this project runs on.
   *
   * Shown only when a host has two or more signed-in accounts and the project
   * has not chosen yet. Enter takes the default account so the common answer
   * costs a keystroke.
   */
  let {
    accounts = [],
    projectName = '',
    /** The account configured as the global default, if the user chose one. */
    defaultAccountId = null,
    dark = false,
    onConfirm = () => {},
    onCancel = () => {},
  } = $props()

  let rememberChoice = $state(true)
  let panel = $state(null)

  // Enter answers with what a project would inherit today: the configured
  // global default, else the account in the default config dir.
  const defaultAccount = $derived(
    accounts.find((account) => account.id === defaultAccountId && account.logged_in) ??
      accounts.find((account) => account.is_default && account.logged_in) ??
      accounts.find((account) => account.logged_in) ??
      null
  )

  const panelTone = $derived(
    dark
      ? 'border-white/[0.08] bg-zinc-950/95 text-zinc-100 shadow-2xl shadow-black/50'
      : 'border-brand-200/60 bg-white text-zinc-900 shadow-xl shadow-brand-900/10'
  )
  const headingTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const optionTone = $derived(
    dark
      ? 'border-white/[0.06] bg-zinc-900/60 hover:border-brand-500/60 hover:bg-zinc-900'
      : 'border-brand-100 bg-brand-50/40 hover:border-brand-400 hover:bg-brand-50'
  )
  const metaTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const badgeTone = $derived(
    dark ? 'bg-brand-500/15 text-brand-300' : 'bg-brand-100 text-brand-700'
  )
  const focusRing = $derived(
    dark
      ? 'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/40'
      : 'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/25'
  )

  function labelFor(account) {
    return String(account?.display_name ?? '').trim() || account?.email || account?.id || ''
  }

  function metaFor(account) {
    return [account?.organization, account?.seat_tier].filter(Boolean).join(' · ')
  }

  function choose(account) {
    if (!account?.logged_in) return
    onConfirm(account.id, rememberChoice)
  }

  function handleKeydown(event) {
    if (event.key === 'Escape') {
      event.preventDefault()
      onCancel()
      return
    }
    if (event.key === 'Enter' && defaultAccount) {
      event.preventDefault()
      choose(defaultAccount)
    }
  }

  $effect(() => {
    panel?.focus()
  })
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  bind:this={panel}
  class="w-[22rem] max-w-full rounded-xl border p-3 {panelTone}"
  role="dialog"
  aria-label="Choose a Claude account"
  tabindex="-1"
  onkeydown={handleKeydown}
  data-testid="claude-account-chooser"
>
  <p class="mb-2 text-[11px] font-semibold uppercase tracking-wider {headingTone}">
    Claude account{projectName ? ` · ${projectName}` : ''}
  </p>

  <div class="flex flex-col gap-1.5">
    {#each accounts as account (account.id)}
      <button
        type="button"
        class="flex w-full items-center gap-2 rounded-lg border px-2.5 py-2 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-50 {optionTone} {focusRing}"
        disabled={!account.logged_in}
        onclick={() => choose(account)}
        data-testid="claude-account-option-{account.id}"
      >
        <span class="min-w-0 flex-1">
          <span class="block truncate text-[13px] font-medium">{labelFor(account)}</span>
          <span class="block truncate text-[11px] {metaTone}">{account.email}</span>
          {#if metaFor(account)}
            <span class="block truncate text-[10px] {metaTone}">{metaFor(account)}</span>
          {/if}
        </span>
        {#if account.id === defaultAccount?.id}
          <span
            class="shrink-0 rounded-full px-1.5 py-0.5 text-[10px] font-medium {badgeTone}"
            data-testid="claude-account-default-badge">Default</span
          >
        {/if}
        {#if !account.logged_in}
          <span class="shrink-0 text-[10px] {metaTone}">Not logged in</span>
        {/if}
      </button>
    {/each}
  </div>

  <label class="mt-3 flex items-center gap-2 text-[12px] {metaTone}">
    <input
      type="checkbox"
      class="h-3.5 w-3.5 accent-brand-500 {focusRing}"
      bind:checked={rememberChoice}
      data-testid="claude-account-remember"
    />
    Remember for this project
  </label>
</div>
