<script>
  import AccountsHome from '../../lib/components/AccountsHome.svelte'
  import AccountUsageBoard from '../../lib/components/AccountUsageBoard.svelte'
  import AccountPicker from '../../lib/components/AccountPicker.svelte'

  let { scenario, theme = null, dark: darkProp = null } = $props()

  const dark = $derived(darkProp ?? theme === 'dark')
  const stageTone = $derived(dark ? 'bg-brand-950' : 'bg-brand-50')
  const panelTone = $derived(dark ? 'bg-zinc-950' : 'bg-white')
  const projects = [
    { id: 'p1', name: 'taurhaus', path: '/projects/taurhaus', accountMemory: {} },
    { id: 'p2', name: 'mir', path: '/projects/mir', accountMemory: {} },
  ]
</script>

<div class="h-screen w-screen p-3 {stageTone}">
  {#if scenario.surface === 'home'}
    <div class="h-full overflow-hidden rounded-xl {panelTone}">
      <AccountsHome
        {dark}
        states={scenario.states}
        {projects}
        selectedProject={projects[0]}
      />
    </div>
  {:else if scenario.surface === 'board'}
    <AccountUsageBoard
      states={scenario.states}
      x={56}
      y={56}
      {dark}
    />
  {:else}
    <div class="flex h-full items-start justify-center pt-20">
      <AccountPicker
        tool="claude"
        accounts={scenario.accounts}
        projectName="taurhaus"
        defaultAccountId="personal"
        skin={scenario.skin}
        {dark}
      />
    </div>
  {/if}
</div>
