<script>
  import UsageMeter from './UsageMeter.svelte'
  import { themeTokens } from '../themeTokens.js'

  let {
    tool,
    account,
    relationships = {},
    defaultAccountId = null,
    expanded = false,
    dark = false,
    usageNote = null,
    onToggle = () => {},
    onRemovePin = () => {},
    onOpenProject = () => {},
    onSetDefault = () => {},
    onSignIn = () => {},
    onReveal = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const pins = $derived(relationships?.pinnedProjects ?? relationships?.pinned_projects ?? [])
  const recent = $derived(relationships?.lastUsedProjects ?? relationships?.last_used_projects ?? [])
  const teams = $derived(relationships?.teams ?? [])
  const directory = $derived(account?.dir ?? account?.config_dir ?? '')
  const label = $derived(account?.display_name ?? account?.label ?? account?.id)
  const isDefault = $derived(defaultAccountId === account?.id)
  const relationshipCount = $derived(pins.length + teams.length)
  const rowTone = $derived(
    dark ? 'border-white/[0.07] bg-zinc-900/45' : 'border-zinc-200 bg-white'
  )
  const detailTone = $derived(dark ? 'border-zinc-800' : 'border-zinc-100')
  const chipTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-800 text-zinc-300 hover:border-zinc-600'
      : 'border-zinc-200 bg-zinc-50 text-zinc-600 hover:border-zinc-300'
  )
  const actionTone = $derived(
    dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700'
  )
  const healthTone = $derived.by(() => {
    if (!account?.logged_in || account?.usage?.status === 'unauthorized') return 'bg-rose-500'
    const worst = Math.max(
      0,
      ...(account?.usage?.windows ?? []).map((window) => Number(window.used_percentage) || 0)
    )
    if (worst >= 100) return 'bg-rose-500'
    if (worst >= 80) return 'bg-amber-500'
    return 'bg-emerald-500'
  })
</script>

<article
  class="overflow-hidden rounded-xl border shadow-sm {rowTone}"
  data-testid="account-row-{account.id}"
>
  <button
    class="grid w-full grid-cols-[minmax(150px,0.9fr)_minmax(190px,1.4fr)_28px] items-center gap-4 px-4 py-3 text-left transition-colors {t.hoverRow}"
    onclick={onToggle}
    aria-expanded={expanded}
    aria-label={`${expanded ? 'Collapse' : 'Expand'} ${label}`}
  >
    <span class="min-w-0">
      <span class="flex items-center gap-2">
        <span class="h-2 w-2 shrink-0 rounded-full {healthTone}"></span>
        <span class="truncate text-[12px] font-semibold {t.textPrimary}">{label}</span>
        {#if isDefault}
          <span class="rounded-full bg-brand-500/10 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-brand-500">Default</span>
        {/if}
      </span>
      <span class="mt-1 block truncate pl-4 text-[10px] {t.textTertiary}">{directory}</span>
      {#if relationshipCount}
        <span class="mt-1 block pl-4 text-[10px] {t.textTertiary}">
          {pins.length} pinned · {teams.length} team{teams.length === 1 ? '' : 's'}
        </span>
      {/if}
    </span>

    <span class="min-w-0">
      {#if account?.usage}
        <UsageMeter {tool} usage={account.usage} {dark} />
      {:else}
        <span class="text-[10px] {t.textTertiary}">
          {account?.logged_in ? (usageNote ?? 'Usage unavailable') : 'Signed out · sign in to refresh'}
        </span>
      {/if}
    </span>

    <span class="text-center text-[12px] {t.textTertiary}" aria-hidden="true">{expanded ? '⌃' : '⌄'}</span>
  </button>

  {#if expanded}
    <div class="border-t px-4 py-3 {detailTone}" data-testid="account-row-details">
      {#if pins.length || teams.length || recent.length}
        <div class="grid gap-3 sm:grid-cols-3">
          {#if pins.length}
            <section>
              <h4 class="mb-1.5 text-[9px] font-semibold uppercase tracking-wider {t.textTertiary}">Pinned projects</h4>
              <div class="flex flex-wrap gap-1.5">
                {#each pins as project (project.id)}
                  <span class="inline-flex items-center rounded-md border {chipTone}">
                    <button class="px-2 py-1 text-[10px]" onclick={() => onOpenProject(project)}>{project.name}</button>
                    <button
                      class="border-l border-current/15 px-1.5 py-1 text-[10px]"
                      aria-label={`Remove ${project.name} pin`}
                      onclick={() => onRemovePin(project)}
                    >×</button>
                  </span>
                {/each}
              </div>
            </section>
          {/if}
          {#if teams.length}
            <section>
              <h4 class="mb-1.5 text-[9px] font-semibold uppercase tracking-wider {t.textTertiary}">Teams</h4>
              <div class="space-y-1">
                {#each teams as team (team.name)}
                  <button class="block text-[10px] {actionTone}" onclick={() => onOpenProject(team)}>{team.name}</button>
                {/each}
              </div>
            </section>
          {/if}
          {#if recent.length}
            <section>
              <h4 class="mb-1.5 text-[9px] font-semibold uppercase tracking-wider {t.textTertiary}">Last used</h4>
              <div class="space-y-1">
                {#each recent as project (project.id)}
                  <button class="block text-[10px] {actionTone}" onclick={() => onOpenProject(project)}>{project.name}</button>
                {/each}
              </div>
            </section>
          {/if}
        </div>
      {/if}

      <div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 border-t pt-3 {detailTone}">
        {#if !isDefault}
          <button class="text-[10px] font-medium {actionTone}" onclick={onSetDefault}>Set as global default</button>
        {/if}
        <button class="text-[10px] font-medium {actionTone}" onclick={onSignIn}>Sign in…</button>
        <button class="text-[10px] font-medium {actionTone}" onclick={onReveal}>Reveal directory</button>
      </div>
    </div>
  {/if}
</article>
