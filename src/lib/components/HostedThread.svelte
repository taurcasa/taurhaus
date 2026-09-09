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
  const lines = $derived((transcript?.thread?.turns ?? []).flatMap(turn =>
    (turn.items ?? []).flatMap(item => item.text ? [item.text] : (item.content ?? []).filter(c => c.type === 'text').map(c => c.text))))
  const requests = $derived(transcript?.requests ?? [])
  const disabled = $derived(submitting || Boolean(transcript?.outcomeUnknown))

  $effect(() => {
    const team = teamName, member = memberName
    let disposed = false, timer
    async function refresh() {
      try {
        const next = await coordinationHosted(team, member, 'transcript', {})
        if (!disposed) { transcript = next; unavailable = false }
      } catch (cause) {
        if (!disposed) {
          const message = String(cause?.message ?? cause)
          unavailable = message.includes('NOT_HOSTED')
          error = message.includes('UNKNOWN_METHOD') ? 'Hosted controls require a daemon update.' : message
        }
      } finally {
        if (!disposed) timer = setTimeout(refresh, 2000)
      }
    }
    refresh()
    return () => { disposed = true; clearTimeout(timer) }
  })

  async function submit(operation, params = {}) {
    if (submitting || !transcript) return
    submitting = true
    error = ''
    try {
      await coordinationHosted(teamName, memberName, operation, { generation: transcript.attachmentGeneration, ...params })
      if (operation === 'input') draft = ''
      transcript = await coordinationHosted(teamName, memberName, 'transcript', {})
    } catch (cause) {
      error = String(cause?.message ?? cause)
      if (operation === 'input') transcript = { ...transcript, outcomeUnknown: true }
    } finally { submitting = false }
  }
</script>

{#if !unavailable && (transcript || error)}
  <section class="space-y-3 rounded-xl border p-4 {t.keyline} {t.textPrimary}" aria-label="Hosted conversation">
    <h3 class="text-sm font-semibold">Conversation</h3>
    {#if transcript}
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
      {#if transcript.outcomeUnknown}<p role="status">Previous input has an unknown outcome. Check the conversation before recovery.</p>{/if}
      <form class="space-y-2" onsubmit={event => { event.preventDefault(); submit('input', { text: draft }) }}>
        <label class="block text-xs" for="hosted-input">Message hosted member</label>
        <textarea id="hosted-input" class="w-full rounded border bg-transparent p-2 text-sm {t.keyline}" bind:value={draft} disabled={disabled} maxlength="8000" rows="3"></textarea>
        <div class="flex gap-3 text-sm">
          <button class="rounded border px-3 py-1 {t.keyline}" disabled={disabled || !draft.trim()} type="submit">Send</button>
          <button disabled={submitting} type="button" onclick={() => submit('interrupt')}>Stop turn</button>
        </div>
      </form>
    {/if}
    {#if error}<p role="alert" class="text-sm">{error}</p>{/if}
  </section>
{/if}
