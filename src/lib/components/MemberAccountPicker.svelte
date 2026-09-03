<script>
  import AccountPicker from './AccountPicker.svelte'
  import { toolDescriptor } from '../toolRegistry.js'

  let {
    memberId,
    tool,
    accountId = null,
    accounts = [],
    defaultAccountId = null,
    degraded = false,
    dark = false,
    onchange = () => {},
  } = $props()

  let open = $state(false)
  const descriptor = $derived(toolDescriptor(tool))
  const isTeamAccount = $derived(Boolean(descriptor?.capabilities?.teamConfigNamespace))
  const canSelect = $derived(
    Boolean(descriptor?.capabilities?.accountSelection) && !isTeamAccount
  )
  const selected = $derived(
    isTeamAccount
      ? accounts.find((account) => account.is_default && account.logged_in) ?? null
      : accounts.find((account) => account.id === accountId && account.logged_in)
        ?? accounts.find((account) => account.id === defaultAccountId && account.logged_in)
        ?? accounts.find((account) => account.is_default && account.logged_in)
        ?? accounts.find((account) => account.logged_in)
        ?? null
  )
  const label = $derived(
    String(selected?.display_name ?? selected?.label ?? selected?.email ?? selected?.id ?? 'Default')
  )
  const controlTone = $derived(
    dark
      ? 'border-white/[0.08] bg-zinc-950/60 text-zinc-100 hover:bg-zinc-900'
      : 'border-brand-200/60 bg-white text-zinc-900 hover:bg-brand-50'
  )
  const truthTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.04] text-zinc-300'
      : 'border-brand-200/60 bg-brand-50 text-brand-800'
  )

  function choose(nextAccountId) {
    open = false
    onchange({ accountId: nextAccountId })
  }
</script>

<div class="relative space-y-1">
  <span class="text-[10px] text-zinc-500">Account</span>
  {#if isTeamAccount}
    <div
      class="flex h-10 items-center rounded-[14px] border px-3 text-xs font-medium {truthTone}"
      data-testid={`mesh-builder-member-account-${memberId}`}
      title="Claude account selection applies to the whole team"
    >
      <span class="truncate">Team account · {label}</span>
    </div>
  {:else if canSelect}
    <button
      type="button"
      class="flex h-10 w-full items-center justify-between gap-2 rounded-[14px] border px-3 text-left text-sm transition {controlTone}"
      aria-expanded={open}
      onclick={() => (open = !open)}
      data-testid={`mesh-builder-member-account-${memberId}`}
    >
      <span class="truncate">{label}</span>
      <span aria-hidden="true">⌄</span>
    </button>
    {#if open}
      <div class="absolute right-0 z-40 mt-1 min-w-[22rem]">
        <AccountPicker
          {tool}
          {accounts}
          {defaultAccountId}
          {degraded}
          preselectedAccountId={accountId ?? defaultAccountId}
          {dark}
          skin="popover"
          showRemember={false}
          testId={`mesh-builder-account-picker-${memberId}`}
          onConfirm={choose}
          onCancel={() => (open = false)}
        />
      </div>
    {/if}
  {/if}
</div>
