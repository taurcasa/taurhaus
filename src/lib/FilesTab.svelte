<script>
  import { getFileTree, readFile, readProjectAsset } from './ipc.js'
  import { classifyFile } from './fileClassifier.js'
  import { pathWasChanged } from './fileChange.js'
  import * as assetCache from './assetCache.js'
  import MarkdownRenderer from './MarkdownRenderer.svelte'
  import CodeViewer from './CodeViewer.svelte'
  import ContextMenu from './ContextMenu.svelte'
  import { themeTokens } from './themeTokens.js'
  import { createAsyncGuard } from './asyncGuard.js'

  let {
    dark,
    codeTheme,
    selectedProject,
    isActive = true,
    position = $bindable(null),
    navTarget = null,
    onClearNavTarget,
    onMarkdownNavigate,
    changedPaths = null,
    onChangedPathsConsumed,
  } = $props()

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens
  const treeIcon = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const TREE_ROW_HEIGHT = 32
  const TREE_OVERSCAN_ROWS = 8
  const TREE_VIRTUALIZE_THRESHOLD = 120

  // File state
  let fileTree = $state([])
  let fileTreeLoading = $state(false)
  let fileTreeError = $state(null)
  let selectedFile = $state(null)
  let fileContent = $state(null)
  let fileContentLoading = $state(false)
  let fileError = $state(null)
  let fileType = $state(null)
  let imageDataUri = $state(null)
  let expandedDirs = $state(new Set())
  let targetLineNumber = $state(null)
  let targetAnchor = $state(null)
  let treeScrollContainer = $state(null)
  let treeScrollTop = $state(0)
  let treeViewportHeight = $state(320)
  let fileTreeRefreshInFlight = false
  let fileTreeRefreshPending = false
  const fileReadGuard = createAsyncGuard()
  let activeFileReadSequence = 0
  let initializedProjectId = null
  let wasActive = false

  // Sync position outward for Shell's per-project position memory
  $effect(() => {
    position = { selectedFile }
  })

  // Load/refresh file tree on project switch or tab activation.
  // Important: ignore selectedProject metadata-only updates (same project id),
  // otherwise in-flight file reads get invalidated and legitimate results are discarded.
  $effect(() => {
    const projectId = selectedProject?.id ?? null
    const active = Boolean(isActive)
    const becameActive = active && !wasActive
    wasActive = active

    if (!projectId) {
      initializedProjectId = null
      return
    }
    if (!active) return

    const projectChanged = projectId !== initializedProjectId
    if (!projectChanged && !becameActive) return

    if (projectChanged) {
      clearSelection()
      fileTree = []
      expandedDirs = new Set()
      treeScrollTop = 0
    } else {
      fileReadGuard.invalidate()
      activeFileReadSequence = 0
    }

    initializedProjectId = projectId
    refreshFileTree(projectId)
  })

  $effect(() => {
    if (!treeScrollContainer) return

    const updateViewport = () => {
      treeViewportHeight = treeScrollContainer?.clientHeight || 320
      treeScrollTop = treeScrollContainer?.scrollTop || 0
    }
    updateViewport()

    if (typeof ResizeObserver !== 'undefined') {
      const observer = new ResizeObserver(updateViewport)
      observer.observe(treeScrollContainer)
      return () => observer.disconnect()
    }

    window.addEventListener('resize', updateViewport)
    return () => window.removeEventListener('resize', updateViewport)
  })

  // Watch navTarget and open the requested file/directory
  $effect(() => {
    if (!navTarget) return
    const { file, lineNumber, anchor, directory } = navTarget
    if (directory !== undefined && directory !== null) {
      openDirectory(directory)
    } else if (file) {
      openFile(file, lineNumber ?? null, anchor ?? null)
    }
    onClearNavTarget?.()
  })

  // React to file changes signaled by Shell's central listener.
  // Refreshes the file tree and re-reads the currently open file if affected.
  //
  // IMPORTANT: Capture changedPaths into a local BEFORE consuming. In Svelte 5,
  // signals propagate eagerly — onChangedPathsConsumed() sets the parent's
  // fileChangePaths to null, which immediately nullifies changedPaths here.
  // Reading changedPaths after consume would see null and skip the file refresh.
  $effect(() => {
    const paths = changedPaths
    if (!paths || !selectedProject?.id || !isActive) return
    onChangedPathsConsumed?.()

    // Refresh file tree (silent — no loading skeleton)
    refreshFileTree(selectedProject.id)

    // Re-read the currently open file if it was among the changes
    if (selectedFile && pathWasChanged(paths, selectedFile)) {
      openFile(selectedFile, targetLineNumber)
    }
  })

  async function loadFileTree(projectId) {
    // Only show skeleton on initial load -- refreshes update silently
    const isInitialLoad = fileTree.length === 0
    if (isInitialLoad) fileTreeLoading = true
    fileTreeError = null
    try {
      fileTree = await getFileTree(projectId)
      fileTreeError = null
      // Auto-select README if no file selected
      if (!selectedFile) {
        const readme = findReadmeInTree(fileTree)
        if (readme) {
          await openFile(readme.path)
        }
      }
    } catch (e) {
      fileTree = []
      fileTreeError = 'Failed to load file tree. Please retry.'
      console.error(`[file] failed to load file tree (project=${projectId}):`, e)
    } finally {
      fileTreeLoading = false
    }
  }

  async function refreshFileTree(projectId) {
    if (fileTreeRefreshInFlight) {
      fileTreeRefreshPending = true
      return
    }

    fileTreeRefreshInFlight = true
    try {
      await loadFileTree(projectId)
    } finally {
      fileTreeRefreshInFlight = false
      if (fileTreeRefreshPending && isActive && selectedProject?.id === projectId) {
        fileTreeRefreshPending = false
        refreshFileTree(projectId)
      } else {
        fileTreeRefreshPending = false
      }
    }
  }

  function findReadmeInTree(nodes) {
    for (const node of nodes) {
      if (!node.is_dir && /^readme/i.test(node.name)) return node
      if (node.is_dir && node.children) {
        const found = findReadmeInTree(node.children)
        if (found) return found
      }
    }
    return null
  }

  function findNodeByPath(nodes, targetPath) {
    for (const node of nodes) {
      if (node.path === targetPath) return node
      if (node.is_dir && node.children) {
        const found = findNodeByPath(node.children, targetPath)
        if (found) return found
      }
    }
    return null
  }

  function findReadmeInDirectory(node) {
    if (!node?.children) return null
    return node.children.find((child) => !child.is_dir && /^readme\.md$/i.test(child.name)) || null
  }

  function clearSelection() {
    fileReadGuard.invalidate()
    activeFileReadSequence = 0
    selectedFile = null
    targetLineNumber = null
    targetAnchor = null
    fileContent = null
    fileContentLoading = false
    fileError = null
    fileType = null
    imageDataUri = null
  }

  async function openDirectory(relativePath) {
    if (!selectedProject) return

    const directoryPath = (relativePath || '').replace(/\/+$/, '')
    const next = new Set(expandedDirs)
    if (directoryPath) {
      const parts = directoryPath.split('/').filter(Boolean)
      let dir = ''
      for (const part of parts) {
        dir = dir ? `${dir}/${part}` : part
        next.add(dir)
      }
    }
    expandedDirs = next

    let tree = fileTree
    if (tree.length === 0) {
      try {
        tree = await getFileTree(selectedProject.id)
        fileTree = tree
      } catch (error) {
        tree = []
        fileTreeError = 'Failed to load file tree. Please retry.'
        console.error(`[file] failed to load file tree while opening directory (project=${selectedProject.id}):`, error)
      }
    }

    const directoryNode = directoryPath
      ? findNodeByPath(tree, directoryPath)
      : { is_dir: true, children: tree }

    if (!directoryNode || !directoryNode.is_dir) {
      console.warn(`[file] directory target not found in tree: ${directoryPath}`)
      return
    }

    const readme = findReadmeInDirectory(directoryNode)
    if (readme) {
      await openFile(readme.path)
      return
    }

    clearSelection()
  }

  function toggleDir(path) {
    const next = new Set(expandedDirs)
    if (next.has(path)) {
      next.delete(path)
    } else {
      next.add(path)
    }
    expandedDirs = next
  }

  function handleRowKeydown(event, action) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      action()
    }
  }

  function handleTreeScroll(event) {
    treeScrollTop = event.currentTarget?.scrollTop || 0
  }

  function flattenVisibleRows(nodes, expanded, depth = 0, rows = []) {
    for (const node of nodes) {
      rows.push({ node, depth })
      if (node.is_dir && expanded.has(node.path) && node.children) {
        flattenVisibleRows(node.children, expanded, depth + 1, rows)
      }
    }
    return rows
  }

  const treeRows = $derived.by(() => flattenVisibleRows(fileTree, expandedDirs))
  const useVirtualizedTree = $derived.by(() => treeRows.length > TREE_VIRTUALIZE_THRESHOLD)

  const treeWindow = $derived.by(() => {
    const total = treeRows.length
    if (!useVirtualizedTree) {
      return { start: 0, end: total, paddingTop: 0, paddingBottom: 0 }
    }

    const viewportRows = Math.max(1, Math.ceil(treeViewportHeight / TREE_ROW_HEIGHT))
    const start = Math.max(0, Math.floor(treeScrollTop / TREE_ROW_HEIGHT) - TREE_OVERSCAN_ROWS)
    const end = Math.min(
      total,
      start + viewportRows + TREE_OVERSCAN_ROWS * 2
    )

    return {
      start,
      end,
      paddingTop: start * TREE_ROW_HEIGHT,
      paddingBottom: Math.max(0, (total - end) * TREE_ROW_HEIGHT),
    }
  })

  async function openFile(relativePath, lineNumber = null, anchor = null) {
    if (!selectedProject) return
    const projectId = selectedProject.id
    const requestPath = relativePath
    const requestSequence = fileReadGuard.next()
    activeFileReadSequence = requestSequence
    const isStale = () =>
      !fileReadGuard.isCurrent(requestSequence)
        || selectedProject?.id !== projectId
        || selectedFile !== requestPath

    selectedFile = relativePath
    targetLineNumber = lineNumber
    targetAnchor = anchor

    // Auto-expand parent directories so the file is visible in the tree
    const parts = relativePath.split('/')
    if (parts.length > 1) {
      const next = new Set(expandedDirs)
      let dir = ''
      for (let i = 0; i < parts.length - 1; i++) {
        dir = dir ? dir + '/' + parts[i] : parts[i]
        next.add(dir)
      }
      expandedDirs = next
    }

    fileContentLoading = true
    fileContent = null
    fileError = null
    imageDataUri = null
    fileType = classifyFile(requestPath)

    try {
      if (fileType === 'image') {
        // Check asset cache first, then IPC
        const cached = assetCache.get(projectId, requestPath)
        if (cached) {
          if (isStale()) return
          imageDataUri = cached
        } else {
          const dataUri = await readProjectAsset(projectId, requestPath)
          if (isStale()) return
          if (dataUri) {
            assetCache.set(projectId, requestPath, dataUri)
            imageDataUri = dataUri
          } else {
            fileError = 'error'
          }
        }
      } else if (fileType === 'binary' || fileType === 'pdf') {
        // Known binary -- no IPC call
        if (isStale()) return
        fileError = fileType
      } else {
        // text or markdown -- read as text
        const content = await readFile(projectId, requestPath)
        if (isStale()) return
        fileContent = content
      }
    } catch (e) {
      if (isStale()) return
      const msg = String(e?.message || e || '')
      console.error(`[file] error loading "${relativePath}" (project=${selectedProject?.id}): ${msg}`)
      if (msg.includes('Binary file') || msg.includes('cannot be read as text')) {
        fileError = 'binary'
      } else if (msg.includes('too large')) {
        fileError = 'too-large'
      } else {
        fileError = 'error'
      }
    } finally {
      if (activeFileReadSequence === requestSequence) {
        fileContentLoading = false
      }
    }
  }

  // --- File tree context menu ---
  let fileCtxMenu = $state(null) // { x, y, path, name }

  function openFileContextMenu(e, path, name) {
    e.preventDefault()
    fileCtxMenu = { x: e.clientX, y: e.clientY, path, name }
  }

  function closeFileContextMenu() {
    fileCtxMenu = null
  }

  function relativePath(fullPath) {
    const base = selectedProject?.path || ''
    if (fullPath.startsWith(base)) {
      const rel = fullPath.slice(base.length)
      return rel.startsWith('/') ? rel.slice(1) : rel
    }
    return fullPath
  }

  const fileCtxMenuItems = $derived(fileCtxMenu ? [
    { label: 'Copy Path', action: () => { navigator.clipboard.writeText(fileCtxMenu.path); closeFileContextMenu() }, icon: '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 0 1-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 0 1 1.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 0 0-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 0 1-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 0 0-3.375-3.375h-1.5a1.125 1.125 0 0 1-1.125-1.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H9.75"/></svg>' },
    { label: 'Copy Relative Path', action: () => { navigator.clipboard.writeText(relativePath(fileCtxMenu.path)); closeFileContextMenu() }, icon: '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 0 1-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 0 1 1.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 0 0-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 0 1-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 0 0-3.375-3.375h-1.5a1.125 1.125 0 0 1-1.125-1.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H9.75"/></svg>' },
  ] : [])
</script>

<div class="flex-1 flex min-h-0 min-w-0 overflow-hidden">

  <!-- File tree (200px fixed) -->
  <div class="w-[200px] shrink-0 {t.listBg} border-r {t.keyline} flex flex-col overflow-hidden" role="list">
    <div
      bind:this={treeScrollContainer}
      class="flex-1 overflow-y-auto pt-2"
      data-testid="file-tree-scroll"
      onscroll={handleTreeScroll}
    >
      {#if fileTreeError}
        <div class="mx-2 mt-2 px-2 py-1.5 rounded text-[11px] flex items-center gap-2 {dark ? 'bg-warning-300/10 text-warning-300' : 'bg-warning-50 text-warning-600'}" data-testid="filetree-error">
          <svg class="w-3 h-3 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.007M4.93 19.5h14.14c1.54 0 2.502-1.667 1.732-3L13.732 4.25c-.77-1.333-2.694-1.333-3.464 0L3.198 16.5c-.77 1.333.192 3 1.732 3Z" />
          </svg>
          <span class="flex-1">{fileTreeError}</span>
          <button class="text-[10px] underline underline-offset-2" onclick={() => fileTreeError = null} data-testid="filetree-error-dismiss">Dismiss</button>
        </div>
      {/if}

      {#if fileTreeLoading}
        <div class="px-3 space-y-1" data-testid="filetree-loading">
          {#each Array(6) as _}
            <div class="flex items-center h-[32px] gap-2 px-2">
              <div class="w-3 h-3 rounded bg-zinc-300/30 animate-pulse"></div>
              <div class="h-2.5 flex-1 rounded bg-zinc-300/20 animate-pulse"></div>
            </div>
          {/each}
        </div>
      {:else if fileTree.length === 0}
        <div class="px-4 pt-6 text-center">
          <p class="text-[12px] {t.textMuted}">No viewable files</p>
          <p class="text-[11px] {t.textTertiary} mt-1">Check ignore patterns in Settings</p>
        </div>
      {:else}
        {#if useVirtualizedTree}
          <div style="height: {treeWindow.paddingTop}px;"></div>
        {/if}

        {#each treeRows.slice(treeWindow.start, treeWindow.end) as row (row.node.path)}
          {@const node = row.node}
          {@const depth = row.depth}
          <div role="listitem">
            {#if node.is_dir}
              <button
                class="w-full flex items-center gap-1.5 h-[32px] text-left text-[13px] {t.textSecondary} {t.listHover} rounded transition-colors"
                style="padding-left: {8 + depth * 16}px"
                onclick={() => toggleDir(node.path)}
                onkeydown={(event) => handleRowKeydown(event, () => toggleDir(node.path))}
                oncontextmenu={(e) => openFileContextMenu(e, node.path, node.name)}
                data-testid="file-tree-node"
              >
                <svg class="w-3 h-3 {treeIcon} shrink-0 transition-transform {expandedDirs.has(node.path) ? 'rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5"/></svg>
                <svg class="w-3.5 h-3.5 shrink-0 {dark ? 'text-zinc-500' : 'text-zinc-400'}" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z"/></svg>
                <span class="truncate">{node.name}</span>
              </button>
            {:else}
              {@const isSelected = selectedFile === node.path}
              <button
                class="w-full flex items-center gap-1.5 h-[32px] text-left text-[13px] rounded transition-colors
                  {isSelected ? t.listSelected : `${dark ? 'text-zinc-400' : 'text-zinc-600'} ${t.listHover}`}"
                style="padding-left: {22 + depth * 16}px"
                onclick={() => openFile(node.path)}
                onkeydown={(event) => handleRowKeydown(event, () => openFile(node.path))}
                oncontextmenu={(e) => openFileContextMenu(e, node.path, node.name)}
                aria-current={isSelected ? 'true' : undefined}
                data-testid="file-tree-node"
              >
                <svg class="w-3.5 h-3.5 shrink-0 {isSelected ? '' : treeIcon}" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/></svg>
                <span class="truncate">{node.name}</span>
              </button>
            {/if}
          </div>
        {/each}

        {#if useVirtualizedTree}
          <div style="height: {treeWindow.paddingBottom}px;"></div>
        {/if}
      {/if}
    </div>
  </div>

  <!-- File content viewer -->
  <div class="flex-1 flex flex-col min-w-0">
    {#if !selectedFile}
      <div class="flex-1 flex items-center justify-center">
        <p class="text-[13px] {t.textMuted}">Select a file from the tree</p>
      </div>
    {:else}
      <!-- File header -->
      <div class="h-[44px] flex items-center px-6 border-b {t.keyline} shrink-0">
        <span class="text-[14px] font-medium {t.textPrimary} truncate">{selectedFile}</span>
        {#if fileType === 'image'}
          <span class="ml-3 text-[11px] {t.textTertiary}">image</span>
        {:else if fileContent?.language}
          <span class="ml-3 text-[11px] {t.textTertiary}">{fileContent.language}</span>
        {/if}
      </div>

      <!-- File content -->
      <div class="flex-1 overflow-auto">
        {#if fileContentLoading}
          <div class="p-6 space-y-2" data-testid="filecontent-loading">
            {#each Array(8) as _}
              <div class="h-3 rounded bg-zinc-200/50 animate-pulse" style="width: {40 + Math.random() * 50}%"></div>
            {/each}
          </div>
        {:else if fileError}
          <div class="flex flex-col items-center justify-center h-full gap-2 {t.textTertiary}">
            {#if fileError === 'binary'}
              <svg class="w-8 h-8 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/></svg>
              <span class="text-[13px]">Binary file — cannot display as text</span>
            {:else if fileError === 'pdf'}
              <svg class="w-8 h-8 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/></svg>
              <span class="text-[13px]">PDF viewer coming soon</span>
            {:else if fileError === 'too-large'}
              <svg class="w-8 h-8 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/></svg>
              <span class="text-[13px]">File too large to display (&gt;5 MB)</span>
            {:else}
              <svg class="w-8 h-8 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z"/></svg>
              <span class="text-[13px]">Error loading file</span>
            {/if}
          </div>
        {:else if imageDataUri}
          <div class="flex items-center justify-center p-6 h-full">
            <img src={imageDataUri} alt={selectedFile} class="max-w-full max-h-full object-contain rounded-lg" />
          </div>
        {:else if fileContent}
          {#if fileType === 'markdown'}
            <div class="p-6 overflow-auto">
              <MarkdownRenderer
                source={fileContent.content}
                {dark}
                {codeTheme}
                projectId={selectedProject?.id}
                filePath={selectedFile}
                scrollToAnchor={targetAnchor}
                onNavigate={onMarkdownNavigate}
              />
            </div>
          {:else}
            <CodeViewer code={fileContent.content} language={fileContent.language || ''} {dark} {codeTheme} scrollToLine={targetLineNumber} />
          {/if}
        {/if}
      </div>
    {/if}
  </div>
</div>

{#if fileCtxMenu}
  <ContextMenu items={fileCtxMenuItems} x={fileCtxMenu.x} y={fileCtxMenu.y} {dark} onClose={closeFileContextMenu} />
{/if}
