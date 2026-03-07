<script>
  import OverviewTab from '../../../lib/OverviewTab.svelte'
  import SearchOverlay from '../../../lib/SearchOverlay.svelte'
  import Sidebar from '../../../lib/Sidebar.svelte'
  import TaskBoard from '../../../lib/TaskBoard.svelte'
  import GitTab from '../../../lib/GitTab.svelte'
  import { setProjectContext } from '../../../lib/context/ProjectContext.js'
  import { setSessionContext } from '../../../lib/context/SessionContext.js'
  import { themeTokens } from '../../../lib/themeTokens.js'
  import MeshSetupView from '../../../lib/components/MeshSetupView.svelte'
  import MeshRuntimeView from '../../../lib/components/MeshRuntimeView.svelte'

  let { scenario, fixtureData, dark = true } = $props()

  const t = $derived.by(() => themeTokens(dark))
  const selectedProject = $derived.by(() => fixtureData.selectedProject)
  const projects = $derived.by(() => fixtureData.projects)

  setProjectContext({
    get projects() { return projects },
    get selectedProject() { return selectedProject },
    selectProject: () => {},
    navigateToCommit: () => {},
    navigateToFile: () => {},
    navigateToCommitRange: () => {},
    onProjectRemoved: () => {},
  })

  setSessionContext({
    daemonStatus: 'connected',
    launchSession: () => {},
    openTerminal: () => {},
    openManageProjects: () => {},
    toggleSettings: () => {},
    retryProjects: () => {},
  })

  const codeTheme = 'github-dark-dimmed'

  const titlebarTabs = [
    { id: 'overview', label: 'Overview' },
    { id: 'tasks', label: 'Tasks' },
    { id: 'search', label: 'Search' },
    { id: 'mesh-setup', label: 'Mesh' },
    { id: 'git', label: 'Git' },
  ]

  const activeTab = $derived.by(() => {
    if (scenario.mode === 'search') return 'overview'
    if (scenario.mode === 'mesh-runtime' || scenario.mode === 'mesh-recovery') return 'mesh-setup'
    return scenario.mode
  })

  const shellActions = {
    onSelectProject: () => {},
    onAddProject: () => {},
    onToggleSettings: () => {},
    onRetry: () => {},
  }

  const overviewActions = {
    onNavigateToCommit: () => {},
    onSelectProject: () => {},
    onLaunchSession: () => {},
    onOpenTerminal: () => {},
  }

  const addAgentDraft = $derived.by(() => ({
    name: 'bug-sweeper',
    tool: 'codex',
    model: 'gpt-5.4 high',
    roleId: '',
    isLocked: false,
    submitting: false,
    projectId: fixtureData.selectedProject.id,
  }))

  const runtimeResumeProgress = {
    items: [
      { memberName: 'team-lead', status: 'succeeded', message: 'Lead session attached' },
      { memberName: 'developer1', status: 'succeeded', message: 'Pane resumed in tmux' },
      { memberName: 'ui-specialist', status: 'failed', message: 'Waiting for tool startup' },
    ],
  }
</script>

<div class="w-screen h-screen overflow-hidden bg-brand-950 p-1.5">
  <div class="flex h-full flex-col rounded-[22px] border border-white/8 bg-brand-950 shadow-[0_24px_80px_rgba(0,0,0,0.35)]">
    <header class="flex h-[46px] items-center gap-3 px-4 text-zinc-100">
      <div class="min-w-[252px] text-[13px] font-semibold tracking-[0.02em] text-zinc-100">taurhaus</div>
      <nav class="flex items-end gap-1">
        {#each titlebarTabs as tab}
          <div
            class={`h-9 rounded-t-lg px-3 pt-2 text-[12px] font-medium ${tab.id === activeTab ? `${t.mainBg} ${t.textPrimary} shadow-[0_-1px_0_rgba(255,255,255,0.06)]` : 'text-zinc-400'}`}
          >
            {tab.label}
          </div>
        {/each}
      </nav>
      <div class="ml-auto flex items-center gap-2 text-[11px] text-zinc-400">
        <span class="rounded-full bg-white/6 px-2 py-1">Dark</span>
        <span class="rounded-full bg-white/6 px-2 py-1">Mesh live</span>
      </div>
    </header>

    <div class="flex min-h-0 flex-1 gap-1.5 px-1.5 pb-1.5">
      <aside class="w-[252px] shrink-0 overflow-hidden rounded-2xl bg-brand-950 border border-white/8">
        <Sidebar
          {dark}
          projects={projects}
          selectedProject={selectedProject}
          daemonStatus="connected"
          actions={shellActions}
        />
      </aside>

      <main data-testid="readme-main-panel" class={`min-w-0 flex-1 overflow-hidden rounded-2xl border border-white/8 ${t.mainBg}`}>
        {#if scenario.mode === 'overview' || scenario.mode === 'search'}
          <OverviewTab
            {dark}
            {codeTheme}
            data={fixtureData.overviewData}
            actions={overviewActions}
          />
        {:else if scenario.mode === 'tasks'}
          <TaskBoard
            {dark}
            {codeTheme}
            projectId={selectedProject.id}
            projectPath={selectedProject.path}
            isActive={true}
          />
        {:else if scenario.mode === 'mesh-setup'}
          <MeshSetupView
            mode="setup"
            {dark}
            projectPath={selectedProject.path}
            teamConfig={fixtureData.mesh.setupTeam}
            selectedNode={fixtureData.mesh.selectedSetupNode}
            selectedNodeId={fixtureData.mesh.selectedSetupNode.id}
            teamName={fixtureData.mesh.teamName}
            canInitialize={true}
            availableProjects={fixtureData.availableProjects}
            slideOver="customizer"
            slideOverContext={null}
          />
        {:else if scenario.mode === 'mesh-runtime'}
          <MeshRuntimeView
            {dark}
            teamName={fixtureData.mesh.teamName}
            teamConfig={fixtureData.mesh.runtimeTeam}
            selectedNode={fixtureData.mesh.selectedRuntimeNode}
            selectedNodeId={fixtureData.mesh.selectedRuntimeNode.id}
            teamRuntimeState="active"
            availableProjects={fixtureData.availableProjects}
            roleTemplates={[]}
            addAgentOpen={false}
            addAgentDraft={addAgentDraft}
            canSubmitAddAgent={true}
          />
        {:else if scenario.mode === 'mesh-recovery'}
          <MeshRuntimeView
            {dark}
            teamName={fixtureData.mesh.teamName}
            teamConfig={fixtureData.mesh.runtimeTeam}
            selectedNode={null}
            selectedNodeId={null}
            teamRuntimeState="cold_resume"
            isResumingTeam={false}
            resumeProgress={runtimeResumeProgress}
            availableProjects={fixtureData.availableProjects}
            roleTemplates={[]}
            addAgentOpen={false}
            addAgentDraft={addAgentDraft}
            canSubmitAddAgent={true}
          />
        {:else if scenario.mode === 'git'}
          <GitTab
            {dark}
            projectPath={selectedProject.path}
            projectId={selectedProject.id}
          />
        {/if}
      </main>
    </div>

    {#if scenario.mode === 'search'}
      <SearchOverlay
        {dark}
        open={true}
        onNavigate={() => {}}
      />
    {/if}
  </div>
</div>
