<script>
  import { toolDescriptor } from '../toolRegistry.js'
  import { themeTokens } from '../themeTokens.js'

  let { tool, delivery = 'tmux', dark = false, disabled = false, hostedSupported = true, hostedReason = null, onchange = () => {} } = $props()
  const hostingSupported = $derived(toolDescriptor(tool)?.hostingSupported === true)
  const t = $derived(themeTokens(dark))
  const fieldTone = $derived(dark ? 'bg-zinc-900 border-zinc-700' : 'bg-white border-zinc-200')
</script>

{#if hostingSupported}
  <label class="block space-y-1">
    <span class="text-xs {t.textMuted}">Delivery</span>
    <select
      class="h-10 w-full rounded-lg border px-3 text-sm {fieldTone} {t.textPrimary}"
      value={delivery ?? 'tmux'}
      {disabled}
      onchange={(event) => onchange({ delivery: event.currentTarget.value })}
    >
      <option value="tmux">typed into the pane (fallback)</option>
      <option value="app_server" disabled={!hostedSupported}>native (app-server; TUI in tmux)</option>
    </select>
  </label>
  {#if !hostedSupported && hostedReason}
    <p class="text-xs {t.textMuted}">{hostedReason}</p>
  {/if}
  <p class="text-xs {t.textMuted}">
    Both run the Codex TUI in the team's tmux pane. Native delivers messages over the
    app-server socket and reads idle from Codex itself; the fallback types messages into the pane.
  </p>
{/if}
