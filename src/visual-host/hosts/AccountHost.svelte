<script>
  import AccountChip from '../../lib/components/AccountChip.svelte'
  import AccountChooser from '../../lib/components/AccountChooser.svelte'

  let { scenario, theme = 'light' } = $props()

  const dark = $derived(theme === 'dark')
  const accounts = $derived(scenario?.accounts ?? [])
  const labelTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const showChooser = $derived(accounts.filter((account) => account.logged_in).length >= 2)
</script>

<div class="flex w-full max-w-3xl flex-col gap-6 p-6">
  <div class="space-y-1.5">
    <span class="block text-[10px] font-medium uppercase tracking-wide {labelTone}">
      Project header chip
    </span>
    <AccountChip
      tool="claude"
      {accounts}
      selectedAccountId={scenario?.selectedAccountId ?? null}
      defaultAccountId={scenario?.defaultAccountId ?? null}
      {dark}
      origin={scenario?.origin ?? 'last_used'}
      onSelect={() => {}}
    />
  </div>

  {#if showChooser}
    <div class="space-y-1.5">
      <span class="block text-[10px] font-medium uppercase tracking-wide {labelTone}">
        Launch chooser
      </span>
      <AccountChooser
        tool="claude"
        {accounts}
        projectName={scenario?.projectName ?? ''}
        defaultAccountId={scenario?.defaultAccountId ?? null}
        {dark}
        onConfirm={() => {}}
        onCancel={() => {}}
      />
    </div>
  {/if}
</div>
