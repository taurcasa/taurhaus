<script>
  /**
   * What a Claude subscription has left, as its status line last reported it.
   *
   * The numbers come from Claude Code's own status-line payload, which only
   * flows while a session of that account is running. Four states follow from
   * that and all four are visible here: fresh numbers with the window that
   * resets next, numbers older than an hour labelled with when they were seen,
   * a window whose own reset has passed — dropped, because it describes a
   * window that no longer exists — and, for an account nothing has reported
   * for, nothing at all. An empty meter is the honest answer; a 0 % bar would
   * be a lie that sends the user to the subscription with the least headroom.
   */
  let {
    usage = null,
    dark = false,
    /** Percentages only, for the chip button where a bar has no room. */
    compact = false,
  } = $props()

  /** Past this, the numbers are labelled by their age instead of their reset. */
  const STALE_MS = 60 * 60 * 1000
  /** How often the clock below moves. Percentages age in minutes, not frames. */
  const TICK_MS = 30 * 1000

  /**
   * The clock every time-dependent value here reads.
   *
   * Reset times and observation ages pass while a chip sits open, and the props
   * do not change when they do. Reading `Date.now()` straight from a `$derived`
   * leaves a mounted meter showing a window that reset ten minutes ago.
   */
  let now = $state(Date.now())
  $effect(() => {
    const timer = setInterval(() => {
      now = Date.now()
    }, TICK_MS)
    return () => clearInterval(timer)
  })

  /**
   * A window taurhaus can still speak for.
   *
   * Once `resets_at` has passed, the percentage beside it describes a window
   * that no longer exists — and the account it belongs to is exactly the one
   * that just got its headroom back. Nothing is the honest answer until its
   * status line reports again.
   */
  function live(window) {
    if (!Number.isFinite(Number(window?.used_percentage))) return null
    const resetsAt = Number(window.resets_at)
    if (Number.isFinite(resetsAt) && resetsAt * 1000 <= now) return null
    return window
  }

  const windows = $derived(
    [
      { key: 'five-hour', label: '5h', window: live(usage?.five_hour) },
      { key: 'seven-day', label: '7d', window: live(usage?.seven_day) },
    ].filter((entry) => entry.window),
  )

  const observedAt = $derived(usage?.observed_at ? Date.parse(usage.observed_at) : Number.NaN)
  const ageMs = $derived(Number.isFinite(observedAt) ? now - observedAt : Number.NaN)
  const stale = $derived(Number.isFinite(ageMs) && ageMs > STALE_MS)

  const nextResetMs = $derived(
    windows
      .map((entry) => Number(entry.window?.resets_at))
      .filter((seconds) => Number.isFinite(seconds))
      .map((seconds) => seconds * 1000 - now)
      .filter((remaining) => remaining > 0)
      .sort((left, right) => left - right)[0] ?? null,
  )

  const trackTone = $derived(dark ? 'bg-white/[0.08]' : 'bg-brand-100')
  const labelTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const valueTone = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const staleTone = $derived(dark ? 'text-zinc-600' : 'text-zinc-400')

  function percentLabel(window) {
    return `${Math.round(Number(window.used_percentage))}%`
  }

  function barWidth(window) {
    return `${Math.min(100, Math.max(0, Number(window.used_percentage)))}%`
  }

  /**
   * Headroom, not brand decoration: the bar turns as the window closes, which
   * is the one thing the user is scanning these two rows for.
   */
  function barTone(window) {
    const used = Number(window.used_percentage)
    if (used >= 90) return dark ? 'bg-rose-400/80' : 'bg-rose-500'
    if (used >= 75) return dark ? 'bg-amber-400/80' : 'bg-amber-500'
    return dark ? 'bg-brand-400/80' : 'bg-brand-500'
  }

  function duration(ms) {
    const minutes = Math.floor(ms / 60_000)
    if (minutes >= 1440) return `${Math.floor(minutes / 1440)}d`
    if (minutes >= 60) return `${Math.floor(minutes / 60)}h`
    return `${Math.max(1, minutes)}m`
  }
</script>

{#if windows.length && compact}
  <!-- One row, because the chip it sits in is a single line tall. -->
  <span
    class="text-[10px] leading-none tabular-nums {stale ? 'opacity-60' : ''} {valueTone}"
    data-testid="claude-usage-meter"
  >
    {windows.map((entry) => `${entry.label} ${percentLabel(entry.window)}`).join(' · ')}
  </span>
{:else if windows.length}
  <!-- Spans, not divs: this meter renders inside the chip button and inside
       each chooser option button, where only phrasing content is valid. -->
  <span
    class="flex flex-col gap-0.5 {stale ? 'opacity-60' : ''}"
    data-testid="claude-usage-meter"
  >
    {#each windows as entry (entry.key)}
      <span class="flex items-center gap-1.5 text-[10px] leading-none">
        <span class="w-4 shrink-0 {labelTone}">{entry.label}</span>
        <span class="h-1 min-w-[3rem] flex-1 overflow-hidden rounded-full {trackTone}">
          <span
            class="block h-full rounded-full {barTone(entry.window)}"
            style="width: {barWidth(entry.window)}"
            data-testid="claude-usage-bar-{entry.key}"
          ></span>
        </span>
        <span class="shrink-0 tabular-nums {valueTone}">{percentLabel(entry.window)}</span>
      </span>
    {/each}

    {#if stale}
      <span class="text-[10px] leading-none {staleTone}" data-testid="claude-usage-stale">
        last seen {duration(ageMs)} ago
      </span>
    {:else if nextResetMs != null}
      <span class="text-[10px] leading-none {staleTone}" data-testid="claude-usage-reset">
        resets in {duration(nextResetMs)}
      </span>
    {/if}
  </span>
{/if}
