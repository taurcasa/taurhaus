<script>
  import UsageMeter from './UsageMeter.svelte'
  import { toolDescriptor } from '../toolRegistry.js'
  import { resetLabel } from '../usageWindows.js'

  let {
    tool,
    accounts = [],
    projectName = '',
    defaultAccountId = null,
    degraded = false,
    reason = null,
    preselectedAccountId = null,
    dark = false,
    skin = 'modal',
    testId = 'account-picker',
    onConfirm = () => {},
    onCancel = () => {},
    onAddAccount = () => {},
    onManageAccounts = () => {},
  } = $props()

  const toolLabel = $derived(toolDescriptor(tool)?.label ?? tool)
  let rememberChoice = $state(true)
  let panel = $state(null)

  const defaultAccount = $derived(
    accounts.find((account) => account.id === defaultAccountId && account.logged_in) ??
      accounts.find((account) => account.is_default && account.logged_in) ??
      accounts.find((account) => account.logged_in) ??
      null
  )
  const enterAccount = $derived(
    accounts.find((account) => account.id === preselectedAccountId && account.logged_in) ??
      defaultAccount
  )
  const reasonSentence = $derived.by(() => {
    if (!reason) return null
    const who = reason.accountLabel || 'This account'
    if (reason.kind === 'unauthorized') {
      return `${who} needs to sign in again. Pick a subscription for this launch.`
    }
    const resets = resetLabel(reason.resetsAt)
    const window = [reason.windowTitle, resets ? `resets ${resets}` : null]
      .filter(Boolean)
      .join(', ')
    return `${who} is out of usage${window ? ` — ${window}` : ''}. Pick a subscription for this launch.`
  })

  const panelTone = $derived(
    dark
      ? 'border-white/[0.08] bg-zinc-950/95 text-zinc-100 shadow-2xl shadow-black/50'
      : 'border-brand-200/60 bg-white text-zinc-900 shadow-xl shadow-brand-900/10'
  )
  const skinTone = $derived.by(() => {
    if (skin === 'select') return 'w-full max-w-[26rem] rounded-lg'
    if (skin === 'popover') return 'w-[22rem] max-w-full rounded-lg'
    return 'w-[22rem] max-w-full max-h-[calc(100vh-4rem)] overflow-y-auto rounded-xl'
  })
  const headingTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const optionTone = $derived(
    dark
      ? 'border-white/[0.06] bg-zinc-900/60 hover:border-brand-500/60 hover:bg-zinc-900'
      : 'border-brand-100 bg-brand-50/40 hover:border-brand-400 hover:bg-brand-50'
  )
  const metaTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const reasonTone = $derived(dark ? 'text-amber-300' : 'text-amber-700')
  const preselectedTone = $derived(
    dark ? 'ring-2 ring-brand-400/50' : 'ring-2 ring-brand-500/40'
  )
  const badgeTone = $derived(
    dark ? 'bg-brand-500/15 text-brand-300' : 'bg-brand-100 text-brand-700'
  )
  const footerBorder = $derived(dark ? 'border-white/[0.07]' : 'border-brand-100')
  const footerAction = $derived(
    dark ? 'text-brand-300 hover:text-brand-200' : 'text-brand-700 hover:text-brand-600'
  )
  const focusRing = $derived(
    dark
      ? 'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/40'
      : 'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/25'
  )

  function labelFor(account) {
    return String(account?.display_name ?? '').trim() || account?.label || account?.id || ''
  }

  function metaFor(account) {
    return [account?.organization, account?.plan].filter(Boolean).join(' · ')
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
    if (event.key === 'Enter' && enterAccount) {
      event.preventDefault()
      choose(enterAccount)
    }
  }

  $effect(() => {
    if (skin === 'modal') panel?.focus()
  })
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  bind:this={panel}
  class="border p-3 {panelTone} {skinTone}"
  role={skin === 'modal' ? 'dialog' : 'group'}
  aria-label={skin === 'modal' ? `Choose a ${toolLabel} account` : `${toolLabel} accounts`}
  tabindex={skin === 'modal' ? '-1' : undefined}
  onkeydown={handleKeydown}
  data-testid={testId}
  data-skin={skin}
>
  <p class="mb-2 text-[11px] font-semibold uppercase tracking-wider {headingTone}">
    {toolLabel} account{projectName ? ` · ${projectName}` : ''}
  </p>

  {#if reasonSentence}
    <p class="mb-2 text-[11px] leading-snug {reasonTone}" data-testid="account-chooser-reason">
      {reasonSentence}
    </p>
  {/if}

  {#if degraded}
    <p class="mb-2 text-[11px] {metaTone}" data-testid="accounts-degraded">
      Accounts unavailable (daemon offline) — using last known
    </p>
  {/if}

  <div class="flex flex-col gap-1.5">
    {#each accounts as account (account.id)}
      <button
        type="button"
        class="flex w-full items-center gap-2 rounded-lg border px-2.5 py-2 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-50 {optionTone} {focusRing} {account.id === enterAccount?.id ? preselectedTone : ''}"
        disabled={!account.logged_in}
        onclick={() => choose(account)}
        data-preselected={account.id === enterAccount?.id ? 'true' : 'false'}
        data-testid="account-option-{account.id}"
      >
        <span class="min-w-0 flex-1">
          <span class="block truncate text-[13px] font-medium">{labelFor(account)}</span>
          <span class="block truncate text-[11px] {metaTone}">{account.label}</span>
          {#if metaFor(account)}
            <span class="block truncate text-[10px] {metaTone}">{metaFor(account)}</span>
          {/if}
          {#if account.usage}
            <span class="mt-1.5 block max-w-[12rem]">
              <UsageMeter {tool} usage={account.usage} {dark} />
            </span>
          {/if}
        </span>
        {#if account.id === defaultAccount?.id}
          <span class="shrink-0 rounded-full px-1.5 py-0.5 text-[10px] font-medium {badgeTone}" data-testid="account-default-badge">Default</span>
        {/if}
        {#if !account.logged_in}
          <span class="shrink-0 text-[10px] {metaTone}">Not logged in</span>
        {/if}
      </button>
    {/each}
  </div>

  <label class="mt-3 flex items-center gap-2 text-[12px] {metaTone}">
    <input type="checkbox" class="h-3.5 w-3.5 accent-brand-500 {focusRing}" bind:checked={rememberChoice} data-testid="account-remember" />
    Use for this project
  </label>
  <p class="mt-1 pl-5.5 text-[10px] {metaTone}">
    {rememberChoice ? 'Otherwise, this launch only.' : 'This launch only.'}
  </p>

  <footer class="mt-3 flex items-center justify-between border-t pt-2 {footerBorder}" data-testid="account-picker-footer">
    <button class="text-[11px] font-medium {footerAction} {focusRing}" onclick={() => onAddAccount(tool)}>Add account…</button>
    <button class="text-[11px] font-medium {footerAction} {focusRing}" onclick={() => onManageAccounts(tool)}>Manage accounts →</button>
  </footer>
</section>
