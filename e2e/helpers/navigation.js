/**
 * Navigation helpers for E2E tests — tab switching, project selection, waits.
 */

/**
 * Switch to a tab by clicking its button and waiting for content.
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 */
export async function switchToTab(tabName) {
  const btn = await $(`[data-testid="tab-${tabName}"]`)
  await btn.click()
  await waitForTabContent(tabName)
}

/**
 * Wait for tab-specific content to appear after switching.
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 */
export async function waitForTabContent(tabName) {
  const selectors = {
    overview: '[data-testid="quick-actions"], [data-testid="overview-readme"]',
    files: '[role="treeitem"], [data-testid="filetree-loading"]',
    git: '[data-testid="git-tab"]',
    tasks: '[data-testid="sub-tab-list"], [data-testid="tasks-loading"]',
  }
  const selector = selectors[tabName]
  if (!selector) return

  await browser.waitUntil(
    async () => {
      for (const sel of selector.split(', ')) {
        const el = await $(sel)
        if (await el.isExisting()) return true
      }
      return false
    },
    { timeout: 10_000, interval: 500, timeoutMsg: `Tab content for "${tabName}" did not appear` }
  )
}

/**
 * Select a project by its name in the sidebar.
 * @param {string} name - Partial or full project name to match
 */
export async function selectProjectByName(name) {
  const projects = await $$('[data-testid="project-item"]')
  for (const project of projects) {
    const text = await browser.execute((el) => el.textContent, project)
    if (text.includes(name)) {
      await project.click()
      // Wait for the h1 to update
      await browser.waitUntil(
        async () => {
          const h1 = await $('h1')
          const h1Text = await h1.getText()
          return h1Text.includes(name)
        },
        { timeout: 5_000, interval: 300, timeoutMsg: `Project "${name}" did not become active` }
      )
      return true
    }
  }
  return false
}

/**
 * Get the current project name from the Overview h1.
 * @returns {Promise<string>}
 */
export async function getCurrentProjectName() {
  const h1 = await $('h1')
  return await h1.getText()
}

/**
 * Wait for sidebar projects to load (skeleton disappears, projects appear).
 */
export async function waitForProjectsLoaded() {
  await browser.waitUntil(
    async () => {
      const skeleton = await $('[data-testid="sidebar-skeleton"]')
      if (await skeleton.isExisting()) return false
      const projects = await $$('[data-testid="project-item"]')
      return projects.length > 0
    },
    { timeout: 15_000, interval: 500, timeoutMsg: 'Projects did not load in sidebar' }
  )
}

/**
 * Wait for file content to appear (code-viewer or markdown-content).
 * Used after navigating to a file via search, Git "Open file", or tree click.
 * @param {number} [timeout=8000]
 * @param {string} [msg='File content did not appear']
 */
export async function waitForFileContent(timeout = 8_000, msg = 'File content did not appear') {
  await browser.waitUntil(
    async () => {
      const codeViewer = await $('[data-testid="code-viewer"]')
      const markdown = await $('[data-testid="markdown-content"]')
      return (await codeViewer.isExisting()) || (await markdown.isExisting())
    },
    { timeout, interval: 300, timeoutMsg: msg }
  )
}

/**
 * Check whether a tab is currently active (has aria-selected or aria-current).
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 * @returns {Promise<boolean>}
 */
export async function isTabActive(tabName) {
  const tab = await $(`[data-testid="tab-${tabName}"]`)
  if (!(await tab.isExisting())) return false
  const ariaSelected = await tab.getAttribute('aria-selected')
  const ariaCurrent = await tab.getAttribute('aria-current')
  if (ariaSelected === 'true' || ariaCurrent === 'true' || ariaCurrent === 'page') return true
  // Fallback: check CSS class for active state (border-brand-500 or font-medium)
  const className = await tab.getAttribute('class') || ''
  return className.includes('border-brand') || className.includes('font-medium')
}

/**
 * Wait for a tab to become active.
 * @param {'overview'|'files'|'git'|'tasks'} tabName
 * @param {number} [timeout=5000]
 */
export async function waitForTabActive(tabName, timeout = 5_000) {
  await browser.waitUntil(
    async () => await isTabActive(tabName),
    { timeout, interval: 300, timeoutMsg: `Tab "${tabName}" did not become active` }
  )
}

/**
 * Right-click an element and wait for the context menu to appear.
 * @param {WebdriverIO.Element} element
 */
export async function openContextMenu(element) {
  await element.click({ button: 'right' })
  await browser.waitUntil(
    async () => (await $('[data-testid="context-menu"]')).isExisting(),
    { timeout: 5_000, interval: 200, timeoutMsg: 'Context menu did not appear after right-click' }
  )
}

/**
 * Dismiss the context menu if it's open.
 */
export async function dismissContextMenu() {
  const menu = await $('[data-testid="context-menu"]')
  if (await menu.isExisting()) {
    await browser.keys(['Escape'])
    await browser.waitUntil(
      async () => !(await (await $('[data-testid="context-menu"]')).isExisting()),
      { timeout: 3_000, interval: 200 }
    )
  }
}
