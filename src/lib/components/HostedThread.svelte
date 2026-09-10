<script>
  import { coordinationHosted } from '../ipc/coordination.js'
  import { themeTokens } from '../themeTokens.js'
  let { teamName, memberName, dark = false } = $props()
  const t = $derived(themeTokens(dark))
  let transcript = $state(null)
  let draft = $state('')
  let error = $state('')
  let submitting = $state(false)
  let unavailable = $state(false)
  let pending = $state('')
  const lines = $derived((transcript?.thread?.turns ?? []).flatMap(turn =>
    (turn.items ?? []).flatMap(item => item.text ? [item.text] : (item.content ?? []).filter(c => c.type === 'text').map(c => c.text))))
  const requests = $derived(transcript?.requests ?? [])
  const inactive = $derived(Boolean(transcript?.stopped || transcript?.orphanProcessId))
  const disabled = $derived(submitting || inactive || Boolean(transcript?.outcomeUnknown) || pending.startsWith('Recovery'))

  $effect(() => {
    const team = teamName, member = memberName
    transcript = null
    draft = ''
    error = ''
    pending = ''
    unavailable = false
    submitting = false
    let disposed = false, terminal = false, timer
    async function refresh() {
      try {
        const next = await coordinationHosted(team, member, 'transcript', {})
        if (!disposed) {
          transcript = next; unavailable = false; error = ''
          if (next.thread?.status?.type === 'idle' || pending === 'Conversation is updating. Please wait.') pending = ''
        }
      } catch (cause) {
        if (!disposed) {
          const message = String(cause?.message ?? cause)
          unavailable = message.includes('NOT_HOSTED')
          const older = /UNKNOWN_METHOD|Unknown method:/.test(message)
          terminal = unavailable || older
          if (message.includes('pending:')) {
            pending = 'Conversation is updating. Please wait.'; error = ''
          } else error = older ? 'Hosted controls require a daemon update.' : 'Conversation unavailable. Stop and resume the hosted member to recover.'
        }
      } finally {
        if (!disposed && !terminal) timer = setTimeout(refresh, 2000)
      }
    }
    refresh()
    return () => { disposed = true; clearTimeout(timer) }
  })

  async function submit(operation, params = {}) {
    if (submitting || !transcript || (inactive && operation !== 'reconcile')) return
    const team = teamName, member = memberName, generation = transcript.attachmentGeneration
    const current = () => team === teamName && member === memberName
    submitting = true
    error = ''
    pending = ''
    let accepted = false
    try {
      await coordinationHosted(team, member, operation, { generation, ...params })
      if (!current()) return
      accepted = true
      if (operation === 'input') draft = ''
      const next = await coordinationHosted(team, member, 'transcript', {})
      if (current()) transcript = next
    } catch (cause) {
      if (!current()) return
      const message = String(cause?.message ?? cause)
      if (accepted) {
        pending = 'Conversation is updating. Please wait.'
      } else if (/pending:|deferred:|host member busy/.test(message)) {
        pending = message.includes('recovery')
          ? 'Recovery must reach the next idle turn. Your draft is saved; send it when the turn is idle.'
          : 'Input deferred. Your draft is saved; retry when the member is ready.'
      } else {
        error = message.startsWith('failed:') ? 'The operation was not accepted. Refresh or resume the member before retrying.'
          : 'The operation could not be confirmed. Check the conversation before retrying.'
      }
    } finally { if (current()) submitting = false }
  }
</script>

{#if !unavailable && (transcript || error || pending)}
  <section class="space-y-3 rounded-xl border p-4 {t.keyline} {t.textPrimary}" aria-label="Hosted conversation">
    <h3 class="text-sm font-semibold">Conversation</h3>
    <p class="text-xs">To return a seat on a team-owned team to a tmux pane, stop it, remove it, then re-add the same name with Delivery set to tmux pane.</p>
    {#if transcript}
      {#if transcript.orphanProcessId}
        <p role="status">Orphaned host process {transcript.orphanProcessId} survived the previous daemon. Verify its recorded process start time, terminate that process manually, then stop and resume this member.</p>
      {:else if transcript.stopped}
        <p role="status">Member is stopped. Resume it before sending input.</p>
      {/if}
      <div class="max-h-64 space-y-2 overflow-auto whitespace-pre-wrap text-sm" aria-label="Hosted transcript">
        {#each lines as line}<p>{line}</p>{/each}
      </div>
      {#each requests as request}
        <div class="space-x-2 text-sm">
          <span>Permission requested</span>
          <pre class="max-h-32 overflow-auto whitespace-pre-wrap text-xs">{JSON.stringify(request.params, null, 2)}</pre>
          <button disabled={submitting} onclick={() => submit('approval', { requestId: request.id, accept: true })}>Allow</button>
          <button disabled={submitting} onclick={() => submit('approval', { requestId: request.id, accept: false })}>Deny</button>
        </div>
      {/each}
      <p class="text-xs">Send starts an idle turn or steers the active turn. Deferred drafts are not queued.</p>
      {#if transcript.outcomeUnknown}
        <p role="status">Previous input has an unknown outcome. Stop the member, then abandon this input without replay before resuming.</p>
        {#if transcript.stopped}
          <button disabled={submitting} onclick={() => submit('reconcile', { abandonUnknown: true })}>Abandon unknown input without replay</button>
        {/if}
      {/if}
      <form class="space-y-2" onsubmit={event => { event.preventDefault(); submit('input', { text: draft }) }}>
        <label class="block text-xs" for="hosted-input">Message hosted member</label>
        <textarea id="hosted-input" class="w-full rounded border bg-transparent p-2 text-sm {t.keyline}" bind:value={draft} disabled={disabled} maxlength="8000" rows="3"></textarea>
        <div class="flex gap-3 text-sm">
          <button class="rounded border px-3 py-1 {t.keyline}" disabled={disabled || !draft.trim()} type="submit">Send</button>
          <button disabled={submitting || inactive} type="button" onclick={() => submit('interrupt')}>Stop turn</button>
        </div>
      </form>
    {/if}
    {#if pending}<p role="status">{pending}</p>{/if}
    {#if error}<p role="alert" class="text-sm">{error}</p>{/if}
  </section>
{/if}
