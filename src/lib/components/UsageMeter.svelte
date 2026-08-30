<script>
  import { compactSelection, resetLabel } from '../usageWindows.js'

  let {
    tool,
    usage = null,
    dark = false,
    compact = false,
  } = $props()

  const TICK_MS = 30 * 1000
  let now = $state(Date.now())
  $effect(() => {
    const timer = setInterval(() => {
      now = Date.now()
    }, TICK_MS)
    return () => clearInterval(timer)
  })

  function live(window) {
    if (!Number.isFinite(Number(window?.used_percentage))) return null
    if (window.resets_at == null) return window
    const reset = Number(window.resets_at)
    return Number.isFinite(reset) && reset * 1000 <= now ? null : window
  }

  // The two-field 0.6.8 shape remains readable during the in-memory upgrade;
  // every new response uses the ordered provider windows.
  const providerWindows = $derived(
    Array.isArray(usage?.windows)
      ? usage.windows
      : [
          usage?.five_hour
            ? { key: 'five-hour', title: '5h', ...usage.five_hour, severity: 'normal' }
            : null,
          usage?.seven_day
            ? { key: 'seven-day', title: '7d', ...usage.seven_day, severity: 'normal' }
            : null,
        ].filter(Boolean)
  )
  const legacy = $derived(!Array.isArray(usage?.windows))
  const windows = $derived(providerWindows.map(live).filter(Boolean))
  // Compact surfaces compare account headroom, so session-only noise is
  // omitted when provider windows include longer-lived limits. The flag narrows
  // the list rather than gating it: a provider that flags nothing still has
  // headroom worth showing, and an empty chip reads as "no subscription".
  const compactWindows = $derived(compactSelection(windows))
  const shown = $derived(compact ? compactWindows : windows)
  const observedAt = $derived(usage?.observed_at ? Date.parse(usage.observed_at) : Number.NaN)
  const ageMs = $derived(Number.isFinite(observedAt) ? Math.max(0, now - observedAt) : Number.NaN)
  const legacyStale = $derived(legacy && Number.isFinite(ageMs) && ageMs > 60 * 60 * 1000)
  const nextResetMs = $derived(
    windows
      .map((window) => Number(window.resets_at) * 1000 - now)
      .filter((remaining) => Number.isFinite(remaining) && remaining > 0)
      .sort((left, right) => left - right)[0] ?? null
  )

  const trackTone = $derived(dark ? 'bg-white/[0.08]' : 'bg-brand-100')
  const labelTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-600')
  const valueTone = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const mutedTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')

  function percent(window) {
    return Math.round(Number(window.used_percentage))
  }

  function compactTitle(window) {
    return String(window.title).replaceAll(' · ', ' ')
  }

  function barWidth(window) {
    return `${Math.min(100, Math.max(0, Number(window.used_percentage)))}%`
  }

  function barTone(window) {
    if (window.severity === 'critical') return dark ? 'bg-rose-400/80' : 'bg-rose-500'
    if (window.severity === 'warning') return dark ? 'bg-amber-400/80' : 'bg-amber-500'
    return dark ? 'bg-brand-400/80' : 'bg-brand-500'
  }

  function barTestId(window, index) {
    const duplicate = shown.some((candidate, otherIndex) =>
      otherIndex !== index && candidate.key === window.key
    )
    return `usage-bar-${window.key}${duplicate ? `-${index}` : ''}`
  }

  function duration(ms) {
    const minutes = Math.floor(ms / 60_000)
    if (minutes >= 1440) return `${Math.floor(minutes / 1440)}d`
    if (minutes >= 60) return `${Math.floor(minutes / 60)}h`
    return `${Math.max(1, minutes)}m`
  }

  function statusSuffix() {
    if (usage?.status === 'unauthorized') return 'sign in again'
    if (usage?.status === 'stale') {
      return Number.isFinite(ageMs) ? `seen ${duration(ageMs)} ago` : 'stale'
    }
    return null
  }
</script>

{#if shown.length && compact}
  <span class="text-[10px] leading-none tabular-nums {valueTone}" data-tool={tool} data-testid="usage-meter">
    {shown.map((window) => `${compactTitle(window)} ${percent(window)}%`).join(' · ')}
    {#if statusSuffix()}<span class="{mutedTone}"> · {statusSuffix()}</span>{/if}
  </span>
{:else if shown.length}
  <span class="flex flex-col gap-1.5" data-tool={tool} data-testid="usage-meter">
    {#each shown as window, index (`${window.key}:${index}`)}
      <span class="flex flex-col gap-0.5">
        <span class="flex items-center justify-between gap-2 text-[10px] leading-none">
          <span class="truncate {labelTone}">{window.title}</span>
          <span class="shrink-0 tabular-nums {valueTone}">{percent(window)}% used</span>
        </span>
        <span class="h-1 overflow-hidden rounded-full {trackTone}">
          <span
            class="block h-full rounded-full {barTone(window)}"
            style="width: {barWidth(window)}"
            data-testid={barTestId(window, index)}
          ></span>
        </span>
        {#if !legacy && resetLabel(window.resets_at, now)}
          <span class="text-[10px] leading-none {mutedTone}" data-testid="usage-reset">
            Resets {resetLabel(window.resets_at, now)}
          </span>
        {/if}
      </span>
    {/each}
    {#if legacyStale}
      <span class="text-[10px] leading-none {mutedTone}" data-testid="usage-stale">
        last seen {duration(ageMs)} ago
      </span>
    {:else if legacy && nextResetMs != null}
      <span class="text-[10px] leading-none {mutedTone}" data-testid="usage-reset">
        resets in {duration(nextResetMs)}
      </span>
    {:else if legacy && Number.isFinite(ageMs)}
      <span class="text-[10px] leading-none {mutedTone}" data-testid="usage-observed">
        seen {duration(ageMs)} ago
      </span>
    {:else if statusSuffix()}
      <span class="text-[10px] leading-none {mutedTone}" data-testid="usage-status">
        {statusSuffix()}
      </span>
    {:else if usage?.note}
      <span class="text-[10px] leading-none {mutedTone}">{usage.note}</span>
    {/if}
  </span>
{/if}
