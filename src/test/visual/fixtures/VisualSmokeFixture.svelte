<script>
  let { dark = false, label = 'Visual smoke fixture' } = $props()
</script>

<main
  class="min-h-screen w-full flex items-center justify-center font-sans"
  data-testid="visual-smoke-stage"
>
  <div
    class="w-[360px] rounded-2xl border px-6 py-5 shadow-[0_18px_45px_rgba(0,0,0,0.18)] {dark
      ? 'bg-brand-950 text-zinc-100 border-white/[0.08]'
      : 'bg-white text-zinc-900 border-brand-900/10'}"
    data-testid="visual-smoke-card"
  >
    <div class="text-[11px] uppercase tracking-[0.08em] text-zinc-500">Visual Smoke</div>
    <h1 class="mt-2 text-[18px] font-semibold tracking-[-0.02em]">{label}</h1>
    <p class="mt-2 text-[13px] leading-[1.45] {dark ? 'text-zinc-300' : 'text-zinc-600'}">
      This fixture validates browser-mode screenshot infrastructure, deterministic viewport sizing, and theme application.
    </p>
  </div>
</main>
