<script>
  import { meshSnapshots } from './meshCache.svelte.js'

  let { projectPath = '' } = $props()
</script>

<div data-testid="mesh-cache-team-name">{meshSnapshots.get(projectPath)?.teamName ?? 'empty'}</div>
