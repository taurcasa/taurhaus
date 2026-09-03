<script>
  import OverviewTab from './OverviewTab.svelte'
  import { setProjectContext } from './context/ProjectContext.js'
  import { setSessionContext } from './context/SessionContext.js'

  let {
    contextSelectedProject = null,
    contextProjects = [],
    data = {},
    actions = {},
    onViewAllCommits,
    onDismissRelationship,
    onMarkdownNavigate,
    onOpenAccounts = () => {},
    onOpenAddAccount = () => {},
  } = $props()

  let projectContextValue = $state({
    projects: [],
    selectedProject: null,
    selectProject: () => {},
    navigateToCommit: () => {},
    navigateToFile: () => {},
    navigateToCommitRange: () => {},
    onProjectRemoved: () => {},
  })
  const projectContext = setProjectContext(projectContextValue)

  let sessionContextValue = $state({
    daemonStatus: null,
    launchSession: () => {},
    openTerminal: () => {},
    openManageProjects: () => {},
    toggleSettings: () => {},
    openAccounts: (...args) => onOpenAccounts(...args),
    openAddAccount: (...args) => onOpenAddAccount(...args),
    retryProjects: () => {},
  })
  setSessionContext(sessionContextValue)

  $effect(() => {
    projectContext.selectedProject = contextSelectedProject
    projectContext.projects = contextProjects
  })
</script>

<OverviewTab
  dark={false}
  codeTheme="github-light"
  {data}
  {actions}
  {onViewAllCommits}
  {onDismissRelationship}
  {onMarkdownNavigate}
/>
