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
    onTeamAccountChange = () => {},
  } = $props()

  let open = $state(false)
  const descriptor = $derived(toolDescriptor(tool))
  const isTeamAccount = $derived(Boolean(descriptor?.capabilities?.teamConfigNamespace))
  const canSelect = $derived(Boolean(descriptor?.capabilities?.accountSelection))
  // What a managed launch actually resolves (`managed_member_account`): the
  // member's own choice, else the tool's registry home — never the app-launch
  // `defaultAccountId`. The row label and the popover's entry point both read
  // this, so opening the picker and confirming without moving is a no-op.
  const selected = $derived(
    accounts.find((account) => account.id === accountId && account.logged_in)
      ?? accounts.find((account) => account.is_default && account.logged_in)
      ?? null
  )
  const label = $derived(
    String(selected?.display_name ?? selected?.label ?? selected?.email ?? selected?.id ?? 'Default')
  )
  const displayLabel = $derived(isTeamAccount ? `Team account · ${label}` : label)
  const controlTone = $derived(
    dark
      ? 'border-white/[0.08] bg-zinc-950/60 text-zinc-100 hover:bg-zinc-900'
      : 'border-brand-200/60 bg-white text-zinc-900 hover:bg-brand-50'
  )

  function choose(nextAccountId) {
    open = false
    if (isTeamAccount) {
      onTeamAccountChange(nextAccountId)
    } else {
      onchange({ accountId: nextAccountId })
    }
  }
</script>

<div class="space-y-1">
  <span class="text-[10px] text-zinc-500">Account</span>
  {#if canSelect}
    <button
      type="button"
      class="flex h-10 w-full items-center justify-between gap-2 rounded-[14px] border px-3 text-left text-sm transition {controlTone}"
      aria-expanded={open}
      onclick={() => (open = !open)}
      data-testid={`mesh-builder-member-account-${memberId}`}
    >
      <span class="truncate">{displayLabel}</span>
      <span aria-hidden="true">⌄</span>
    </button>
    {#if open}
      <div class="absolute left-3 right-3 z-40 mt-1">
        <AccountPicker
          {tool}
          {accounts}
          {defaultAccountId}
          {degraded}
          preselectedAccountId={selected?.id ?? null}
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
