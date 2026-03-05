<script>
  let {
    issues = [],
    dark = false,
  } = $props()

  let expanded = $state(false)

  const normalizedIssues = $derived.by(() => {
    if (!Array.isArray(issues)) return []
    return issues
      .map((issue) => ({
        severity: String(issue?.severity ?? 'warning').toLowerCase() === 'error' ? 'error' : 'warning',
        member: String(issue?.member ?? '').trim(),
        message: String(issue?.message ?? '').trim(),
      }))
      .filter((issue) => issue.message)
  })
  const errorCount = $derived(
    normalizedIssues.reduce((count, issue) => count + (issue.severity === 'error' ? 1 : 0), 0)
  )
  const issueCount = $derived(normalizedIssues.length)
  const summaryTone = $derived(
    dark
      ? 'border-[var(--validation-bar-border-dark)] bg-[var(--validation-bar-bg-dark)] text-[var(--validation-bar-text-dark)]'
      : 'border-[var(--validation-bar-border-light)] bg-linear-to-b from-[var(--validation-bar-bg-light-from)] to-[var(--validation-bar-bg-light-to)] text-[var(--validation-bar-text-light)]'
  )
  const mutedTone = $derived(dark ? 'text-zinc-400' : 'text-brand-700')
  const chevronTone = $derived(dark ? 'text-zinc-500' : 'text-brand-700/70')
  const errorBadgeTone = $derived(dark ? 'bg-danger-500/20 text-danger-300' : 'bg-danger-50 text-danger-600 border border-danger-200')

  function summaryText() {
    if (issueCount === 0) return '0 issues'
    if (errorCount > 0) {
      return `${issueCount} issue${issueCount === 1 ? '' : 's'}`
    }
    return `${issueCount} issue${issueCount === 1 ? '' : 's'}`
  }

  function toggleExpanded() {
    if (issueCount === 0) return
    expanded = !expanded
  }

  $effect(() => {
    if (errorCount > 0) {
      expanded = true
    }
  })
</script>

<section
  class="rounded-md border {summaryTone}"
  data-testid="validation-bar"
>
  <button
    class="flex h-9 w-full items-center gap-2 px-2.5 text-left"
    type="button"
    onclick={toggleExpanded}
    data-testid="validation-bar-toggle"
  >
    <span class="text-xs font-medium" data-testid="validation-bar-summary">{summaryText()}</span>
    {#if errorCount > 0}
      <span class="rounded-full px-1.5 py-0.5 text-[10px] font-semibold {errorBadgeTone}" data-testid="validation-bar-error-badge">
        {errorCount} error{errorCount === 1 ? '' : 's'}
      </span>
    {/if}
    <span class="ml-auto text-xs {chevronTone}" aria-hidden="true">
      {expanded ? '▾' : '▸'}
    </span>
  </button>

  {#if expanded && issueCount > 0}
    <ul class="border-t px-2.5 py-2 space-y-1.5 {dark ? 'border-zinc-700' : 'border-brand-200'}" data-testid="validation-bar-list">
      {#each normalizedIssues as issue, index}
        <li class="flex items-start gap-2 text-xs {mutedTone}" data-testid={`validation-issue-${index}`}>
          <span class={issue.severity === 'error' ? 'text-danger-500' : 'text-warning-500'} aria-hidden="true">
            ●
          </span>
          <span class="font-medium {dark ? 'text-zinc-200' : 'text-brand-900'}">
            {issue.member || 'Team'}:
          </span>
          <span>{issue.message}</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>
