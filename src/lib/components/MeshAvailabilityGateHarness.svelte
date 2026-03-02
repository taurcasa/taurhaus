<script>
  import MeshAvailabilityGate from './MeshAvailabilityGate.svelte'

  let {
    dark = false,
    projectPath = '',
    captureWarnings = false,
  } = $props()
</script>

<MeshAvailabilityGate {dark} {projectPath}>
  {#snippet children(agentWarnings)}
    {#if captureWarnings}
      <div data-testid="mesh-gate-warning-count">{agentWarnings.length}</div>
      <div data-testid="mesh-gate-warning-message">{agentWarnings[0]?.message ?? ''}</div>
    {:else}
      <div data-testid="mesh-gate-child">Setup Form</div>
    {/if}
  {/snippet}
</MeshAvailabilityGate>
