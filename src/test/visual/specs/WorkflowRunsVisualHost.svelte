<script>
  import '../../../app.css'
  import WorkflowRunsPanel from '../../../lib/components/WorkflowRunsPanel.svelte'
  import { themeTokens } from '../../../lib/themeTokens.js'

  let { scenario, dark = false } = $props()

  const t = $derived(themeTokens(dark))
  const projectId = $derived(String(scenario?.projectId ?? ''))
  const sessions = $derived(Array.isArray(scenario?.sessions) ? scenario.sessions : [])
</script>

<main class="workflow-runs-stage {t.mainBg}" data-testid="workflow-runs-visual-stage">
  <div class="max-w-3xl w-full px-7">
    <div class="pt-6 pb-1 text-[18px] font-semibold tracking-[-0.02em] {t.textPrimary}">
      taurhaus
    </div>
    <WorkflowRunsPanel {projectId} {sessions} {dark} />
    {#if scenario?.emptyNote}
      <p class="py-5 text-[13px] {t.textMuted}" data-testid="workflow-runs-empty-note">
        {scenario.emptyNote}
      </p>
    {/if}
  </div>
</main>

<style>
  .workflow-runs-stage {
    min-height: 100vh;
    width: 100%;
    display: flex;
    justify-content: center;
    box-sizing: border-box;
  }
</style>
