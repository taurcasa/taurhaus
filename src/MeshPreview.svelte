<script>
  import MeshSetupForm from './lib/components/MeshSetupForm.svelte'

  let dark = $state(true)

  const mockProjects = [
    { id: '/home/mstie/projects/taurhaus', name: 'taurhaus' },
    { id: '/home/mstie/projects/mesh', name: 'mesh' },
  ]

  function handleInit(payload) {
    console.log('Initialize:', JSON.stringify(payload, null, 2))
  }
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
      <MeshSetupForm
        {dark}
        projectPath="/home/mstie/projects/taurhaus"
        availableProjects={mockProjects}
        preflightWarnings={[]}
        oninitialize={handleInit}
      />
    </div>
  </div>
</div>
