<script>
  import { toolDescriptor } from '../toolRegistry.js'
  import { themeTokens } from '../themeTokens.js'

  let { tool, delivery = 'tmux', dark = false, disabled = false, onchange = () => {} } = $props()
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
      <option value="tmux">tmux pane (fallback)</option>
      <option value="app_server">app-server (native; TUI attached in tmux)</option>
    </select>
  </label>
{/if}
