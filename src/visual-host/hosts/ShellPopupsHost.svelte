<script>
  /**
   * The account popups inside the markup they actually live in.
   *
   * Everything structural here is copied from the app, not approximated: the
   * `.shell-frame` column and its floating panels from `Shell.svelte`, the
   * `px-7 pt-5 pb-4 shrink-0` header and `flex-1 overflow-y-auto` body from
   * `OverviewTab.svelte`. A popup that positions itself against an ancestor is
   * only worth shooting where those ancestors exist.
   */
  import AccountChip from '../../lib/components/AccountChip.svelte'
  import AccountChooser from '../../lib/components/AccountChooser.svelte'
  import ContextMenu from '../../lib/ContextMenu.svelte'
  import { buildAccountMenuChildren } from '../../lib/accountMenu.js'

  let { scenario, theme = 'light' } = $props()

  const dark = $derived(theme === 'dark')
  const accounts = $derived(scenario?.accounts ?? [])
  const surface = $derived(scenario?.surface ?? 'chooser')

  const sidebarItems = $derived([
    { label: 'Copy Path' },
    { separator: true },
    { label: 'Open in Terminal' },
    { separator: true },
    {
      label: 'New Claude Session',
      children: buildAccountMenuChildren({
        accounts,
        activeAccountId: 'account-1',
        onSelect: () => {},
      }),
    },
    {
      label: 'Resume Claude',
      children: buildAccountMenuChildren({
        accounts,
        activeAccountId: 'account-1',
        onSelect: () => {},
      }),
    },
  ])
</script>

<div class="shell-frame h-full flex flex-col font-sans antialiased">
  <!-- Titlebar: 46 px, per the layout table in CLAUDE.md. -->
  <div class="h-[46px] shrink-0 flex items-center gap-3 px-3 text-[12px] text-white/40">
    <span class="font-semibold tracking-[-0.02em] text-white/70">taurhaus</span>
    <span class="h-[36px] rounded-t-lg bg-white/[0.06] px-4 leading-[36px] text-white/70">Overview</span>
  </div>

  <div class="flex-1 flex gap-1.5 p-1.5 pt-0 min-h-0">
    <aside class="w-[252px] bg-brand-950 rounded-lg flex flex-col shrink-0 border border-white/[0.06] overflow-hidden">
      <div class="px-3 pt-3 pb-1">
        <div class="flex items-center gap-2 px-3 h-[32px] rounded-md bg-white/[0.05] border border-white/[0.07] text-[13px] text-white/25">
          Filter...
        </div>
      </div>
      <div class="flex-1 overflow-y-auto px-1.5 pt-1">
        {#each ['taurhaus', 'mesh', 'taureval'] as name}
          <div class="flex h-[36px] items-center rounded-md px-2.5 text-[13px] text-white/60">{name}</div>
        {/each}
      </div>
    </aside>

    <main
      class="shell-main-surface shell-main-panel flex-1 rounded-b-lg rounded-tr-lg flex flex-col min-w-0 overflow-hidden {dark ? 'text-zinc-200' : 'text-zinc-800'}"
    >
      <div class="flex-1 flex flex-col min-w-0 overflow-hidden">
        <!-- Overview header -->
        <div class="px-7 pt-5 pb-4 shrink-0">
          <div class="flex items-center gap-3">
            <h1 class="text-[18px] font-semibold tracking-[-0.02em]">taurhaus</h1>
            <span class="text-[11px] font-mono opacity-50 self-baseline">main</span>
            <AccountChip
              tool={scenario?.tool ?? 'claude'}
              {accounts}
              selectedAccountId={scenario?.selectedAccountId ?? null}
              defaultAccountId={scenario?.defaultAccountId ?? null}
              {dark}
              open={surface === 'chip'}
              onSelect={() => {}}
              onRequestUsage={() => {}}
            />
            <div class="ml-auto flex items-center gap-1 shrink-0">
              {#each ['C', 'X', 'G', 'T'] as glyph}
                <span class="w-7 h-7 flex items-center justify-center rounded-md text-[11px] opacity-40">{glyph}</span>
              {/each}
            </div>
          </div>
        </div>

        <!-- Scrollable body -->
        <div class="flex-1 overflow-y-auto">
          <div class="max-w-3xl px-7 pb-8">
            {#each Array(14) as _, index}
              <p class="py-2 text-[13px] opacity-40">
                Overview body line {index + 1} — the header above does not scroll with it.
              </p>
            {/each}
          </div>
        </div>
      </div>
    </main>
  </div>

  {#if surface === 'chooser'}
    <!-- Mounted exactly as `Shell.svelte` mounts it: a direct child of the
         frame, with the chooser's own overlay and nothing wrapped around it.
         A wrapper here would hide the very bug this fixture exists to catch —
         `.shell-frame > *` is `position: relative` unless the child opts out. -->
    <AccountChooser
      tool={scenario?.tool ?? 'claude'}
      {accounts}
      projectName={scenario?.projectName ?? ''}
      defaultAccountId={scenario?.defaultAccountId ?? null}
      {dark}
      onConfirm={() => {}}
      onCancel={() => {}}
    />
  {/if}
</div>

{#if surface === 'sidebar'}
  <ContextMenu items={sidebarItems} x={120} y={180} dark={true} onClose={() => {}} openChildOf="New Claude Session" />
{/if}
