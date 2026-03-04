<script>
  let dark = $state(true)
</script>

<div class="min-h-screen transition-colors {dark ? 'bg-brand-950' : 'bg-zinc-100'}">
  <div class="flex items-center gap-3 px-6 py-3 border-b {dark ? 'border-zinc-800' : 'border-zinc-200'}">
    <span class="text-xs font-medium {dark ? 'text-zinc-400' : 'text-zinc-600'}">Preview Mode</span>
    <button
      class="text-xs px-2 py-1 rounded {dark ? 'bg-zinc-800 text-zinc-300' : 'bg-zinc-200 text-zinc-700'}"
      onclick={() => { dark = !dark }}
    >{dark ? 'Switch to Light' : 'Switch to Dark'}</button>
  </div>

  <div class="max-w-2xl mx-auto p-6">
    <div class="rounded-xl {dark ? 'bg-zinc-900' : 'bg-white'} p-6">
      <p class="text-sm {dark ? 'text-zinc-300' : 'text-zinc-700'}">
        Mesh setup preview moved to the integrated Mesh tab flow.
      </p>
    </div>
  </div>
</div>
