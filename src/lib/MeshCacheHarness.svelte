<script>
  import { getMeshCache } from './meshCache.svelte.js'

  let { projectPath = '' } = $props()
</script>

<div data-testid="mesh-cache-team-name">{getMeshCache(projectPath)?.teamName ?? 'empty'}</div>
