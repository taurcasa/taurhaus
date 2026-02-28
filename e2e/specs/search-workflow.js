/**
 * Search Workflow e2e tests — open/close, result navigation, keyboard nav.
 * Replaces old search.js with workflow-focused tests.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForFileContent, clickTestId, selectProjectByName } from '../helpers/navigation.js'
import { openSearch, closeSearch, dismissSearch } from '../helpers/search.js'
import { isWindows } from '../helpers/platform.js'
import { WAIT_INSTANT, WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG, TIMEOUT_LONG, TIMEOUT_XLONG } from '../helpers/timing.js'

/**
 * Walk a FileTreeNode[] to collect text files inside subdirectories.
 * Returns an array of relative paths (e.g. ["src/main.rs", "src/lib.rs"]).
 * Prioritizes common source extensions that are likely to be indexed.
 */
function collectSubdirFiles(tree, parentPath = '', limit = 10) {
  if (!tree) return []
  const textExts = ['rs', 'js', 'ts', 'svelte', 'py', 'md', 'toml', 'css', 'html']
  const results = []
  // First pass: files in immediate subdirectories
  for (const node of tree) {
    if (!node.is_dir || !node.children?.length) continue
    for (const child of node.children) {
      if (child.is_dir) continue
      const ext = child.name.split('.').pop()?.toLowerCase()
      if (textExts.includes(ext)) {
        const path = parentPath ? `${parentPath}/${node.name}/${child.name}` : `${node.name}/${child.name}`
        results.push(path)
        if (results.length >= limit) return results
      }
    }
  }
  // Second pass: recurse deeper
  if (results.length < limit) {
    for (const node of tree) {
      if (!node.is_dir || !node.children?.length) continue
      const deeper = collectSubdirFiles(node.children, parentPath ? `${parentPath}/${node.name}` : node.name, limit - results.length)
      results.push(...deeper)
      if (results.length >= limit) break
    }
  }
  return results
}

describe('Search Workflow', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
  })

  afterEach(async () => {
    // Ensure search overlay is closed between tests
    await dismissSearch()
  })

  describe('open and close', () => {
    it('Ctrl+K opens search overlay', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      const overlay = await $('[data-testid="search-overlay"]')
      expect(await overlay.isDisplayed()).toBe(true)
    })

    it('search input is focused on open', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      const input = await $('[data-testid="search-input"]')
      expect(await input.isFocused()).toBe(true)
    })

    it('Escape closes the overlay', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      await closeSearch()

      const overlayAfter = await $('[data-testid="search-overlay"]')
      expect(await overlayAfter.isExisting()).toBe(false)
    })

    it('reopening clears prior input and results', async function () {
      if (!mainApp) return this.skip()

      // Open, type something, close, reopen
      await openSearch()
      const input = await $('[data-testid="search-input"]')
      await input.setValue('stale query')

      await closeSearch()
      await openSearch()

      const freshInput = await $('[data-testid="search-input"]')
      const value = await freshInput.getValue()
      expect(value).toBe('')

      const results = await $$('[data-testid="search-result"]')
      expect(results.length).toBe(0)

      await closeSearch()
    })

    it('Ctrl+K toggles overlay open and closed', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      const overlay = await $('[data-testid="search-overlay"]')
      expect(await overlay.isDisplayed()).toBe(true)

      await closeSearch()

      const overlayAfter = await $('[data-testid="search-overlay"]')
      expect(await overlayAfter.isExisting()).toBe(false)
    })
  })

  describe('search and navigate', () => {
    it('typing "README" shows results with non-empty titles', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      const input = await $('[data-testid="search-input"]')
      await input.setValue('README')

      // Wait for results to appear
      await browser.waitUntil(
        async () => {
          const results = await $$('[data-testid="search-result"]')
          return results.length > 0
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Search results for "README" did not appear' }
      )

      const results = await $$('[data-testid="search-result"]')
      expect(results.length).toBeGreaterThan(0)

      // Each result should have non-empty text
      const firstText = await results[0].getText()
      expect(firstText.trim().length).toBeGreaterThan(0)
    })

    it('clicking a result closes overlay and loads file in Files tab', async function () {
      if (!mainApp) return this.skip()

      const overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) {
        await openSearch()
        const input = await $('[data-testid="search-input"]')
        await input.setValue('README')
        await browser.waitUntil(
          async () => (await $$('[data-testid="search-result"]')).length > 0,
          { ...WAIT_MEDIUM, timeoutMsg: 'No results to click' }
        )
      }

      const firstResult = await $('[data-testid="search-result"]')
      if (!(await firstResult.isExisting())) return this.skip()

      await clickTestId('search-result')

      // Overlay must close
      await browser.waitUntil(
        async () => {
          const o = await $('[data-testid="search-overlay"]')
          return !(await o.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Overlay did not close after clicking result' }
      )

      // Files tab should now be active — look for file content
      await waitForFileContent(TIMEOUT_LONG, 'File content did not load after clicking search result')
    })

    it('gibberish query shows no-results state', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      // Type gibberish using keyboard to ensure oninput fires
      await clickTestId('search-input')
      await browser.keys('xyzzy999qqq'.split(''))

      // Wait for search to settle — either "No matches" text or loading-but-stable overlay
      await browser.waitUntil(
        async () => {
          const container = await $('[data-testid="search-results"]')
          if (!(await container.isExisting())) return false
          const text = await browser.execute(
            (el) => el.textContent,
            container
          )
          // Accept "No matches" OR still loading (backend may be slow on Windows)
          return text.includes('No matches') || text.includes('Type to search') === false
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Search results container did not settle' }
      )

      // Overlay should still be open (no crash) — this is the core assertion
      const overlay = await $('[data-testid="search-overlay"]')
      expect(await overlay.isExisting()).toBe(true)

      // Should have zero actual result buttons (no false positives)
      const results = await $$('[data-testid="search-result"]')
      expect(results.length).toBe(0)
    })
  })

  // ── Cross-filesystem file loading ─────────────────────────────────────
  // Regression: v0.3.7 backslash path bug — search index stored Windows-
  // style paths (src\main.rs) that the Linux daemon couldn't resolve.
  // These tests dynamically discover projects and files via IPC, then
  // search for a file IN A SUBDIRECTORY (not root-level like README)
  // to exercise the path separator handling.
  //
  // On Windows: tests both WSL projects (\\wsl$\...) and Windows FS
  // projects (D:\...) if available. On Linux/macOS: tests native paths.
  describe('file loading (cross-filesystem)', () => {
    // Dynamically discovered at runtime via IPC
    let discoveredProjects = []

    before(async function () {
      if (!mainApp) return

      // Call list_projects IPC to get all registered projects with paths
      const projects = await browser.execute(() =>
        window.__TAURI_INTERNALS__.invoke('list_projects')
      )

      if (!projects || projects.length === 0) return

      // On Windows, partition into WSL vs Windows-filesystem projects.
      // On other platforms, all projects are native — just pick up to 2.
      if (isWindows) {
        const wslProject = projects.find(p => p.path.includes('\\\\wsl'))
        const winProject = projects.find(p => /^[A-Z]:/i.test(p.path))
        if (wslProject) discoveredProjects.push({ ...wslProject, fsType: 'WSL' })
        if (winProject) discoveredProjects.push({ ...winProject, fsType: 'Windows' })
      } else {
        // Native filesystem — pick up to 2 different projects
        discoveredProjects.push({ ...projects[0], fsType: 'native' })
        if (projects.length > 1) discoveredProjects.push({ ...projects[1], fsType: 'native' })
      }

      // For each project, discover candidate files in subdirectories via get_file_tree.
      // We collect multiple candidates because not all files in the tree are in the
      // search index (e.g. gitignored files, binary files, unindexed projects).
      for (const proj of discoveredProjects) {
        const tree = await browser.execute((id) =>
          window.__TAURI_INTERNALS__.invoke('get_file_tree', { projectId: id })
        , proj.id)

        proj.subdirCandidates = collectSubdirFiles(tree)
      }
    })

    /**
     * Search for a subdirectory file, click the result, verify it loads.
     * Tries multiple candidate filenames from the file tree until one
     * returns search results (not all tree files are in the search index).
     * @param {object} proj - Project with id, name, fsType, subdirCandidates
     */
    async function searchAndVerifyFile(proj) {
      // Switch to a different project first so we know the search
      // navigates back to the correct project
      const otherProject = discoveredProjects.find(p => p.id !== proj.id)
      if (otherProject) {
        await selectProjectByName(otherProject.name)
      }

      // Try candidates until one returns search results
      let foundFileName = null
      for (const candidate of proj.subdirCandidates) {
        const fileName = candidate.split('/').pop()
        await openSearch()
        const input = await $('[data-testid="search-input"]')
        await input.setValue(fileName)

        // Brief wait for results
        const hasResults = await browser.waitUntil(
          async () => (await $$('[data-testid="search-result"]')).length > 0,
          { timeout: 3000, interval: 100 }
        ).catch(() => false)

        if (hasResults) {
          foundFileName = fileName
          break
        }
        // No results — close overlay and try next candidate
        await dismissSearch()
      }

      if (!foundFileName) {
        throw new Error(`None of ${proj.subdirCandidates.length} candidate files returned search results (${proj.fsType} project: ${proj.name})`)
      }

      // Click the first document result (overlay is still open with results)
      await clickTestId('search-result')

      // Overlay must close
      await browser.waitUntil(
        async () => browser.execute(() =>
          document.querySelector('[data-testid="search-overlay"]') === null
        ),
        { ...WAIT_MEDIUM, timeoutMsg: 'Overlay did not close after clicking result' }
      )

      // File content must load
      await waitForFileContent(
        TIMEOUT_XLONG,
        `File "${foundFileName}" did not load (${proj.fsType} project: ${proj.name})`
      )

      // Core assertion: no "Error loading file" — this catches the backslash
      // path bug where the search index stored Windows-style paths that the
      // Linux daemon couldn't resolve.
      const mainText = await browser.execute(() =>
        document.querySelector('main')?.textContent || ''
      )
      expect(mainText).not.toContain('Error loading file')
    }

    it('WSL project: search→navigate to subdirectory file loads without error', async function () {
      if (!mainApp) return this.skip()

      const wslProject = discoveredProjects.find(p => p.fsType === 'WSL')
      if (!wslProject?.subdirCandidates?.length) {
        // On non-Windows, run with the first native project instead
        const nativeProject = discoveredProjects.find(p => p.fsType === 'native' && p.subdirCandidates?.length)
        if (!nativeProject) return this.skip()
        await searchAndVerifyFile(nativeProject)
        return
      }

      await searchAndVerifyFile(wslProject)
    })

    it('Windows FS project: search→navigate to subdirectory file loads without error', async function () {
      if (!mainApp) return this.skip()

      const winProject = discoveredProjects.find(p => p.fsType === 'Windows')
      if (!winProject?.subdirCandidates?.length) {
        // On non-Windows, run with the second native project instead
        const natives = discoveredProjects.filter(p => p.fsType === 'native' && p.subdirCandidates?.length)
        if (natives.length < 2) return this.skip()
        await searchAndVerifyFile(natives[1])
        return
      }

      await searchAndVerifyFile(winProject)
    })

    it('cross-project search: navigates from one project to another', async function () {
      if (!mainApp) return this.skip()
      if (discoveredProjects.filter(p => p.subdirCandidates?.length).length < 2) return this.skip()

      // searchAndVerifyFile already switches to a different project first,
      // then searches for a file — this exercises cross-project navigation.
      const [, projB] = discoveredProjects.filter(p => p.subdirCandidates?.length)
      await searchAndVerifyFile(projB)
    })
  })

  describe('keyboard navigation', () => {
    it('ArrowDown highlights first result', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      const input = await $('[data-testid="search-input"]')
      await input.setValue('README')

      await browser.waitUntil(
        async () => (await $$('[data-testid="search-result"]')).length > 0,
        { ...WAIT_MEDIUM, timeoutMsg: 'No results for ArrowDown test' }
      )

      await browser.keys('ArrowDown')

      // First result should be highlighted via CSS class (bg-zinc-800 dark / bg-zinc-100 light)
      const results = await $$('[data-testid="search-result"]')
      if (results.length === 0) return this.skip()

      const firstResult = results[0]
      const className = await firstResult.getAttribute('class') ?? ''

      const isHighlighted =
        className.includes('bg-zinc-800') ||
        className.includes('bg-zinc-100')

      expect(isHighlighted).toBe(true)
    })

    it('Enter on highlighted result navigates and closes overlay', async function () {
      if (!mainApp) return this.skip()

      const overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) {
        await openSearch()
        const input = await $('[data-testid="search-input"]')
        await input.setValue('README')
        await browser.waitUntil(
          async () => (await $$('[data-testid="search-result"]')).length > 0,
          { ...WAIT_MEDIUM, timeoutMsg: 'No results for Enter test' }
        )
        await browser.keys('ArrowDown')
      }

      await browser.keys('Enter')

      // Overlay must close
      await browser.waitUntil(
        async () => {
          const o = await $('[data-testid="search-overlay"]')
          return !(await o.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Overlay did not close after Enter' }
      )

      // File content must load
      await waitForFileContent(TIMEOUT_LONG, 'File did not load after Enter on result')
    })
  })
})
